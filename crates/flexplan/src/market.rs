//! Example *downstream* event kinds and composable feature nodes. Nothing in the engine
//! core (`graph`, `planner`, `engine`) depends on this module: it uses only the public API,
//! exactly as a separate library would.

use crate::event::Event;
use crate::graph::{GraphBuilder, NodeHandle, ValueId};
use crate::ring::RingBuffer;

/// Side of the order book.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    /// Buy side.
    Bid,
    /// Sell side.
    Ask,
}

/// Set the resting size at one price level (size 0 removes the level).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BookUpdate {
    /// Which side of the book the level is on.
    pub side: Side,
    /// Level price.
    pub price: f64,
    /// Resting size at the level; 0 removes it.
    pub size: f64,
}
impl Event for BookUpdate {
    const NAME: &'static str = "book_update";
    /// Policy: price must be finite (negative prices are allowed: some instruments trade
    /// below zero); size must be finite and non-negative (0 removes the level).
    fn validate(&self) -> Result<(), &'static str> {
        if !self.price.is_finite() {
            return Err("price is not finite");
        }
        if !self.size.is_finite() || self.size < 0.0 {
            return Err("size must be finite and >= 0");
        }
        Ok(())
    }
}

/// A trade print.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Trade {
    /// Trade price.
    pub price: f64,
    /// Traded size.
    pub size: f64,
}
impl Event for Trade {
    const NAME: &'static str = "trade";
    /// Policy: price finite; size finite and strictly positive.
    fn validate(&self) -> Result<(), &'static str> {
        if !self.price.is_finite() {
            return Err("price is not finite");
        }
        if !self.size.is_finite() || self.size <= 0.0 {
            return Err("size must be finite and > 0");
        }
        Ok(())
    }
}

/// A clock tick carrying the current time. Timed: subject to the engine's clock policy.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timer {
    /// Current time in nanoseconds.
    pub now_ns: u64,
}
impl Event for Timer {
    const NAME: &'static str = "timer";
    fn time_ns(&self) -> Option<u64> {
        Some(self.now_ns)
    }
}

/// Fixed-capacity sorted price levels (best first). Never allocates after construction.
#[derive(Clone, Debug)]
pub struct Levels {
    px: Box<[f64]>,
    sz: Box<[f64]>,
    len: usize,
    bids: bool,
}

impl Levels {
    /// Empty side with room for `capacity` levels; `bids` selects ordering (highest first).
    pub fn new(capacity: usize, bids: bool) -> Self {
        Levels {
            px: vec![0.0; capacity].into_boxed_slice(),
            sz: vec![0.0; capacity].into_boxed_slice(),
            len: 0,
            bids,
        }
    }
    #[inline]
    fn better(&self, a: f64, b: f64) -> bool {
        if self.bids {
            a > b
        } else {
            a < b
        }
    }
    /// Apply a level update. Levels beyond capacity are dropped (worst first). Non-finite
    /// prices or sizes are ignored (callers normally reject them via `Event::validate`).
    pub fn set(&mut self, price: f64, size: f64) {
        if !price.is_finite() || !size.is_finite() {
            return;
        }
        let mut i = 0;
        while i < self.len && self.better(self.px[i], price) {
            i += 1;
        }
        let exists = i < self.len && self.px[i] == price;
        if exists {
            if size > 0.0 {
                self.sz[i] = size;
            } else {
                self.px.copy_within(i + 1..self.len, i);
                self.sz.copy_within(i + 1..self.len, i);
                self.len -= 1;
            }
            return;
        }
        if size <= 0.0 {
            return;
        }
        let cap = self.px.len();
        if i >= cap {
            return; // worse than every retained level and book is full
        }
        let new_len = (self.len + 1).min(cap);
        // shift right, dropping the worst level if full
        let mut j = new_len - 1;
        while j > i {
            self.px[j] = self.px[j - 1];
            self.sz[j] = self.sz[j - 1];
            j -= 1;
        }
        self.px[i] = price;
        self.sz[i] = size;
        self.len = new_len;
    }
    /// Best price, or NaN if the side is empty.
    #[inline]
    pub fn best_px(&self) -> f64 {
        if self.len > 0 {
            self.px[0]
        } else {
            f64::NAN
        }
    }
    /// Size at the best price, or 0 if the side is empty.
    #[inline]
    pub fn best_sz(&self) -> f64 {
        if self.len > 0 {
            self.sz[0]
        } else {
            0.0
        }
    }
    /// Number of populated levels.
    pub fn depth(&self) -> usize {
        self.len
    }
}

/// State of the `book` node.
#[derive(Clone, Debug)]
pub struct BookState {
    /// Bid side.
    pub bids: Levels,
    /// Ask side.
    pub asks: Levels,
    /// Number of book updates handled.
    pub updates: u64,
}

/// Handles to the `book` node and its values.
pub struct BookValues {
    /// The book node.
    pub node: NodeHandle<BookState>,
    /// Best bid price (NaN if none).
    pub best_bid: ValueId,
    /// Best ask price (NaN if none).
    pub best_ask: ValueId,
    /// Size at the best bid.
    pub bid_size: ValueId,
    /// Size at the best ask.
    pub ask_size: ValueId,
}

/// Our copy of the order book. Stateful; handles `BookUpdate` only.
pub fn book(b: &mut GraphBuilder, depth: usize) -> BookValues {
    let node = b.node(
        "book",
        BookState {
            bids: Levels::new(depth, true),
            asks: Levels::new(depth, false),
            updates: 0,
        },
    );
    b.on::<BookUpdate, _, _>(node, &[], |s, u, _| {
        s.updates += 1;
        match u.side {
            Side::Bid => s.bids.set(u.price, u.size),
            Side::Ask => s.asks.set(u.price, u.size),
        }
    });
    let best_bid = b.add_named_eval(node, "best_bid", &[], |s, _| s.bids.best_px());
    let best_ask = b.add_named_eval(node, "best_ask", &[], |s, _| s.asks.best_px());
    let bid_size = b.add_named_eval(node, "bid_size", &[], |s, _| s.bids.best_sz());
    let ask_size = b.add_named_eval(node, "ask_size", &[], |s, _| s.asks.best_sz());
    BookValues {
        node,
        best_bid,
        best_ask,
        bid_size,
        ask_size,
    }
}

/// Stateless: halfway between best bid and best ask.
pub fn mid(b: &mut GraphBuilder, bid: ValueId, ask: ValueId) -> ValueId {
    let n = b.node("mid", ());
    b.add_eval(n, &[bid, ask], |_, r| 0.5 * (r.get(0) + r.get(1)))
}

/// Stateless: best ask minus best bid.
pub fn spread(b: &mut GraphBuilder, bid: ValueId, ask: ValueId) -> ValueId {
    let n = b.node("spread", ());
    b.add_eval(n, &[bid, ask], |_, r| r.get(1) - r.get(0))
}

/// Stateless difference `a - minus`.
pub fn diff(b: &mut GraphBuilder, name: &str, a: ValueId, minus: ValueId) -> ValueId {
    let n = b.node(name, ());
    b.add_eval(n, &[a, minus], |_, r| r.get(0) - r.get(1))
}

/// State of the `trade_price` node.
#[derive(Clone, Debug, Default)]
pub struct LastTrade {
    /// Most recent trade price (NaN before any).
    pub price: f64,
    /// Number of trades handled.
    pub count: u64,
}

/// Stateful: price of the most recent trade (NaN before any) and a trade counter.
pub fn last_trade(b: &mut GraphBuilder) -> (NodeHandle<LastTrade>, ValueId, ValueId) {
    let n = b.node(
        "trade_price",
        LastTrade {
            price: f64::NAN,
            count: 0,
        },
    );
    b.on::<Trade, _, _>(n, &[], |s, t, _| {
        s.price = t.price;
        s.count += 1;
    });
    let px = b.add_eval(n, &[], |s, _| s.price);
    let cnt = b.add_named_eval(n, "count", &[], |s, _| s.count as f64);
    (n, px, cnt)
}

/// Generic composable history feature: on every event of kind `E`, push the current value
/// of `input` into a window of `n`; eval returns the window mean. Non-finite inputs (e.g. a
/// one-sided book) are skipped — a policy choice of this PoC.
pub fn average_over_last_n<E: Event>(
    b: &mut GraphBuilder,
    name: &str,
    n: usize,
    input: ValueId,
) -> (NodeHandle<RingBuffer>, ValueId) {
    let h = b.node(name, RingBuffer::new(n));
    b.on::<E, _, _>(h, &[input], |s, _, r| {
        let x = r.get(0);
        if x.is_finite() {
            s.push(x)
        }
    });
    let v = b.add_eval(h, &[], |s, _| s.mean());
    (h, v)
}

/// The article's running example, shaped as one node: handle `Trade` reading `mid`,
/// push `trade.price - mid`, eval = window mean.
pub fn recent_trade_vs_mid(
    b: &mut GraphBuilder,
    n: usize,
    mid: ValueId,
) -> (NodeHandle<RingBuffer>, ValueId) {
    let h = b.node("recent_trade_vs_mid", RingBuffer::new(n));
    b.on::<Trade, _, _>(h, &[mid], |s, t, r| {
        let d = t.price - r.get(0);
        if d.is_finite() {
            s.push(d)
        }
    });
    let v = b.add_eval(h, &[], |s, _| s.mean());
    (h, v)
}

/// State of an [`ema_on`] node.
#[derive(Clone, Debug)]
pub struct Ema {
    /// Smoothing factor in (0, 1].
    pub alpha: f64,
    /// Current average (NaN until the first finite input).
    pub value: f64,
}

/// Exponential moving average of `input`, updated on every event of kind `E`.
pub fn ema_on<E: Event>(b: &mut GraphBuilder, name: &str, alpha: f64, input: ValueId) -> ValueId {
    let h = b.node(
        name,
        Ema {
            alpha,
            value: f64::NAN,
        },
    );
    b.on::<E, _, _>(h, &[input], |s, _, r| {
        let x = r.get(0);
        if x.is_finite() {
            s.value = if s.value.is_nan() {
                x
            } else {
                s.alpha * x + (1.0 - s.alpha) * s.value
            };
        }
    });
    b.add_eval(h, &[], |s, _| s.value)
}

/// State of the [`seconds_since_last_trade`] node.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    /// Latest timer time.
    pub now_ns: u64,
    /// Timer time at the most recent trade.
    pub last_trade_ns: Option<u64>,
}

/// Multi-event feature: seconds since the last trade, using the latest timer as the clock.
/// Under [`crate::ClockPolicy::AllowBackwards`] the timer may step back; the result then
/// saturates at 0 rather than underflowing.
pub fn seconds_since_last_trade(b: &mut GraphBuilder) -> ValueId {
    let h = b.node("secs_since_trade", Clock::default());
    b.on::<Timer, _, _>(h, &[], |s, t, _| s.now_ns = t.now_ns);
    b.on::<Trade, _, _>(h, &[], |s, _, _| s.last_trade_ns = Some(s.now_ns));
    b.add_eval(h, &[], |s, _| match s.last_trade_ns {
        Some(t) => s.now_ns.saturating_sub(t) as f64 * 1e-9,
        None => f64::NAN,
    })
}

/// Handles to the article-shaped example graph.
pub struct ExampleGraph {
    /// Book node and values.
    pub book: BookValues,
    /// The mid value.
    pub mid: ValueId,
    /// The `recent_trade_vs_mid` node and its value.
    pub recent: (NodeHandle<RingBuffer>, ValueId),
    /// Request for the model's feature vector.
    pub request: crate::graph::RequestKey,
}

/// Build the article's running example: `mid`, then the average of (trade price − mid)
/// over the last `n` trades, requested by a model.
pub fn article_example(b: &mut GraphBuilder, n: usize) -> ExampleGraph {
    let bk = book(b, 16);
    let m = mid(b, bk.best_bid, bk.best_ask);
    let recent = recent_trade_vs_mid(b, n, m);
    let request = b.request("model", &[recent.1]);
    ExampleGraph {
        book: bk,
        mid: m,
        recent,
        request,
    }
}
