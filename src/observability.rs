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

/// A LOCAL information diagnostic evaluated from the interpolated forward dictionary.
/// Scores use the stated Gaussian measurement covariance, but exclude latent future
/// drift. This is not a calibrated acceptance rule or a claim of identifiability.
#[derive(Clone, Debug, Serialize)]
pub struct PositionInformation {
    pub state: [f64; 3],
    pub unadjusted_weakest_per_mm2: f64,
    pub nuisance_adjusted_weakest_per_mm2: f64,
    pub nuisance_adjusted_strongest_per_mm2: f64,
    pub nuisance_adjusted_condition: Option<f64>,
    pub material_information: f64,
    pub material_eliminated: bool,
}

/// Compute the local Fisher-information surrogate for x/y, projecting out
/// unknown dielectric permittivity with a Schur complement.
/// The x/y derivatives are in mm and permittivity is dimensionless.
pub fn local_position_information(
    dictionary: &Dictionary,
    state: [f64; 3],
    calibration_frames: usize,
) -> PositionInformation {
    let steps = [0.0002, 0.0002, 0.1];
    let mut jacobian = [[0.0_f64; 10]; 3];
    for d in 0..3 {
        let lo = dictionary.axes[d][0];
        let hi = *dictionary.axes[d].last().unwrap();
        let mut a = state;
        let mut b = state;
        a[d] = (a[d] - steps[d]).max(lo);
        b[d] = (b[d] + steps[d]).min(hi);
        let sa = dictionary.signal(a);
        let sb = dictionary.signal(b);
        let delta = std::array::from_fn(|c| sb[c] - sa[c]);
        let whitened = whiten(&delta, calibration_frames);
        let scale = if d < 2 { 1000.0 } else { 1.0 };
        let denom = (b[d] - a[d]) * scale;
        for (c, entry) in whitened.into_iter().enumerate() {
            jacobian[d][c] = entry / denom;
        }
    }
    let mut information = [[0.0_f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            information[i][j] = (0..10)
                .map(|c| jacobian[i][c] * jacobian[j][c])
                .sum::<f64>();
        }
    }
    let eigen = |a: f64, b: f64, d: f64| {
        let disc = ((a - d).powi(2) + 4.0 * b * b).sqrt();
        (
            ((a + d - disc) / 2.0).max(0.0),
            ((a + d + disc) / 2.0).max(0.0),
        )
    };
    let (unadjusted_min, _) = eigen(information[0][0], information[0][1], information[1][1]);
    let material_info = information[2][2];
    let material_eliminated = material_info > 1e-14;
    let adjusted = |i: usize, j: usize| {
        if material_eliminated {
            information[i][j] - information[i][2] * information[2][j] / material_info
        } else {
            information[i][j]
        }
    };
    let (min_info, max_info) = eigen(adjusted(0, 0), adjusted(0, 1), adjusted(1, 1));
    let condition = if min_info > 1e-12 * max_info.max(1.0) {
        Some(max_info / min_info)
    } else {
        None
    };
    PositionInformation {
        state,
        unadjusted_weakest_per_mm2: unadjusted_min,
        nuisance_adjusted_weakest_per_mm2: min_info,
        nuisance_adjusted_strongest_per_mm2: max_info,
        nuisance_adjusted_condition: condition,
        material_information: material_info,
        material_eliminated,
    }
}

/// Re-evaluate a LOCAL, dictionary-interpolated information map without
/// reading saved target truth or modifying the original baseline experiment.
pub fn run_information_audit(root: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    use serde_json::json;
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("results/summary.json"))?)?;
    let key = summary["dictionary_key"]
        .as_str()
        .context("missing dictionary cache key")?;
    ensure!(key.len() >= 20, "invalid dictionary cache key");
    let cache = root.join(format!("results/cache/dictionary-{}.json", &key[..20]));
    let dictionary: Dictionary = serde_json::from_slice(&std::fs::read(cache)?)?;
    ensure!(dictionary.cache_key == key, "dictionary cache mismatch");
    let axes = &dictionary.axes;
    let mut points = Vec::new();
    for ix in 0..axes[0].len() - 1 {
        for iy in 0..axes[1].len() - 1 {
            for ie in 0..axes[2].len() - 1 {
                let state = [
                    (axes[0][ix] + axes[0][ix + 1]) / 2.0,
                    (axes[1][iy] + axes[1][iy + 1]) / 2.0,
                    (axes[2][ie] + axes[2][ie + 1]) / 2.0,
                ];
                points.push(local_position_information(&dictionary, state, 24));
            }
        }
    }
    let report = json!({
        "kind": "local_nuisance_adjusted_dictionary_information",
        "dictionary_key": key,
        "point_count": points.len(),
        "points": points,
        "limits": [
            "Computed at interpolated cell midpoints, not finite-element derivatives at a continuous unknown state.",
            "Accounts for nuisance dielectric permittivity but not unknown latent future drift.",
            "Noise covariance is the design assumption, not a calibrated commercial instrument.",
            "Local Fisher information cannot prove global identifiability or guarantee confidence calibration.",
            "Not an inference-time acceptance threshold, not Method D and not evidence of selective improvement."
        ]
    });
    let out = root.join("results/information-audit.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Wrote {} local observability diagnostics to {}",
        points.len(),
        out.display()
    );
    Ok(())
}
