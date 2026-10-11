//! Versioned C2 versus D study. NO historical test observations or test labels
//! are used for fitting, tuning, calibration, or inference.
use crate::{
    evaluation::hash,
    forward::Model,
    inverse::{Dictionary, Estimator},
    measurements::{self, Channels, Sequence, whiten},
    numerical_audit::read_dictionary,
    observability::local_position_information,
};
use anyhow::{ensure, Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

const FRAMES: usize = 12;
const CAL_FRAMES: usize = 24;
const TOLERANCE_M: f64 = 0.005;
const TARGET_COVERAGE: f64 = 0.80;
const POSTERIOR_TEMPERATURE: f64 = 2.0;
const D_PHYSICS_WEIGHT: f64 = 0.25;

#[derive(Clone, Debug, Serialize)]
pub struct SelectorScore {
    pub sequence_id: u64,
    pub frame: usize,
    pub estimated_xy_m: [f64; 2],
    pub c2: f64,
    pub d: f64,
    pub local_information: f64,
    pub dictionary_global_ambiguity: f64,
}
#[derive(Clone)]
struct Labeled {
    score: SelectorScore,
    error_mm: f64,
    event: bool,
}
#[derive(Clone, Copy, Serialize)]
struct Platt {
    intercept: f64,
    slope: f64,
}
impl Platt {
    fn probability(self, x: f64) -> f64 {
        let z = (self.intercept + self.slope * x).clamp(-35.0, 35.0);
        1.0 / (1.0 + (-z).exp())
    }
    fn fit(rows: &[Labeled], method: &str) -> Self {
        let base = (rows.iter().filter(|r| r.event).count() as f64 + 0.5)
            / (rows.len() as f64 + 1.0);
        let mut fit = Self {
            intercept: (base / (1.0 - base)).ln(),
            slope: 0.0,
        };
        for _ in 0..1000 {
            let mut grad_a = 0.0;
            let mut grad_b = 0.0;
            for row in rows {
                let x = score(row, method);
                let delta = fit.probability(x) - if row.event { 1.0 } else { 0.0 };
                grad_a += delta;
                grad_b += delta * x;
            }
            grad_a /= rows.len() as f64;
            grad_b = grad_b / rows.len() as f64 + 0.001 * fit.slope;
            fit.intercept = (fit.intercept - 0.5 * grad_a).clamp(-25.0, 25.0);
            fit.slope = (fit.slope - 0.5 * grad_b).clamp(0.0, 25.0);
        }
        fit
    }
}
fn score(row: &Labeled, method: &str) -> f64 {
    if method == "c2" { row.score.c2 } else { row.score.d }
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2)).sqrt()
}
fn global_ambiguity(dictionary: &Dictionary) -> Vec<f64> {
    // An independent GEOMETRY/NOISE diagnostic, independent of test labels.
    // Each location is compared to materially different positions >5 mm away.
    let w: Vec<_> = dictionary.signals.iter().map(|s| whiten(s, CAL_FRAMES)).collect();
    dictionary.states.iter().enumerate().map(|(i, x)| {
        let mut best = f64::INFINITY;
        for (j, y) in dictionary.states.iter().enumerate() {
            if dist(*x, *y) <= TOLERANCE_M { continue; }
            let sq: f64 = (0..10).map(|c| (w[i][c] - w[j][c]).powi(2)).sum();
            best = best.min(sq);
        }
        if best.is_finite() { 1.0/(1.0 + best) } else { 0.0 }
    }).collect()
}
pub fn score_frame(
    dictionary: &Dictionary,
    measurement: &Channels,
    estimate: [f64; 3],
    ambiguity: &[f64],
    sequence_id: u64,
    frame: usize,
) -> SelectorScore {
    // Normalized finite dictionary posterior; uncertainty is conditional on this
    // discrete approximate likelihood/prior, NOT calibrated by construction.
    let log_weights: Vec<f64> = dictionary.signals.iter().map(|s| {
        let residual = whiten(&std::array::from_fn(|c| s[c] - measurement[c]), CAL_FRAMES);
        -0.5 * residual.iter().map(|x| x*x).sum::<f64>()
            / POSTERIOR_TEMPERATURE.powi(2)
    }).collect();
    let max_log = log_weights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut weights: Vec<f64> = log_weights.iter().map(|v| (v-max_log).exp()).collect();
    let norm: f64 = weights.iter().sum();
    for w in &mut weights { *w /= norm; }
    let c2: f64 = dictionary.states.iter().zip(&weights)
        .filter(|(s, _)| dist(**s, estimate) > TOLERANCE_M)
        .map(|(_, weight)| *weight).sum();
    let global: f64 = weights.iter().zip(ambiguity).map(|(p,a)| p*a).sum();
    let info = local_position_information(dictionary, estimate, CAL_FRAMES)
        .nuisance_adjusted_weakest_per_mm2;
    let local = 1.0/(1.0 + info.max(0.0));
    let physical_uncertainty = 0.5 * global + 0.5 * local;
    let d = (1.0-D_PHYSICS_WEIGHT)*c2 + D_PHYSICS_WEIGHT*physical_uncertainty;
    SelectorScore {
        sequence_id, frame, estimated_xy_m: [estimate[0],estimate[1]],
        c2, d, local_information: info, dictionary_global_ambiguity: global,
    }
}
fn fresh_split(
    dictionary: &Dictionary,
    split: &str,
    seed: u64,
    sequence_count: usize,
) -> Result<Vec<Sequence>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    // Completely new OFF-GRID physical states for each split, including the final
    // test. Historical target pools are never loaded; the FEM is refined.
    let states: Vec<[f64;3]> = (0..24).map(|_| [
        rng.gen_range(-0.0237..0.0237),
        rng.gen_range(0.0083..0.0277),
        rng.gen_range(2.1..7.9),
    ]).collect();
    let model = Model::new(0.0005);
    let signals: Vec<Channels> = states.iter().map(|s| model.solve(Some(*s)))
        .collect::<Result<Vec<_>>>()?.iter().map(|s| s.signal).collect();
    let mut sequences = Vec::new();
    for _ in 0..sequence_count {
        let mut targets = Vec::new();
        let mut measurements = Vec::new();
        for _ in 0..FRAMES {
            let i = rng.gen_range(0..states.len());
            targets.push(states[i]);
            measurements.push(signals[i]);
        }
        let id = rng.gen::<u64>();
        sequences.push(measurements::simulate(
            &measurements, targets, id, "strong_drift", split, CAL_FRAMES,
        ));
    }
    // Sanity: the saved original dictionary was only used for inference.
    ensure!(!dictionary.states.is_empty(), "empty inference dictionary");
    Ok(sequences)
}
fn predict(
    dictionary: &Dictionary,
    sequences: &[Sequence],
    ambiguity: &[f64],
) -> Vec<SelectorScore> {
    let estimator = Estimator { dictionary, calibration_frames: CAL_FRAMES };
    let mut result = Vec::new();
    for sequence in sequences {
        for (frame, observation) in sequence.estimator_inputs().iter().enumerate() {
            let estimate = estimator.estimate(observation);
            result.push(score_frame(
                dictionary, observation, estimate.state, ambiguity, sequence.sequence_id, frame,
            ));
        }
    }
    result
}
fn label(predictions: Vec<SelectorScore>, sequences: &[Sequence]) -> Vec<Labeled> {
    let truth: BTreeMap<_,_> = sequences.iter().map(|s| (s.sequence_id, s)).collect();
    predictions.into_iter().map(|p| {
        let actual = truth[&p.sequence_id].truth_states[p.frame];
        let error_mm = 1000.0 * (((p.estimated_xy_m[0]-actual[0]).powi(2)
            + (p.estimated_xy_m[1]-actual[1]).powi(2)).sqrt());
        Labeled { event: error_mm > TOLERANCE_M*1000.0, error_mm, score:p }
    }).collect()
}
fn threshold(rows: &[Labeled], method: &str) -> f64 {
    let mut values: Vec<f64> = rows.iter().map(|r| score(r, method)).collect();
    values.sort_by(f64::total_cmp);
    let n = ((TARGET_COVERAGE * values.len() as f64).ceil() as usize)
        .clamp(1, values.len());
    values[n-1]
}
fn matched_mask(rows: &[Labeled], method: &str) -> Vec<bool> {
    let mut ranked: Vec<usize> = (0..rows.len()).collect();
    ranked.sort_by(|a,b| score(&rows[*a],method).total_cmp(&score(&rows[*b],method))
        .then_with(|| rows[*a].score.sequence_id.cmp(&rows[*b].score.sequence_id))
        .then_with(|| rows[*a].score.frame.cmp(&rows[*b].score.frame)));
    let mut chosen = vec![false; rows.len()];
    let n = (TARGET_COVERAGE * rows.len() as f64).floor() as usize;
    for &index in ranked.iter().take(n) { chosen[index] = true; }
    chosen
}
fn summarize(rows: &[Labeled], mask: &[bool], fit: Platt, method: &str) -> Value {
    let accepted = mask.iter().filter(|x| **x).count();
    let events = rows.iter().zip(mask).filter(|(r,m)| **m && r.event).count();
    let mean = rows.iter().zip(mask).filter(|(_,m)| **m)
        .map(|(r,_)| r.error_mm).sum::<f64>() / accepted.max(1) as f64;
    let brier = rows.iter().map(|r| {
        let p = fit.probability(score(r,method));
        (p - if r.event {1.0} else {0.0}).powi(2)
    }).sum::<f64>() / rows.len() as f64;
    let mut bins = Vec::new();
    let mut ece = 0.0;
    for i in 0..10 {
        let bucket: Vec<_> = rows.iter().filter(|r| {
            let p = fit.probability(score(r,method));
            p >= i as f64/10.0 && (i==9 || p < (i+1) as f64/10.0)
        }).collect();
        if bucket.is_empty() {continue;}
        let observed = bucket.iter().filter(|r| r.event).count() as f64 / bucket.len() as f64;
        let predicted = bucket.iter().map(|r| fit.probability(score(r,method)))
            .sum::<f64>() / bucket.len() as f64;
        ece += bucket.len() as f64 / rows.len() as f64 * (predicted-observed).abs();
        bins.push(json!({"n":bucket.len(),"predicted":predicted,"observed":observed}));
    }
    json!({
        "coverage": accepted as f64 / rows.len() as f64,
        "accepted_error_over_5mm": events,
        "accepted_risk": if accepted == 0 {Value::Null} else {json!(events as f64/accepted as f64)},
        "incorrect_acceptance_frequency": events as f64 / rows.len() as f64,
        "accepted_mean_error_mm": mean,
        "brier_calibrated": brier,
        "ece_10_equal_width_bins": ece,
        "reliability_bins": bins,
    })
}
fn risk(rows: &[Labeled], method: &str) -> f64 {
    let mask = matched_mask(rows, method);
    let accepted = mask.iter().filter(|x| **x).count();
    if accepted == 0 { return 0.0; }
    rows.iter().zip(mask).filter(|(r,accepted)| *accepted && r.event).count()
        as f64 / accepted as f64
}
fn paired_cluster_interval(rows: &[Labeled]) -> [f64;2] {
    let mut groups: BTreeMap<u64, Vec<Labeled>> = BTreeMap::new();
    for row in rows { groups.entry(row.score.sequence_id).or_default().push(row.clone()); }
    let sequences: Vec<_> = groups.values().collect();
    let mut rng = ChaCha8Rng::seed_from_u64(901773);
    let mut differences = Vec::new();
    // Each sampled unit is a complete independent drift/calibration sequence.
    for _ in 0..500 {
        let mut sample = Vec::with_capacity(rows.len());
        for _ in 0..sequences.len() {
            sample.extend((*sequences[rng.gen_range(0..sequences.len())]).iter().cloned());
        }
        differences.push(risk(&sample,"c2") - risk(&sample,"d"));
    }
    differences.sort_by(f64::total_cmp);
    [differences[12], differences[487]]
}
pub fn run(root: &Path) -> Result<()> {
    let path = root.join("experiments/selective-v3.json");
    let bytes = std::fs::read(&path)?;
    let cfg: Value = serde_json::from_slice(&bytes)?;
    ensure!(cfg["protocol_version"] == "3.0-independent-selective", "wrong protocol");
    ensure!(cfg["primary_scenario"] == "strong_drift"
        && cfg["error_threshold_mm"] == 5.0
        && cfg["target_coverage"] == TARGET_COVERAGE
        && cfg["frames_per_sequence"] == FRAMES
        && cfg["posterior_temperature"] == POSTERIOR_TEMPERATURE
        && cfg["d_physics_weight"] == D_PHYSICS_WEIGHT,
        "source and frozen protocol disagree");
    let output = root.join("results/selective-v3");
    ensure!(!output.join("summary.json").exists(), "selective-v3 already run: refuse overwrite");
    std::fs::create_dir_all(&output)?;
    let dictionary = read_dictionary(root)?;
    let original_cfg = std::fs::read(root.join("experiments/config.json"))?;
    let validation: Value = serde_json::from_slice(&std::fs::read(root.join("results/validation.json"))?)?;
    ensure!(validation["passed"] == true && validation["config_sha256"] == hash(&original_cfg),
        "frozen baseline validation is missing or invalid");
    let numerical: Value = serde_json::from_slice(
        &std::fs::read(root.join("results/numerical-audit-v3.json"))
            .context("run 'triad numerical' first")?)?;
    ensure!(numerical["dictionary_key"] == dictionary.cache_key,
        "numerical audit does not match inference dictionary");
    let ambiguity = global_ambiguity(&dictionary);
    // Generation uses independent seeds, disjoint latent states, and simulation
    // streams. Scores are produced BEFORE any test truth is joined for evaluation.
    let development = fresh_split(&dictionary, "development", 9301101, 64)?;
    let calibration = fresh_split(&dictionary, "calibration", 9301102, 64)?;
    let test = fresh_split(&dictionary, "test", 9301103, 300)?;
    let train = label(predict(&dictionary, &development, &ambiguity), &development);
    let calibrated = label(predict(&dictionary, &calibration, &ambiguity), &calibration);
    let test_scores = predict(&dictionary, &test, &ambiguity);
    std::fs::write(output.join("test-scores.json"), serde_json::to_vec(&test_scores)?)?;
    let test_rows = label(test_scores, &test);
    let c2_fit = Platt::fit(&train, "c2");
    let d_fit = Platt::fit(&train, "d");
    let c2_threshold = threshold(&calibrated,"c2");
    let d_threshold = threshold(&calibrated,"d");
    let deployment_c2: Vec<bool> = test_rows.iter().map(|r| r.score.c2 <= c2_threshold).collect();
    let deployment_d: Vec<bool> = test_rows.iter().map(|r| r.score.d <= d_threshold).collect();
    let matched_c2 = matched_mask(&test_rows,"c2");
    let matched_d = matched_mask(&test_rows,"d");
    let diff = risk(&test_rows,"c2") - risk(&test_rows,"d");
    let interval = paired_cluster_interval(&test_rows);
    let event_count = test_rows.iter().filter(|r| r.event).count();
    let adequate_events = event_count >= cfg["minimum_test_failures"].as_u64().unwrap() as usize;
    let numerical_ready = numerical["likelihood_ready_for_claims"] == true;
    let report = json!({
        "experiment": "selective-v3",
        "protocol_sha256": hash(&bytes),
        "dictionary_key": dictionary.cache_key,
        "scenario": "strong_drift",
        "independent_sequence_counts": {"development":64,"calibration":64,"test":300},
        "test_frames": test_rows.len(),
        "test_errors_above_5mm": event_count,
        "sufficient_failure_events": adequate_events,
        "numerical_inspection_ready": numerical_ready,
        "primary_matched_80_percent": {
            "c2": summarize(&test_rows,&matched_c2,c2_fit,"c2"),
            "d": summarize(&test_rows,&matched_d,d_fit,"d"),
            "risk_reduction_c2_minus_d": diff,
            "paired_whole_sequence_bootstrap_95_percent": interval,
            "retrospective_ranking_not_deployment_calibration": true
        },
        "deployment_thresholds_calibration_only": {
            "c2_threshold":c2_threshold, "d_threshold":d_threshold,
            "c2":summarize(&test_rows,&deployment_c2,c2_fit,"c2"),
            "d":summarize(&test_rows,&deployment_d,d_fit,"d")
        },
        "probability_calibration_development_only": {
            "c2":c2_fit, "d":d_fit,
            "warning": "These logistic calibrators were trained on development labels; reliability is measured on untouched independent test sequences."
        },
        "supports_declared_primary_hypothesis": numerical_ready && adequate_events
            && diff >= 0.02 && interval[0] > 0.0,
        "interpretation": "Exploratory synthetic study. Do not claim reliable calibration, physical sensing, or improvement without checking numerical likelihood error, event power, and independent reproduction.",
        "limitations": [
            "Finite, discrete position/material posterior with an explicitly tempered Gaussian likelihood; C2 is optimal only under its approximate model.",
            "D is a preregistered untrained blend of C2 and independently computable local/global observability indicators.",
            "Test frames share spatial pools within each split; sequence bootstrap addresses temporal clustering, not all model misspecification.",
            "No hardware or 3D validation, and no nested cross-validation of the prescribed physics weight.",
            "Calibration thresholds come exclusively from disjoint calibration sequences.",
            "Both selectors share the same measurement channels, posterior, priors, calibration history, and point estimator."
        ]
    });
    std::fs::write(output.join("summary.json"), serde_json::to_vec_pretty(&report)?)?;
    // Test truth is written only after the frozen prediction scores have been saved.
    let truth_rows: Vec<_> = test_rows.iter().map(|r| json!({
        "sequence_id":r.score.sequence_id,"frame":r.score.frame,
        "error_mm":r.error_mm,"incorrect":r.event,
    })).collect();
    std::fs::write(output.join("evaluation.json"), serde_json::to_vec(&truth_rows)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platt_probabilities_are_bounded_and_monotone() {
        let fit=Platt {intercept:-4.0,slope:5.0};
        assert!(fit.probability(0.0) < fit.probability(1.0));
        assert!(fit.probability(1.0) < 1.0);
    }
    #[test]
    fn matched_mask_accepts_exactly_eighty_percent() {
        let rows:Vec<Labeled>=(0..50).map(|i| Labeled {
            score:SelectorScore {
                sequence_id:i/10,frame:i as usize,
                estimated_xy_m:[0.0,0.0],c2:i as f64/50.0,d:(50-i) as f64/50.0,
                local_information:1.0,dictionary_global_ambiguity:0.0,
            },
            error_mm:0.0,event:false,
        }).collect();
        assert_eq!(matched_mask(&rows,"c2").iter().filter(|x| **x).count(),40);
        assert_eq!(matched_mask(&rows,"d").iter().filter(|x| **x).count(),40);
        assert_ne!(matched_mask(&rows,"c2"),matched_mask(&rows,"d"));
    }
}
