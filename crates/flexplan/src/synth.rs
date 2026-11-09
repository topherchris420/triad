//! Synthetic feature graphs of configurable size, for scaling benchmarks and differential
//! tests. Feature `i` cycles through four shapes:
//!
//! * 0: trade-subscribed window average of (trade price − mid − offset)   (history)
//! * 1: book-subscribed EMA of top-of-book size imbalance                   (state)
//! * 2: timer-subscribed decayed accumulator of the spread                  (state, timer)
//! * 3: stateless composition of two earlier features                       (eval only)

use crate::graph::{GraphBuilder, RequestKey, ValueId};
use crate::market::{self, BookUpdate, Timer, Trade};
use crate::replay::MarketKeys;
use crate::ring::RingBuffer;

/// A synthetic graph plus the keys needed to drive it.
pub struct Synthetic {
    /// Market event keys.
    pub keys: MarketKeys,
    /// Request covering every feature.
    pub request: RequestKey,
    /// Feature values in creation order.
    pub features: Vec<ValueId>,
}

/// Build a synthetic graph with `n_features` features (see module docs for the four shapes).
pub fn build_synthetic(b: &mut GraphBuilder, n_features: usize) -> Synthetic {
    let keys = MarketKeys::register(b);
    let bk = market::book(b, 16);
    let mid = market::mid(b, bk.best_bid, bk.best_ask);
    let spread = market::spread(b, bk.best_bid, bk.best_ask);
    let n_imb = b.node("imbalance", ());
    let imbalance = b.add_eval(n_imb, &[bk.bid_size, bk.ask_size], |_, r| {
        let (bs, a) = (r.get(0), r.get(1));
        if bs + a > 0.0 {
            (bs - a) / (bs + a)
        } else {
            0.0
        }
    });
    let mut features: Vec<ValueId> = Vec::with_capacity(n_features);
    for i in 0..n_features {
        let v = match i % 4 {
            0 => {
                let off = (i % 7) as f64 * 0.001;
                let h = b.node(format!("f{i}_trade_vs_mid"), RingBuffer::new(8 + i % 32));
                b.on::<Trade, _, _>(h, &[mid], move |s, t, r| {
                    let d = t.price - r.get(0) - off;
                    if d.is_finite() {
                        s.push(d)
                    }
                });
                b.add_eval(h, &[], |s, _| s.mean())
            }
            1 => {
                let alpha = 0.01 + (i % 13) as f64 * 0.01;
                let h = b.node(format!("f{i}_imb_ema"), market::Ema { alpha, value: 0.0 });
                b.on::<BookUpdate, _, _>(h, &[imbalance], |s, _, r| {
                    s.value = s.alpha * r.get(0) + (1.0 - s.alpha) * s.value;
                });
                b.add_eval(h, &[], |s, _| s.value)
            }
            2 => {
                let decay = 0.9 + (i % 9) as f64 * 0.01;
                let h = b.node(format!("f{i}_spread_decay"), (0.0f64, decay));
                b.on::<Timer, _, _>(h, &[spread], |s, _, r| {
                    let x = r.get(0);
                    if x.is_finite() {
                        s.0 = s.1 * s.0 + x;
                    }
                });
                b.add_eval(h, &[], |s, _| s.0)
            }
            _ => {
                let a = features[i - 3];
                let c = features[i - 2];
                let h = b.node(format!("f{i}_combo"), ());
                b.add_eval(h, &[a, c], |_, r| {
                    let (x, y) = (r.get(0), r.get(1));
                    if x.is_finite() && y.is_finite() {
                        x * 0.5 + y
                    } else {
                        0.0
                    }
                })
            }
        };
        features.push(v);
    }
    let request = b.request("all_features", &features);
    Synthetic {
        keys,
        request,
        features,
    }
}
