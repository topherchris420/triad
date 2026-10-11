//! Provenance checks for the archived exploratory Milestone 3 experiment.
//! These assert archival integrity, not scientific validity of the proposed method.
use serde_json::Value;
use std::{fs, path::Path};
use triad::evaluation::hash;

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn committed_selective_artifacts_match_recorded_sha256() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("results/selective-v3");
    let summary = read_json(&dir.join("summary.json"));
    assert_eq!(summary["experiment"], "selective-v3");
    assert_eq!(
        summary["protocol_sha256"].as_str().unwrap(),
        hash(&fs::read(root.join("experiments/selective-v3.json")).unwrap())
    );
    for name in ["run.json", "test-scores.json", "evaluation.json"] {
        let expected = summary["artifact_sha256"][name].as_str().unwrap();
        let actual = hash(&fs::read(dir.join(name)).unwrap());
        assert_eq!(
            expected, actual,
            "archived selective artifact changed: {name}"
        );
    }
}

#[test]
fn reported_readiness_and_decision_remain_faithful_to_recorded_results() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let numerical = read_json(&root.join("results/numerical-audit-v3.json"));
    let selective = read_json(&root.join("results/selective-v3/summary.json"));
    let geometry = read_json(&root.join("results/geometry-screen-v1.json"));

    assert_eq!(
        numerical["likelihood_ready_for_claims"].as_bool(),
        Some(false)
    );
    assert_eq!(
        numerical["baseline_config_sha256"].as_str().unwrap(),
        hash(&fs::read(root.join("experiments/config.json")).unwrap())
    );
    assert_eq!(
        selective["numerical_inspection_ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        selective["supports_declared_primary_hypothesis"].as_bool(),
        Some(false)
    );
    assert_eq!(selective["test_frames"].as_u64(), Some(3_600));
    assert_eq!(selective["test_errors_above_5mm"].as_u64(), Some(132));
    assert_eq!(
        geometry["status"].as_str(),
        Some("exploratory_surrogate_not_optimized_selective_risk")
    );
    assert_eq!(
        geometry["numerical_inspection_ready"].as_bool(),
        Some(false)
    );
    assert_eq!(
        geometry["protocol_dependency_sha256"].as_str(),
        selective["protocol_sha256"].as_str()
    );
}
