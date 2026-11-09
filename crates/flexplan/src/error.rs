//! Run-time errors. Every variant holds only `Copy` data, so returning an error never
//! allocates on the hot path.

use std::fmt;

/// Why a dispatch or request was refused. In every case the engine state is unchanged.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DispatchError {
    /// The key came from a different [`crate::GraphBuilder`] than this engine.
    ForeignKey,
    /// [`crate::Event::validate`] rejected the event.
    InvalidEvent {
        /// Event kind name.
        event: &'static str,
        /// Reason supplied by the event type.
        reason: &'static str,
    },
    /// A timed event was older than the engine clock under [`crate::ClockPolicy::RejectBackwards`].
    ClockWentBackwards {
        /// Event kind name.
        event: &'static str,
        /// Engine clock (newest time accepted so far).
        previous_ns: u64,
        /// The rejected event's time.
        next_ns: u64,
    },
    /// The output buffer passed to `request` has the wrong length.
    OutputLength {
        /// Width of the request.
        expected: usize,
        /// Length of the buffer supplied.
        got: usize,
    },
    /// A handler or eval panicked during an earlier call, so state may be inconsistent.
    /// The engine refuses all further work; rebuild it from configuration.
    Poisoned,
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DispatchError::ForeignKey => write!(f, "key belongs to a different graph"),
            DispatchError::InvalidEvent { event, reason } => {
                write!(f, "invalid `{event}` event: {reason}")
            }
            DispatchError::ClockWentBackwards {
                event,
                previous_ns,
                next_ns,
            } => {
                write!(
                    f,
                    "`{event}` event at {next_ns} ns is older than engine clock {previous_ns} ns"
                )
            }
            DispatchError::OutputLength { expected, got } => {
                write!(
                    f,
                    "output buffer has length {got}, request width is {expected}"
                )
            }
            DispatchError::Poisoned => write!(f, "engine poisoned by an earlier panic; rebuild it"),
        }
    }
}

impl std::error::Error for DispatchError {}
