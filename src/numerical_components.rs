//! Milestone 4 development-only diagnostic: separate FEM discretization error
//! from dictionary interpolation error. Do NOT reinterpret the frozen v3 study.
use crate::{
    evaluation::hash,
    forward::Model,
    inverse::Dictionary,
    measurements::{whiten, Channels},
    numerical_audit::read_dictionary,
    research_integrity::write_new_json,
};
use anyhow::{ensure, Result};
use serde_json::json;
use std::path::Path;

pub const DEVELOPMENT_PROBES: [[f64; 3]; 10] = [
    // Six documented v3 development probes, reused only for diagnostic comparison.
    [-0.0193, 0.0097, 5.4],
    [-0.0079, 0.0143, 3.7],
    [0.0047, 0.0189, 6.2],
    [0.0163, 0.0241, 4.6],
    [0.0217, 0.0119, 7.1],
    [-0.0141, 0.0263, 2.7],
    // Additional prespecified development probes; NOT independent test data.
    [-0.0221, 0.0091, 7.4],
    [-0.0023, 0.0127, 3.3],
    [0.0109, 0.0213, 5.8],
    [0.0197, 0.0261, 2.4],
];

fn whitened_distance(a: &Channels, b: &Channels) -> f64 {
    let residual = std::array::from_fn(|i| a[i] - b[i]);
    whiten(&residual, 24)
        .iter()
        .map(|v| v * v)
        .sum::<f64>()
        .sqrt()
}

fn local_bracket(axis: &[f64], value: f64, subdivisions: usize) -> Result<[f64; 3]> {
    ensure!(subdivisions > 0, "at least one subdivision required");
    ensure!(
        axis.len() >= 2 && value.is_finite(),
        "invalid interpolation axis/value"
    );
    ensure!(
        axis.iter().all(|v| v.is_finite()) && axis.windows(2).all(|v| v[1] > v[0]),
        "non-finite or non-increasing interpolation axis"
    );
    ensure!(
        value >= axis[0] && value <= *axis.last().unwrap(),
        "candidate state lies outside the fitted dictionary"
    );
    let cell = axis
        .partition_point(|v| *v <= value)
        .saturating_sub(1)
        .min(axis.len() - 2);
    let x0 = axis[cell];
    let step = (axis[cell + 1] - x0) / subdivisions as f64;
    let slice = (((value - x0) / step).floor() as usize).min(subdivisions - 1);
    let lower = x0 + slice as f64 * step;
    let upper = x0 + (slice + 1) as f64 * step;
    Ok([
        lower,
        upper,
        ((value - lower) / (upper - lower)).clamp(0.0, 1.0),
    ])
}

/// Trilinear interpolation of an *independent* locally sampled physical stencil.
/// Not a validated or deployed replacement for TRIAD's frozen coarse dictionary.
/// The supplied callback can be a FEM model or an analytic synthetic test function.
pub fn local_patch<F>(
    axes: &[Vec<f64>; 3],
    state: [f64; 3],
    subdivisions: usize,
    mut sample: F,
) -> Result<Channels>
where
    F: FnMut([f64; 3]) -> Result<Channels>,
{
    let b0 = local_bracket(&axes[0], state[0], subdivisions)?;
    let b1 = local_bracket(&axes[1], state[1], subdivisions)?;
    let b2 = local_bracket(&axes[2], state[2], subdivisions)?;
    let brackets = [b0, b1, b2];
    let mut out = [0.0; 10];
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                let bits = [a, b, c];
                let at = std::array::from_fn(|d| brackets[d][bits[d]]);
                let weight: f64 = (0..3)
                    .map(|d| {
                        if bits[d] == 0 {
                            1.0 - brackets[d][2]
                        } else {
                            brackets[d][2]
                        }
                    })
                    .product();
                let signal = sample(at)?;
                for k in 0..10 {
                    out[k] += weight * signal[k];
                }
            }
        }
    }
    Ok(out)
}

pub fn run(root: &Path) -> Result<()> {
    let output = root.join("results/numerical-components-v4.json");
    ensure!(
        !output.exists(),
        "development diagnostic already exists; refuse overwrite"
    );

    // Read the original dictionary without modifying its cache or the v3 evidence.
    let dictionary: Dictionary = read_dictionary(root)?;
    let coarse_model = Model::new(0.001);
    let fine_model = Model::new(0.0005);
    let mut rows = Vec::new();
    let mut largest_mesh = 0.0_f64;
    let mut largest_original_interpolation = 0.0_f64;
    let mut largest_candidate_interpolation = 0.0_f64;
    let mut largest_candidate_total = 0.0_f64;
    for state in DEVELOPMENT_PROBES {
        let coarse = coarse_model.solve(Some(state))?.signal;
        let fine = fine_model.solve(Some(state))?.signal;
        let historical_dictionary = dictionary.signal(state);
        // Four subcells per original dictionary interval, eight fresh 1 mm
        // FEM solves around each development state. Explicitly exploratory.
        let patch = local_patch(&dictionary.axes, state, 4, |point| {
            Ok(coarse_model.solve(Some(point))?.signal)
        })?;
        let mesh_error = whitened_distance(&coarse, &fine);
        let dictionary_interpolation = whitened_distance(&historical_dictionary, &coarse);
        let dictionary_total = whitened_distance(&historical_dictionary, &fine);
        let patch_interpolation = whitened_distance(&patch, &coarse);
        let patch_total = whitened_distance(&patch, &fine);
        ensure!(
            dictionary_total <= mesh_error + dictionary_interpolation + 1e-8
                && patch_total <= mesh_error + patch_interpolation + 1e-8,
            "the whitened-distance decomposition must obey triangle inequality"
        );
        largest_mesh = largest_mesh.max(mesh_error);
        largest_original_interpolation =
            largest_original_interpolation.max(dictionary_interpolation);
        largest_candidate_interpolation = largest_candidate_interpolation.max(patch_interpolation);
        largest_candidate_total = largest_candidate_total.max(patch_total);
        rows.push(json!({
            "development_state": state,
            "mesh_1mm_vs_0_5mm_noise_units": mesh_error,
            "original_dictionary_vs_exact_1mm_noise_units": dictionary_interpolation,
            "original_dictionary_vs_exact_0_5mm_noise_units": dictionary_total,
            "local_patch_vs_exact_1mm_noise_units": patch_interpolation,
            "local_patch_vs_exact_0_5mm_noise_units": patch_total,
            "local_patch_subdivisions_per_dictionary_cell": 4
        }));
    }
    let baseline = std::fs::read(root.join("experiments/config.json"))?;
    let report = json!({
        "kind": "numerical_components_v4_development_only",
        "scientific_status": "diagnostic_not_validated_selector_or_inference_model",
        "baseline_config_sha256": hash(&baseline),
        "dictionary_key": dictionary.cache_key,
        "development_probe_count": DEVELOPMENT_PROBES.len(),
        "max_mesh_discrepancy_noise_units": largest_mesh,
        "max_original_interpolation_noise_units": largest_original_interpolation,
        "max_local_patch_interpolation_noise_units": largest_candidate_interpolation,
        "max_local_patch_total_noise_units": largest_candidate_total,
        "readiness_for_new_selective_claims": largest_mesh <= 1.0 && largest_candidate_total <= 1.0,
        "rows": rows,
        "limitations": [
            "Previously inspected v3 development probes and four additional declared development probes; no untouched scientific holdout.",
            "Localized interpolation samples eight 1 mm FEM signals per diagnostic state; it is NOT integrated into the inverse solver.",
            "A local-patch improvement cannot remove 1 mm-to-0.5 mm FEM discretization error.",
            "A one-noise-unit threshold is an inspection criterion, not proof of calibrated likelihoods.",
            "No v3 results, original dictionary, observations, or predictions were changed."
        ]
    });
    write_new_json(&output, &report)?;
    println!("Numerical components report: {}", output.display());
    println!(
        "Ready for independent selective claims: {}",
        report["readiness_for_new_selective_claims"]
    );
    Ok(())
}
