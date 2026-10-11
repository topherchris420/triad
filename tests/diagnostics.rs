use triad::{diagnostics, experiment::EvaluationRow};

fn row(scenario: &str, error_mm: f64, residual: f64, converged: bool) -> EvaluationRow {
    EvaluationRow {
        scenario: scenario.to_owned(),
        sequence_id: 1,
        frame: 0,
        truth: [0.0, 0.0, 4.0],
        estimate: [0.0, 0.0, 4.0],
        error_mm,
        residual,
        converged,
    }
}

#[test]
fn distinguishes_optimizer_flags_from_large_localization_errors() {
    let rows = vec![
        row("drift", 8.0, 1.0, true),
        row("drift", 1.0, 3.0, false),
        row("drift", 7.0, 2.0, false),
        row("clean", 0.5, 0.4, true),
    ];
    let result = diagnostics::audit(&rows, "fixed-input-digest");
    assert_eq!(result["all"]["total"]["count"], 4);
    assert_eq!(result["all"]["nonconverged"]["count"], 2);
    assert_eq!(result["all"]["nonconverged"]["over_5mm_count"], 1);
    assert_eq!(result["large_errors_with_converged_flag"], 1);
    assert_eq!(result["scenarios"]["drift"]["total"]["count"], 3);
    assert_eq!(result["scenarios"]["clean"]["converged"]["count"], 1);
    assert_eq!(result["source_sha256"], "fixed-input-digest");
}

#[test]
fn empty_partition_remains_explicitly_null_not_nan() {
    let rows = vec![row("clean", 0.2, 0.1, true)];
    let result = diagnostics::audit(&rows, "digest");
    assert_eq!(result["scenarios"]["clean"]["nonconverged"]["count"], 0);
    assert!(result["scenarios"]["clean"]["nonconverged"]["over_5mm_rate"].is_null());
}

#[test]
fn audit_is_deterministic_for_frozen_input() {
    let rows = vec![
        row("b", 1.0, 3.5, false),
        row("a", 5.1, 4.0, true),
    ];
    assert_eq!(diagnostics::audit(&rows, "sha"), diagnostics::audit(&rows, "sha"));
}
