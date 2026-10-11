//! Bounded candidate search. Truth is intentionally absent from this API.
use crate::{
    inverse::Dictionary,
    measurements::{whiten, Channels},
};
use anyhow::{ensure, Result};
use serde::Serialize;
use std::{collections::BTreeMap, time::Instant};

pub fn distance(a: &Channels, b: &Channels) -> f64 {
    whiten(&std::array::from_fn(|i| a[i] - b[i]), 24)
        .iter()
        .fold(0.0_f64, |n, x| n.hypot(*x))
}
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub state: [f64; 3],
    pub residual_noise_units: f64,
    pub interpolation_discrepancy_noise_units: f64,
    /// Triangle-inequality envelope versus the exact callback, NOT physical truth.
    pub residual_envelope_noise_units: [f64; 2],
}
#[derive(Debug, Serialize)]
pub struct Refinement {
    pub candidates: Vec<Candidate>,
    pub forward_solves: usize,
    pub cache_hits: usize,
    pub elapsed_seconds: f64,
    pub abstain: bool,
    pub numerical_status: &'static str,
}
pub fn refine<F>(
    dictionary: &Dictionary,
    measurement: &Channels,
    regions: usize,
    budget: usize,
    mut sample: F,
) -> Result<Refinement>
where
    F: FnMut([f64; 3]) -> Result<Channels>,
{
    ensure!(
        (1..=16).contains(&regions) && budget > 0 && budget <= 10_000,
        "invalid refinement budget or region count"
    );
    ensure!(
        measurement.iter().all(|x| x.is_finite()),
        "invalid measurement"
    );
    for a in &dictionary.axes {
        ensure!(
            a.len() >= 2 && a.iter().all(|x| x.is_finite()) && a.windows(2).all(|v| v[0] < v[1]),
            "invalid dictionary axes"
        );
    }
    ensure!(
        dictionary.states.len() == dictionary.signals.len()
            && dictionary.states.len() == dictionary.axes.iter().map(Vec::len).product::<usize>(),
        "invalid dictionary shape"
    );
    ensure!(
        dictionary
            .states
            .iter()
            .flatten()
            .chain(dictionary.signals.iter().flatten())
            .all(|x| x.is_finite()),
        "nonfinite dictionary"
    );
    let start = Instant::now();
    // Rank cells by observed residual at centers. Retain multiple cells;
    // no assertion that this bounded shortlist exhausts plausible modes.
    let axes = &dictionary.axes;
    let mut cells = Vec::new();
    for i in 0..axes[0].len() - 1 {
        for j in 0..axes[1].len() - 1 {
            for k in 0..axes[2].len() - 1 {
                let idx = [i, j, k];
                let state = std::array::from_fn(|d| (axes[d][idx[d]] + axes[d][idx[d] + 1]) * 0.5);
                cells.push((idx, distance(&dictionary.signal(state), measurement)));
            }
        }
    }
    cells.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut cache = BTreeMap::new();
    let mut hits = 0;
    let mut candidates = Vec::new();
    let mut exhausted = false;
    'regions: for (idx, _) in cells.into_iter().take(regions) {
        // 3^3 direct forward candidates per selected cell; deterministic order.
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    let steps = [i, j, k];
                    let state = std::array::from_fn(|d| {
                        axes[d][idx[d]]
                            + (axes[d][idx[d] + 1] - axes[d][idx[d]]) * steps[d] as f64 / 2.0
                    });
                    let key = state.map(f64::to_bits);
                    let signal = if let Some(s) = cache.get(&key) {
                        hits += 1;
                        *s
                    } else {
                        if cache.len() == budget {
                            exhausted = true;
                            break 'regions;
                        }
                        let s = sample(state)?;
                        ensure!(
                            s.iter().all(|x| x.is_finite()),
                            "nonfinite forward solution"
                        );
                        cache.insert(key, s);
                        s
                    };
                    let residual = distance(&signal, measurement);
                    let delta = distance(&signal, &dictionary.signal(state));
                    candidates.push(Candidate {
                        state,
                        residual_noise_units: residual,
                        interpolation_discrepancy_noise_units: delta,
                        residual_envelope_noise_units: [
                            (residual - delta).max(0.0),
                            residual + delta,
                        ],
                    });
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.residual_noise_units
            .total_cmp(&b.residual_noise_units)
            .then_with(|| a.state[0].total_cmp(&b.state[0]))
    });
    candidates.dedup_by(|a, b| a.state == b.state);
    // Physical mesh error is unknown even if local interpolation is excellent.
    Ok(Refinement {
        candidates,
        forward_solves: cache.len(),
        cache_hits: hits,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        abstain: true,
        numerical_status: if exhausted {
            "budget_exhausted"
        } else {
            "mesh_and_search_completeness_unresolved"
        },
    })
}
