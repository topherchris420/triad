//! Prospective sensitivity calculations, not empirical power certification.
use anyhow::{ensure, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Planning uses development labels only; exposed test labels are excluded.
pub fn pilot_summary(input: &crate::metrology::Input) -> Result<Value> {
    crate::metrology::validate(input)?;
    let rows: Vec<_> = input
        .records
        .iter()
        .filter(|r| r.split == crate::metrology::Split::Development)
        .collect();
    ensure!(
        !rows.is_empty(),
        "independent development/pilot records are required; test data cannot be used as a pilot"
    );
    ensure!(
        rows.iter().all(|r| r.truth.is_some()),
        "pilot labels must be complete"
    );
    let mut scenarios = std::collections::BTreeMap::new();
    for r in rows {
        scenarios
            .entry(&r.scenario)
            .or_insert_with(Vec::new)
            .push(r);
    }
    let mut out = Vec::new();
    for (scenario, records) in scenarios {
        let errors = records
            .iter()
            .map(|r| crate::metrology::event(r, input))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .filter(|e| **e == Some(true))
            .count();
        let p = errors as f64 / records.len() as f64;
        let planning = if p > 0.0 && p < 1.0 {
            Some(required_sample(
                p,
                0.15 * p,
                12,
                0.1,
                input.target_coverage,
            )?)
        } else {
            None
        };
        out.push(json!({"scenario":scenario,"records":records.len(),"errors":errors,"pilot_unselected_error_rate":p,"illustrative_required_predictions":planning,"warning":"Unselected pilot rate is not accepted C2 risk; effect/dependence assumptions need independent justification. Extreme event counts cannot support the normal approximation."}));
    }
    Ok(
        json!({"status":"pilot_planning_only","experiment":input.experiment_id,"pilot_scenarios":out,"test_labels_used":false}),
    )
}
use serde_json::{json, Value};

/// Synthetic sensitivity; independent pools contain four dependent sequences.
/// Shared uniform coupling is optimistic pairing, not empirical C2/D power.
pub fn simulated_power(p: f64, delta: f64, pools: usize, shared: bool, seed: u64) -> Result<Value> {
    ensure!(
        p.is_finite()
            && delta.is_finite()
            && p > 0.0
            && p < 0.5
            && delta >= 0.0
            && delta < p
            && (2..=10000).contains(&pools),
        "invalid power simulation inputs"
    );
    let replicates = 200;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut reject = 0;
    for _ in 0..replicates {
        let mut differences = Vec::new();
        for _ in 0..pools {
            let multiplier = if shared {
                if rng.gen::<bool>() {
                    1.5
                } else {
                    0.5
                }
            } else {
                1.0
            };
            let pool_uniform = rng.gen::<f64>();
            let mut diff = 0.0;
            for _ in 0..4 {
                let sequence_uniform = rng.gen::<f64>();
                for _ in 0..12 {
                    let u = if shared && rng.gen::<f64>() < 0.2 {
                        pool_uniform
                    } else if rng.gen::<f64>() < 0.3 {
                        sequence_uniform
                    } else {
                        rng.gen::<f64>()
                    };
                    diff += f64::from(u < p * multiplier) - f64::from(u < (p - delta) * multiplier);
                }
            }
            differences.push(diff / 48.0);
        }
        let mean = differences.iter().sum::<f64>() / pools as f64;
        let variance =
            differences.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (pools - 1) as f64;
        let se = (variance / pools as f64).sqrt();
        if se > 0.0 && mean > 1.96 * se {
            reject += 1;
        }
    }
    let power = reject as f64 / replicates as f64;
    Ok(
        json!({"seed":seed,"baseline_risk":p,"absolute_effect":delta,"independent_pools":pools,"sequences_per_pool":4,"accepted_frames_per_sequence":12,"shared_pool_dependence":shared,"rejections":reject,"replicates":replicates,"estimated_power":power,"monte_carlo_standard_error":(power*(1.0-power)/replicates as f64).sqrt(),"pairing":"monotone shared uniform; optimistic, not empirical","test":"positive paired pool mean exceeds 1.96 pool standard errors; approximate"}),
    )
}

pub fn required_sample(
    p: f64,
    improvement: f64,
    cluster_size: usize,
    rho: f64,
    coverage: f64,
) -> Result<usize> {
    ensure!(
        p.is_finite()
            && improvement.is_finite()
            && rho.is_finite()
            && coverage.is_finite()
            && p > 0.0
            && p < 1.0
            && improvement > 0.0
            && improvement < p
            && cluster_size > 0
            && (0.0..=1.0).contains(&rho)
            && coverage > 0.0
            && coverage <= 1.0,
        "invalid planning assumptions"
    );
    let q = p - improvement;
    // Normal approximation, independent arms, two-sided alpha=.05, power=.8.
    // Conservative for positively paired differences, not valid for arbitrary pairing.
    let accepted =
        (1.96_f64 + 0.841621).powi(2) * (p * (1.0 - p) + q * (1.0 - q)) / improvement.powi(2);
    let n = accepted * (1.0 + (cluster_size - 1) as f64 * rho) / coverage;
    ensure!(
        n.is_finite() && n < usize::MAX as f64,
        "sample size overflow"
    );
    Ok(n.ceil() as usize)
}
pub fn run() -> Result<Value> {
    let mut rows = Vec::new();
    for p in [0.005, 0.02, 0.05] {
        for effect in [0.002, 0.15 * p] {
            for rho in [0.0, 0.1, 0.5] {
                for size in [12, 48] {
                    rows.push(json!({"risk":p,"absolute_improvement":effect,"cluster_size":size,"intracluster_correlation":rho,"coverage":0.8,"required_predictions_per_arm":required_sample(p,effect,size,rho,0.8)?}));
                }
            }
        }
    }
    let mut simulations = Vec::new();
    for p in [0.01, 0.04] {
        for pools in [50, 200] {
            for shared in [false, true] {
                for seed in [52001, 52002, 52003] {
                    simulations.push(simulated_power(p, 0.15 * p, pools, shared, seed)?);
                }
            }
        }
    }
    Ok(
        json!({"status":"planning_only_not_frozen_confirmatory_design","assumed_power":0.8,"two_sided_alpha":0.05,"rows":rows,"simulations":simulations,"limitations":["Illustrative rates and effects, not independent pilot estimates.","Normal approximation may fail for rare events.","Simulation models nested shared pools and paired errors; crossed pools and actual selector pairing remain unvalidated.","No final practical effect selected without scientific utility and independent pilot.","No final test data accessed or generated; confirmatory study blocked by numerical readiness."]}),
    )
}
