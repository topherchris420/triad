//! Extensible event API.
//!
//! Any `'static` type can be an event. The planner only looks at [`Event::NAME`], so a
//! timer, a new feed or an inter-process message can be added in an ordinary downstream
//! module without modifying the engine.

use std::fmt;
use std::marker::PhantomData;

/// A kind of event the engine can dispatch.
pub trait Event: 'static {
    /// Stable name the planner matches handlers against. Two different Rust types may not
    /// share a name inside one graph.
    const NAME: &'static str;

    /// Reject malformed input before any handler runs. A rejected event changes no engine
    /// state. The default accepts everything.
    #[inline]
    fn validate(&self) -> Result<(), &'static str> {
        Ok(())
    }

    /// Event time in nanoseconds, if this kind carries one. Times from all event kinds
    /// feed one engine clock governed by [`ClockPolicy`]. The default is `None` (untimed).
    #[inline]
    fn time_ns(&self) -> Option<u64> {
        None
    }
}

/// What the engine does when a timed event is older than the newest time already seen.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClockPolicy {
    /// Reject the event with [`crate::DispatchError::ClockWentBackwards`]. Equal times are
    /// accepted. This is the default.
    #[default]
    RejectBackwards,
    /// Accept it. The engine clock stays at its high-water mark; features must tolerate
    /// non-monotonic time themselves.
    AllowBackwards,
}

/// A typed, pre-resolved key for dispatching events of type `E`. Obtained at startup from
/// [`crate::GraphBuilder::event`] so that dispatch needs no lookup.
pub struct EventKey<E> {
    pub(crate) kind: u32,
    pub(crate) graph: u64,
    pub(crate) _p: PhantomData<fn(&E)>,
}

impl<E> Clone for EventKey<E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E> Copy for EventKey<E> {}

impl<E: Event> fmt::Debug for EventKey<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EventKey<{}>(#{})", E::NAME, self.kind)
    }
}
