//! `flexplan` — an independent, clean-room proof of concept of an event-driven,
//! statically planned feature-engineering engine.
//!
//! It implements the architectural ideas described publicly in Jane Street's blog post
//! "Flex Model: an event-driven feature engineering system" (2026-10-13). It is NOT Jane
//! Street's code, was written without access to any Jane Street source for Flex Model
//! (none is public), and makes no claim to match their implementation.
//!
//! Core concepts:
//! * **Nodes** own private state. Each node may expose zero or more float **values**
//!   through *eval* functions (pure functions of state + read values) and may register one
//!   *handler* per **event kind** it cares about (mutates state).
//! * Events are user-defined Rust types implementing [`Event`]; the planner matches them to
//!   handlers by name only, so new event kinds need no changes to this crate.
//! * At [`GraphBuilder::build`] time the **planner** validates the dependency graph and
//!   compiles one ordered list of steps per event kind and per feature request.
//! * At run time the [`Engine`] executes those plans in order with no cutoffs, no
//!   dynamic scheduling and no heap allocation. Invalid input is rejected before any
//!   state changes; see the contract in [`engine`].
//!
//! # Example
//!
//! ```
//! use flexplan::{DispatchError, GraphBuilder};
//! use flexplan::market::{self, BookUpdate, Side, Trade};
//! use flexplan::replay::MarketKeys;
//!
//! let mut b = GraphBuilder::new();
//! let k = MarketKeys::register(&mut b);
//! let book = market::book(&mut b, 16);
//! let mid = market::mid(&mut b, book.best_bid, book.best_ask);
//! let (_, recent) = market::recent_trade_vs_mid(&mut b, 10, mid);
//! let req = b.request("model", &[recent]);
//! let mut engine = b.build()?;
//!
//! engine.dispatch(k.book, &BookUpdate { side: Side::Bid, price: 100.0, size: 5.0 })?;
//! engine.dispatch(k.book, &BookUpdate { side: Side::Ask, price: 101.0, size: 5.0 })?;
//! engine.dispatch(k.trade, &Trade { price: 100.0, size: 1.0 })?;
//!
//! let bad = engine.dispatch(k.trade, &Trade { price: f64::NAN, size: 1.0 });
//! assert!(matches!(bad, Err(DispatchError::InvalidEvent { .. })));
//!
//! let mut v = [0.0];
//! engine.request(req, &mut v)?;
//! assert_eq!(v, [-0.5]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod baseline;
pub mod engine;
pub mod error;
pub mod event;
pub mod graph;
pub mod market;
pub mod planner;
pub mod replay;
pub mod ring;
pub mod synth;

pub use baseline::BaselineEngine;
pub use engine::{Engine, EngineStats};
pub use error::DispatchError;
pub use event::{ClockPolicy, Event, EventKey};
pub use graph::{BuildError, BuildErrors, GraphBuilder, NodeHandle, Reads, RequestKey, ValueId};
pub use planner::{Step, StepOp};
pub use ring::RingBuffer;
