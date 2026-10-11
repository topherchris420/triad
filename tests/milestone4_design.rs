use triad::study_design::risk_reduction_is_feasible;

#[test]
fn cannot_demand_more_absolute_improvement_than_observed_baseline_risk() {
    // Retrospective diagnostic of v3, NOT a novel inferential result.
    let c2_risk = 42.0 / 2880.0;
    assert!(!risk_reduction_is_feasible(c2_risk, 0.02).unwrap());
    assert!(risk_reduction_is_feasible(c2_risk, 0.002).unwrap());
    assert!(risk_reduction_is_feasible(c2_risk, c2_risk).unwrap());
}

#[test]
fn invalid_risks_and_effects_fail_closed() {
    for (risk, effect) in [
        (f64::NAN, 0.02),
        (0.4, f64::INFINITY),
        (-0.1, 0.01),
        (1.1, 0.01),
        (0.5, -0.1),
    ] {
        assert!(risk_reduction_is_feasible(risk, effect).is_err());
    }
}
