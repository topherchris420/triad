//! The planner: turns node definitions into one ordered step list per event kind and per
//! feature request. Runs once at startup.
//!
//! Ordering rules (the first is stated in the source article; the second is our inference,
//! see ARCHITECTURE.md §4):
//! 1. Every value a step reads is evaluated earlier in the same plan.
//! 2. If a node handles the event, its handler runs before any of its own evals in that
//!    plan, so downstream readers see post-event state.
//!
//! Unconstrained steps are ordered by (node registration index, handler-before-eval,
//! sub-index), which makes plans deterministic and independent of hash ordering.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt::Write as _;

use crate::graph::{BuildError, GraphBuilder, ValueDef};

/// What a plan step does.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StepOp {
    /// Run eval `eval_idx` of the node and store the result in value slot `out`.
    Eval {
        /// Which of the node's evals.
        eval_idx: u16,
        /// Destination value slot.
        out: u32,
    },
    /// Run the node's handler `handler_idx` with the current event.
    /// Run the node's handler `handler_idx` with the current event.
    Handle {
        /// Which of the node's handlers.
        handler_idx: u16,
    },
}

/// One step of a compiled plan.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Node the step belongs to.
    pub node: u32,
    /// Operation.
    pub op: StepOp,
    /// Offset of this step's reads in [`Plan::reads`].
    pub reads_start: u32,
    /// Number of reads.
    pub reads_len: u32,
}

/// A precompiled, immutable execution plan.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Plan name (for diagnostics).
    pub name: String,
    /// Ordered steps.
    pub steps: Vec<Step>,
    /// Concatenated read lists of all steps.
    pub reads: Vec<u32>,
}

impl Plan {
    /// The value ids a step reads.
    #[inline(always)]
    pub fn reads_of(&self, s: &Step) -> &[u32] {
        &self.reads[s.reads_start as usize..(s.reads_start + s.reads_len) as usize]
    }
}

/// Resolved (placeholder-free) view of the graph used for planning and validation.
pub(crate) struct Resolved {
    /// canonical value id for every value id (identity for eval values).
    pub canon: Vec<u32>,
    /// For each canonical eval value: (node, eval_idx, reads (canonical)).
    pub evals: Vec<Option<(u32, u16, Vec<u32>)>>,
    /// Per node: list of (kind, handler_idx, reads (canonical)).
    pub handlers: Vec<Vec<(u32, u16, Vec<u32>)>>,
    /// Per node: its canonical eval values.
    pub node_evals: Vec<Vec<u32>>,
    pub node_names: Vec<String>,
    pub value_labels: Vec<String>,
    pub kind_names: Vec<&'static str>,
    pub requests: Vec<(String, Vec<u32>)>,
}

#[allow(clippy::needless_range_loop)]
pub(crate) fn resolve(b: &GraphBuilder) -> Result<Resolved, Vec<BuildError>> {
    let n = b.values.len();
    let mut errors = Vec::new();
    let mut canon = vec![u32::MAX; n];
    for v in 0..n {
        let mut cur = v;
        let mut hops = 0usize;
        loop {
            match &b.values[cur] {
                ValueDef::Eval { .. } => {
                    canon[v] = cur as u32;
                    break;
                }
                ValueDef::Placeholder { target: None } => {
                    errors.push(BuildError::UnboundPlaceholder {
                        placeholder: cur as u32,
                    });
                    break;
                }
                ValueDef::Placeholder { target: Some(t) } => {
                    cur = *t as usize;
                    hops += 1;
                    if hops > n {
                        errors.push(BuildError::PlaceholderCycle {
                            placeholder: v as u32,
                        });
                        break;
                    }
                }
            }
        }
    }
    // Only report unbound placeholders that are actually used somewhere.
    let mut used = vec![false; n];
    for vd in &b.values {
        if let ValueDef::Eval { reads, .. } = vd {
            for &r in reads {
                used[r as usize] = true;
            }
        }
    }
    for m in &b.node_meta {
        for h in &m.handlers {
            for &r in &h.reads {
                used[r as usize] = true;
            }
        }
    }
    for r in &b.requests {
        for &o in &r.outputs {
            used[o as usize] = true;
        }
    }
    errors.retain(|e| match e {
        BuildError::UnboundPlaceholder { placeholder } => used[*placeholder as usize],
        _ => true,
    });
    errors.dedup();
    if !errors.is_empty() {
        return Err(errors);
    }
    let c = |v: &u32| canon[*v as usize];
    let mut evals = vec![None; n];
    for (v, vd) in b.values.iter().enumerate() {
        if let ValueDef::Eval {
            node,
            eval_idx,
            reads,
        } = vd
        {
            evals[v] = Some((*node, *eval_idx, reads.iter().map(c).collect()));
        }
    }
    let handlers = b
        .node_meta
        .iter()
        .map(|m| {
            m.handlers
                .iter()
                .map(|h| (h.kind, h.handler_idx, h.reads.iter().map(c).collect()))
                .collect()
        })
        .collect();
    Ok(Resolved {
        evals,
        handlers,
        node_evals: b.node_meta.iter().map(|m| m.evals.clone()).collect(),
        node_names: b.node_meta.iter().map(|m| m.name.clone()).collect(),
        value_labels: b.value_labels.clone(),
        kind_names: b.kinds.iter().map(|k| k.name).collect(),
        requests: b
            .requests
            .iter()
            .map(|r| (r.name.clone(), r.outputs.iter().map(c).collect()))
            .collect(),
        canon,
    })
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Key {
    // (node, rank, sub) — rank 0 = handle, 1 = eval
    S(u32, u8, u32),
}

/// Build the plan for one set of handlers plus a set of root values.
pub(crate) fn plan(
    r: &Resolved,
    name: &str,
    handlers: &[(u32, u16, Vec<u32>)], // (node, handler_idx, reads)
    roots: &[u32],
) -> Result<Plan, BuildError> {
    let nvals = r.evals.len();
    // Step table.
    struct S {
        key: Key,
        node: u32,
        op: StepOp,
        reads: Vec<u32>,
    }
    let mut steps: Vec<S> = Vec::new();
    let mut eval_step = vec![usize::MAX; nvals];
    let mut handle_step_of_node: Vec<(u32, usize)> = Vec::new();
    for (node, hidx, reads) in handlers {
        handle_step_of_node.push((*node, steps.len()));
        steps.push(S {
            key: Key::S(*node, 0, *hidx as u32),
            node: *node,
            op: StepOp::Handle { handler_idx: *hidx },
            reads: reads.clone(),
        });
    }
    // Transitive closure of needed evals.
    let mut work: Vec<u32> = roots.to_vec();
    for (_, _, reads) in handlers {
        work.extend_from_slice(reads);
    }
    while let Some(v) = work.pop() {
        if eval_step[v as usize] != usize::MAX {
            continue;
        }
        let (node, eidx, reads) = r.evals[v as usize]
            .as_ref()
            .expect("resolved value has eval");
        eval_step[v as usize] = steps.len();
        steps.push(S {
            key: Key::S(*node, 1, *eidx as u32),
            node: *node,
            op: StepOp::Eval {
                eval_idx: *eidx,
                out: v,
            },
            reads: reads.clone(),
        });
        work.extend_from_slice(reads);
    }
    // Edges.
    let ns = steps.len();
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); ns];
    let mut indeg = vec![0usize; ns];
    for (i, s) in steps.iter().enumerate() {
        for &rv in &s.reads {
            let p = eval_step[rv as usize];
            succ[p].push(i);
            indeg[i] += 1;
        }
    }
    for &(node, hs) in &handle_step_of_node {
        for &v in &r.node_evals[node as usize] {
            let es = eval_step[v as usize];
            if es != usize::MAX {
                succ[hs].push(es);
                indeg[es] += 1;
            }
        }
    }
    // Deterministic Kahn.
    let mut heap = BinaryHeap::new();
    for i in 0..ns {
        if indeg[i] == 0 {
            heap.push(Reverse((steps[i].key, i)));
        }
    }
    let mut order = Vec::with_capacity(ns);
    while let Some(Reverse((_, i))) = heap.pop() {
        order.push(i);
        for &j in &succ[i] {
            indeg[j] -= 1;
            if indeg[j] == 0 {
                heap.push(Reverse((steps[j].key, j)));
            }
        }
    }
    if order.len() != ns {
        let mut stuck: Vec<String> = (0..ns)
            .filter(|&i| indeg[i] > 0)
            .map(|i| step_label(r, steps[i].node, steps[i].op))
            .collect();
        stuck.sort();
        return Err(BuildError::Cycle {
            plan: name.to_string(),
            steps: stuck,
        });
    }
    let mut out = Plan {
        name: name.to_string(),
        steps: Vec::with_capacity(ns),
        reads: Vec::new(),
    };
    for i in order {
        let s = &steps[i];
        let start = out.reads.len() as u32;
        out.reads.extend_from_slice(&s.reads);
        out.steps.push(Step {
            node: s.node,
            op: s.op,
            reads_start: start,
            reads_len: s.reads.len() as u32,
        });
    }
    Ok(out)
}

pub(crate) fn step_label(r: &Resolved, node: u32, op: StepOp) -> String {
    match op {
        StepOp::Handle { .. } => format!("handle {}", r.node_names[node as usize]),
        StepOp::Eval { out, .. } => format!("eval {}", r.value_labels[out as usize]),
    }
}

/// Human-readable dump in the spirit of the source article's plan listing.
pub(crate) fn describe(r: &Resolved, p: &Plan) -> String {
    let mut s = String::new();
    for st in &p.steps {
        let (who, what) = match st.op {
            StepOp::Handle { .. } => (r.node_names[st.node as usize].clone(), "handle"),
            StepOp::Eval { out, .. } => (r.value_labels[out as usize].clone(), "eval"),
        };
        let _ = writeln!(s, "  {who:<26}{what}");
    }
    s
}

/// Plans for every event kind (indexed by kind) and every request (indexed by request).
pub(crate) fn plan_all(r: &Resolved) -> Result<(Vec<Plan>, Vec<Plan>), Vec<BuildError>> {
    let mut errors = Vec::new();
    // Global eval-cycle check over every eval, needed or not.
    let all: Vec<u32> = (0..r.evals.len() as u32)
        .filter(|&v| r.evals[v as usize].is_some())
        .collect();
    if let Err(e) = plan(r, "<all evals>", &[], &all) {
        errors.push(e);
    }
    let mut event_plans = Vec::new();
    for (k, kname) in r.kind_names.iter().enumerate() {
        let hs: Vec<(u32, u16, Vec<u32>)> = r
            .handlers
            .iter()
            .enumerate()
            .flat_map(|(n, list)| {
                list.iter()
                    .filter(|(kind, _, _)| *kind as usize == k)
                    .map(move |(_, hidx, reads)| (n as u32, *hidx, reads.clone()))
            })
            .collect();
        match plan(r, &format!("on {kname}"), &hs, &[]) {
            Ok(p) => event_plans.push(p),
            Err(e) => {
                errors.push(e);
                event_plans.push(Plan::default())
            }
        }
    }
    let mut request_plans = Vec::new();
    for (name, outs) in &r.requests {
        match plan(r, &format!("request {name}"), &[], outs) {
            Ok(p) => request_plans.push(p),
            Err(e) => {
                errors.push(e);
                request_plans.push(Plan::default())
            }
        }
    }
    if errors.is_empty() {
        for p in event_plans.iter().chain(request_plans.iter()) {
            if let Err(e) = validate(r, &p.steps, &p.reads) {
                panic!("planner produced invalid plan `{}`: {e}", p.name);
            }
        }
        Ok((event_plans, request_plans))
    } else {
        Err(errors)
    }
}

/// Independent static checker: every read must see a value evaluated earlier in this plan
/// and not invalidated since by a handler of the producing node or any upstream node.
pub(crate) fn validate(r: &Resolved, steps: &[Step], reads: &[u32]) -> Result<(), String> {
    let nv = r.evals.len();
    // dependents[v] = values whose eval reads v directly.
    let mut dependents: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (v, e) in r.evals.iter().enumerate() {
        if let Some((_, _, rs)) = e {
            for &x in rs {
                dependents[x as usize].push(v as u32);
            }
        }
    }
    let mut fresh = vec![false; nv];
    for (i, s) in steps.iter().enumerate() {
        let rs = &reads[s.reads_start as usize..(s.reads_start + s.reads_len) as usize];
        for &x in rs {
            if !fresh[x as usize] {
                return Err(format!(
                    "step {i} ({}) reads `{}` which is not fresh at that point",
                    step_label(r, s.node, s.op),
                    r.value_labels[x as usize]
                ));
            }
        }
        match s.op {
            StepOp::Eval { out, .. } => fresh[out as usize] = true,
            StepOp::Handle { .. } => {
                let mut stack: Vec<u32> = r.node_evals[s.node as usize].clone();
                while let Some(v) = stack.pop() {
                    if fresh[v as usize] {
                        fresh[v as usize] = false;
                        stack.extend_from_slice(&dependents[v as usize]);
                    }
                }
            }
        }
    }
    Ok(())
}
