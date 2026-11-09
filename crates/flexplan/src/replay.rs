//! Deterministic replay of recorded event logs, plus a seeded synthetic market generator.
//!
//! Log format (one event per line, `#` comments allowed):
//! ```text
//! B b 100.0 5     # book update: side (b|a), price, size (0 removes level)
//! T 100.5 2       # trade: price, size
//! H 1000000       # timer: now in ns
//! R               # model requests the feature vector
//! ```

use std::fmt;

use crate::baseline::BaselineEngine;
use crate::engine::Engine;
use crate::error::DispatchError;
use crate::event::EventKey;
use crate::graph::{GraphBuilder, RequestKey};
use crate::market::{BookUpdate, Side, Timer, Trade};

/// One item of a replay stream.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Ev {
    /// Order-book update.
    Book(BookUpdate),
    /// Trade.
    Trade(Trade),
    /// Timer tick.
    Timer(Timer),
    /// The model requests its feature vector.
    Request,
}

/// Dispatch keys for the three market event kinds.
#[derive(Copy, Clone, Debug)]
pub struct MarketKeys {
    /// Key for [`BookUpdate`].
    pub book: EventKey<BookUpdate>,
    /// Key for [`Trade`].
    pub trade: EventKey<Trade>,
    /// Key for [`Timer`].
    pub timer: EventKey<Timer>,
}

impl MarketKeys {
    /// Register the three market event kinds with a builder.
    pub fn register(b: &mut GraphBuilder) -> Self {
        MarketKeys {
            book: b.event(),
            trade: b.event(),
            timer: b.event(),
        }
    }
}

/// A replay that stopped because the engine refused an item.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReplayError {
    /// Index of the refused item in the event slice.
    pub index: usize,
    /// Why the engine refused it.
    pub error: DispatchError,
}
impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "replay item {}: {}", self.index, self.error)
    }
}
impl std::error::Error for ReplayError {}

/// Anything that can consume a replay (planned engine or baseline).
pub trait Runner {
    /// Dispatch one non-request item.
    fn on_event(&mut self, k: &MarketKeys, ev: &Ev) -> Result<(), DispatchError>;
    /// Serve one feature request.
    fn on_request(&mut self, req: RequestKey, out: &mut [f64]) -> Result<(), DispatchError>;
}

impl Runner for Engine {
    #[inline]
    fn on_event(&mut self, k: &MarketKeys, ev: &Ev) -> Result<(), DispatchError> {
        match ev {
            Ev::Book(e) => self.dispatch(k.book, e),
            Ev::Trade(e) => self.dispatch(k.trade, e),
            Ev::Timer(e) => self.dispatch(k.timer, e),
            Ev::Request => Ok(()),
        }
    }
    #[inline]
    fn on_request(&mut self, req: RequestKey, out: &mut [f64]) -> Result<(), DispatchError> {
        self.request(req, out)
    }
}

impl Runner for BaselineEngine {
    #[inline]
    fn on_event(&mut self, k: &MarketKeys, ev: &Ev) -> Result<(), DispatchError> {
        match ev {
            Ev::Book(e) => self.dispatch(k.book, e),
            Ev::Trade(e) => self.dispatch(k.trade, e),
            Ev::Timer(e) => self.dispatch(k.timer, e),
            Ev::Request => Ok(()),
        }
    }
    #[inline]
    fn on_request(&mut self, req: RequestKey, out: &mut [f64]) -> Result<(), DispatchError> {
        self.request(req, out)
    }
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[inline]
fn fnv(mut h: u64, x: u64) -> u64 {
    for i in 0..8 {
        h ^= (x >> (i * 8)) & 0xff;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Replay `events`, folding every requested feature vector (exact bit patterns) into an
/// FNV-1a digest. `scratch.len()` must equal `req.width()`. Allocation-free. Stops at the
/// first refused item.
pub fn replay_digest<R: Runner>(
    r: &mut R,
    k: &MarketKeys,
    req: RequestKey,
    events: &[Ev],
    scratch: &mut [f64],
) -> Result<u64, ReplayError> {
    let mut h = FNV_OFFSET;
    for (index, ev) in events.iter().enumerate() {
        match ev {
            Ev::Request => {
                r.on_request(req, scratch)
                    .map_err(|error| ReplayError { index, error })?;
                for &x in scratch.iter() {
                    // Canonicalise NaN so the digest is stable across NaN payloads.
                    let bits = if x.is_nan() {
                        0x7ff8_0000_0000_0000
                    } else {
                        x.to_bits()
                    };
                    h = fnv(h, bits);
                }
            }
            e => r
                .on_event(k, e)
                .map_err(|error| ReplayError { index, error })?,
        }
    }
    Ok(h)
}

/// Replay and collect every requested vector (allocates; for tests and tooling).
pub fn replay_collect<R: Runner>(
    r: &mut R,
    k: &MarketKeys,
    req: RequestKey,
    events: &[Ev],
) -> Result<Vec<Vec<f64>>, ReplayError> {
    let mut out = Vec::new();
    let mut scratch = vec![0.0; req.width()];
    for (index, ev) in events.iter().enumerate() {
        match ev {
            Ev::Request => {
                r.on_request(req, &mut scratch)
                    .map_err(|error| ReplayError { index, error })?;
                out.push(scratch.clone());
            }
            e => r
                .on_event(k, e)
                .map_err(|error| ReplayError { index, error })?,
        }
    }
    Ok(out)
}

/// Parse the text log format described in the module docs. Values are parsed, not
/// validated: invalid events are rejected later by the engine.
pub fn parse_log(text: &str) -> Result<Vec<Ev>, String> {
    let mut out = Vec::new();
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> Result<f64, String> {
            f.get(i)
                .ok_or(format!("line {}: missing field {i}", ln + 1))?
                .parse::<f64>()
                .map_err(|e| format!("line {}: {e}", ln + 1))
        };
        let ev = match f[0] {
            "B" => {
                let side = match f.get(1).copied() {
                    Some("b") => Side::Bid,
                    Some("a") => Side::Ask,
                    other => return Err(format!("line {}: bad side {other:?}", ln + 1)),
                };
                Ev::Book(BookUpdate {
                    side,
                    price: num(2)?,
                    size: num(3)?,
                })
            }
            "T" => Ev::Trade(Trade {
                price: num(1)?,
                size: num(2)?,
            }),
            "H" => Ev::Timer(Timer {
                now_ns: f
                    .get(1)
                    .ok_or(format!("line {}: missing time", ln + 1))?
                    .parse()
                    .map_err(|e| format!("line {}: {e}", ln + 1))?,
            }),
            "R" => Ev::Request,
            other => return Err(format!("line {}: unknown record `{other}`", ln + 1)),
        };
        out.push(ev);
    }
    Ok(out)
}

/// Render events in the text log format (round-trips through [`parse_log`]).
pub fn format_log(events: &[Ev]) -> String {
    let mut s = String::new();
    for e in events {
        match e {
            Ev::Book(u) => s.push_str(&format!(
                "B {} {:?} {:?}\n",
                if u.side == Side::Bid { "b" } else { "a" },
                u.price,
                u.size
            )),
            Ev::Trade(t) => s.push_str(&format!("T {:?} {:?}\n", t.price, t.size)),
            Ev::Timer(t) => s.push_str(&format!("H {}\n", t.now_ns)),
            Ev::Request => s.push_str("R\n"),
        }
    }
    s
}

/// xorshift64* — tiny deterministic PRNG (no external crates).
#[derive(Clone, Debug)]
pub struct Rng(u64);
impl Rng {
    /// Seeded generator; seed 0 is mapped to 1.
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform-ish integer in `0..n` (modulo bias is irrelevant for tests).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// Synthetic market stream: ~70% book updates, ~25% trades (half repeat the previous trade
/// price exactly), ~5% timers; a request every `request_every` events. Prices on a 0.01
/// tick grid around a random-walk mid.
pub fn synth_stream(seed: u64, n: usize, request_every: usize) -> Vec<Ev> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n + n / request_every.max(1) + 1);
    let mut mid_ticks: i64 = 10_000;
    let mut last_trade: f64 = 100.0;
    let mut now: u64 = 0;
    for i in 0..n {
        let r = rng.below(100);
        if r < 70 {
            if rng.below(10) == 0 {
                mid_ticks += rng.below(3) as i64 - 1;
            }
            let side = if rng.below(2) == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            let off = 1 + rng.below(5) as i64;
            let px_ticks = if side == Side::Bid {
                mid_ticks - off
            } else {
                mid_ticks + off
            };
            let size = if rng.below(6) == 0 {
                0.0
            } else {
                (1 + rng.below(20)) as f64
            };
            out.push(Ev::Book(BookUpdate {
                side,
                price: px_ticks as f64 / 100.0,
                size,
            }));
        } else if r < 95 {
            let price = if rng.below(2) == 0 {
                last_trade
            } else {
                (mid_ticks + rng.below(3) as i64 - 1) as f64 / 100.0
            };
            last_trade = price;
            out.push(Ev::Trade(Trade {
                price,
                size: (1 + rng.below(10)) as f64,
            }));
        } else {
            now += 1_000_000 + rng.below(1_000_000);
            out.push(Ev::Timer(Timer { now_ns: now }));
        }
        if request_every > 0 && (i + 1) % request_every == 0 {
            out.push(Ev::Request);
        }
    }
    out
}
