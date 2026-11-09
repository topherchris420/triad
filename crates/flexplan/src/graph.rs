//! Node registry and graph builder (startup-time only; may allocate freely).

use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::event::{ClockPolicy, Event, EventKey};

static NEXT_GRAPH_ID: AtomicU64 = AtomicU64::new(1);

/// Identifies one float value produced by one node's eval function (or a placeholder that
/// will be bound to such a value before build).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId {
    pub(crate) idx: u32,
    pub(crate) graph: u64,
}

impl ValueId {
    /// Dense index of this value inside its graph.
    pub fn index(self) -> usize {
        self.idx as usize
    }
}

/// Typed handle to a node whose private state has type `S`.
pub struct NodeHandle<S> {
    pub(crate) idx: u32,
    pub(crate) graph: u64,
    _p: PhantomData<fn() -> S>,
}
impl<S> Clone for NodeHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for NodeHandle<S> {}
impl<S> fmt::Debug for NodeHandle<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeHandle(#{})", self.idx)
    }
}
impl<S> NodeHandle<S> {
    /// Dense index of this node inside its graph.
    pub fn index(self) -> usize {
        self.idx as usize
    }
}

/// Key for a precompiled feature request ("the model asks for features").
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RequestKey {
    pub(crate) idx: u32,
    pub(crate) graph: u64,
    pub(crate) width: u32,
}
impl RequestKey {
    /// Number of floats the request writes.
    pub fn width(self) -> usize {
        self.width as usize
    }
}

/// Positional view of the values a step declared it reads. `get(i)` returns the i-th
/// declared read. Reads are guaranteed fresh by the planner (see `planner::validate`).
pub struct Reads<'a> {
    pub(crate) vals: &'a [f64],
    pub(crate) ids: &'a [u32],
}
impl<'a> Reads<'a> {
    /// The `i`-th declared read.
    ///
    /// # Panics
    /// If `i >= self.len()`.
    #[inline(always)]
    pub fn get(&self, i: usize) -> f64 {
        self.vals[self.ids[i] as usize]
    }
    /// Number of declared reads.
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    /// Whether the step declared no reads.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

type EvalFn<S> = Box<dyn Fn(&S, &Reads) -> f64 + Send>;
type HandlerFn<S> = Box<dyn FnMut(&mut S, &dyn Any, &Reads) + Send>;

/// Type-erased node used by the engines. `Send` so a built engine can move to a
/// dedicated (e.g. pinned) thread.
pub(crate) trait ErasedNode: Send {
    fn eval(&self, idx: usize, r: &Reads) -> f64;
    fn handle(&mut self, idx: usize, ev: &dyn Any, r: &Reads);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct NodeImpl<S> {
    pub(crate) state: S,
    evals: Vec<EvalFn<S>>,
    handlers: Vec<HandlerFn<S>>,
}

impl<S: Send + 'static> ErasedNode for NodeImpl<S> {
    #[inline]
    fn eval(&self, idx: usize, r: &Reads) -> f64 {
        (self.evals[idx])(&self.state, r)
    }
    #[inline]
    fn handle(&mut self, idx: usize, ev: &dyn Any, r: &Reads) {
        (self.handlers[idx])(&mut self.state, ev, r)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ValueDef {
    Eval {
        node: u32,
        eval_idx: u16,
        reads: Vec<u32>,
    },
    Placeholder {
        target: Option<u32>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct HandlerMeta {
    pub kind: u32,
    pub handler_idx: u16,
    pub reads: Vec<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct NodeMeta {
    pub name: String,
    pub evals: Vec<u32>,
    pub handlers: Vec<HandlerMeta>,
}

#[derive(Clone, Debug)]
pub(crate) struct EventKindMeta {
    pub name: &'static str,
    pub type_id: TypeId,
    pub type_name: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct RequestMeta {
    pub name: String,
    pub outputs: Vec<u32>,
}

/// A configuration error detected at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// A node registered two handlers for the same event kind.
    DuplicateHandler {
        /// Node name.
        node: String,
        /// Event kind name.
        event: &'static str,
    },
    /// Two different Rust types use the same [`Event::NAME`].
    EventNameCollision {
        /// The shared name.
        name: &'static str,
        /// First type registered under it.
        first: &'static str,
        /// Second type registered under it.
        second: &'static str,
    },
    /// [`GraphBuilder::bind`] was called twice for one placeholder.
    PlaceholderAlreadyBound {
        /// Placeholder value index.
        placeholder: u32,
    },
    /// A placeholder is read but was never bound.
    UnboundPlaceholder {
        /// Placeholder value index.
        placeholder: u32,
    },
    /// A placeholder is bound, directly or through other placeholders, to itself.
    PlaceholderCycle {
        /// Placeholder value index.
        placeholder: u32,
    },
    /// Two requests share a name.
    DuplicateRequestName(String),
    /// The dependency graph for this plan has a cycle.
    Cycle {
        /// Plan in which the cycle was found (e.g. `on trade`, `<all evals>`).
        plan: String,
        /// Steps that could not be ordered.
        steps: Vec<String>,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::DuplicateHandler { node, event } => {
                write!(
                    f,
                    "node `{node}` registers more than one handler for event `{event}`"
                )
            }
            BuildError::EventNameCollision {
                name,
                first,
                second,
            } => {
                write!(
                    f,
                    "event name `{name}` used by both `{first}` and `{second}`"
                )
            }
            BuildError::PlaceholderAlreadyBound { placeholder } => {
                write!(f, "placeholder #{placeholder} bound twice")
            }
            BuildError::UnboundPlaceholder { placeholder } => {
                write!(f, "placeholder #{placeholder} is read but never bound")
            }
            BuildError::PlaceholderCycle { placeholder } => {
                write!(
                    f,
                    "placeholder #{placeholder} is bound (transitively) to itself"
                )
            }
            BuildError::DuplicateRequestName(n) => write!(f, "duplicate request name `{n}`"),
            BuildError::Cycle { plan, steps } => {
                write!(
                    f,
                    "dependency cycle in plan `{plan}` among: {}",
                    steps.join(", ")
                )
            }
        }
    }
}

/// All errors found at build time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildErrors(pub Vec<BuildError>);
impl fmt::Display for BuildErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}
impl std::error::Error for BuildErrors {}

/// Builds the node registry. Everything here runs at startup.
pub struct GraphBuilder {
    pub(crate) id: u64,
    pub(crate) nodes: Vec<Box<dyn ErasedNode>>,
    pub(crate) node_meta: Vec<NodeMeta>,
    pub(crate) values: Vec<ValueDef>,
    pub(crate) value_labels: Vec<String>,
    pub(crate) kinds: Vec<EventKindMeta>,
    pub(crate) requests: Vec<RequestMeta>,
    pub(crate) errors: Vec<BuildError>,
    pub(crate) clock_policy: ClockPolicy,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBuilder {
    /// An empty builder with a fresh graph identity.
    pub fn new() -> Self {
        GraphBuilder {
            id: NEXT_GRAPH_ID.fetch_add(1, Ordering::Relaxed),
            nodes: Vec::new(),
            node_meta: Vec::new(),
            values: Vec::new(),
            value_labels: Vec::new(),
            kinds: Vec::new(),
            requests: Vec::new(),
            errors: Vec::new(),
            clock_policy: ClockPolicy::default(),
        }
    }

    fn own<S>(&self, h: NodeHandle<S>, ctx: &str) {
        assert_eq!(
            h.graph, self.id,
            "{ctx}: node handle belongs to a different GraphBuilder"
        );
    }

    fn own_value(&self, v: ValueId, ctx: &str) -> u32 {
        assert_eq!(
            v.graph, self.id,
            "{ctx}: ValueId belongs to a different GraphBuilder"
        );
        v.idx
    }

    /// Set how the engine treats timed events that arrive out of order.
    pub fn clock_policy(&mut self, policy: ClockPolicy) {
        self.clock_policy = policy;
    }

    /// Register a node with initial private state. State must be `Send` so the built
    /// engine can be moved to another thread.
    pub fn node<S: Send + 'static>(&mut self, name: impl Into<String>, state: S) -> NodeHandle<S> {
        let idx = self.nodes.len() as u32;
        self.nodes.push(Box::new(NodeImpl {
            state,
            evals: Vec::new(),
            handlers: Vec::new(),
        }));
        self.node_meta.push(NodeMeta {
            name: name.into(),
            evals: Vec::new(),
            handlers: Vec::new(),
        });
        NodeHandle {
            idx,
            graph: self.id,
            _p: PhantomData,
        }
    }

    /// Add an eval function to `node`, producing a new value. Eval functions must be pure
    /// functions of the node's state and the declared reads.
    pub fn add_eval<S, F>(&mut self, node: NodeHandle<S>, reads: &[ValueId], f: F) -> ValueId
    where
        S: Send + 'static,
        F: Fn(&S, &Reads) -> f64 + Send + 'static,
    {
        self.add_named_eval(node, "", reads, f)
    }

    /// Like [`Self::add_eval`] with a label used in plan dumps.
    pub fn add_named_eval<S, F>(
        &mut self,
        node: NodeHandle<S>,
        label: &str,
        reads: &[ValueId],
        f: F,
    ) -> ValueId
    where
        S: Send + 'static,
        F: Fn(&S, &Reads) -> f64 + Send + 'static,
    {
        self.own(node, "add_eval");
        let reads: Vec<u32> = reads
            .iter()
            .map(|&v| self.own_value(v, "add_eval"))
            .collect();
        let ni = node.idx as usize;
        let imp = self.nodes[ni]
            .as_any_mut()
            .downcast_mut::<NodeImpl<S>>()
            .expect("node handle type mismatch");
        let eval_idx = imp.evals.len();
        assert!(eval_idx < u16::MAX as usize, "too many evals on one node");
        imp.evals.push(Box::new(f));
        let vid = self.values.len() as u32;
        self.values.push(ValueDef::Eval {
            node: node.idx,
            eval_idx: eval_idx as u16,
            reads,
        });
        let name = &self.node_meta[ni].name;
        self.value_labels.push(if label.is_empty() {
            name.clone()
        } else {
            format!("{name}.{label}")
        });
        self.node_meta[ni].evals.push(vid);
        ValueId {
            idx: vid,
            graph: self.id,
        }
    }

    /// Register (or look up) an event kind and return its dispatch key.
    pub fn event<E: Event>(&mut self) -> EventKey<E> {
        let kind = self.kind_index::<E>();
        EventKey {
            kind,
            graph: self.id,
            _p: PhantomData,
        }
    }

    fn kind_index<E: Event>(&mut self) -> u32 {
        let tid = TypeId::of::<E>();
        if let Some(i) = self.kinds.iter().position(|k| k.name == E::NAME) {
            if self.kinds[i].type_id != tid {
                let err = BuildError::EventNameCollision {
                    name: E::NAME,
                    first: self.kinds[i].type_name,
                    second: std::any::type_name::<E>(),
                };
                if !self.errors.contains(&err) {
                    self.errors.push(err);
                }
            }
            return i as u32;
        }
        self.kinds.push(EventKindMeta {
            name: E::NAME,
            type_id: tid,
            type_name: std::any::type_name::<E>(),
        });
        (self.kinds.len() - 1) as u32
    }

    /// Subscribe `node` to events of type `E`. The handler may mutate the node's state and
    /// read the declared values, which the planner guarantees are evaluated first.
    pub fn on<E, S, F>(&mut self, node: NodeHandle<S>, reads: &[ValueId], mut f: F)
    where
        E: Event,
        S: Send + 'static,
        F: FnMut(&mut S, &E, &Reads) + Send + 'static,
    {
        self.own(node, "on");
        let reads: Vec<u32> = reads.iter().map(|&v| self.own_value(v, "on")).collect();
        let kind = self.kind_index::<E>();
        let ni = node.idx as usize;
        if self.node_meta[ni].handlers.iter().any(|h| h.kind == kind) {
            self.errors.push(BuildError::DuplicateHandler {
                node: self.node_meta[ni].name.clone(),
                event: E::NAME,
            });
            return;
        }
        let imp = self.nodes[ni]
            .as_any_mut()
            .downcast_mut::<NodeImpl<S>>()
            .expect("node handle type mismatch");
        let handler_idx = imp.handlers.len() as u16;
        imp.handlers
            .push(Box::new(move |s: &mut S, ev: &dyn Any, r: &Reads| {
                // Cannot fail: the planner only routes events of kind `E` here.
                let ev = ev
                    .downcast_ref::<E>()
                    .expect("planner routed wrong event type");
                f(s, ev, r)
            }));
        self.node_meta[ni].handlers.push(HandlerMeta {
            kind,
            handler_idx,
            reads,
        });
    }

    /// A value whose producer is supplied later with [`Self::bind`]. Allows forward
    /// references (and therefore makes eval cycles expressible, so they can be rejected).
    pub fn placeholder(&mut self) -> ValueId {
        let vid = self.values.len() as u32;
        self.values.push(ValueDef::Placeholder { target: None });
        self.value_labels.push(format!("placeholder#{vid}"));
        ValueId {
            idx: vid,
            graph: self.id,
        }
    }

    /// Bind a [`Self::placeholder`] to the value it stands for. Each placeholder must be bound
    /// exactly once before `build`.
    pub fn bind(&mut self, placeholder: ValueId, target: ValueId) {
        let p = self.own_value(placeholder, "bind");
        let t = self.own_value(target, "bind");
        match &mut self.values[p as usize] {
            ValueDef::Placeholder {
                target: slot @ None,
            } => *slot = Some(t),
            ValueDef::Placeholder { target: Some(_) } => self
                .errors
                .push(BuildError::PlaceholderAlreadyBound { placeholder: p }),
            ValueDef::Eval { .. } => panic!("bind: value #{p} is not a placeholder"),
        }
    }

    /// Declare a feature request: an ordered list of values to evaluate on demand.
    pub fn request(&mut self, name: &str, outputs: &[ValueId]) -> RequestKey {
        let outs: Vec<u32> = outputs
            .iter()
            .map(|&v| self.own_value(v, "request"))
            .collect();
        if self.requests.iter().any(|r| r.name == name) {
            self.errors
                .push(BuildError::DuplicateRequestName(name.to_string()));
        }
        self.requests.push(RequestMeta {
            name: name.to_string(),
            outputs: outs,
        });
        RequestKey {
            idx: (self.requests.len() - 1) as u32,
            graph: self.id,
            width: outputs.len() as u32,
        }
    }

    /// Human-readable label of a value (`node` or `node.label`), as used in plan dumps.
    pub fn value_label(&self, v: ValueId) -> &str {
        &self.value_labels[v.index()]
    }

    /// Validate, plan and produce the planned engine.
    pub fn build(self) -> Result<crate::Engine, BuildErrors> {
        crate::engine::Engine::from_builder(self)
    }

    /// Produce the recompute-everything baseline engine from the same definitions.
    pub fn build_baseline(self) -> Result<crate::BaselineEngine, BuildErrors> {
        crate::baseline::BaselineEngine::from_builder(self)
    }
}
