//! Finite dictionary diagnostics only; no selective method implemented.
use crate::{inverse::Dictionary, measurements::whiten};
use serde::Serialize;

#[derive(Serialize)]
pub struct Ambiguity {
    pub state: [f64; 3],
    pub alternative: [f64; 3],
    pub distance_noise_units: f64,
    pub separation_mm: f64,
}
pub fn ambiguity(dictionary: &Dictionary) -> Vec<Ambiguity> {
    let w: Vec<_> = dictionary.signals.iter().map(|s| whiten(s, 24)).collect();
    dictionary
        .states
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut best = (usize::MAX, f64::INFINITY);
            for (j, t) in dictionary.states.iter().enumerate() {
                let separation = ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2)).sqrt();
                if separation <= 0.005 {
                    continue;
                }
                let d = (0..10)
                    .map(|c| (w[i][c] - w[j][c]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if d < best.1 {
                    best = (j, d);
                }
            }
            let t = dictionary.states[best.0];
            Ambiguity {
                state: *s,
                alternative: t,
                distance_noise_units: best.1,
                separation_mm: 1000.0 * ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2)).sqrt(),
            }
        })
        .collect()
}
pub fn position_sensitivity(dictionary: &Dictionary, state: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for d in 0..2 {
        let mut a = state;
        let mut b = state;
        let axis = &dictionary.axes[d];
        a[d] = (a[d] - 0.0002).max(axis[0]);
        b[d] = (b[d] + 0.0002).min(*axis.last().unwrap());
        let z = dictionary.signal(a);
        let w = dictionary.signal(b);
        for c in 0..10 {
            sum += ((w[c] - z[c]) / (b[d] - a[d])).powi(2);
        }
    }
    sum.sqrt() / 1000.0 // pF/m per mm of position displacement
}
