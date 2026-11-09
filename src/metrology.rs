//! Descriptive selective-risk measurement. No inferred deployment guarantee.
use anyhow::{ensure, Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    Training,
    Development,
    Calibration,
    Test,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub prediction: Vec<f64>,
    pub truth: Option<Vec<f64>>,
    pub uncertainty: f64,
    pub error_probability: Option<f64>,
    pub sequence: String,
    pub spatial_cluster: Option<String>,
    pub split: Split,
    pub scenario: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Loss {
    Euclidean,
    Classification,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub numerical_required: bool,
    /// Each dimension requires evidence; a single mesh agreement is insufficient.
    pub checks: BTreeMap<String, Option<bool>>,
    pub evidence_resources: Vec<String>,
    pub evaluated_scenarios: Vec<String>,
    pub prospective_power_supported: bool,
    pub independent_replication: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub schema_version: String,
    pub model_id: String,
    pub experiment_id: String,
    pub loss: Loss,
    pub tolerance: f64,
    pub target_coverage: f64,
    pub calibration_scenario: String,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: u64,
    pub reliability_bins: usize,
    pub evidence: Evidence,
    pub records: Vec<Record>,
}

pub fn validate(input: &Input) -> Result<()> {
    ensure!(
        input.schema_version == "triad-metrology-1",
        "unsupported schema_version"
    );
    ensure!(
        !input.model_id.trim().is_empty() && !input.experiment_id.trim().is_empty(),
        "model_id and experiment_id are required"
    );
    ensure!(
        input.tolerance.is_finite() && input.tolerance >= 0.0,
        "tolerance must be finite and nonnegative"
    );
    if matches!(input.loss, Loss::Classification) {
        ensure!(
            input.tolerance < 1.0,
            "classification tolerance must be below 1"
        );
    }
    ensure!(
        input.target_coverage.is_finite() && (0.0..=1.0).contains(&input.target_coverage),
        "target_coverage must be in [0,1]"
    );
    ensure!(
        (100..=100_000).contains(&input.bootstrap_replicates),
        "bootstrap_replicates must be 100..100000"
    );
    ensure!(
        (1..=1000).contains(&input.reliability_bins),
        "reliability_bins must be 1..1000"
    );
    ensure!(!input.records.is_empty(), "records must not be empty");
    let mut ids = BTreeSet::new();
    let mut sequences = BTreeMap::new();
    let mut spatial = BTreeMap::new();
    let dim = input.records[0].prediction.len();
    ensure!(dim > 0, "prediction vectors must not be empty");
    for r in &input.records {
        ensure!(
            !r.id.trim().is_empty() && ids.insert(&r.id),
            "empty or duplicate record id: {}",
            r.id
        );
        ensure!(
            !r.sequence.trim().is_empty() && !r.scenario.trim().is_empty(),
            "{}: sequence and scenario required",
            r.id
        );
        if let Some(s) = sequences.insert(&r.sequence, &r.split) {
            ensure!(
                s == &r.split,
                "sequence {} crosses dataset splits",
                r.sequence
            );
        }
        if let Some(c) = &r.spatial_cluster {
            ensure!(!c.trim().is_empty(), "empty spatial cluster");
            if let Some(s) = spatial.insert(c, &r.split) {
                ensure!(s == &r.split, "spatial cluster {c} crosses dataset splits");
            }
        }
        ensure!(
            r.uncertainty.is_finite(),
            "{}: uncertainty must be finite",
            r.id
        );
        ensure!(
            r.prediction.len() == dim && r.prediction.iter().all(|x| x.is_finite()),
            "{}: invalid prediction",
            r.id
        );
        if let Some(y) = &r.truth {
            ensure!(
                y.len() == dim && y.iter().all(|x| x.is_finite()),
                "{}: invalid truth",
                r.id
            );
        }
        if let Some(p) = r.error_probability {
            ensure!(
                p.is_finite() && (0.0..=1.0).contains(&p),
                "{}: error_probability must be in [0,1]",
                r.id
            );
        }
        if matches!(input.loss, Loss::Classification) {
            ensure!(
                dim == 1
                    && r.prediction[0].fract() == 0.0
                    && r.truth.as_ref().is_none_or(|y| y[0].fract() == 0.0),
                "classification requires scalar integer class labels"
            );
        }
    }
    ensure!(
        input
            .records
            .iter()
            .any(|r| r.split == Split::Calibration && r.scenario == input.calibration_scenario),
        "no records for calibration_scenario in calibration split"
    );
    ensure!(
        input.records.iter().any(|r| r.split == Split::Test),
        "test split is empty"
    );
    Ok(())
}

pub fn event(r: &Record, input: &Input) -> Result<Option<bool>> {
    r.truth
        .as_ref()
        .map(|y| {
            let loss = match input.loss {
                Loss::Classification => {
                    if r.prediction == *y {
                        0.0
                    } else {
                        1.0
                    }
                }
                Loss::Euclidean => r
                    .prediction
                    .iter()
                    .zip(y)
                    .fold(0.0_f64, |norm, (a, b)| norm.hypot(a - b)),
            };
            ensure!(loss.is_finite(), "{}: loss overflow", r.id);
            Ok(loss > input.tolerance)
        })
        .transpose()
}

/// Calibration reads scores only. Boundary ties are all accepted. None means reject all.
pub fn threshold(input: &Input) -> Option<f64> {
    let mut scores: Vec<_> = input
        .records
        .iter()
        .filter(|r| r.split == Split::Calibration && r.scenario == input.calibration_scenario)
        .map(|r| r.uncertainty)
        .collect();
    scores.sort_by(f64::total_cmp);
    let n = (input.target_coverage * scores.len() as f64).ceil() as usize;
    if n == 0 {
        None
    } else {
        scores.get(n - 1).copied()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub count: usize,
    pub accepted: usize,
    pub labeled: usize,
    pub accepted_labeled: usize,
    pub errors: usize,
    pub accepted_errors: usize,
    pub coverage: f64,
    pub abstention_frequency: f64,
    pub selective_error_risk: Option<f64>,
    pub incorrect_acceptance_frequency: Option<f64>,
    pub error_rate: Option<f64>,
}
pub fn metrics(rows: &[&Record], accepted: &[bool], input: &Input) -> Result<Metrics> {
    ensure!(
        !rows.is_empty() && rows.len() == accepted.len(),
        "metric rows/acceptance mismatch or empty input"
    );
    let events: Vec<_> = rows
        .iter()
        .map(|r| event(r, input))
        .collect::<Result<_>>()?;
    let count = rows.len();
    let n = accepted.iter().filter(|a| **a).count();
    let labeled = events.iter().filter(|e| e.is_some()).count();
    let accepted_labeled = events
        .iter()
        .zip(accepted)
        .filter(|(e, a)| e.is_some() && **a)
        .count();
    let errors = events.iter().filter(|e| **e == Some(true)).count();
    let accepted_errors = events
        .iter()
        .zip(accepted)
        .filter(|(e, a)| **e == Some(true) && **a)
        .count();
    Ok(Metrics {
        count,
        accepted: n,
        labeled,
        accepted_labeled,
        errors,
        accepted_errors,
        coverage: n as f64 / count as f64,
        abstention_frequency: 1.0 - n as f64 / count as f64,
        selective_error_risk: if n > 0 && accepted_labeled == n {
            Some(accepted_errors as f64 / n as f64)
        } else {
            None
        },
        incorrect_acceptance_frequency: if labeled == count {
            Some(accepted_errors as f64 / count as f64)
        } else {
            None
        },
        error_rate: if labeled == count {
            Some(errors as f64 / count as f64)
        } else {
            None
        },
    })
}

fn bootstrap(rows: &[&Record], accepted: &[bool], input: &Input, spatial: bool) -> Result<Value> {
    if spatial && rows.iter().any(|r| r.spatial_cluster.is_none()) {
        return Ok(json!({"status":"missing_cluster_ids","ci95":null}));
    }
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        groups
            .entry(if spatial {
                r.spatial_cluster.as_deref().unwrap()
            } else {
                &r.sequence
            })
            .or_default()
            .push(i);
    }
    let groups: Vec<_> = groups.values().collect();
    let base = metrics(rows, accepted, input)?;
    if groups.len() < 2
        || base.selective_error_risk.is_none()
        || base.accepted_errors == 0
        || base.accepted_errors == base.accepted
    {
        return Ok(
            json!({"status":"degenerate_or_insufficient_evidence","clusters":groups.len(),"ci95":null,"reason":"Percentile bootstrap cannot bound unseen events or infer from one cluster."}),
        );
    }
    let mut rng = ChaCha8Rng::seed_from_u64(input.bootstrap_seed);
    let mut risks = Vec::new();
    let mut empty = 0;
    for _ in 0..input.bootstrap_replicates {
        let mut sample = Vec::new();
        let mut mask = Vec::new();
        for _ in &groups {
            for &i in groups[rng.gen_range(0..groups.len())] {
                sample.push(rows[i]);
                mask.push(accepted[i]);
            }
        }
        if let Some(risk) = metrics(&sample, &mask, input)?.selective_error_risk {
            risks.push(risk);
        } else {
            empty += 1;
        }
    }
    // Do not condition away replicates with no accepted observations.
    let ci = if empty == 0 {
        Some([
            crate::evaluation::quantile(&risks, 0.025),
            crate::evaluation::quantile(&risks, 0.975),
        ])
    } else {
        None
    };
    Ok(
        json!({"status":if empty==0 {"descriptive_percentile"} else {"undefined_replicates"},"clusters":groups.len(),"ci95":ci,"undefined_replicates":empty,"replicates":input.bootstrap_replicates,"seed":input.bootstrap_seed,"threshold_refitted":false}),
    )
}

fn summarize(rows: &[&Record], input: &Input, tau: Option<f64>) -> Result<Value> {
    let mask: Vec<_> = rows
        .iter()
        .map(|r| tau.is_some_and(|t| r.uncertainty <= t))
        .collect();
    let operational = metrics(rows, &mask, input)?;
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| {
        a.uncertainty
            .total_cmp(&b.uncertainty)
            .then(a.id.cmp(&b.id))
    });
    let k = (input.target_coverage * sorted.len() as f64).floor() as usize;
    let ranked: Vec<_> = (0..sorted.len()).map(|i| i < k).collect();
    let retrospective = metrics(&sorted, &ranked, input)?;
    let mut curve = Vec::new();
    let mut area = 0.0;
    let mut previous = 0;
    let all_labeled = sorted.iter().all(|r| r.truth.is_some());
    let mut cumulative_errors = 0;
    for i in 0..sorted.len() {
        cumulative_errors += usize::from(event(sorted[i], input)? == Some(true));
        if i + 1 == sorted.len() || sorted[i].uncertainty != sorted[i + 1].uncertainty {
            let risk = if all_labeled {
                Some(cumulative_errors as f64 / (i + 1) as f64)
            } else {
                None
            };
            if let Some(r) = risk {
                area += (i + 1 - previous) as f64 / sorted.len() as f64 * r;
            }
            curve.push(json!({"threshold":sorted[i].uncertainty,"coverage":(i+1) as f64/sorted.len() as f64,"risk":risk}));
            previous = i + 1;
        }
    }
    let mut bins = vec![(0usize, 0.0, 0usize); input.reliability_bins];
    let mut brier = 0.0;
    let mut probabilities = 0;
    for r in rows {
        if let (Some(p), Some(e)) = (r.error_probability, event(r, input)?) {
            let i = ((p * input.reliability_bins as f64).floor() as usize)
                .min(input.reliability_bins - 1);
            bins[i].0 += 1;
            bins[i].1 += p;
            bins[i].2 += usize::from(e);
            probabilities += 1;
            brier += (p - f64::from(e)).powi(2);
        }
    }
    let mut ece = 0.0;
    let reliability: Vec<_> = bins.iter().enumerate().map(|(i,(n,p,e))| {
        if *n>0 {ece+=(*p-*e as f64).abs();}
        json!({"lower":i as f64/input.reliability_bins as f64,"upper":(i+1) as f64/input.reliability_bins as f64,"count":n,"events":e,"mean_probability":if *n>0 {Some(p/ *n as f64)} else {None},"event_rate":if *n>0 {Some(*e as f64/ *n as f64)} else {None}})
    }).collect();
    let complete_probability = probabilities == rows.len();
    Ok(
        json!({"operational":operational,"retrospective_matched_coverage":retrospective,
        "risk_coverage_curve":curve,"aurc":if all_labeled {Some(area)} else {None},
        "aurc_definition":"Sum over complete score tie groups: delta coverage times risk after accepting group (right endpoint step integral).",
        "ece":if complete_probability {Some(ece/probabilities as f64)} else {None},"brier":if complete_probability {Some(brier/probabilities as f64)} else {None},
        "probability_labeled_count":probabilities,"reliability":reliability,
        "sequence_bootstrap":bootstrap(rows,&mask,input,false)?,"spatial_bootstrap":bootstrap(rows,&mask,input,true)?,
        "calibration_warning":"Equal-width bins; small ECE and zero observed failures do not establish calibration, especially with rare events. Partial probability diagnostics describe only supplied labeled probabilities."}),
    )
}

pub fn evaluate(input: &Input) -> Result<Value> {
    validate(input)?;
    let tau = threshold(input);
    let mut test: Vec<_> = input
        .records
        .iter()
        .filter(|r| r.split == Split::Test)
        .collect();
    test.sort_by(|a, b| a.id.cmp(&b.id));
    let scenarios: BTreeSet<_> = test.iter().map(|r| r.scenario.as_str()).collect();
    let mut reports = BTreeMap::new();
    for scenario in scenarios {
        reports.insert(
            scenario,
            summarize(
                &test
                    .iter()
                    .copied()
                    .filter(|r| r.scenario == scenario)
                    .collect::<Vec<_>>(),
                input,
                tau,
            )?,
        );
    }
    let required = [
        "mesh",
        "solver",
        "interpolation",
        "boundary",
        "drift",
        "observability",
    ];
    let numerical_supported = input.evidence.numerical_required
        && !input.evidence.evidence_resources.is_empty()
        && required
            .iter()
            .all(|k| input.evidence.checks.get(*k) == Some(&Some(true)));
    let missing = input.evidence.evidence_resources.is_empty()
        || (input.evidence.numerical_required
            && required
                .iter()
                .any(|k| input.evidence.checks.get(*k).is_none_or(Option::is_none)))
        || test.iter().any(|r| r.truth.is_none());
    let outside: BTreeSet<_> = test
        .iter()
        .filter(|r| !input.evidence.evaluated_scenarios.contains(&r.scenario))
        .map(|r| &r.scenario)
        .collect();
    let validity = json!({"numerical_status":if !input.evidence.numerical_required {"not_applicable"} else if numerical_supported {"numerically_validated_as_attested"} else {"numerically_unresolved"},
        "statistical_status":if input.evidence.prospective_power_supported {"power_supported_as_attested"} else {"statistically_underpowered_or_unestablished"},
        "out_of_evaluated_distribution":outside,"missing_required_evidence":missing,
        "scientifically_unsupported":!input.evidence.independent_replication || missing || !outside.is_empty() || !input.evidence.prospective_power_supported || (input.evidence.numerical_required && !numerical_supported),
        "validated_physical_confidence":false,"interpretation":"descriptive_exploratory; input attestations are not independently verified by the evaluator"});
    Ok(
        json!({"schema_version":"triad-metrology-report-1","software_version":env!("CARGO_PKG_VERSION"),
        "evaluator_source_sha256":crate::evaluation::hash(include_bytes!("metrology.rs")),"dependency_lock_sha256":crate::evaluation::hash(include_bytes!("../Cargo.lock")),
        "configuration_sha256":crate::evaluation::hash(&serde_json::to_vec(input)?),"input":input,"threshold":tau,
        "threshold_policy":"calibration-scenario score quantile, ceil(n*c), accept all boundary ties; zero coverage rejects all",
        "retrospective_policy":"floor(n*c), score then record-id ordering; descriptive ranking only",
        "overall":summarize(&test,input,tau)?,"scenarios":reports,"validity":validity}),
    )
}

/// Validate by recomputing the entire deterministic payload, not only its shape.
pub fn validate_report(report: &Value) -> Result<()> {
    let input: Input =
        serde_json::from_value(report.get("input").context("report missing input")?.clone())?;
    let expected = evaluate(&input)?;
    ensure!(
        report.get("analysis").is_none(),
        "pass the report payload, not an envelope"
    );
    ensure!(
        &expected == report,
        "report differs from recomputed metrics/provenance; do not edit derived evidence"
    );
    Ok(())
}
