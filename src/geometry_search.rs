//! Exploratory four-electrode geometry screening. Never infer hardware performance
//! from these idealized 2D FEM information-distance surrogates.
use crate::{
    evaluation::hash,
    forward::{ElectrodeLayout, Model},
    measurements::{whiten, Channels},
    research_integrity::{validate_selective_protocol, write_new_json},
};
use anyhow::{ensure, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::json;
use std::path::Path;

fn candidate_layouts() -> Vec<(&'static str, ElectrodeLayout)> {
    vec![
        ("frozen_baseline", ElectrodeLayout::default()),
        (
            "compact",
            ElectrodeLayout {
                centers: [-0.024, -0.008, 0.008, 0.024],
                half_width: 0.004,
            },
        ),
        (
            "wide",
            ElectrodeLayout {
                centers: [-0.030, -0.010, 0.010, 0.030],
                half_width: 0.004,
            },
        ),
        (
            "split_outer",
            ElectrodeLayout {
                centers: [-0.030, -0.008, 0.008, 0.030],
                half_width: 0.004,
            },
        ),
        (
            "split_inner",
            ElectrodeLayout {
                centers: [-0.024, -0.010, 0.010, 0.024],
                half_width: 0.004,
            },
        ),
    ]
}
fn sites(seed: u64) -> Vec<[f64; 3]> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..24)
        .map(|_| {
            [
                rng.gen_range(-0.023..0.023),
                rng.gen_range(0.0085..0.0275),
                rng.gen_range(2.1..7.9),
            ]
        })
        .collect()
}
fn profile(layout: ElectrodeLayout, states: &[[f64; 3]]) -> Result<serde_json::Value> {
    let model = Model::new(0.001);
    layout.validate(model.geometry)?;
    let signals: Vec<Channels> = states
        .iter()
        .map(|s| model.solve_with_layout(Some(*s), layout))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|sol| sol.signal)
        .collect();
    let mut distances = Vec::new();
    for i in 0..states.len() {
        for j in 0..i {
            let separation = ((states[i][0] - states[j][0]).powi(2)
                + (states[i][1] - states[j][1]).powi(2))
            .sqrt();
            if separation <= 0.005 {
                continue;
            }
            let diff = std::array::from_fn(|c| signals[i][c] - signals[j][c]);
            let w = whiten(&diff, 24);
            let distance = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            distances.push(distance);
        }
    }
    ensure!(!distances.is_empty(), "not enough spatial pairs");
    distances.sort_by(f64::total_cmp);
    let low = (distances.len() / 10).min(distances.len() - 1);
    let within_two = distances.iter().filter(|d| **d < 2.0).count();
    Ok(json!({
        "site_count":states.len(),
        "pairs_at_least_5mm_apart":distances.len(),
        "minimum_distance_in_noise_units":distances[0],
        "p10_pairwise_distance_in_noise_units":distances[low],
        "fraction_pairs_within_two_noise_units":within_two as f64 / distances.len() as f64
    }))
}
pub fn run(root: &Path) -> Result<()> {
    let result_dir = root.join("results");
    let out = result_dir.join("geometry-screen-v1.json");
    ensure!(
        !out.exists(),
        "geometry screen already exists; refuse overwrite"
    );
    // This gate enforces research ordering; it does NOT assert that inference succeeded.
    let previous: serde_json::Value = serde_json::from_slice(&std::fs::read(
        result_dir.join("selective-v3/summary.json"),
    )?)?;
    let protocol = std::fs::read(root.join("experiments/selective-v3.json"))?;
    validate_selective_protocol(&serde_json::from_slice(&protocol)?)?;
    ensure!(
        previous["experiment"] == "selective-v3" && previous["protocol_sha256"] == hash(&protocol),
        "selective study does not match current protocol"
    );
    for name in ["run.json", "test-scores.json", "evaluation.json"] {
        let bytes = std::fs::read(result_dir.join("selective-v3").join(name))?;
        ensure!(
            previous["artifact_sha256"][name] == hash(&bytes),
            "selective study artifact missing or changed: {name}"
        );
    }
    let numerical_ready = previous["numerical_inspection_ready"] == true;
    let development = sites(9317001);
    let holdout = sites(9317002);
    let mut candidates = Vec::new();
    for (name, layout) in candidate_layouts() {
        let score = profile(layout, &development)?;
        candidates.push((name, layout, score));
    }
    candidates.sort_by(|a, b| {
        b.2["p10_pairwise_distance_in_noise_units"]
            .as_f64()
            .unwrap()
            .total_cmp(
                &a.2["p10_pairwise_distance_in_noise_units"]
                    .as_f64()
                    .unwrap(),
            )
    });
    let selected = &candidates[0];
    // The holdout is evaluated only AFTER selecting on the development set.
    let selected_holdout = profile(selected.1, &holdout)?;
    let baseline_holdout = profile(ElectrodeLayout::default(), &holdout)?;
    let report = json!({
        "kind":"geometry_screen_v1",
        "status":"exploratory_surrogate_not_optimized_selective_risk",
        "selection_objective":"maximize 10th-percentile whitened pairwise distance of states separated over 5 mm",
        "selection": {
            "layout":selected.0,
            "centers_m":selected.1.centers,
            "half_width_m":selected.1.half_width,
            "development":selected.2,
            "untouched_holdout":selected_holdout
        },
        "default_holdout":baseline_holdout,
        "all_development_candidates": candidates.iter().map(|(name,layout,metrics)| json!({
            "name":name,"layout":layout,"metrics":metrics
        })).collect::<Vec<_>>(),
        "same_region_channels_width_noise_and_fem_mesh":true,
        "independent_site_seeds":[9317001,9317002],
        "protocol_dependency_sha256":hash(&std::fs::read(root.join("experiments/selective-v3.json"))?),
        "numerical_inspection_ready":numerical_ready,
        "limits":[
            "Screening is NOT a validated geometry improvement in selective risk.",
            "All layouts have four strips and ten theoretical channels, but capacitance readout parasitics are omitted.",
            "The pair-distance metric is an incomplete surrogate for globally reliable localization.",
            "The same synthetic area, material range and covariance are used for all candidates.",
            "The selected layout needs its own inverse dictionary, C2/D calibration and fresh independent final test sequences.",
            "No 3D or physical-device evidence, no manufacturing constraints, and no original sensor superiority claim."
        ]
    });
    write_new_json(&out, &report)?;
    println!("Wrote preliminary electrode screen to {}", out.display());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_layouts_preserve_channel_count_and_electrode_width() {
        let model = Model::new(0.001);
        for (_, layout) in candidate_layouts() {
            layout.validate(model.geometry).unwrap();
            assert_eq!(layout.half_width, 0.004);
            assert_eq!(layout.centers.len(), 4);
        }
    }
    #[test]
    fn invalid_overlapping_electrodes_fail_closed() {
        let layout = ElectrodeLayout {
            centers: [-0.02, -0.015, 0.01, 0.02],
            half_width: 0.004,
        };
        assert!(layout.validate(Model::new(0.001).geometry).is_err());
    }
}
