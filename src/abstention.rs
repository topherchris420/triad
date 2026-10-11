//! Reusable, sensor-agnostic selective-prediction controller.
//!
//! This module does not improve an estimator's accuracy. It can only refuse
//! predictions whose *predeclared* risk score fails a calibration gate.
//! Statistical bounds require independent, identically distributed SEQUENCES
//! with equal frame counts; frames within a sequence may be dependent.
//! No guarantee transfers to a new device, drift regime, geometry, or site.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationFrame {
    /// Smaller means more trustworthy; e.g. TRIAD's C2 or D score.
    pub score: f64,
    /// True when verified position error exceeds the declared tolerance.
    pub incorrect: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationSequence {
    /// Unique, independently sampled sequence or deployment session.
    pub id: String,
    pub frames: Vec<CalibrationFrame>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitSpecification {
    pub method_id: String,
    /// Accepted error conditional on acceptance, not error on all frames.
    pub maximum_accepted_error_rate: f64,
    /// Minimum conservative coverage lower bound.
    pub minimum_coverage: f64,
    /// Family-wise error probability for both Hoeffding bounds.
    pub delta: f64,
    /// Fixed *before* observing calibration scores or labels. Data-derived
    /// thresholds invalidate the finite-candidate union-bound argument.
    pub candidate_thresholds: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FitRequest {
    pub spec: FitSpecification,
    pub sequences: Vec<CalibrationSequence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Ready,
    Refused,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateAudit {
    pub threshold: f64,
    pub observed_coverage: f64,
    pub observed_conditional_error: Option<f64>,
    pub coverage_lower_bound: f64,
    pub conditional_error_upper_bound: f64,
    pub qualifies: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FittedPolicy {
    pub schema_version: u32,
    pub status: PolicyStatus,
    pub method_id: String,
    pub threshold: Option<f64>,
    pub independent_sequences: usize,
    pub frames_per_sequence: usize,
    pub delta: f64,
    pub maximum_accepted_error_rate: f64,
    pub minimum_coverage: f64,
    pub candidates: Vec<CandidateAudit>,
    pub warning: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accept,
    WithholdUncalibrated,
    WithholdInvalidScore,
    WithholdAboveThreshold,
}

impl FittedPolicy {
    /// Fail closed for missing/nonfinite scores and refused or malformed policies.
    pub fn decide(&self, score: Option<f64>) -> Decision {
        let Some(score) = score else {
            return Decision::WithholdInvalidScore;
        };
        if !score.is_finite() {
            return Decision::WithholdInvalidScore;
        }
        if !matches!(self.status, PolicyStatus::Ready) {
            return Decision::WithholdUncalibrated;
        }
        let Some(t) = self.threshold else {
            return Decision::WithholdUncalibrated;
        };
        if !t.is_finite()
            || !self.candidates.iter().any(|c| {
                c.qualifies && c.threshold == t && c.conditional_error_upper_bound.is_finite()
            })
        {
            return Decision::WithholdUncalibrated;
        }
        if score <= t {
            Decision::Accept
        } else {
            Decision::WithholdAboveThreshold
        }
    }
}

/// One-sided, simultaneous cluster-level bounds for *fixed* thresholds.
/// Each sequence has identical length and is one independent unit. The ratio
/// bounds accepted errors / accepted observations for a random sequence/frame.
/// The result is NOT distribution-free under temporal or population shift.
pub fn fit(spec: &FitSpecification, sequences: &[CalibrationSequence]) -> Result<FittedPolicy> {
    ensure!(
        !spec.method_id.trim().is_empty() && spec.method_id.len() <= 128,
        "method_id must identify a single frozen scoring model"
    );
    ensure!(
        spec.maximum_accepted_error_rate.is_finite()
            && (0.0..=1.0).contains(&spec.maximum_accepted_error_rate),
        "invalid maximum accepted error rate"
    );
    ensure!(
        spec.minimum_coverage.is_finite()
            && spec.minimum_coverage > 0.0
            && spec.minimum_coverage <= 1.0,
        "invalid minimum coverage"
    );
    ensure!(
        spec.delta.is_finite() && spec.delta > 0.0 && spec.delta < 1.0,
        "invalid family-wise delta"
    );
    ensure!(
        !spec.candidate_thresholds.is_empty()
            && spec.candidate_thresholds.len() <= 1024
            && spec.candidate_thresholds.iter().all(|x| x.is_finite())
            && spec
                .candidate_thresholds
                .windows(2)
                .all(|w| w[0] < w[1]),
        "candidate thresholds must be predeclared, finite and strictly increasing"
    );
    ensure!(!sequences.is_empty(), "no independent calibration sequences");
    let frames_per_sequence = sequences[0].frames.len();
    ensure!(frames_per_sequence > 0, "empty calibration sequence");
    let mut ids = HashSet::new();
    for seq in sequences {
        ensure!(
            !seq.id.trim().is_empty() && ids.insert(seq.id.as_str()),
            "missing or duplicate calibration sequence id"
        );
        ensure!(
            seq.frames.len() == frames_per_sequence,
            "all independent sequences must have equal frame counts"
        );
        ensure!(
            seq.frames.iter().all(|f| f.score.is_finite()),
            "non-finite calibration score"
        );
    }

    let n = sequences.len() as f64;
    let k = spec.candidate_thresholds.len() as f64;
    // Simultaneous bounds: 2 quantities x K fixed candidates, total delta.
    // Hoeffding is applied to per-sequence fractions in [0, 1].
    let radius = ((2.0 * k / spec.delta).ln() / (2.0 * n)).sqrt();
    let mut candidates = Vec::with_capacity(spec.candidate_thresholds.len());
    let mut best: Option<(f64, f64)> = None; // threshold, lower coverage
    for &threshold in &spec.candidate_thresholds {
        let mut accepted = 0usize;
        let mut bad_accepted = 0usize;
        for seq in sequences {
            for frame in &seq.frames {
                if frame.score <= threshold {
                    accepted += 1;
                    if frame.incorrect {
                        bad_accepted += 1;
                    }
                }
            }
        }
        let total = sequences.len() * frames_per_sequence;
        let coverage = accepted as f64 / total as f64;
        let joint_error = bad_accepted as f64 / total as f64;
        let lower = (coverage - radius).max(0.0);
        let upper_joint = (joint_error + radius).min(1.0);
        let upper_risk = if lower > 0.0 {
            (upper_joint / lower).min(1.0)
        } else {
            1.0
        };
        let qualifies =
            lower >= spec.minimum_coverage && upper_risk <= spec.maximum_accepted_error_rate;
        if qualifies && best.is_none_or(|(_, previous)| lower > previous) {
            best = Some((threshold, lower));
        }
        candidates.push(CandidateAudit {
            threshold,
            observed_coverage: coverage,
            observed_conditional_error: (accepted > 0)
                .then_some(bad_accepted as f64 / accepted.max(1) as f64),
            coverage_lower_bound: lower,
            conditional_error_upper_bound: upper_risk,
            qualifies,
        });
    }
    let status = if best.is_some() {
        PolicyStatus::Ready
    } else {
        PolicyStatus::Refused
    };
    Ok(FittedPolicy {
        schema_version: 1,
        status,
        method_id: spec.method_id.clone(),
        threshold: best.map(|(t, _)| t),
        independent_sequences: sequences.len(),
        frames_per_sequence,
        delta: spec.delta,
        maximum_accepted_error_rate: spec.maximum_accepted_error_rate,
        minimum_coverage: spec.minimum_coverage,
        candidates,
        warning: "Bounds rely on independent identically distributed sequences, identical frame counts, fixed candidate thresholds, trustworthy labels and a frozen scoring model. No assurance under drift, hardware shift, numerical model mismatch or reuse of calibration as test evidence.".into(),
    })
}

pub fn fit_file(path: &Path) -> Result<FittedPolicy> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let request: FitRequest =
        serde_json::from_slice(&bytes).context("parse abstention calibration JSON")?;
    fit(&request.spec, &request.sequences)
}
