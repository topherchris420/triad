use serde_json::json;
use std::collections::BTreeMap;
use triad::metrology::*;
fn fixture() -> Input {
    let mut rows = Vec::new();
    for (prefix, split) in [("cal", Split::Calibration), ("test", Split::Test)] {
        for (i, (score, error)) in [(0.1, false), (0.2, true), (0.3, false), (0.4, true)]
            .into_iter()
            .enumerate()
        {
            rows.push(Record {
                id: format!("{prefix}-{i}"),
                prediction: vec![0.0],
                truth: Some(vec![if error { 1.0 } else { 0.0 }]),
                uncertainty: score,
                error_probability: Some(score),
                sequence: format!("{prefix}-{i}"),
                spatial_cluster: Some(format!("{prefix}-space-{i}")),
                split: split.clone(),
                scenario: "iid".into(),
            });
        }
    }
    Input {
        schema_version: "triad-metrology-1".into(),
        model_id: "fixture".into(),
        experiment_id: "hand-check".into(),
        loss: Loss::Classification,
        tolerance: 0.0,
        target_coverage: 0.5,
        calibration_scenario: "iid".into(),
        bootstrap_replicates: 100,
        bootstrap_seed: 5,
        reliability_bins: 2,
        evidence: Evidence {
            numerical_required: true,
            checks: BTreeMap::new(),
            evidence_resources: vec![],
            evaluated_scenarios: vec!["iid".into()],
            prospective_power_supported: false,
            independent_replication: false,
        },
        records: rows,
    }
}
#[test]
fn hand_verified_metrics() {
    let r = evaluate(&fixture()).unwrap();
    let m = &r["overall"]["operational"];
    assert_eq!(m["coverage"], 0.5);
    assert_eq!(m["selective_error_risk"], 0.5);
    assert_eq!(m["incorrect_acceptance_frequency"], 0.25);
    assert!(
        (r["overall"]["aurc"].as_f64().unwrap() - (0.0 + 0.5 + 1.0 / 3.0 + 0.5) / 4.0).abs()
            < 1e-12
    );
    assert!((r["overall"]["brier"].as_f64().unwrap() - 0.275).abs() < 1e-12);
    assert_eq!(r["overall"]["ece"], 0.25);
    validate_report(&r).unwrap();
    let mut corrupt = r;
    corrupt["overall"]["operational"]["coverage"] = json!(0.9);
    assert!(validate_report(&corrupt).is_err());
}
#[test]
fn labels_never_select_threshold_and_splits_fail_closed() {
    let mut f = fixture();
    let t = threshold(&f);
    for r in &mut f.records {
        r.truth = Some(vec![99.0]);
    }
    assert_eq!(t, threshold(&f));
    f.records[4].sequence = f.records[0].sequence.clone();
    assert!(validate(&f).is_err());
    let mut f = fixture();
    f.records[4].spatial_cluster = f.records[0].spatial_cluster.clone();
    assert!(validate(&f).is_err());
}
#[test]
fn tie_groups_and_order_are_deterministic() {
    let mut f = fixture();
    for r in &mut f.records {
        r.uncertainty = 0.1;
    }
    let a = evaluate(&f).unwrap();
    assert_eq!(a["overall"]["operational"]["coverage"], 1.0);
    assert_eq!(
        a["overall"]["retrospective_matched_coverage"]["coverage"],
        0.5
    );
    assert_eq!(a["overall"]["aurc"], 0.5);
    f.records.reverse();
    let b = evaluate(&f).unwrap();
    assert_eq!(a["overall"], b["overall"]);
}
#[test]
fn empty_missing_rare_and_invalid_are_explicit() {
    let mut f = fixture();
    f.target_coverage = 0.0;
    let r = evaluate(&f).unwrap();
    assert!(r["overall"]["operational"]["selective_error_risk"].is_null());
    let mut f = fixture();
    f.records[4].truth = None;
    let r = evaluate(&f).unwrap();
    assert!(r["overall"]["aurc"].is_null());
    assert!(r["overall"]["operational"]["selective_error_risk"].is_null());
    for r in &mut f.records {
        r.truth = Some(r.prediction.clone());
    }
    let r = evaluate(&f).unwrap();
    assert!(r["overall"]["sequence_bootstrap"]["ci95"].is_null());
    f.records[0].uncertainty = f64::NAN;
    assert!(evaluate(&f).is_err());
    let mut f = fixture();
    f.records[4].id = f.records[0].id.clone();
    assert!(evaluate(&f).is_err());
    let mut f = fixture();
    f.schema_version = "v3".into();
    assert!(evaluate(&f).is_err());
    let mut f = fixture();
    f.records[4].error_probability = Some(1.1);
    assert!(evaluate(&f).is_err());
}
#[test]
fn bootstrap_resamples_whole_clusters_and_reproduces() {
    let mut f = fixture();
    f.target_coverage = 1.0;
    for i in 4..8 {
        f.records[i].sequence = if i % 2 == 0 { "correct" } else { "wrong" }.into();
    }
    let a = evaluate(&f).unwrap();
    let b = evaluate(&f).unwrap();
    assert_eq!(a, b);
    assert_eq!(a["overall"]["sequence_bootstrap"]["clusters"], 2);
    assert_eq!(
        a["overall"]["sequence_bootstrap"]["ci95"],
        json!([0.0, 1.0])
    );
}
#[test]
fn numerical_failure_cannot_be_hidden_by_small_interpolation_or_convergence() {
    let mut f = fixture();
    f.evidence.checks.insert("interpolation".into(), Some(true));
    f.evidence.checks.insert("solver".into(), Some(true));
    f.evidence.checks.insert("mesh".into(), Some(false));
    let r = evaluate(&f).unwrap();
    assert_eq!(r["validity"]["numerical_status"], "numerically_unresolved");
    assert_eq!(r["validity"]["validated_physical_confidence"], false);
    let e = triad::inverse::Estimate {
        state: [0.0, 0.0, 5.0],
        normalized_residual: 0.0,
        iterations: 1,
        converged: true,
    };
    let r = triad::metrology_demo::capacitive_record(
        "adversarial".into(),
        &e,
        Some([0.02, 0.0]),
        0.0,
        "s".into(),
        Split::Test,
        "iid".into(),
    );
    f.loss = Loss::Euclidean;
    f.tolerance = 0.005;
    assert_eq!(event(&r, &f).unwrap(), Some(true));
}
#[test]
fn trained_model_demonstrates_shift_failure_without_refitting() {
    let (f, model) = triad::metrology_demo::generate();
    assert_ne!(model["weights"], json!([0.0, 0.0, 0.0]));
    let r = evaluate(&f).unwrap();
    let iid = &r["scenarios"]["in_distribution"];
    let shifted = &r["scenarios"]["concept_shift"];
    assert!(
        shifted["operational"]["selective_error_risk"]
            .as_f64()
            .unwrap()
            > iid["operational"]["selective_error_risk"].as_f64().unwrap()
    );
    assert_eq!(
        r["validity"]["out_of_evaluated_distribution"],
        json!(["concept_shift"])
    );
    assert_eq!(triad::metrology_demo::generate().1, model);
}
#[test]
fn historical_hash_manifest_matches_bytes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../docs/metrology/baseline-hashes.json")).unwrap();
    for (file, digest) in manifest["files"].as_object().unwrap() {
        assert_eq!(
            triad::evaluation::hash(&std::fs::read(root.join(file)).unwrap()),
            digest.as_str().unwrap(),
            "{file}"
        );
    }
}
#[test]
fn planning_exposes_dependence_and_rejects_impossible_effects() {
    use triad::metrology_planning::required_sample;
    assert!(
        required_sample(0.02, 0.002, 12, 0.5, 0.8).unwrap()
            > required_sample(0.02, 0.002, 12, 0.0, 0.8).unwrap()
    );
    assert!(required_sample(0.001, 0.002, 12, 0.0, 0.8).is_err());
    assert!(triad::metrology_planning::pilot_summary(&fixture()).is_err());
    let (mut f, _) = triad::metrology_demo::generate();
    let a = triad::metrology_planning::pilot_summary(&f).unwrap();
    for r in &mut f.records {
        if r.split == Split::Test {
            r.truth = Some(vec![999.0]);
        }
    }
    assert_eq!(a, triad::metrology_planning::pilot_summary(&f).unwrap());
    let null = triad::metrology_planning::simulated_power(0.02, 0.0, 10, true, 5).unwrap();
    assert_eq!(null["rejections"], 0);
    assert_eq!(
        null,
        triad::metrology_planning::simulated_power(0.02, 0.0, 10, true, 5).unwrap()
    );
}
