//! Read-only numerical/model-likelihood audit on NEW development probes.
//! This never reads test labels or modifies the frozen milestone-2 artifacts.
use crate::{
    evaluation::hash,
    forward::Model,
    inverse::{Dictionary, Estimator},
    measurements::whiten,
    research_integrity::write_new_json,
};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::path::Path;

fn squared_norm(values: impl IntoIterator<Item = f64>) -> f64 {
    values.into_iter().map(|x| x * x).sum()
}
fn whitened_distance(a: &[f64; 10], b: &[f64; 10]) -> f64 {
    squared_norm(whiten(&std::array::from_fn(|i| a[i] - b[i]), 24)).sqrt()
}
pub fn read_dictionary(root: &Path) -> Result<Dictionary> {
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("results/summary.json"))?)?;
    let key = summary["dictionary_key"]
        .as_str()
        .context("no dictionary key")?;
    ensure!(
        key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid dictionary key"
    );
    let file = root.join(format!("results/cache/dictionary-{}.json", &key[..20]));
    let dictionary: Dictionary = serde_json::from_slice(&std::fs::read(file)?)?;
    ensure!(dictionary.cache_key == key, "dictionary cache changed");
    Ok(dictionary)
}
pub fn run(root: &Path) -> Result<()> {
    let out = root.join("results/numerical-audit-v3.json");
    ensure!(
        !out.exists(),
        "numerical audit already exists; refuse overwrite"
    );
    let dictionary = read_dictionary(root)?;
    // These off-grid coordinates were not selected using the historical test labels.
    let probes = [
        [-0.0193, 0.0097, 5.4],
        [-0.0079, 0.0143, 3.7],
        [0.0047, 0.0189, 6.2],
        [0.0163, 0.0241, 4.6],
        [0.0217, 0.0119, 7.1],
        [-0.0141, 0.0263, 2.7],
    ];
    let estimator = Estimator {
        dictionary: &dictionary,
        calibration_frames: 24,
    };
    let mut rows = Vec::new();
    let mut largest_interpolation = 0.0_f64;
    let mut largest_mesh = 0.0_f64;
    let mut largest_objective_gap = 0.0_f64;
    for state in probes {
        // Refinement and interpolation errors are measured using the FULL correlated
        // observation covariance rather than nominal independent-channel sigma.
        let fine = Model::new(0.0005).solve(Some(state))?.signal;
        let coarse = Model::new(0.001).solve(Some(state))?.signal;
        let interpolated = dictionary.signal(state);
        let mesh = whitened_distance(&coarse, &fine);
        let interpolation = whitened_distance(&interpolated, &fine);
        let estimate = estimator.estimate(&fine);
        let best_discrete = dictionary
            .signals
            .iter()
            .map(|signal| whitened_distance(signal, &fine).powi(2))
            .fold(f64::INFINITY, f64::min);
        // A multistart continuous fit should not be worse than every grid state,
        // except for numerical optimizer failures.
        let fit_objective = estimate.normalized_residual.powi(2);
        let gap = (fit_objective - best_discrete).max(0.0);
        largest_interpolation = largest_interpolation.max(interpolation);
        largest_mesh = largest_mesh.max(mesh);
        largest_objective_gap = largest_objective_gap.max(gap);
        rows.push(json!({
            "development_state": state,
            "mesh_1mm_to_0_5mm_mahalanobis": mesh,
            "interpolation_to_refined_mahalanobis": interpolation,
            "optimizer_objective": fit_objective,
            "best_discrete_objective": best_discrete,
            "objective_gap_above_discrete": gap,
            "optimizer_converged": estimate.converged,
            "estimated_state": estimate.state,
        }));
    }
    // Conservative inspection flag, not a proof of a numerically exact likelihood.
    let likelihood_ready_for_claims =
        largest_interpolation <= 1.0 && largest_mesh <= 1.0 && largest_objective_gap <= 1e-6;
    let cfg = std::fs::read(root.join("experiments/config.json"))?;
    let report = json!({
        "kind": "independent_development_numerical_audit_v3",
        "baseline_config_sha256": hash(&cfg),
        "dictionary_key": dictionary.cache_key,
        "likelihood_ready_for_claims": likelihood_ready_for_claims,
        "max_mesh_error_noise_units": largest_mesh,
        "max_surrogate_error_noise_units": largest_interpolation,
        "max_optimization_objective_gap": largest_objective_gap,
        "rows": rows,
        "limitations": [
            "Synthetic 2D electrostatics; probes are not physical measurements.",
            "This is a diagnostic sampling of model mismatch, not a uniform error bound.",
            "The 1 noise-unit inspection criterion is provisional and not a validation of posterior calibration.",
            "Test labels and the historical frozen dataset were not used.",
            "A failed inspection gate does not prevent exploratory selectors, but prevents validated inference claims."
        ]
    });
    write_new_json(&out, &report)?;
    println!("Numerical audit: {}", out.display());
    println!(
        "Claim-readiness inspection passed: {}",
        likelihood_ready_for_claims
    );
    Ok(())
}
