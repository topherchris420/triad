//! Quantifies feasibility of an *exploratory* practical-effect threshold.
//! Does not choose the study parameters or license reused final test labels.
use anyhow::{ensure, Result};

pub fn risk_reduction_is_feasible(
    c2_accepted_error_risk: f64,
    required_absolute_reduction: f64,
) -> Result<bool> {
    ensure!(
        c2_accepted_error_risk.is_finite()
            && (0.0..=1.0).contains(&c2_accepted_error_risk)
            && required_absolute_reduction.is_finite()
            && (0.0..=1.0).contains(&required_absolute_reduction),
        "risk and improvement must each be valid probabilities"
    );
    // Risk cannot fall below zero. The observed baseline risk therefore upper
    // bounds achievable absolute improvement ON THESE PARTICULAR OBSERVATIONS.
    Ok(required_absolute_reduction <= c2_accepted_error_risk)
}
