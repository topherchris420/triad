//! The planned engine: executes precompiled plans with no cutoffs and no allocation.
//!
//! # Contract
//! * `dispatch` and `request` return `Err` **without changing any state** when the key is
//!   foreign, the event fails [`Event::validate`], a timed event violates the
//!   [`ClockPolicy`], or the output buffer has the wrong length.
//! * Handlers and evals are infallible. If one panics, the engine is **poisoned**: every
//!   later call returns [`DispatchError::Poisoned`], because state may be half-updated.
//! * Neither call allocates. Errors carry only `Copy` data.

use std::any::Any;

use crate::error::DispatchError;
use crate::event::{ClockPolicy, Event, EventKey};
use crate::graph::{
    BuildErrors, ErasedNode, GraphBuilder, NodeHandle, NodeImpl, Reads, RequestKey, ValueId,
};
use crate::planner::{self, Plan, Resolved, Step, StepOp};

/// Counters for observability. Incremented on the hot path with plain adds.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct EngineStats {
    /// Events whose plan ran.
    pub events: u64,
    /// Feature requests served.
    pub requests: u64,
    /// Calls refused (invalid event, clock, foreign key, buffer length, poisoned).
    pub rejected: u64,
}

/// Input gate shared by the planned and baseline engines, so both apply identical
/// validation, clock and poisoning rules.
#[derive(Clone, Debug)]
pub(crate) struct Gate {
    pub graph: u64,
    pub clock_policy: ClockPolicy,
    pub clock_ns: Option<u64>,
    pub poisoned: bool,
    pub stats: EngineStats,
}

impl Gate {
    pub fn new(graph: u64, clock_policy: ClockPolicy) -> Self {
        Gate {
            graph,
            clock_policy,
            clock_ns: None,
            poisoned: false,
            stats: EngineStats::default(),
        }
    }

    #[inline]
    fn reject(&mut self, e: DispatchError) -> Result<(), DispatchError> {
        self.stats.rejected += 1;
        Err(e)
    }

    /// Check everything that can fail before any handler runs. On success, marks the
    /// engine as "in progress" (poisoned until [`Gate::finish_event`]).
    #[inline]
    pub fn begin_event<E: Event>(&mut self, key: EventKey<E>, ev: &E) -> Result<(), DispatchError> {
        if key.graph != self.graph {
            return self.reject(DispatchError::ForeignKey);
        }
        if self.poisoned {
            return self.reject(DispatchError::Poisoned);
        }
        if let Err(reason) = ev.validate() {
            return self.reject(DispatchError::InvalidEvent {
                event: E::NAME,
                reason,
            });
        }
        if let (Some(now), Some(prev)) = (ev.time_ns(), self.clock_ns) {
            if now < prev && self.clock_policy == ClockPolicy::RejectBackwards {
                return self.reject(DispatchError::ClockWentBackwards {
                    event: E::NAME,
                    previous_ns: prev,
                    next_ns: now,
                });
            }
        }
        self.poisoned = true;
        Ok(())
    }

    #[inline]
    pub fn finish_event(&mut self, time_ns: Option<u64>) {
        if let Some(now) = time_ns {
            self.clock_ns = Some(self.clock_ns.map_or(now, |p| p.max(now)));
        }
        self.stats.events += 1;
        self.poisoned = false;
    }

    #[inline]
    pub fn begin_request(&mut self, key: RequestKey, out_len: usize) -> Result<(), DispatchError> {
        if key.graph != self.graph {
            return self.reject(DispatchError::ForeignKey);
        }
        if self.poisoned {
            return self.reject(DispatchError::Poisoned);
        }
        if out_len != key.width() {
            return self.reject(DispatchError::OutputLength {
                expected: key.width(),
                got: out_len,
            });
        }
        self.poisoned = true;
        Ok(())
    }

    #[inline]
    pub fn finish_request(&mut self) {
        self.stats.requests += 1;
        self.poisoned = false;
    }
}

/// A validated, planned feature engine. Built by [`GraphBuilder::build`].
pub struct Engine {
    gate: Gate,
    nodes: Vec<Box<dyn ErasedNode>>,
    values: Vec<f64>,
    value_epoch: Vec<u64>,
    epoch: u64,
    event_plans: Vec<Plan>,
    request_plans: Vec<Plan>,
    request_outputs: Vec<Vec<u32>>,
    resolved: Resolved,
}

#[inline(always)]
fn run_plan(
    plan: &Plan,
    nodes: &mut [Box<dyn ErasedNode>],
    values: &mut [f64],
    value_epoch: &mut [u64],
    epoch: u64,
    ev: &dyn Any,
) {
    for s in &plan.steps {
        let ids = plan.reads_of(s);
        #[cfg(debug_assertions)]
        for &i in ids {
            // Internal invariant, proven at build time by `planner::validate`; re-checked
            // here in debug builds only.
            debug_assert_eq!(
                value_epoch[i as usize], epoch,
                "stale read of value #{i} in `{}`",
                plan.name
            );
        }
        match s.op {
            StepOp::Eval { eval_idx, out } => {
                let x =
                    nodes[s.node as usize].eval(eval_idx as usize, &Reads { vals: values, ids });
                values[out as usize] = x;
                value_epoch[out as usize] = epoch;
            }
            StepOp::Handle { handler_idx } => {
                nodes[s.node as usize].handle(
                    handler_idx as usize,
                    ev,
                    &Reads { vals: values, ids },
                );
            }
        }
    }
}

impl Engine {
    pub(crate) fn from_builder(b: GraphBuilder) -> Result<Engine, BuildErrors> {
        let mut errors = b.errors.clone();
        let resolved = match planner::resolve(&b) {
            Ok(r) => r,
            Err(mut e) => {
                errors.append(&mut e);
                return Err(BuildErrors(errors));
            }
        };
        let (event_plans, request_plans) = match planner::plan_all(&resolved) {
            Ok(p) => p,
            Err(mut e) => {
                errors.append(&mut e);
                return Err(BuildErrors(errors));
            }
        };
        if !errors.is_empty() {
            return Err(BuildErrors(errors));
        }
        let nv = b.values.len();
        Ok(Engine {
            gate: Gate::new(b.id, b.clock_policy),
            nodes: b.nodes,
            values: vec![f64::NAN; nv],
            value_epoch: vec![0; nv],
            epoch: 0,
            event_plans,
            request_plans,
            request_outputs: resolved.requests.iter().map(|(_, o)| o.clone()).collect(),
            resolved,
        })
    }

    /// Dispatch one event: validate it, then run its precompiled plan. Zero allocation.
    /// On `Err`, no state changed.
    #[inline]
    pub fn dispatch<E: Event>(&mut self, key: EventKey<E>, ev: &E) -> Result<(), DispatchError> {
        self.gate.begin_event(key, ev)?;
        self.epoch += 1;
        let plan = &self.event_plans[key.kind as usize];
        run_plan(
            plan,
            &mut self.nodes,
            &mut self.values,
            &mut self.value_epoch,
            self.epoch,
            ev,
        );
        self.gate.finish_event(ev.time_ns());
        Ok(())
    }

    /// "The model asks for features": evaluate the request plan and write its outputs into
    /// `out`, which must have length `key.width()`. Zero allocation. On `Err`, no state
    /// changed and `out` is untouched.
    #[inline]
    pub fn request(&mut self, key: RequestKey, out: &mut [f64]) -> Result<(), DispatchError> {
        self.gate.begin_request(key, out.len())?;
        self.epoch += 1;
        let i = key.idx as usize;
        run_plan(
            &self.request_plans[i],
            &mut self.nodes,
            &mut self.values,
            &mut self.value_epoch,
            self.epoch,
            &(),
        );
        for (o, &v) in out.iter_mut().zip(self.request_outputs[i].iter()) {
            *o = self.values[v as usize];
        }
        self.gate.finish_request();
        Ok(())
    }

    fn own_value(&self, v: ValueId) -> usize {
        assert_eq!(
            v.graph, self.gate.graph,
            "ValueId belongs to a different graph"
        );
        self.resolved.canon[v.index()] as usize
    }

    /// Last computed value (may be stale; check with [`Self::is_fresh`]).
    ///
    /// # Panics
    /// If `v` comes from a different graph.
    pub fn value(&self, v: ValueId) -> f64 {
        self.values[self.own_value(v)]
    }

    /// Whether `v` was evaluated during the most recent dispatch or request.
    ///
    /// # Panics
    /// If `v` comes from a different graph.
    pub fn is_fresh(&self, v: ValueId) -> bool {
        self.value_epoch[self.own_value(v)] == self.epoch
    }

    /// Monotone counter incremented by every successful dispatch and request.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Newest event time accepted so far, if any timed event has been dispatched.
    pub fn clock_ns(&self) -> Option<u64> {
        self.gate.clock_ns
    }

    /// Whether a handler or eval panicked; if so, every call now returns `Poisoned`.
    pub fn is_poisoned(&self) -> bool {
        self.gate.poisoned
    }

    /// Observability counters.
    pub fn stats(&self) -> EngineStats {
        self.gate.stats
    }

    /// Borrow a node's private state (for tests and inspection).
    ///
    /// # Panics
    /// If `h` comes from a different graph.
    pub fn state<S: 'static>(&self, h: NodeHandle<S>) -> &S {
        assert_eq!(
            h.graph, self.gate.graph,
            "NodeHandle belongs to a different graph"
        );
        &self.nodes[h.index()]
            .as_any()
            .downcast_ref::<NodeImpl<S>>()
            .expect("node handle type mismatch")
            .state
    }

    fn kind_index(&self, name: &str) -> Option<usize> {
        self.resolved.kind_names.iter().position(|k| *k == name)
    }

    /// Plan for an event kind, rendered as text.
    pub fn describe_event_plan(&self, event_name: &str) -> Option<String> {
        self.kind_index(event_name)
            .map(|k| planner::describe(&self.resolved, &self.event_plans[k]))
    }

    /// Plan for a feature request, rendered as text.
    pub fn describe_request_plan(&self, key: RequestKey) -> String {
        assert_eq!(
            key.graph, self.gate.graph,
            "RequestKey belongs to a different graph"
        );
        planner::describe(&self.resolved, &self.request_plans[key.idx as usize])
    }

    /// Full textual plan dump for every event kind and request.
    pub fn describe_all(&self) -> String {
        let mut s = String::new();
        for (k, name) in self.resolved.kind_names.iter().enumerate() {
            s.push_str(&format!("on {name}:\n"));
            s.push_str(&planner::describe(&self.resolved, &self.event_plans[k]));
        }
        for (i, (name, _)) in self.resolved.requests.iter().enumerate() {
            s.push_str(&format!("when request `{name}` is made:\n"));
            s.push_str(&planner::describe(&self.resolved, &self.request_plans[i]));
        }
        s
    }

    /// Raw steps of an event plan (for tests that construct deliberately invalid plans).
    pub fn event_plan_steps(&self, event_name: &str) -> Option<(Vec<Step>, Vec<u32>)> {
        self.kind_index(event_name).map(|k| {
            (
                self.event_plans[k].steps.clone(),
                self.event_plans[k].reads.clone(),
            )
        })
    }

    /// Run the independent static freshness checker on an arbitrary step list.
    pub fn validate_steps(&self, steps: &[Step], reads: &[u32]) -> Result<(), String> {
        planner::validate(&self.resolved, steps, reads)
    }

    /// Number of steps in an event kind's plan.
    pub fn event_plan_len(&self, event_name: &str) -> Option<usize> {
        self.kind_index(event_name)
            .map(|k| self.event_plans[k].steps.len())
    }

    /// Number of steps in a request plan.
    pub fn request_plan_len(&self, key: RequestKey) -> usize {
        assert_eq!(
            key.graph, self.gate.graph,
            "RequestKey belongs to a different graph"
        );
        self.request_plans[key.idx as usize].steps.len()
    }

    /// Number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
