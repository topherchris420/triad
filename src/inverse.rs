//! Multistart bounded generalized least squares. Unknown dielectric nuisance.
use crate::{
    evaluation::hash,
    forward::Model,
    measurements::{whiten, Channels},
};
use anyhow::{ensure, Result};
use fenris::nalgebra::{SMatrix, SVector};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Dictionary {
    pub model: Model,
    pub axes: [Vec<f64>; 3],
    pub states: Vec<[f64; 3]>,
    pub signals: Vec<Channels>,
    pub cache_key: String,
}
impl Dictionary {
    pub fn generate(model: Model, axes: [Vec<f64>; 3], cache_dir: &str) -> Result<Self> {
        let key = hash(&serde_json::to_vec(&(
            "fenris-conforming-v1",
            model,
            &axes,
        ))?);
        let path = format!("{cache_dir}/dictionary-{}.json", &key[..20]);
        if std::path::Path::new(&path).exists() {
            let cached: Self = serde_json::from_slice(&std::fs::read(&path)?)?;
            ensure!(cached.cache_key == key, "cache mismatch");
            return Ok(cached);
        }
        let mut states = Vec::new();
        let mut signals = Vec::new();
        for x in &axes[0] {
            for y in &axes[1] {
                for e in &axes[2] {
                    let state = [*x, *y, *e];
                    signals.push(model.solve(Some(state))?.signal);
                    states.push(state);
                }
            }
            println!("Dictionary x={x:.3}: {} states", states.len());
        }
        let result = Self {
            model,
            axes,
            states,
            signals,
            cache_key: key,
        };
        std::fs::create_dir_all(cache_dir)?;
        std::fs::write(path, serde_json::to_vec(&result)?)?;
        Ok(result)
    }
    pub fn signal(&self, state: [f64; 3]) -> Channels {
        let mut index = [0; 3];
        let mut fraction = [0.0; 3];
        for d in 0..3 {
            let a = &self.axes[d];
            let v = state[d].clamp(a[0], *a.last().unwrap());
            index[d] = a
                .partition_point(|x| *x <= v)
                .saturating_sub(1)
                .min(a.len() - 2);
            fraction[d] = (v - a[index[d]]) / (a[index[d] + 1] - a[index[d]]);
        }
        let ny = self.axes[1].len();
        let ne = self.axes[2].len();
        let mut out = [0.0; 10];
        for a in 0..2 {
            for b in 0..2 {
                for c in 0..2 {
                    let corners = [a, b, c];
                    let mut w = 1.0;
                    for d in 0..3 {
                        w *= if corners[d] == 1 {
                            fraction[d]
                        } else {
                            1.0 - fraction[d]
                        };
                    }
                    let j = ((index[0] + a) * ny + index[1] + b) * ne + index[2] + c;
                    for i in 0..10 {
                        out[i] += w * self.signals[j][i];
                    }
                }
            }
        }
        out
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Estimate {
    pub state: [f64; 3],
    pub normalized_residual: f64,
    pub iterations: usize,
    pub converged: bool,
}

pub struct Estimator<'a> {
    pub dictionary: &'a Dictionary,
    pub calibration_frames: usize,
}
impl Estimator<'_> {
    fn lo(&self) -> [f64; 3] {
        std::array::from_fn(|d| self.dictionary.axes[d][0])
    }
    fn span(&self) -> [f64; 3] {
        std::array::from_fn(|d| self.dictionary.axes[d].last().unwrap() - self.lo()[d])
    }
    fn residual(&self, q: [f64; 3], measurement: &Channels) -> SVector<f64, 10> {
        let state = std::array::from_fn(|d| self.lo()[d] + q[d] * self.span()[d]);
        let signal = self.dictionary.signal(state);
        SVector::from_row_slice(&whiten(
            &std::array::from_fn(|i| signal[i] - measurement[i]),
            self.calibration_frames,
        ))
    }
    fn fit(&self, start: [f64; 3], measurement: &Channels) -> Estimate {
        let mut q = std::array::from_fn(|d| (start[d] - self.lo()[d]) / self.span()[d]);
        let mut damping = 1e-3;
        let mut iterations = 0;
        let mut converged = false;
        for iter in 0..100 {
            iterations = iter + 1;
            let r = self.residual(q, measurement);
            let cost = r.norm_squared();
            let mut j = SMatrix::<f64, 10, 3>::zeros();
            for d in 0..3 {
                let mut a = q;
                let mut b = q;
                a[d] = (a[d] - 1e-4).max(0.0);
                b[d] = (b[d] + 1e-4).min(1.0);
                j.set_column(
                    d,
                    &((self.residual(b, measurement) - self.residual(a, measurement))
                        / (b[d] - a[d])),
                );
            }
            let mut h = j.transpose() * j;
            for d in 0..3 {
                h[(d, d)] += damping * (h[(d, d)] + 1.0);
            }
            let gradient = j.transpose() * r;
            let Some(step) = h.lu().solve(&(-gradient)) else {
                break;
            };
            let candidate = std::array::from_fn(|d| (q[d] + step[d]).clamp(0.0, 1.0));
            let new_cost = self.residual(candidate, measurement).norm_squared();
            if new_cost < cost {
                let change = (0..3)
                    .map(|d| (q[d] - candidate[d]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                q = candidate;
                damping = (damping / 3.0).max(1e-10);
                if change < 1e-7 || (cost - new_cost) < 1e-9 * (1.0 + cost) {
                    converged = true;
                    break;
                }
            } else {
                // A projected step of zero can be a valid constrained optimum.
                if (0..3).all(|d| (candidate[d] - q[d]).abs() < 1e-8) {
                    converged = true;
                    break;
                }
                damping *= 10.0;
                if damping > 1e12 {
                    break;
                }
            }
        }
        Estimate {
            state: std::array::from_fn(|d| self.lo()[d] + q[d] * self.span()[d]),
            normalized_residual: self.residual(q, measurement).norm(),
            iterations,
            converged,
        }
    }
    pub fn estimate(&self, measurement: &Channels) -> Estimate {
        let mut ranking: Vec<_> = self
            .dictionary
            .states
            .iter()
            .zip(&self.dictionary.signals)
            .enumerate()
            .map(|(i, (_, s))| {
                let r = whiten(
                    &std::array::from_fn(|c| s[c] - measurement[c]),
                    self.calibration_frames,
                );
                (i, r.iter().map(|v| v * v).sum::<f64>())
            })
            .collect();
        ranking.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut starts: Vec<[f64; 3]> = Vec::new();
        for (i, _) in ranking {
            let s = self.dictionary.states[i];
            if starts
                .iter()
                .all(|t| ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2)).sqrt() > 0.006)
            {
                starts.push(s);
            }
            if starts.len() == 5 {
                break;
            }
        }
        starts
            .into_iter()
            .map(|s| self.fit(s, measurement))
            .min_by(|a, b| a.normalized_residual.total_cmp(&b.normalized_residual))
            .unwrap()
    }
}
