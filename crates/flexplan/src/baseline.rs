//! Recompute-everything baseline, modelled on the pre-Flex-Model behaviour the article
//! describes ("every feature was computed for every market on every evaluation"). On every
//! event it runs the subscribed handlers and then re-evaluates **every** value; requests
//! just copy values.
//!
//! The step order is a value-level topological order (not node-level), built here with an
//! independent depth-first algorithm rather than the planner's code, so differential tests
//! between the two engines are a genuine cross-check. Rules, per event kind:
//! * an eval runs after every value it reads, and after its own node's handler;
//! * a handler runs after every value it reads.
//!
//! It shares node code and the input gate (`engine::Gate`) with the planned engine, so cost differences
//! come only from *what* is run.

use std::any::Any;

use crate::engine::{EngineStats, Gate};
use crate::error::DispatchError;
use crate::event::{Event, EventKey};
use crate::graph::{BuildError, BuildErrors, ErasedNode, GraphBuilder, Reads, RequestKey};
use crate::planner::{self, Resolved};

#[derive(Clone, Debug)]
enum BStep {
    Handle {
        node: u32,
        handler: u16,
        reads: Vec<u32>,
    },
    Eval {
        node: u32,
        eval_idx: u16,
        out: u32,
        reads: Vec<u32>,
    },
}

/// The recompute-everything reference engine. Built by [`GraphBuilder::build_baseline`].
pub struct BaselineEngine {
    gate: Gate,
    nodes: Vec<Box<dyn ErasedNode>>,
    values: Vec<f64>,
    /// One full step list per event kind.
    per_kind: Vec<Vec<BStep>>,
    request_outputs: Vec<Vec<u32>>,
}

/// Depth-first topological order over (handlers of `kind`) ∪ (all evals).
fn order(r: &Resolved, kind: Option<u32>, label: &str) -> Result<Vec<BStep>, BuildError> {
    let nv = r.evals.len();
    let nn = r.node_names.len();
    // handler of node n for this kind, if any
    let handler_of: Vec<Option<(u16, Vec<u32>)>> = (0..nn)
        .map(|n| {
            kind.and_then(|k| {
                r.handlers[n]
                    .iter()
                    .find(|(kk, _, _)| *kk == k)
                    .map(|(_, h, reads)| (*h, reads.clone()))
            })
        })
        .collect();
    // Step ids: 0..nv = eval of value v; nv..nv+nn = handler of node n.
    let total = nv + nn;
    let deps = |id: usize| -> Vec<usize> {
        if id < nv {
            let (node, _, reads) = r.evals[id].as_ref().expect("canonical eval");
            let mut d: Vec<usize> = reads.iter().map(|&x| x as usize).collect();
            if handler_of[*node as usize].is_some() {
                d.push(nv + *node as usize);
            }
            d
        } else {
            handler_of[id - nv]
                .as_ref()
                .map(|(_, reads)| reads.iter().map(|&x| x as usize).collect())
                .unwrap_or_default()
        }
    };
    let exists = |id: usize| {
        if id < nv {
            r.evals[id].is_some()
        } else {
            handler_of[id - nv].is_some()
        }
    };
    // 0 = unvisited, 1 = on stack, 2 = done
    let mut color = vec![0u8; total];
    let mut out: Vec<usize> = Vec::new();
    for root in 0..total {
        if !exists(root) || color[root] != 0 {
            continue;
        }
        // Iterative DFS with explicit stack of (id, next-dependency index).
        let mut stack: Vec<(usize, Vec<usize>, usize)> = vec![(root, deps(root), 0)];
        color[root] = 1;
        while let Some(top) = stack.last_mut() {
            if top.2 < top.1.len() {
                let d = top.1[top.2];
                top.2 += 1;
                match color[d] {
                    0 => {
                        color[d] = 1;
                        let dd = deps(d);
                        stack.push((d, dd, 0));
                    }
                    1 => {
                        let mut steps: Vec<String> =
                            stack.iter().map(|(i, _, _)| step_name(r, nv, *i)).collect();
                        steps.sort();
                        return Err(BuildError::Cycle {
                            plan: format!("baseline {label}"),
                            steps,
                        });
                    }
                    _ => {}
                }
            } else {
                let (id, _, _) = stack.pop().unwrap();
                color[id] = 2;
                out.push(id);
            }
        }
    }
    Ok(out
        .into_iter()
        .map(|id| {
            if id < nv {
                let (node, eidx, reads) = r.evals[id].as_ref().unwrap();
                BStep::Eval {
                    node: *node,
                    eval_idx: *eidx,
                    out: id as u32,
                    reads: reads.clone(),
                }
            } else {
                let (h, reads) = handler_of[id - nv].as_ref().unwrap();
                BStep::Handle {
                    node: (id - nv) as u32,
                    handler: *h,
                    reads: reads.clone(),
                }
            }
        })
        .collect())
}

fn step_name(r: &Resolved, nv: usize, id: usize) -> String {
    if id < nv {
        format!("eval {}", r.value_labels[id])
    } else {
        format!("handle {}", r.node_names[id - nv])
    }
}

#[inline(always)]
fn run(steps: &[BStep], nodes: &mut [Box<dyn ErasedNode>], values: &mut [f64], ev: &dyn Any) {
    for s in steps {
        match s {
            BStep::Handle {
                node,
                handler,
                reads,
            } => nodes[*node as usize].handle(
                *handler as usize,
                ev,
                &Reads {
                    vals: values,
                    ids: reads,
                },
            ),
            BStep::Eval {
                node,
                eval_idx,
                out,
                reads,
            } => {
                let x = nodes[*node as usize].eval(
                    *eval_idx as usize,
                    &Reads {
                        vals: values,
                        ids: reads,
                    },
                );
                values[*out as usize] = x;
            }
        }
    }
}

impl BaselineEngine {
    pub(crate) fn from_builder(b: GraphBuilder) -> Result<Self, BuildErrors> {
        let mut errors = b.errors.clone();
        let r = match planner::resolve(&b) {
            Ok(r) => r,
            Err(mut e) => {
                errors.append(&mut e);
                return Err(BuildErrors(errors));
            }
        };
        let initial = order(&r, None, "<initial>").map_err(|e| vec![e]);
        let mut per_kind = Vec::new();
        for (k, name) in r.kind_names.iter().enumerate() {
            match order(&r, Some(k as u32), &format!("on {name}")) {
                Ok(s) => per_kind.push(s),
                Err(e) => errors.push(e),
            }
        }
        let initial = match initial {
            Ok(s) => s,
            Err(mut e) => {
                errors.append(&mut e);
                Vec::new()
            }
        };
        if !errors.is_empty() {
            return Err(BuildErrors(errors));
        }
        let mut eng = BaselineEngine {
            gate: Gate::new(b.id, b.clock_policy),
            nodes: b.nodes,
            values: vec![f64::NAN; b.values.len()],
            per_kind,
            request_outputs: r.requests.iter().map(|(_, o)| o.clone()).collect(),
        };
        run(&initial, &mut eng.nodes, &mut eng.values, &());
        Ok(eng)
    }

    /// Dispatch one event: run subscribed handlers, then recompute every value.
    #[inline]
    pub fn dispatch<E: Event>(&mut self, key: EventKey<E>, ev: &E) -> Result<(), DispatchError> {
        self.gate.begin_event(key, ev)?;
        run(
            &self.per_kind[key.kind as usize],
            &mut self.nodes,
            &mut self.values,
            ev,
        );
        self.gate.finish_event(ev.time_ns());
        Ok(())
    }

    /// Copy the requested values (already up to date) into `out`.
    #[inline]
    pub fn request(&mut self, key: RequestKey, out: &mut [f64]) -> Result<(), DispatchError> {
        self.gate.begin_request(key, out.len())?;
        for (o, &v) in out
            .iter_mut()
            .zip(self.request_outputs[key.idx as usize].iter())
        {
            *o = self.values[v as usize];
        }
        self.gate.finish_request();
        Ok(())
    }

    /// Observability counters.
    pub fn stats(&self) -> EngineStats {
        self.gate.stats
    }
}
