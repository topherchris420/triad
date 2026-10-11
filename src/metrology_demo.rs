//! Seeded logistic regression trained only on training data.
use crate::metrology::{Evidence, Input, Loss, Record, Split};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}
fn data(seed: u64, n: usize, shifted: bool) -> Vec<([f64; 3], f64)> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| {
            let x = [1.0, rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0)];
            let logit = if shifted {
                -1.5 * x[1] + 0.7 * x[2] + 0.4
            } else {
                1.5 * x[1] - 0.7 * x[2] + 0.4
            };
            (
                x,
                if rng.gen::<f64>() < sigmoid(logit) {
                    1.0
                } else {
                    0.0
                },
            )
        })
        .collect()
}
pub fn generate() -> (Input, Value) {
    let training = data(51001, 800, false);
    let mut weights = [0.0; 3];
    for _ in 0..1500 {
        let mut grad = [0.0; 3];
        for (x, y) in &training {
            let delta = sigmoid((0..3).map(|j| weights[j] * x[j]).sum()) - y;
            for j in 0..3 {
                grad[j] += delta * x[j] / training.len() as f64;
            }
        }
        for j in 0..3 {
            weights[j] -= 0.15 * grad[j];
        }
    }
    let mut records = Vec::new();
    for (split, scenario, seed, n, shift) in [
        (Split::Training, "in_distribution", 51001, 800, false),
        (Split::Development, "in_distribution", 51006, 400, false),
        (Split::Calibration, "in_distribution", 51002, 400, false),
        (Split::Test, "in_distribution", 51003, 600, false),
        (Split::Test, "concept_shift", 51004, 600, true),
    ] {
        for (i, (x, y)) in data(seed, n, shift).into_iter().enumerate() {
            let p = sigmoid((0..3).map(|j| weights[j] * x[j]).sum());
            records.push(Record {
                id: format!("{seed}-{i}"),
                prediction: vec![if p >= 0.5 { 1.0 } else { 0.0 }],
                truth: Some(vec![y]),
                uncertainty: p.min(1.0 - p),
                error_probability: Some(p.min(1.0 - p)),
                sequence: format!("{seed}-{i}"),
                spatial_cluster: None,
                split: split.clone(),
                scenario: scenario.into(),
            });
        }
    }
    let input = Input {
        schema_version: "triad-metrology-1".into(),
        model_id: "trained-logistic-regression-v1".into(),
        experiment_id: "synthetic-transfer-demo-v5".into(),
        loss: Loss::Classification,
        tolerance: 0.0,
        target_coverage: 0.8,
        calibration_scenario: "in_distribution".into(),
        bootstrap_replicates: 500,
        bootstrap_seed: 51005,
        reliability_bins: 10,
        evidence: Evidence {
            numerical_required: false,
            checks: BTreeMap::new(),
            evidence_resources: vec!["src/metrology_demo.rs".into()],
            evaluated_scenarios: vec!["in_distribution".into()],
            prospective_power_supported: false,
            independent_replication: false,
        },
        records,
    };
    let model = json!({"algorithm":"batch logistic regression","weights":weights,"training_seed":51001,"development_seed":51006,"calibration_seed":51002,"test_seeds":[51003,51004],"training_rows":800,"iterations":1500,"learning_rate":0.15,"training_data_sha256":crate::evaluation::hash(&serde_json::to_vec(&training).unwrap()),"shift":"test concept logit reverses feature coefficients; no refitting","claim":"demonstrates calculation for a trained non-capacitive model only"});
    (input, model)
}

/// Scalar uncertainty is supplied by the caller; optimizer convergence is not confidence.
pub fn capacitive_record(
    id: String,
    estimate: &crate::inverse::Estimate,
    truth_xy_m: Option<[f64; 2]>,
    uncertainty: f64,
    sequence: String,
    split: Split,
    scenario: String,
) -> Record {
    Record {
        id,
        prediction: estimate.state[..2].to_vec(),
        truth: truth_xy_m.map(|x| x.to_vec()),
        uncertainty,
        error_probability: None,
        sequence,
        spatial_cluster: None,
        split,
        scenario,
    }
}
