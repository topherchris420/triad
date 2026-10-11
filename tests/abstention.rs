use triad::abstention::{
    fit, CalibrationFrame, CalibrationSequence, Decision, FitSpecification, PolicyStatus,
};

fn spec() -> FitSpecification {
    FitSpecification {
        method_id: "triad.c2.vnext.dev".into(),
        maximum_accepted_error_rate: 0.12,
        minimum_coverage: 0.8,
        delta: 0.1,
        candidate_thresholds: vec![0.2, 0.5],
    }
}
fn sequences(n: usize) -> Vec<CalibrationSequence> {
    (0..n)
        .map(|i| CalibrationSequence {
            id: format!("independent-{i}"),
            frames: vec![
                CalibrationFrame {
                    score: 0.1,
                    incorrect: false,
                },
                CalibrationFrame {
                    score: 0.2,
                    incorrect: false,
                },
                CalibrationFrame {
                    score: 0.4,
                    incorrect: false,
                },
            ],
        })
        .collect()
}
#[test]
fn calibrates_only_from_many_independent_sequences() {
    let policy = fit(&spec(), &sequences(250)).unwrap();
    assert!(matches!(policy.status, PolicyStatus::Ready));
    assert_eq!(policy.threshold, Some(0.5));
    assert_eq!(policy.independent_sequences, 250);
    assert_eq!(policy.decide(Some(0.2)), Decision::Accept);
    assert_eq!(policy.decide(Some(0.6)), Decision::WithholdAboveThreshold);
    assert_eq!(policy.decide(None), Decision::WithholdInvalidScore);
    assert_eq!(
        policy.decide(Some(f64::NAN)),
        Decision::WithholdInvalidScore
    );
}
#[test]
fn insufficient_independent_calibration_refuses_every_score() {
    let policy = fit(&spec(), &sequences(2)).unwrap();
    assert!(matches!(policy.status, PolicyStatus::Refused));
    assert!(policy.threshold.is_none());
    assert_eq!(policy.decide(Some(0.0)), Decision::WithholdUncalibrated);
}
#[test]
fn high_observed_error_refuses_even_with_many_sequences() {
    let mut rows = sequences(250);
    for seq in &mut rows {
        seq.frames[0].incorrect = true;
    }
    let policy = fit(&spec(), &rows).unwrap();
    assert!(matches!(policy.status, PolicyStatus::Refused));
}
#[test]
fn invalid_study_design_fails_closed() {
    let mut cfg = spec();
    cfg.candidate_thresholds = vec![0.3, 0.3];
    assert!(fit(&cfg, &sequences(250)).is_err());
    let mut cfg = spec();
    cfg.delta = 0.0;
    assert!(fit(&cfg, &sequences(250)).is_err());
    let mut rows = sequences(4);
    rows[1].id = rows[0].id.clone();
    assert!(fit(&spec(), &rows).is_err());
    let mut rows = sequences(4);
    rows[1].frames.pop();
    assert!(fit(&spec(), &rows).is_err());
    let mut rows = sequences(4);
    rows[0].frames[0].score = f64::INFINITY;
    assert!(fit(&spec(), &rows).is_err());
}
#[test]
fn serialization_round_trip_retains_fail_closed_behavior() {
    let p = fit(&spec(), &sequences(250)).unwrap();
    let json = serde_json::to_string(&p).unwrap();
    let restored: triad::abstention::FittedPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.decide(Some(0.1)), Decision::Accept);
    let mut damaged = restored;
    damaged.threshold = Some(f64::NAN);
    assert_eq!(damaged.decide(Some(0.1)), Decision::WithholdUncalibrated);
}
