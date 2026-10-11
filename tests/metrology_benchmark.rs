use serde_json::json;
use triad::metrology::{Evidence, Input, Loss, Record, Split};
use triad::metrology_benchmark::{self as b, Manifest, Model, Window};

fn fixture() -> Manifest {
    let records = (0..128)
        .map(|i| {
            let split = if i < 64 {
                Split::Calibration
            } else {
                Split::Test
            };
            Record {
                id: format!("{i:05}"),
                prediction: vec![(i % 2) as f64],
                truth: Some(vec![((i + i / 5) % 2) as f64]),
                uncertainty: (i % 5) as f64 / 10.,
                error_probability: Some((i % 5) as f64 / 10.),
                sequence: if i < 64 { "cal" } else { "test" }.into(),
                spatial_cluster: None,
                scenario: if i < 64 {
                    "cal".into()
                } else {
                    format!("window-{}", i / 16)
                },
                split,
            }
        })
        .collect();
    let input = Input {
        schema_version: "triad-metrology-1".into(),
        model_id: "fixture".into(),
        experiment_id: "test".into(),
        loss: Loss::Classification,
        tolerance: 0.,
        target_coverage: 0.8,
        calibration_scenario: "cal".into(),
        bootstrap_replicates: 100,
        bootstrap_seed: 61003,
        reliability_bins: 5,
        evidence: Evidence {
            numerical_required: false,
            checks: Default::default(),
            evidence_resources: vec![],
            evaluated_scenarios: vec![],
            prospective_power_supported: false,
            independent_replication: false,
        },
        records,
    };
    let windows = (0..8)
        .map(|i| Window {
            id: format!("window-{i}"),
            split: if i < 4 {
                Split::Calibration
            } else {
                Split::Test
            },
            start_day: i * 4,
            end_day: (i + 1) * 4,
            covariate: (i % 3) as f64,
            confidence: (i % 4) as f64,
        })
        .collect();
    Manifest {
        schema_version: "triad-benchmark-1".into(),
        provenance: json!({"source_url":"fixture","sha256":"a".repeat(64),"chronology":"ordered fixture","license_as_reported":"test"}),
        protocol_sha256: "b".repeat(64),
        rows_per_day: 4,
        block_lengths: vec![1, 4, 8],
        alert_quantile: 0.8,
        degradation_margin: 0.05,
        models: vec![Model {
            model_sha256: "c".repeat(64),
            input,
            windows,
        }],
    }
}
#[test]
fn benchmark_replay_and_tamper() {
    let mut report = b::evaluate(&fixture()).unwrap();
    b::validate_report(&report).unwrap();
    report["models"][0]["policies"][0]["test_alerts"] = json!(99);
    assert!(b::validate_report(&report).is_err());
}
#[test]
fn benchmark_rejects_bad_partitions_and_assurance() {
    let mut m = fixture();
    m.models[0].windows[1].start_day = 0;
    assert!(b::validate(&m).is_err());
    let mut m = fixture();
    m.models[0].input.records.swap(1, 2);
    assert!(b::validate(&m).is_err());
    let mut m = fixture();
    m.models[0].input.evidence.independent_replication = true;
    assert!(b::validate(&m).is_err());
    let mut m = fixture();
    m.models[0].windows[0].covariate = f64::NAN;
    assert!(b::validate(&m).is_err());
    let mut v = serde_json::to_value(fixture()).unwrap();
    v["unsupported_guarantee"] = json!(true);
    assert!(serde_json::from_value::<Manifest>(v).is_err());
}
#[test]
fn final_labels_cannot_change_alerts_or_thresholds() {
    let mut m = fixture();
    let before = b::evaluate(&m).unwrap();
    for r in &mut m.models[0].input.records {
        if r.split == Split::Test {
            r.truth = None;
        }
    }
    let after = b::evaluate(&m).unwrap();
    for j in 0..3 {
        assert_eq!(
            before["models"][0]["policies"][j]["threshold"],
            after["models"][0]["policies"][j]["threshold"]
        );
        assert_eq!(
            before["models"][0]["policies"][j]["test_alerts"],
            after["models"][0]["policies"][j]["test_alerts"]
        );
        assert!(after["models"][0]["policies"][j]["gated"]["selective_error_risk"].is_null());
    }
}
#[test]
fn bootstrap_rare_missing_small_and_paired_identity() {
    let days = vec![[10., 8., 2., 8., 2., 3.]; 30];
    let r = b::block_intervals(&days, 7, 100, 5, true);
    assert_eq!(r["ci95"]["risk_b_minus_a"], json!([0., 0.]));
    assert!(b::block_intervals(&days, 20, 100, 5, true)["ci95"].is_null());
    assert!(b::block_intervals(&days, 7, 100, 5, false)["ci95"]["risk_a"].is_null());
    let rare = vec![[10., 8., 0., 8., 0., 0.]; 30];
    assert!(b::block_intervals(&rare, 7, 100, 5, true)["ci95"]["risk_a"].is_null());
}
