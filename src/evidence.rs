//! Claim-relative evidence sufficiency (research stage, Milestone 7).
//!
//! Classifies typed reliability claims against typed evidence with explicit,
//! inspectable rules (docs/metrology/evidence-sufficiency-method.md). It never
//! produces a confidence percentage. Unknown evidence never passes, attestations
//! are reported but never relied upon, and a matching hash establishes byte
//! consistency only. `standard_checks` implements the pre-declared baseline C.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Dependence {
    Independent,
    Temporal,
    Clustered,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntervalMethod {
    ClopperPearson,
    Wilson,
    NormalIid,
    BlockBootstrap,
    ClusterBootstrap,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Interval {
    pub method: IntervalMethod,
    pub level: f64,
    pub lower: f64,
    pub upper: f64,
}
/// Ordered from weakest to strongest.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    SelfReported,
    RepositoryRecord,
    ExternalSource,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Equals,
    AtMost,
    AtLeast,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ordering {
    Temporal,
    Random,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    Forecast,
    Iid,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Invalidates,
    Qualifies,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FeatureTime {
    pub name: String,
    pub available_offset_hours: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub features: Vec<String>,
    pub relation: String,
    pub rows: u64,
    pub max_abs_residual: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SplitRange {
    pub name: String,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Evidence {
    MetricReport {
        id: String,
        metric: String,
        population: String,
        conditions: Vec<String>,
        unit: String,
        n: u64,
        events: Option<u64>,
        value: Option<f64>,
        dependence: Dependence,
        interval: Option<Interval>,
        design_effect: Option<f64>,
        source: String,
    },
    OutcomeRecords {
        id: String,
        metric: String,
        population: String,
        conditions: Vec<String>,
        unit: String,
        dependence: Dependence,
        outcomes: Vec<bool>,
        design_effect: Option<f64>,
    },
    Calibration {
        id: String,
        population: String,
        conditions: Vec<String>,
        condition_counts: BTreeMap<String, u64>,
        n: u64,
        unit: String,
        dependence: Dependence,
        method: String,
    },
    ShiftDiagnostic {
        id: String,
        reference: String,
        target: String,
        statistic: String,
        value: f64,
        threshold: f64,
    },
    SupportCheck {
        id: String,
        statistic: String,
        value: f64,
        support_max: f64,
    },
    Completeness {
        id: String,
        required: u64,
        available: u64,
    },
    NumericalChecks {
        id: String,
        component: String,
        checks: BTreeMap<String, Option<bool>>,
    },
    SystemDescription {
        id: String,
        component: String,
        numerical_solver: bool,
    },
    FileHash {
        id: String,
        path: String,
        expected_sha256: String,
        authority: Authority,
    },
    EmbeddedArtifact {
        id: String,
        content: String,
        expected_sha256: String,
        authority: Authority,
    },
    FeatureTiming {
        id: String,
        origin: String,
        features: Vec<FeatureTime>,
    },
    TargetConstruction {
        id: String,
        target: String,
        identities: Vec<Identity>,
    },
    Partition {
        id: String,
        ordering: Ordering,
        splits: Vec<SplitRange>,
        training_split: String,
        evaluation_split: String,
        preprocessing_fit_splits: Vec<String>,
    },
    DependenceDiagnostic {
        id: String,
        unit: String,
        n: u64,
        lag1_autocorrelation: f64,
    },
    PhysicalValidation {
        id: String,
        domain: String,
        trials: u64,
        passed: bool,
    },
    Limitation {
        id: String,
        statement: String,
        affects_claim_types: Vec<String>,
        effect: Effect,
    },
    Attestation {
        id: String,
        statement: String,
        value: bool,
        attester: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Claim {
    MetricValue {
        id: String,
        statement: String,
        evidence: Vec<String>,
        metric: String,
        population: String,
        conditions: Vec<String>,
        comparator: Comparator,
        claimed_value: f64,
        tolerance: f64,
    },
    MetricBound {
        id: String,
        statement: String,
        evidence: Vec<String>,
        metric: String,
        population: String,
        conditions: Vec<String>,
        comparator: Comparator,
        bound: f64,
        confidence: f64,
        claimed_value: Option<f64>,
        tolerance: f64,
    },
    CoverageGuarantee {
        id: String,
        statement: String,
        evidence: Vec<String>,
        population: String,
        conditions: Vec<String>,
        level: f64,
    },
    NumericalAdequacy {
        id: String,
        statement: String,
        evidence: Vec<String>,
        component: String,
        required_checks: Vec<String>,
    },
    Provenance {
        id: String,
        statement: String,
        evidence: Vec<String>,
        artifact: String,
        authority_required: Authority,
    },
    NoLeakage {
        id: String,
        statement: String,
        evidence: Vec<String>,
        task: Task,
    },
    Independence {
        id: String,
        statement: String,
        evidence: Vec<String>,
        unit: String,
    },
    PhysicalGeneralization {
        id: String,
        statement: String,
        evidence: Vec<String>,
        from_domain: String,
        to_domain: String,
    },
    UniversalReliability {
        id: String,
        statement: String,
        evidence: Vec<String>,
        metric: String,
        reliability_bound: f64,
        confidence: f64,
    },
    OperationalRisk {
        id: String,
        statement: String,
        evidence: Vec<String>,
        population: String,
        recent_population: String,
        condition: String,
        bound: f64,
        confidence: f64,
        min_condition_support: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Assessment {
    pub schema_version: String,
    pub assessment_id: String,
    pub evidence: Vec<Evidence>,
    pub claims: Vec<Claim>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    SupportedWithinScope,
    InsufficientEvidence,
    Contradicted,
    NotApplicable,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Support,
    Insufficient,
    Contradict,
    NotApplicable,
    Note,
}
/// Rule families, used for documentation and ablation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Evidence,
    Integrity,
    Scope,
    Dependence,
    Statistical,
    CalibrationApplicability,
    Representativeness,
    Completeness,
    AttestationPolicy,
    Authority,
    LeakageMechanics,
    Limitations,
    Universality,
    Applicability,
}
pub const FAMILIES: [Family; 13] = [
    Family::Integrity,
    Family::Scope,
    Family::Dependence,
    Family::Statistical,
    Family::CalibrationApplicability,
    Family::Representativeness,
    Family::Completeness,
    Family::AttestationPolicy,
    Family::Authority,
    Family::LeakageMechanics,
    Family::Limitations,
    Family::Universality,
    Family::Applicability,
];
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Computed,
    Supplied,
    Attested,
    None,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Reason {
    pub rule: String,
    pub family: Family,
    pub dimension: String,
    pub outcome: Outcome,
    pub detail: String,
    pub evidence: Vec<String>,
    pub basis: Basis,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub claim_id: String,
    pub claim_type: String,
    pub classification: Class,
    pub reasons: Vec<Reason>,
    pub attestations_not_relied_upon: Vec<String>,
}

pub const Z95: f64 = 1.959963984540054;

// ---------------------------------------------------------------------------
// Statistical primitives (independently re-implemented in scripts/verify-milestone7.py)

fn ln_gamma(x: f64) -> f64 {
    // Lanczos approximation, g = 7, n = 9; reflection below 0.5.
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        (pi / (pi * x).sin().abs()).ln() - ln_gamma(1.0 - x)
    } else {
        let x = x - 1.0;
        let mut a = G[0];
        let t = x + 7.5;
        for (i, g) in G.iter().enumerate().skip(1) {
            a += g / (x + i as f64);
        }
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}
fn beta_continued_fraction(a: f64, b: f64, x: f64) -> f64 {
    // Modified Lentz evaluation of the incomplete-beta continued fraction.
    let tiny = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..1000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h
}
/// Regularised incomplete beta I_x(a, b) for a, b > 0.
pub fn regularized_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_continued_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_continued_fraction(b, a, 1.0 - x) / b
    }
}
/// Quantile of Beta(a, b) by bisection on the monotone CDF.
pub fn beta_quantile(p: f64, a: f64, b: f64) -> f64 {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if regularized_beta(a, b, mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-15 {
            break;
        }
    }
    0.5 * (lo + hi)
}
/// One-sided level-`gamma` Clopper-Pearson bounds with real-valued counts
/// (effective counts after a design effect). Returns (lower, upper).
pub fn clopper_pearson(events: f64, n: f64, gamma: f64) -> (f64, f64) {
    let lower = if events <= 0.0 {
        0.0
    } else {
        beta_quantile(1.0 - gamma, events, n - events + 1.0)
    };
    let upper = if events >= n {
        1.0
    } else {
        beta_quantile(gamma, events + 1.0, n - events)
    };
    (lower, upper)
}
/// Two-sided Wilson score interval for a proportion.
pub fn wilson(successes: usize, n: usize) -> Option<[f64; 2]> {
    if n == 0 {
        return None;
    }
    let (p, n) = (successes as f64 / n as f64, n as f64);
    let z2 = Z95 * Z95;
    let centre = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let half = Z95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    Some([centre - half, centre + half])
}
/// Exact two-sided McNemar p-value from discordant counts.
pub fn mcnemar_exact(b: usize, c: usize) -> f64 {
    let n = b + c;
    if n == 0 {
        return 1.0;
    }
    let k = b.min(c);
    let mut tail = 0.0;
    for i in 0..=k {
        tail += (ln_choose(n, i) - n as f64 * std::f64::consts::LN_2).exp();
    }
    (2.0 * tail).min(1.0)
}
fn ln_choose(n: usize, k: usize) -> f64 {
    ln_gamma(n as f64 + 1.0) - ln_gamma(k as f64 + 1.0) - ln_gamma((n - k) as f64 + 1.0)
}
/// Bartlett white-noise band for a lag-1 autocorrelation.
pub fn lag1_significant(rho: f64, n: u64) -> bool {
    n > 0 && rho.abs() > Z95 / (n as f64).sqrt()
}

// ---------------------------------------------------------------------------
// Validation

fn hash_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn finite(x: f64, what: &str) -> Result<()> {
    ensure!(x.is_finite(), "{what} must be finite");
    Ok(())
}
impl Evidence {
    pub fn id(&self) -> &str {
        match self {
            Evidence::MetricReport { id, .. }
            | Evidence::OutcomeRecords { id, .. }
            | Evidence::Calibration { id, .. }
            | Evidence::ShiftDiagnostic { id, .. }
            | Evidence::SupportCheck { id, .. }
            | Evidence::Completeness { id, .. }
            | Evidence::NumericalChecks { id, .. }
            | Evidence::SystemDescription { id, .. }
            | Evidence::FileHash { id, .. }
            | Evidence::EmbeddedArtifact { id, .. }
            | Evidence::FeatureTiming { id, .. }
            | Evidence::TargetConstruction { id, .. }
            | Evidence::Partition { id, .. }
            | Evidence::DependenceDiagnostic { id, .. }
            | Evidence::PhysicalValidation { id, .. }
            | Evidence::Limitation { id, .. }
            | Evidence::Attestation { id, .. } => id,
        }
    }
    fn basis(&self) -> Basis {
        match self {
            Evidence::OutcomeRecords { .. }
            | Evidence::FileHash { .. }
            | Evidence::EmbeddedArtifact { .. } => Basis::Computed,
            Evidence::Attestation { .. } => Basis::Attested,
            _ => Basis::Supplied,
        }
    }
    fn validate(&self) -> Result<()> {
        ensure!(!self.id().trim().is_empty(), "empty evidence id");
        match self {
            Evidence::MetricReport {
                value,
                interval,
                design_effect,
                ..
            } => {
                if let Some(v) = value {
                    finite(*v, "value")?;
                }
                if let Some(i) = interval {
                    finite(i.lower, "interval")?;
                    finite(i.upper, "interval")?;
                    ensure!(
                        i.lower <= i.upper && i.level > 0.0 && i.level < 1.0,
                        "invalid interval"
                    );
                }
                if let Some(d) = design_effect {
                    ensure!(d.is_finite() && *d >= 1.0, "design_effect must be >= 1");
                }
            }
            Evidence::OutcomeRecords { design_effect, .. } => {
                if let Some(d) = design_effect {
                    ensure!(d.is_finite() && *d >= 1.0, "design_effect must be >= 1");
                }
            }
            Evidence::ShiftDiagnostic {
                value, threshold, ..
            } => {
                finite(*value, "shift value")?;
                finite(*threshold, "shift threshold")?;
            }
            Evidence::SupportCheck {
                value, support_max, ..
            } => {
                finite(*value, "support value")?;
                finite(*support_max, "support maximum")?;
            }
            Evidence::Completeness { required, .. } => ensure!(*required > 0, "required >= 1"),
            Evidence::FileHash {
                path,
                expected_sha256,
                ..
            } => {
                ensure!(hash_ok(expected_sha256), "invalid sha256");
                let p = Path::new(path);
                ensure!(
                    !path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_))),
                    "file paths must be relative to the root without '..'"
                );
            }
            Evidence::EmbeddedArtifact {
                expected_sha256, ..
            } => ensure!(hash_ok(expected_sha256), "invalid sha256"),
            Evidence::FeatureTiming { features, .. } => {
                for f in features {
                    finite(f.available_offset_hours, "feature offset")?;
                }
            }
            Evidence::TargetConstruction { identities, .. } => {
                for i in identities {
                    ensure!(
                        i.max_abs_residual.is_finite() && i.max_abs_residual >= 0.0,
                        "invalid identity residual"
                    );
                }
            }
            Evidence::DependenceDiagnostic {
                lag1_autocorrelation,
                ..
            } => ensure!(
                lag1_autocorrelation.is_finite() && lag1_autocorrelation.abs() <= 1.0,
                "autocorrelation must be in [-1,1]"
            ),
            _ => {}
        }
        Ok(())
    }
}
impl Claim {
    pub fn id(&self) -> &str {
        self.common().0
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Claim::MetricValue { .. } => "metric_value",
            Claim::MetricBound { .. } => "metric_bound",
            Claim::CoverageGuarantee { .. } => "coverage_guarantee",
            Claim::NumericalAdequacy { .. } => "numerical_adequacy",
            Claim::Provenance { .. } => "provenance",
            Claim::NoLeakage { .. } => "no_leakage",
            Claim::Independence { .. } => "independence",
            Claim::PhysicalGeneralization { .. } => "physical_generalization",
            Claim::UniversalReliability { .. } => "universal_reliability",
            Claim::OperationalRisk { .. } => "operational_risk",
        }
    }
    fn common(&self) -> (&str, &[String]) {
        match self {
            Claim::MetricValue { id, evidence, .. }
            | Claim::MetricBound { id, evidence, .. }
            | Claim::CoverageGuarantee { id, evidence, .. }
            | Claim::NumericalAdequacy { id, evidence, .. }
            | Claim::Provenance { id, evidence, .. }
            | Claim::NoLeakage { id, evidence, .. }
            | Claim::Independence { id, evidence, .. }
            | Claim::PhysicalGeneralization { id, evidence, .. }
            | Claim::UniversalReliability { id, evidence, .. }
            | Claim::OperationalRisk { id, evidence, .. } => (id, evidence),
        }
    }
    fn validate(&self) -> Result<()> {
        ensure!(!self.id().trim().is_empty(), "empty claim id");
        let probability = |x: f64, what: &str| -> Result<()> {
            ensure!(
                x.is_finite() && (0.0..=1.0).contains(&x),
                "{what} must be in [0,1]"
            );
            Ok(())
        };
        let confidence = |x: f64| -> Result<()> {
            ensure!(
                x.is_finite() && x > 0.5 && x < 1.0,
                "confidence must be in (0.5,1)"
            );
            Ok(())
        };
        match self {
            Claim::MetricValue {
                claimed_value,
                tolerance,
                ..
            } => {
                finite(*claimed_value, "claimed_value")?;
                ensure!(tolerance.is_finite() && *tolerance >= 0.0, "tolerance >= 0");
            }
            Claim::MetricBound {
                comparator,
                bound,
                confidence: c,
                claimed_value,
                tolerance,
                ..
            } => {
                ensure!(
                    *comparator != Comparator::Equals,
                    "metric_bound requires at_most or at_least"
                );
                finite(*bound, "bound")?;
                confidence(*c)?;
                if let Some(v) = claimed_value {
                    finite(*v, "claimed_value")?;
                }
                ensure!(tolerance.is_finite() && *tolerance >= 0.0, "tolerance >= 0");
            }
            Claim::CoverageGuarantee { level, .. } => {
                ensure!(
                    level.is_finite() && *level > 0.0 && *level < 1.0,
                    "level in (0,1)"
                )
            }
            Claim::UniversalReliability {
                reliability_bound,
                confidence: c,
                ..
            } => {
                probability(*reliability_bound, "reliability_bound")?;
                confidence(*c)?;
            }
            Claim::OperationalRisk {
                bound,
                confidence: c,
                ..
            } => {
                probability(*bound, "bound")?;
                confidence(*c)?;
            }
            _ => {}
        }
        Ok(())
    }
}
pub fn validate(a: &Assessment) -> Result<()> {
    ensure!(
        a.schema_version == "triad-evidence-1",
        "unsupported schema_version (expected triad-evidence-1)"
    );
    ensure!(!a.assessment_id.trim().is_empty(), "assessment_id required");
    ensure!(!a.claims.is_empty(), "at least one claim is required");
    let mut ids = BTreeSet::new();
    for e in &a.evidence {
        e.validate()
            .with_context(|| format!("evidence {}", e.id()))?;
        ensure!(ids.insert(e.id()), "duplicate evidence id {}", e.id());
    }
    let mut claims = BTreeSet::new();
    for c in &a.claims {
        c.validate().with_context(|| format!("claim {}", c.id()))?;
        ensure!(claims.insert(c.id()), "duplicate claim id {}", c.id());
        let mut linked = BTreeSet::new();
        for id in c.common().1 {
            ensure!(
                ids.contains(id.as_str()),
                "claim {} links unknown evidence {id}",
                c.id()
            );
            ensure!(linked.insert(id), "claim {} links {id} twice", c.id());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared evidence views

struct Rate {
    n: f64,
    events: Option<f64>,
    value: Option<f64>,
    dependence: Dependence,
    interval: Option<Interval>,
    design_effect: Option<f64>,
    conditions: Vec<String>,
    id: String,
}
fn rate_view(e: &Evidence) -> Option<(&str, &str, Rate)> {
    match e {
        Evidence::MetricReport {
            id,
            metric,
            population,
            conditions,
            n,
            events,
            value,
            dependence,
            interval,
            design_effect,
            ..
        } => Some((
            metric,
            population,
            Rate {
                n: *n as f64,
                events: events.map(|k| k as f64),
                value: *value,
                dependence: *dependence,
                interval: interval.clone(),
                design_effect: *design_effect,
                conditions: conditions.clone(),
                id: id.clone(),
            },
        )),
        Evidence::OutcomeRecords {
            id,
            metric,
            population,
            conditions,
            dependence,
            outcomes,
            design_effect,
            ..
        } => Some((
            metric,
            population,
            Rate {
                n: outcomes.len() as f64,
                events: Some(outcomes.iter().filter(|x| **x).count() as f64),
                value: None,
                dependence: *dependence,
                interval: None,
                design_effect: *design_effect,
                conditions: conditions.clone(),
                id: id.clone(),
            },
        )),
        _ => None,
    }
}
impl Rate {
    /// Point value; Err(detail) when counts are impossible or absent.
    fn point(&self) -> std::result::Result<f64, String> {
        match (self.events, self.value) {
            (Some(k), _) if k > self.n => Err(format!("{k} events exceed n = {}", self.n)),
            (Some(_), _) if self.n <= 0.0 => Err("no units".into()),
            (Some(k), Some(v)) if (k / self.n - v).abs() > 1e-9 => Err(format!(
                "value {v} disagrees with events/n = {}",
                k / self.n
            )),
            (Some(k), _) => Ok(k / self.n),
            (None, Some(v)) => Ok(v),
            (None, None) => Err("neither counts nor value".into()),
        }
    }
    fn dependence_aware(&self) -> bool {
        self.interval.as_ref().is_some_and(|i| {
            matches!(
                i.method,
                IntervalMethod::BlockBootstrap | IntervalMethod::ClusterBootstrap
            )
        })
    }
    /// One-sided bounds at `gamma`. `honour_dependence=false` is baseline C.
    fn bounds(&self, gamma: f64, honour_dependence: bool) -> Option<(f64, f64, String)> {
        if let Some(i) = &self.interval {
            let usable = !honour_dependence
                || self.dependence == Dependence::Independent
                || self.dependence_aware();
            if usable && i.level >= gamma {
                return Some((
                    i.lower,
                    i.upper,
                    format!("supplied {:?} interval", i.method),
                ));
            }
        }
        let k = self.events?;
        if k > self.n || self.n <= 0.0 {
            return None;
        }
        let d = if honour_dependence {
            self.design_effect.unwrap_or(1.0)
        } else {
            1.0
        };
        let (lo, hi) = clopper_pearson(k / d, self.n / d, gamma);
        Some((
            lo,
            hi,
            format!(
                "Clopper-Pearson, design effect {d}, effective n {}",
                self.n / d
            ),
        ))
    }
}

struct Ctx<'a> {
    linked: Vec<&'a Evidence>,
    disabled: &'a BTreeSet<Family>,
    root: Option<&'a Path>,
    reasons: Vec<Reason>,
}
impl Ctx<'_> {
    fn on(&self, f: Family) -> bool {
        !self.disabled.contains(&f)
    }
    fn push(
        &mut self,
        rule: &str,
        family: Family,
        dimension: &str,
        outcome: Outcome,
        detail: String,
        evidence: &[&str],
    ) {
        let basis = evidence
            .iter()
            .filter_map(|id| self.linked.iter().find(|e| e.id() == *id))
            .map(|e| e.basis())
            .fold(Basis::None, |acc, b| match (acc, b) {
                (Basis::Attested, _) | (_, Basis::Attested) => Basis::Attested,
                (Basis::Supplied, _) | (_, Basis::Supplied) => Basis::Supplied,
                (Basis::Computed, _) | (_, Basis::Computed) => Basis::Computed,
                _ => Basis::None,
            });
        self.reasons.push(Reason {
            rule: rule.into(),
            family,
            dimension: dimension.into(),
            outcome,
            detail,
            evidence: evidence.iter().map(|s| s.to_string()).collect(),
            basis,
        });
    }
    fn rates(&self, metric: &str, population: Option<&str>) -> Vec<Rate> {
        self.linked
            .iter()
            .filter_map(|e| rate_view(e))
            .filter(|(m, p, _)| *m == metric && population.is_none_or(|q| q == *p))
            .map(|(_, _, r)| r)
            .collect()
    }
    fn attestation(&self) -> Option<(&str, bool)> {
        self.linked.iter().find_map(|e| match e {
            Evidence::Attestation { id, value, .. } => Some((id.as_str(), *value)),
            _ => None,
        })
    }
}

/// Hash status of an integrity-bearing item: Some(Ok(true)) match, Some(Ok(false))
/// mismatch, Some(Err(_)) unavailable, None for other kinds.
fn hash_status(e: &Evidence, root: Option<&Path>) -> Option<std::result::Result<bool, String>> {
    match e {
        Evidence::FileHash {
            path,
            expected_sha256,
            ..
        } => Some(match root {
            None => Err("no file root available".into()),
            Some(root) => match std::fs::read(root.join(path)) {
                Ok(bytes) => Ok(crate::evaluation::hash(&bytes) == *expected_sha256),
                Err(_) => Err(format!("{path} is not available")),
            },
        }),
        Evidence::EmbeddedArtifact {
            content,
            expected_sha256,
            ..
        } => Some(Ok(
            crate::evaluation::hash(content.as_bytes()) == *expected_sha256
        )),
        _ => None,
    }
}

fn subset(claim: &[String], evaluated: &[String]) -> Vec<String> {
    claim
        .iter()
        .filter(|c| !evaluated.contains(c))
        .cloned()
        .collect()
}

fn bound_rule(ctx: &mut Ctx, rule: &str, r: &Rate, comparator: Comparator, bound: f64, gamma: f64) {
    let id = r.id.clone();
    if !ctx.on(Family::Statistical) {
        match r.point() {
            Ok(v) => {
                let ok = match comparator {
                    Comparator::AtMost => v <= bound,
                    _ => v >= bound,
                };
                let o = if ok {
                    Outcome::Support
                } else {
                    Outcome::Contradict
                };
                ctx.push(
                    rule,
                    Family::Evidence,
                    "statistical_sample",
                    o,
                    format!("point value {v} against {bound} (statistical rules ablated)"),
                    &[&id],
                );
            }
            Err(e) => ctx.push(
                rule,
                Family::Evidence,
                "statistical_sample",
                Outcome::Insufficient,
                e,
                &[&id],
            ),
        }
        return;
    }
    match r.bounds(gamma, ctx.on(Family::Dependence)) {
        None => ctx.push(
            rule,
            Family::Statistical,
            "statistical_sample",
            Outcome::Insufficient,
            "no usable interval or counts".into(),
            &[&id],
        ),
        Some((lo, hi, how)) => {
            let (support, contradict) = match comparator {
                Comparator::AtMost => (hi <= bound, lo > bound),
                _ => (lo >= bound, hi < bound),
            };
            let o = if support {
                Outcome::Support
            } else if contradict {
                Outcome::Contradict
            } else {
                Outcome::Insufficient
            };
            ctx.push(
                rule,
                Family::Statistical,
                "statistical_sample",
                o,
                format!("{how}: one-sided bounds [{lo:.6}, {hi:.6}] versus {bound} at {gamma}"),
                &[&id],
            );
        }
    }
}

fn dependence_rule(ctx: &mut Ctx, r: &Rate) {
    if !ctx.on(Family::Dependence) {
        return;
    }
    let id = r.id.clone();
    match r.dependence {
        Dependence::Unknown => ctx.push(
            "A5_dependence",
            Family::Dependence,
            "temporal_dependence",
            Outcome::Insufficient,
            "dependence of the evaluation units is unknown".into(),
            &[&id],
        ),
        Dependence::Temporal | Dependence::Clustered
            if !r.dependence_aware() && r.design_effect.is_none() =>
        {
            ctx.push(
                "A5_dependence",
                Family::Dependence,
                "temporal_dependence",
                Outcome::Insufficient,
                format!(
                    "{:?} units without a dependence-aware interval or design effect",
                    r.dependence
                ),
                &[&id],
            )
        }
        Dependence::Independent => {
            let diag: Vec<_> = ctx
                .linked
                .iter()
                .filter_map(|e| match e {
                    Evidence::DependenceDiagnostic {
                        id,
                        n,
                        lag1_autocorrelation,
                        ..
                    } if lag1_significant(*lag1_autocorrelation, *n) => {
                        Some((id.clone(), *lag1_autocorrelation, *n))
                    }
                    _ => None,
                })
                .collect();
            for (did, rho, n) in diag {
                ctx.push(
                    "A5_declared_independence",
                    Family::Dependence,
                    "temporal_dependence",
                    Outcome::Insufficient,
                    format!("declared independent, but lag-1 autocorrelation {rho} over {n} units is significant"),
                    &[&id, &did],
                );
            }
        }
        _ => {}
    }
}

fn limitation_rule(ctx: &mut Ctx, claim_type: &str) {
    if !ctx.on(Family::Limitations) {
        return;
    }
    let hits: Vec<_> = ctx
        .linked
        .iter()
        .filter_map(|e| match e {
            Evidence::Limitation {
                id,
                statement,
                affects_claim_types,
                effect,
            } if affects_claim_types.iter().any(|t| t == claim_type) => {
                Some((id.clone(), statement.clone(), *effect))
            }
            _ => None,
        })
        .collect();
    for (id, statement, effect) in hits {
        let o = match effect {
            Effect::Invalidates => Outcome::Insufficient,
            Effect::Qualifies => Outcome::Note,
        };
        ctx.push(
            "known_limitation",
            Family::Limitations,
            "known_limitation",
            o,
            statement,
            &[&id],
        );
    }
}

fn metric_claim(
    ctx: &mut Ctx,
    metric: &str,
    population: &str,
    conditions: &[String],
    check: impl Fn(&mut Ctx, &Rate),
) {
    let rates = ctx.rates(metric, Some(population));
    if rates.is_empty() {
        ctx.push(
            "metric_evidence_present",
            Family::Evidence,
            "missing_evidence",
            Outcome::Insufficient,
            format!("no linked {metric} evidence for population '{population}'"),
            &[],
        );
        return;
    }
    for r in &rates {
        if ctx.on(Family::Scope) {
            let outside = subset(conditions, &r.conditions);
            if !outside.is_empty() {
                ctx.push(
                    "A4_scope",
                    Family::Scope,
                    "population_scope",
                    Outcome::Insufficient,
                    format!("claimed conditions not evaluated: {outside:?}"),
                    &[&r.id],
                );
            }
        }
        if let Err(e) = r.point() {
            ctx.push(
                "evidence_consistency",
                Family::Integrity,
                "integrity",
                Outcome::Insufficient,
                e,
                &[&r.id],
            );
            continue;
        }
        check(ctx, r);
    }
}

fn classify_claim(claim: &Claim, ctx: &mut Ctx) {
    let ty = claim.type_name();
    match claim {
        Claim::MetricValue {
            metric,
            population,
            conditions,
            comparator,
            claimed_value,
            tolerance,
            ..
        } => {
            metric_claim(ctx, metric, population, conditions, |ctx, r| {
                let v = r.point().unwrap();
                let ok = match comparator {
                    Comparator::Equals => (v - claimed_value).abs() <= *tolerance,
                    Comparator::AtMost => v <= claimed_value + tolerance,
                    Comparator::AtLeast => v >= claimed_value - tolerance,
                };
                let o = if ok {
                    Outcome::Support
                } else {
                    Outcome::Contradict
                };
                ctx.push(
                "realised_value",
                Family::Evidence,
                "numerical_validity",
                o,
                format!("realised {v} {comparator:?} claimed {claimed_value} (tolerance {tolerance})"),
                &[&r.id],
            );
            })
        }
        Claim::MetricBound {
            metric,
            population,
            conditions,
            comparator,
            bound,
            confidence,
            claimed_value,
            tolerance,
            ..
        } => metric_claim(ctx, metric, population, conditions, |ctx, r| {
            if let Some(c) = claimed_value {
                let v = r.point().unwrap();
                if (v - c).abs() > *tolerance {
                    ctx.push(
                        "claimed_value",
                        Family::Evidence,
                        "numerical_validity",
                        Outcome::Contradict,
                        format!("claimed {c} but evidence gives {v}"),
                        &[&r.id],
                    );
                }
            }
            dependence_rule(ctx, r);
            bound_rule(ctx, "bound", r, *comparator, *bound, *confidence);
        }),
        Claim::CoverageGuarantee {
            population,
            conditions,
            level,
            ..
        } => {
            let cal = ctx.linked.iter().find_map(|e| match e {
                Evidence::Calibration {
                    id,
                    conditions,
                    condition_counts,
                    n,
                    dependence,
                    ..
                } => Some((
                    id.clone(),
                    conditions.clone(),
                    condition_counts.clone(),
                    *n,
                    *dependence,
                )),
                _ => None,
            });
            let Some((cid, cconds, counts, n, dep)) = cal else {
                ctx.push(
                    "calibration_present",
                    Family::Evidence,
                    "missing_evidence",
                    Outcome::Insufficient,
                    "no calibration evidence".into(),
                    &[],
                );
                return;
            };
            ctx.push(
                "conformal_marginal_guarantee",
                Family::Evidence,
                "calibration_applicability",
                Outcome::Support,
                "split-conformal marginal coverage holds under exchangeability of calibration and target".into(),
                &[&cid],
            );
            if ctx.on(Family::CalibrationApplicability) {
                let outside: Vec<_> = conditions
                    .iter()
                    .filter(|c| !cconds.contains(c) || counts.get(*c).copied().unwrap_or(0) == 0)
                    .cloned()
                    .collect();
                if !outside.is_empty() {
                    ctx.push(
                        "calibration_conditions",
                        Family::CalibrationApplicability,
                        "calibration_applicability",
                        Outcome::Insufficient,
                        format!("claimed conditions absent from calibration: {outside:?}"),
                        &[&cid],
                    );
                }
                let needed = (1.0 / (1.0 - level)).ceil() - 1.0;
                if (n as f64) < needed {
                    ctx.push(
                        "calibration_size",
                        Family::CalibrationApplicability,
                        "statistical_sample",
                        Outcome::Insufficient,
                        format!("{n} calibration units cannot give a finite {level} quantile"),
                        &[&cid],
                    );
                }
            }
            if ctx.on(Family::Dependence) && dep != Dependence::Independent {
                ctx.push(
                    "exchangeability",
                    Family::Dependence,
                    "temporal_dependence",
                    Outcome::Insufficient,
                    format!("calibration units are {dep:?}; exchangeability is not established"),
                    &[&cid],
                );
            }
            if ctx.on(Family::Representativeness) {
                let shifts: Vec<_> = ctx
                    .linked
                    .iter()
                    .filter_map(|e| match e {
                        Evidence::ShiftDiagnostic {
                            id,
                            value,
                            threshold,
                            ..
                        } if value > threshold => Some((id.clone(), *value, *threshold)),
                        _ => None,
                    })
                    .collect();
                for (sid, v, t) in shifts {
                    ctx.push(
                        "shift_detected",
                        Family::Representativeness,
                        "distribution_representativeness",
                        Outcome::Insufficient,
                        format!("shift statistic {v} exceeds {t}"),
                        &[&sid],
                    );
                }
            }
            let realised = ctx.rates("interval_coverage", Some(population));
            for r in realised {
                bound_rule(
                    ctx,
                    "realised_coverage",
                    &r,
                    Comparator::AtLeast,
                    *level,
                    0.95,
                );
                // Only a contradiction is informative here; support comes from calibration.
                if let Some(last) = ctx.reasons.last_mut() {
                    if last.outcome == Outcome::Insufficient || last.outcome == Outcome::Support {
                        last.outcome = Outcome::Note;
                    }
                }
            }
        }
        Claim::NumericalAdequacy {
            required_checks, ..
        } => {
            if ctx.on(Family::Applicability) {
                let none: Vec<_> = ctx
                    .linked
                    .iter()
                    .filter_map(|e| match e {
                        Evidence::SystemDescription {
                            id,
                            numerical_solver: false,
                            ..
                        } => Some(id.clone()),
                        _ => None,
                    })
                    .collect();
                if let Some(id) = none.first() {
                    ctx.push(
                        "has_numerical_component",
                        Family::Applicability,
                        "numerical_adequacy",
                        Outcome::NotApplicable,
                        "the described system has no numerical solver".into(),
                        &[id],
                    );
                    return;
                }
            }
            let checks = ctx.linked.iter().find_map(|e| match e {
                Evidence::NumericalChecks { id, checks, .. } => Some((id.clone(), checks.clone())),
                _ => None,
            });
            let Some((id, checks)) = checks else {
                ctx.push(
                    "numerical_checks_present",
                    Family::Evidence,
                    "missing_evidence",
                    Outcome::Insufficient,
                    "no numerical checks".into(),
                    &[],
                );
                return;
            };
            let failed: Vec<_> = required_checks
                .iter()
                .filter(|c| checks.get(*c) == Some(&Some(false)))
                .collect();
            let unknown: Vec<_> = required_checks
                .iter()
                .filter(|c| checks.get(*c).is_none_or(Option::is_none))
                .collect();
            let (o, d) = if !failed.is_empty() {
                (Outcome::Contradict, format!("failed: {failed:?}"))
            } else if !unknown.is_empty() {
                (
                    Outcome::Insufficient,
                    format!("absent or null: {unknown:?}"),
                )
            } else {
                (Outcome::Support, "all required checks passed".into())
            };
            ctx.push(
                "numerical_checks",
                Family::Evidence,
                "numerical_adequacy",
                o,
                d,
                &[&id],
            );
        }
        Claim::Provenance {
            authority_required, ..
        } => {
            let items: Vec<_> = ctx
                .linked
                .iter()
                .filter_map(|e| match e {
                    Evidence::FileHash { id, authority, .. }
                    | Evidence::EmbeddedArtifact { id, authority, .. } => {
                        Some((id.clone(), *authority, hash_status(e, ctx.root).unwrap()))
                    }
                    _ => None,
                })
                .collect();
            if items.is_empty() {
                ctx.push(
                    "hash_present",
                    Family::Evidence,
                    "provenance",
                    Outcome::Insufficient,
                    "no hash evidence".into(),
                    &[],
                );
            }
            for (id, authority, status) in items {
                match status {
                    Err(e) => ctx.push(
                        "hash_available",
                        Family::Evidence,
                        "provenance",
                        Outcome::Insufficient,
                        e,
                        &[&id],
                    ),
                    Ok(false) => ctx.push(
                        "hash_match",
                        Family::Evidence,
                        "provenance",
                        Outcome::Contradict,
                        "computed SHA-256 differs from the recorded value".into(),
                        &[&id],
                    ),
                    Ok(true) => {
                        ctx.push(
                            "hash_match",
                            Family::Evidence,
                            "provenance",
                            Outcome::Support,
                            "computed SHA-256 equals the recorded value (byte consistency only)"
                                .into(),
                            &[&id],
                        );
                        if ctx.on(Family::Authority) && authority < *authority_required {
                            ctx.push(
                                "A3_authority",
                                Family::Authority,
                                "provenance",
                                Outcome::Insufficient,
                                format!("recorded by {authority:?}; claim requires {authority_required:?}"),
                                &[&id],
                            );
                        }
                    }
                }
            }
        }
        Claim::NoLeakage { task, .. } => {
            if ctx.on(Family::LeakageMechanics) {
                leakage_rules(ctx, *task);
            }
            attestation_fallback(ctx, "leakage");
        }
        Claim::Independence { .. } => {
            if ctx.on(Family::Dependence) {
                let diag = ctx.linked.iter().find_map(|e| match e {
                    Evidence::DependenceDiagnostic {
                        id,
                        n,
                        lag1_autocorrelation,
                        ..
                    } => Some((id.clone(), *n, *lag1_autocorrelation)),
                    _ => None,
                });
                match diag {
                    None => ctx.push(
                        "dependence_diagnostic_present",
                        Family::Dependence,
                        "temporal_dependence",
                        Outcome::Insufficient,
                        "no dependence diagnostic".into(),
                        &[],
                    ),
                    Some((id, n, rho)) => {
                        let o = if lag1_significant(rho, n) {
                            Outcome::Contradict
                        } else {
                            Outcome::Support
                        };
                        ctx.push(
                            "lag1_band",
                            Family::Dependence,
                            "temporal_dependence",
                            o,
                            format!(
                                "lag-1 autocorrelation {rho} versus band {:.6} (n = {n})",
                                Z95 / (n as f64).sqrt()
                            ),
                            &[&id],
                        );
                    }
                }
            }
            attestation_fallback(ctx, "temporal_dependence");
        }
        Claim::PhysicalGeneralization { to_domain, .. } => {
            let v = ctx.linked.iter().find_map(|e| match e {
                Evidence::PhysicalValidation {
                    id, domain, passed, ..
                } if domain == to_domain => Some((id.clone(), *passed)),
                _ => None,
            });
            match v {
                None => ctx.push(
                    "target_domain_validation",
                    Family::Scope,
                    "population_scope",
                    Outcome::Insufficient,
                    format!("no physical validation in {to_domain}"),
                    &[],
                ),
                Some((id, passed)) => ctx.push(
                    "target_domain_validation",
                    Family::Scope,
                    "population_scope",
                    if passed {
                        Outcome::Support
                    } else {
                        Outcome::Contradict
                    },
                    format!("physical validation in {to_domain} passed: {passed}"),
                    &[&id],
                ),
            }
        }
        Claim::UniversalReliability {
            metric,
            reliability_bound,
            confidence,
            ..
        } => {
            let rates = ctx.rates(metric, None);
            if ctx.on(Family::Universality) {
                ctx.push(
                    "universal_scope",
                    Family::Universality,
                    "population_scope",
                    Outcome::Insufficient,
                    "universal quantification over unobserved distributions exceeds any finite evaluation".into(),
                    &[],
                );
                for r in &rates {
                    if let Some((lo, _, how)) = r.bounds(*confidence, ctx.on(Family::Dependence)) {
                        if lo > *reliability_bound {
                            ctx.push(
                                "observed_failure",
                                Family::Statistical,
                                "statistical_sample",
                                Outcome::Contradict,
                                format!("{how}: lower bound {lo:.6} exceeds {reliability_bound}"),
                                &[&r.id],
                            );
                        }
                    }
                }
            } else {
                for r in &rates {
                    bound_rule(
                        ctx,
                        "bound",
                        r,
                        Comparator::AtMost,
                        *reliability_bound,
                        *confidence,
                    );
                }
            }
        }
        Claim::OperationalRisk {
            recent_population,
            condition,
            bound,
            confidence,
            min_condition_support,
            ..
        } => operational_rules(
            ctx,
            recent_population,
            condition,
            *bound,
            *confidence,
            *min_condition_support,
        ),
    }
    limitation_rule(ctx, ty);
}

fn attestation_fallback(ctx: &mut Ctx, dimension: &str) {
    if let Some((id, value)) = ctx.attestation() {
        let id = id.to_string();
        if ctx.on(Family::AttestationPolicy) {
            ctx.push(
                "A2_attestation",
                Family::AttestationPolicy,
                dimension,
                Outcome::Note,
                "attestation reported but not relied upon".into(),
                &[&id],
            );
        } else {
            ctx.push(
                "attestation_accepted",
                Family::AttestationPolicy,
                dimension,
                if value {
                    Outcome::Support
                } else {
                    Outcome::Contradict
                },
                "attestation accepted (ablation)".into(),
                &[&id],
            );
        }
    }
}

fn leakage_rules(ctx: &mut Ctx, task: Task) {
    let timing = ctx.linked.iter().find_map(|e| match e {
        Evidence::FeatureTiming { id, features, .. } => Some((id.clone(), features.clone())),
        _ => None,
    });
    let partition = ctx.linked.iter().find_map(|e| match e {
        Evidence::Partition {
            id,
            ordering,
            splits,
            training_split,
            evaluation_split,
            preprocessing_fit_splits,
        } => Some((
            id.clone(),
            *ordering,
            splits.clone(),
            training_split.clone(),
            evaluation_split.clone(),
            preprocessing_fit_splits.clone(),
        )),
        _ => None,
    });
    let construction = ctx.linked.iter().find_map(|e| match e {
        Evidence::TargetConstruction { id, identities, .. } => {
            Some((id.clone(), identities.clone()))
        }
        _ => None,
    });
    let fam = Family::LeakageMechanics;
    match &timing {
        None => ctx.push(
            "feature_timing_present",
            fam,
            "leakage",
            Outcome::Insufficient,
            "no feature-timing evidence".into(),
            &[],
        ),
        Some((id, features)) => {
            let late: Vec<_> = features
                .iter()
                .filter(|f| f.available_offset_hours > 0.0)
                .map(|f| f.name.clone())
                .collect();
            let o = if late.is_empty() {
                Outcome::Support
            } else {
                Outcome::Contradict
            };
            ctx.push(
                "feature_availability",
                fam,
                "leakage",
                o,
                format!("predictors available after the origin: {late:?}"),
                &[id],
            );
        }
    }
    match partition {
        None => ctx.push(
            "partition_present",
            fam,
            "leakage",
            Outcome::Insufficient,
            "no partition evidence".into(),
            &[],
        ),
        Some((id, ordering, splits, train, eval, fit)) => {
            let mut problems = Vec::new();
            for (i, a) in splits.iter().enumerate() {
                for b in &splits[i + 1..] {
                    if a.start <= b.end && b.start <= a.end {
                        problems.push(format!("{} overlaps {}", a.name, b.name));
                    }
                }
            }
            let find = |n: &str| splits.iter().find(|s| s.name == n);
            match (find(&train), find(&eval)) {
                (Some(t), Some(e)) => {
                    if task == Task::Forecast && t.end >= e.start {
                        problems.push("training does not precede evaluation".into());
                    }
                }
                _ => problems.push("training or evaluation split undefined".into()),
            }
            if task == Task::Forecast && ordering == Ordering::Random {
                problems.push("random ordering for a forecasting task".into());
            }
            let bad_fit: Vec<_> = fit.iter().filter(|s| **s != train).cloned().collect();
            if !bad_fit.is_empty() {
                problems.push(format!("preprocessing fitted on {bad_fit:?}"));
            }
            let o = if problems.is_empty() {
                Outcome::Support
            } else {
                Outcome::Contradict
            };
            ctx.push(
                "partition_integrity",
                fam,
                "leakage",
                o,
                format!("{problems:?}"),
                &[&id],
            );
        }
    }
    match construction {
        None => ctx.push(
            "target_construction_present",
            fam,
            "leakage",
            Outcome::Insufficient,
            "no target-construction evidence".into(),
            &[],
        ),
        Some((id, identities)) => match &timing {
            None => ctx.push(
                "target_construction",
                fam,
                "leakage",
                Outcome::Insufficient,
                "predictor set unknown".into(),
                &[&id],
            ),
            Some((_, features)) => {
                let names: BTreeSet<_> = features.iter().map(|f| f.name.as_str()).collect();
                let exact: Vec<_> = identities
                    .iter()
                    .filter(|i| {
                        i.rows > 0
                            && i.max_abs_residual == 0.0
                            && i.features.iter().all(|f| names.contains(f.as_str()))
                    })
                    .map(|i| i.features.clone())
                    .collect();
                let o = if exact.is_empty() {
                    Outcome::Support
                } else {
                    Outcome::Contradict
                };
                ctx.push(
                    "target_reconstruction",
                    fam,
                    "leakage",
                    o,
                    format!("predictor sets reconstructing the target exactly: {exact:?}"),
                    &[&id],
                );
            }
        },
    }
}

fn operational_rules(
    ctx: &mut Ctx,
    recent_population: &str,
    condition: &str,
    bound: f64,
    gamma: f64,
    min_support: u64,
) {
    let cal = ctx.linked.iter().find_map(|e| match e {
        Evidence::Calibration {
            id,
            condition_counts,
            ..
        } => Some((
            id.clone(),
            condition_counts.get(condition).copied().unwrap_or(0),
        )),
        _ => None,
    });
    let Some((cid, count)) = cal else {
        ctx.push(
            "calibration_present",
            Family::Evidence,
            "missing_evidence",
            Outcome::Insufficient,
            "no calibration evidence".into(),
            &[],
        );
        return;
    };
    if !ctx.on(Family::CalibrationApplicability) || count >= min_support {
        ctx.push(
            "R1_condition_support",
            Family::CalibrationApplicability,
            "calibration_applicability",
            Outcome::Support,
            format!("{count} calibration units for '{condition}'"),
            &[&cid],
        );
    } else {
        ctx.push(
            "R1_condition_support",
            Family::CalibrationApplicability,
            "calibration_applicability",
            Outcome::Insufficient,
            format!("{count} calibration units for '{condition}' (< {min_support})"),
            &[&cid],
        );
    }
    if ctx.on(Family::Representativeness) {
        let s = ctx.linked.iter().find_map(|e| match e {
            Evidence::SupportCheck {
                id,
                value,
                support_max,
                ..
            } => Some((id.clone(), *value, *support_max)),
            _ => None,
        });
        match s {
            None => ctx.push(
                "R2_representativeness",
                Family::Representativeness,
                "missing_evidence",
                Outcome::Insufficient,
                "no recent monitoring statistic".into(),
                &[],
            ),
            Some((id, v, m)) if v > m => ctx.push(
                "R2_representativeness",
                Family::Representativeness,
                "distribution_representativeness",
                Outcome::Insufficient,
                format!("recent statistic {v} exceeds calibration maximum {m}"),
                &[&id],
            ),
            Some(_) => {}
        }
    }
    if ctx.on(Family::Completeness) {
        let c = ctx.linked.iter().find_map(|e| match e {
            Evidence::Completeness {
                id,
                required,
                available,
            } => Some((id.clone(), *required, *available)),
            _ => None,
        });
        match c {
            None => ctx.push(
                "R3_completeness",
                Family::Completeness,
                "missing_evidence",
                Outcome::Insufficient,
                "no completeness evidence".into(),
                &[],
            ),
            Some((id, r, a)) if a < r => ctx.push(
                "R3_completeness",
                Family::Completeness,
                "missing_evidence",
                Outcome::Insufficient,
                format!("{a} of {r} required recent units available"),
                &[&id],
            ),
            Some(_) => {}
        }
    }
    if ctx.on(Family::Statistical) {
        let recent: Vec<_> = ctx
            .linked
            .iter()
            .filter_map(|e| rate_view(e))
            .filter(|(_, p, _)| *p == recent_population)
            .map(|(_, _, r)| r)
            .collect();
        for r in recent {
            if r.n == 0.0 {
                continue;
            }
            if ctx.on(Family::Dependence)
                && matches!(r.dependence, Dependence::Temporal | Dependence::Clustered)
                && r.design_effect.is_none()
            {
                dependence_rule(ctx, &r);
                continue;
            }
            if let Some((lo, _, how)) = r.bounds(gamma, ctx.on(Family::Dependence)) {
                if lo > bound {
                    ctx.push(
                        "R4_recent_outcomes",
                        Family::Statistical,
                        "statistical_sample",
                        Outcome::Contradict,
                        format!("{how}: lower bound {lo:.6} exceeds {bound}"),
                        &[&r.id],
                    );
                }
            }
        }
    }
}

/// Classify every claim. `root` resolves `file_hash` paths; `disabled` ablates rule families.
pub fn assess(
    a: &Assessment,
    root: Option<&Path>,
    disabled: &BTreeSet<Family>,
) -> Result<Vec<Finding>> {
    validate(a)?;
    let mut out = Vec::new();
    for claim in &a.claims {
        let linked: Vec<_> = claim
            .common()
            .1
            .iter()
            .map(|id| a.evidence.iter().find(|e| e.id() == id).unwrap())
            .collect();
        let mut ctx = Ctx {
            linked,
            disabled,
            root,
            reasons: Vec::new(),
        };
        let provenance = matches!(claim, Claim::Provenance { .. });
        if !provenance && ctx.on(Family::Integrity) {
            let broken: Vec<_> = ctx
                .linked
                .iter()
                .filter_map(|e| hash_status(e, root).map(|s| (e.id().to_string(), s)))
                .filter(|(_, s)| !matches!(s, Ok(true)))
                .collect();
            for (id, s) in broken {
                ctx.push(
                    "integrity",
                    Family::Integrity,
                    "integrity",
                    Outcome::Insufficient,
                    match s {
                        Err(e) => e,
                        _ => "linked artifact hash mismatch".into(),
                    },
                    &[&id],
                );
            }
        }
        let integrity_failed = ctx.reasons.iter().any(|r| r.rule == "integrity");
        classify_claim(claim, &mut ctx);
        let outcomes: Vec<_> = ctx.reasons.iter().map(|r| r.outcome).collect();
        let classification = if outcomes.contains(&Outcome::NotApplicable) {
            Class::NotApplicable
        } else if integrity_failed {
            Class::InsufficientEvidence
        } else if outcomes.contains(&Outcome::Contradict) {
            Class::Contradicted
        } else if outcomes.contains(&Outcome::Insufficient) || !outcomes.contains(&Outcome::Support)
        {
            // Fail closed: supported requires at least one positive finding.
            Class::InsufficientEvidence
        } else {
            Class::SupportedWithinScope
        };
        let attestations = ctx
            .linked
            .iter()
            .filter(|e| matches!(e, Evidence::Attestation { .. }))
            .map(|e| e.id().to_string())
            .collect();
        out.push(Finding {
            claim_id: claim.id().into(),
            claim_type: claim.type_name().into(),
            classification,
            reasons: ctx.reasons,
            attestations_not_relied_upon: if disabled.contains(&Family::AttestationPolicy) {
                Vec::new()
            } else {
                attestations
            },
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Baseline C: standard evidence checks (section 5), optionally with the scope rule.

fn c_reason(dimension: &str, outcome: Outcome, detail: impl Into<String>) -> Reason {
    Reason {
        rule: "standard_check".into(),
        family: Family::Evidence,
        dimension: dimension.into(),
        outcome,
        detail: detail.into(),
        evidence: Vec::new(),
        basis: Basis::None,
    }
}
pub fn standard_checks(
    a: &Assessment,
    root: Option<&Path>,
    with_scope: bool,
) -> Result<Vec<Finding>> {
    validate(a)?;
    let mut out = Vec::new();
    for claim in &a.claims {
        let linked: Vec<_> = claim
            .common()
            .1
            .iter()
            .map(|id| a.evidence.iter().find(|e| e.id() == id).unwrap())
            .collect();
        let mut reasons = Vec::new();
        for e in &linked {
            match hash_status(e, root) {
                Some(Ok(false)) => {
                    reasons.push(c_reason("provenance", Outcome::Contradict, "hash mismatch"))
                }
                Some(Err(m)) => reasons.push(c_reason("provenance", Outcome::Insufficient, m)),
                Some(Ok(true)) => {
                    reasons.push(c_reason("provenance", Outcome::Support, "hash matches"))
                }
                None => {}
            }
        }
        let rates = |metric: &str, population: Option<&str>| -> Vec<Rate> {
            linked
                .iter()
                .filter_map(|e| rate_view(e))
                .filter(|(m, p, _)| *m == metric && population.is_none_or(|q| q == *p))
                .map(|(_, _, r)| r)
                .collect()
        };
        let attestation = linked.iter().find_map(|e| match e {
            Evidence::Attestation { value, .. } => Some(*value),
            _ => None,
        });
        let scope_fail =
            |claimed: &[String], r: &Rate| with_scope && !subset(claimed, &r.conditions).is_empty();
        let naive = |r: &Rate| -> std::result::Result<f64, String> {
            match (r.events, r.value) {
                (Some(k), _) if k > r.n => Err("events exceed n".into()),
                (Some(k), _) if r.n > 0.0 => Ok(k / r.n),
                (_, Some(v)) => Ok(v),
                _ => Err("no value".into()),
            }
        };
        let bound = |r: &Rate, cmp: Comparator, b: f64, g: f64, reasons: &mut Vec<Reason>| match r
            .bounds(g, false)
        {
            None => reasons.push(c_reason(
                "statistical_sample",
                Outcome::Insufficient,
                "no interval",
            )),
            Some((lo, hi, _)) => {
                let (s, c) = match cmp {
                    Comparator::AtMost => (hi <= b, lo > b),
                    _ => (lo >= b, hi < b),
                };
                reasons.push(c_reason(
                    "statistical_sample",
                    if s {
                        Outcome::Support
                    } else if c {
                        Outcome::Contradict
                    } else {
                        Outcome::Insufficient
                    },
                    format!("[{lo:.6}, {hi:.6}] versus {b}"),
                ));
            }
        };
        match claim {
            Claim::MetricValue {
                metric,
                population,
                conditions,
                comparator,
                claimed_value,
                tolerance,
                ..
            } => {
                let rs = rates(metric, Some(population));
                if rs.is_empty() {
                    reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing metric evidence",
                    ));
                }
                for r in rs {
                    if scope_fail(conditions, &r) {
                        reasons.push(c_reason(
                            "population_scope",
                            Outcome::Insufficient,
                            "conditions not evaluated",
                        ));
                    }
                    match naive(&r) {
                        Err(e) => {
                            reasons.push(c_reason("numerical_validity", Outcome::Contradict, e))
                        }
                        Ok(v) => {
                            let ok = match comparator {
                                Comparator::Equals => (v - claimed_value).abs() <= *tolerance,
                                Comparator::AtMost => v <= claimed_value + tolerance,
                                Comparator::AtLeast => v >= claimed_value - tolerance,
                            };
                            reasons.push(c_reason(
                                "numerical_validity",
                                if ok {
                                    Outcome::Support
                                } else {
                                    Outcome::Contradict
                                },
                                format!("{v}"),
                            ));
                        }
                    }
                }
            }
            Claim::MetricBound {
                metric,
                population,
                conditions,
                comparator,
                bound: b,
                confidence,
                claimed_value,
                tolerance,
                ..
            } => {
                let rs = rates(metric, Some(population));
                if rs.is_empty() {
                    reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing metric evidence",
                    ));
                }
                for r in rs {
                    if scope_fail(conditions, &r) {
                        reasons.push(c_reason(
                            "population_scope",
                            Outcome::Insufficient,
                            "conditions not evaluated",
                        ));
                    }
                    match naive(&r) {
                        Err(e) => {
                            reasons.push(c_reason("numerical_validity", Outcome::Contradict, e))
                        }
                        Ok(v) => {
                            if claimed_value.is_some_and(|c| (v - c).abs() > *tolerance) {
                                reasons.push(c_reason(
                                    "numerical_validity",
                                    Outcome::Contradict,
                                    "claimed value differs",
                                ));
                            }
                            bound(&r, *comparator, *b, *confidence, &mut reasons);
                        }
                    }
                }
            }
            Claim::CoverageGuarantee {
                population,
                conditions,
                level,
                ..
            } => {
                let cal = linked.iter().find_map(|e| match e {
                    Evidence::Calibration { conditions, .. } => Some(conditions.clone()),
                    _ => None,
                });
                match cal {
                    None => reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing calibration",
                    )),
                    Some(cc) => {
                        if with_scope && !subset(conditions, &cc).is_empty() {
                            reasons.push(c_reason(
                                "calibration_applicability",
                                Outcome::Insufficient,
                                "conditions not calibrated",
                            ));
                        }
                        reasons.push(c_reason(
                            "calibration_applicability",
                            Outcome::Support,
                            "calibration present",
                        ));
                        for r in rates("interval_coverage", Some(population)) {
                            if naive(&r).is_ok_and(|v| v < *level) {
                                reasons.push(c_reason(
                                    "statistical_sample",
                                    Outcome::Contradict,
                                    "realised coverage below level",
                                ));
                            }
                        }
                    }
                }
            }
            Claim::NumericalAdequacy {
                required_checks, ..
            } => {
                match linked.iter().find_map(|e| match e {
                    Evidence::NumericalChecks { checks, .. } => Some(checks.clone()),
                    _ => None,
                }) {
                    None => reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing numerical checks",
                    )),
                    Some(ch) => {
                        let o = if required_checks
                            .iter()
                            .any(|c| ch.get(c) == Some(&Some(false)))
                        {
                            Outcome::Contradict
                        } else if required_checks
                            .iter()
                            .any(|c| ch.get(c).is_none_or(Option::is_none))
                        {
                            Outcome::Insufficient
                        } else {
                            Outcome::Support
                        };
                        reasons.push(c_reason("numerical_adequacy", o, "reported checks"));
                    }
                }
            }
            Claim::Provenance { .. } => {
                if !linked.iter().any(|e| hash_status(e, root).is_some()) {
                    reasons.push(c_reason("provenance", Outcome::Insufficient, "no hash"));
                }
            }
            Claim::NoLeakage { .. } | Claim::Independence { .. } => {
                let dim = if matches!(claim, Claim::NoLeakage { .. }) {
                    "leakage"
                } else {
                    "temporal_dependence"
                };
                match attestation {
                    None => reasons.push(c_reason(
                        dim,
                        Outcome::Insufficient,
                        "checklist item not answered",
                    )),
                    Some(v) => reasons.push(c_reason(
                        dim,
                        if v {
                            Outcome::Support
                        } else {
                            Outcome::Contradict
                        },
                        "checklist attestation",
                    )),
                }
            }
            Claim::PhysicalGeneralization { to_domain, .. } => {
                match linked.iter().find_map(|e| match e {
                    Evidence::PhysicalValidation { domain, passed, .. } => {
                        Some((domain.clone(), *passed))
                    }
                    _ => None,
                }) {
                    None => reasons.push(c_reason(
                        "population_scope",
                        Outcome::Insufficient,
                        "missing physical validation",
                    )),
                    Some((d, p)) => {
                        if with_scope && d != *to_domain {
                            reasons.push(c_reason(
                                "population_scope",
                                Outcome::Insufficient,
                                "different domain",
                            ));
                        }
                        reasons.push(c_reason(
                            "population_scope",
                            if p {
                                Outcome::Support
                            } else {
                                Outcome::Contradict
                            },
                            "physical validation",
                        ));
                    }
                }
            }
            Claim::UniversalReliability {
                metric,
                reliability_bound,
                confidence,
                ..
            } => {
                let rs = rates(metric, None);
                if rs.is_empty() {
                    reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing metric evidence",
                    ));
                }
                for r in rs {
                    bound(
                        &r,
                        Comparator::AtMost,
                        *reliability_bound,
                        *confidence,
                        &mut reasons,
                    );
                }
            }
            Claim::OperationalRisk {
                recent_population,
                bound: b,
                ..
            } => {
                if !linked
                    .iter()
                    .any(|e| matches!(e, Evidence::Calibration { .. }))
                {
                    reasons.push(c_reason(
                        "missing_evidence",
                        Outcome::Insufficient,
                        "missing calibration",
                    ));
                } else {
                    reasons.push(c_reason(
                        "calibration_applicability",
                        Outcome::Support,
                        "calibration present",
                    ));
                    for (_, p, r) in linked.iter().filter_map(|e| rate_view(e)) {
                        if p == recent_population && naive(&r).is_ok_and(|v| v > *b) {
                            reasons.push(c_reason(
                                "statistical_sample",
                                Outcome::Contradict,
                                "recent risk above bound",
                            ));
                        }
                    }
                }
            }
        }
        let o: Vec<_> = reasons.iter().map(|r| r.outcome).collect();
        let classification = if o.contains(&Outcome::Contradict) {
            Class::Contradicted
        } else if o.contains(&Outcome::Insufficient) || !o.contains(&Outcome::Support) {
            Class::InsufficientEvidence
        } else {
            Class::SupportedWithinScope
        };
        out.push(Finding {
            claim_id: claim.id().into(),
            claim_type: claim.type_name().into(),
            classification,
            reasons,
            attestations_not_relied_upon: Vec::new(),
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Reports and replay

pub fn report(a: &Assessment, root: Option<&Path>) -> Result<Value> {
    let none = BTreeSet::new();
    let findings = assess(a, root, &none)?;
    let standard = standard_checks(a, root, false)?;
    let mut counts = BTreeMap::new();
    for f in &findings {
        *counts.entry(f.classification).or_insert(0usize) += 1;
    }
    Ok(json!({
        "schema_version":"triad-evidence-report-1",
        "software_version":env!("CARGO_PKG_VERSION"),
        "evaluator_source_sha256":crate::evaluation::hash(include_bytes!("evidence.rs")),
        "input_sha256":crate::evaluation::hash(&serde_json::to_vec(a)?),
        "input":a,
        "findings":findings,
        "standard_checks_baseline":standard,
        "classification_counts":counts,
        "interpretation":"Rule-based, claim-relative classification. Supported within scope means every requirement for this claim type was met by computed or supplied structured evidence and nothing contradicted it; it is not proof that the claim is true. Supplied data and attestations are not verified against the world. A matching hash establishes byte consistency only."
    }))
}
pub fn validate_report(report_value: &Value, root: Option<&Path>) -> Result<()> {
    let a: Assessment = serde_json::from_value(
        report_value
            .get("input")
            .context("evidence report missing input")?
            .clone(),
    )?;
    let expected = report(&a, root)?;
    if &expected != report_value {
        bail!("evidence report differs from deterministic replay (evaluator, input or referenced files changed)");
    }
    Ok(())
}
