//! Sharper certification of a fixed-candidate abstention policy.
//!
//! [`crate::abstention::fit`] bounds selective risk as a *ratio of two
//! Hoeffding bounds*. That spends the deviation radius twice and ignores how
//! small the error rate is. This module tests the same family-wise claim with
//! established tools from the risk-control literature:
//!
//! * **Linearisation.** For a fixed threshold `t`, conditional risk
//!   `R(t) = J(t) / C(t)` satisfies `R(t) <= alpha` exactly when
//!   `J(t) - alpha * C(t) <= 0`. Per independent sequence the variable
//!   `Z = e - alpha * a + alpha`, with `a` the accepted fraction and `e` the
//!   accepted-incorrect fraction, lies in `[0, 1]` and has mean `alpha` exactly
//!   at the risk boundary. One bounded-mean test replaces a ratio of bounds.
//! * **Hoeffding–Bentkus p-values** (Bates, Angelopoulos, Lei, Malik and
//!   Jordan, 2021, *Distribution-free, risk-controlling prediction sets*;
//!   Angelopoulos, Bates, Candès, Jordan and Lei, 2021, *Learn then Test*).
//!   The Chernoff–KL term dominates additive Hoeffding by Pinsker's inequality.
//! * **Betting** (Waudby-Smith and Ramdas, 2024, *Estimating means of bounded
//!   random variables by betting*, JRSS-B). A predictable-plug-in capital
//!   process with Ville's inequality adapts to the sequence-level variance.
//!
//! Every candidate is tested at `delta / (2K)` for risk and for coverage, the
//! same Bonferroni budget as the legacy controller. Consequently the
//! Hoeffding–Bentkus method qualifies every candidate the legacy controller
//! qualifies and never reports a smaller coverage lower bound (proved in
//! `docs/certification.md`; enforced and tested here). The betting method
//! has no such dominance guarantee; it is a separate, predeclared choice.
//!
//! Assumptions are unchanged: independent, identically distributed sequences
//! of equal length, predeclared candidate thresholds, trustworthy labels and a
//! frozen scoring model. Frames inside a sequence may be dependent. Choose the
//! method *before* seeing calibration data; trying both and keeping the one
//! that certifies doubles the error budget.
use crate::abstention::{CalibrationSequence, Decision, FitSpecification, PolicyStatus};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// The existing `abstention::fit` construction, recomputed from counts.
    LegacyHoeffdingRatio,
    /// Linearised risk, Hoeffding–Bentkus p-value, Chernoff–KL coverage bound.
    HoeffdingBentkus,
    /// Linearised risk and coverage, predictable-plug-in betting p-values.
    Betting,
    /// Comparator only: treats every frame as independent (exact binomial
    /// risk test, Chernoff coverage over frames). Invalid when frames within a
    /// sequence are dependent; never offered as a deployable policy.
    NaiveFrameBinomial,
}

impl Method {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "legacy-hoeffding-ratio" => Some(Self::LegacyHoeffdingRatio),
            "hoeffding-bentkus" => Some(Self::HoeffdingBentkus),
            "betting" => Some(Self::Betting),
            "naive-frame-binomial" => Some(Self::NaiveFrameBinomial),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::LegacyHoeffdingRatio => "legacy-hoeffding-ratio",
            Self::HoeffdingBentkus => "hoeffding-bentkus",
            Self::Betting => "betting",
            Self::NaiveFrameBinomial => "naive-frame-binomial",
        }
    }
    /// Whether the family-wise guarantee survives dependent frames inside
    /// independent sequences.
    pub fn valid_under_sequence_dependence(self) -> bool {
        !matches!(self, Self::NaiveFrameBinomial)
    }
}

/// Bernoulli Kullback–Leibler divergence kl(p || q), with 0 log 0 = 0.
pub fn kl_bernoulli(p: f64, q: f64) -> f64 {
    let term = |x: f64, y: f64| if x <= 0.0 { 0.0 } else { x * (x / y).ln() };
    if q <= 0.0 {
        return if p <= 0.0 { 0.0 } else { f64::INFINITY };
    }
    if q >= 1.0 {
        return if p >= 1.0 { 0.0 } else { f64::INFINITY };
    }
    (term(p, q) + term(1.0 - p, 1.0 - q)).max(0.0)
}

/// P(Binomial(n, p) <= k), summed in log space so large `n` cannot underflow.
pub fn binomial_cdf(k: i64, n: u64, p: f64) -> f64 {
    if k < 0 {
        return 0.0;
    }
    if k as u64 >= n || p <= 0.0 {
        return 1.0;
    }
    if p >= 1.0 {
        return 0.0;
    }
    let (lp, lq) = (p.ln(), (-p).ln_1p());
    let mut log_pmf = n as f64 * lq;
    let mut acc = f64::NEG_INFINITY;
    for i in 0..=k as u64 {
        if i > 0 {
            log_pmf += ((n - i + 1) as f64 / i as f64).ln() + lp - lq;
        }
        let (hi, lo) = if acc > log_pmf {
            (acc, log_pmf)
        } else {
            (log_pmf, acc)
        };
        acc = hi + (lo - hi).exp().ln_1p();
    }
    acc.exp().min(1.0)
}

/// Hoeffding–Bentkus p-value for H0: E[X] >= alpha, X in [0, 1], from the
/// sample mean of `n` independent observations (Bates et al. 2021, eq. 18).
pub fn hoeffding_bentkus_p_value(mean: f64, n: usize, alpha: f64) -> f64 {
    if alpha >= 1.0 {
        return 0.0;
    }
    if alpha <= 0.0 || n == 0 {
        return 1.0;
    }
    let hoeffding = (-(n as f64) * kl_bernoulli(mean.min(alpha), alpha)).exp();
    let bentkus =
        std::f64::consts::E * binomial_cdf((n as f64 * mean).ceil() as i64, n as u64, alpha);
    hoeffding.min(bentkus).min(1.0)
}

/// One-sided Chernoff–KL lower confidence bound for the mean of `n`
/// independent observations in [0, 1]. Never below the Hoeffding bound.
pub fn kl_lower_bound(mean: f64, n: usize, delta: f64) -> f64 {
    if n == 0 || mean <= 0.0 {
        return 0.0;
    }
    let budget = (1.0 / delta).ln() / n as f64;
    let hoeffding = mean - (budget / 2.0).sqrt();
    // kl(mean || c) decreases in c on [0, mean]; keep `lo` on the rejected side.
    let (mut lo, mut hi) = (0.0_f64, mean);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if kl_bernoulli(mean, mid) > budget {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo.max(hoeffding).max(0.0)
}

/// Betting p-value (Waudby-Smith and Ramdas 2024) for observations in [0, 1].
/// `upper == false` tests H0: E[X] >= m (evidence the mean is small);
/// `upper == true` tests H0: E[X] <= m (evidence the mean is large).
/// Bets are predictable plug-ins tuned for a fixed sample of `xs.len()`;
/// Ville's inequality makes `1 / max capital` a valid p-value for any order
/// that does not depend on the observations.
pub fn betting_p_value(xs: &[f64], m: f64, delta: f64, upper: bool) -> f64 {
    let n = xs.len();
    let room = if upper { m } else { 1.0 - m };
    if n == 0 || room <= 0.0 {
        return 1.0;
    }
    let cap = 0.5 / room;
    let scale = 2.0 * (1.0 / delta).ln() / n as f64;
    let (mut sum, mut sq, mut log_capital, mut best) = (0.0, 0.0, 0.0_f64, 0.0_f64);
    for (i, &x) in xs.iter().enumerate() {
        let variance = (0.25 + sq) / (i as f64 + 1.0);
        let lambda = (scale / variance).sqrt().min(cap);
        let gain = if upper { x - m } else { m - x };
        log_capital += (lambda * gain).ln_1p();
        best = best.max(log_capital);
        sum += x;
        let mean = (0.5 + sum) / (i as f64 + 2.0);
        sq += (x - mean) * (x - mean);
    }
    (-best).exp().min(1.0)
}

/// Accepted and accepted-incorrect frame counts for every (sequence, candidate).
#[derive(Clone, Debug)]
pub struct CountTable {
    pub thresholds: Vec<f64>,
    pub frames_per_sequence: usize,
    sequences: usize,
    accepted: Vec<u32>,
    incorrect: Vec<u32>,
}

impl CountTable {
    pub fn new(thresholds: Vec<f64>, frames_per_sequence: usize) -> Self {
        Self {
            thresholds,
            frames_per_sequence,
            sequences: 0,
            accepted: Vec::new(),
            incorrect: Vec::new(),
        }
    }
    /// Append one sequence as (score, incorrect) frames.
    pub fn push(&mut self, frames: impl IntoIterator<Item = (f64, bool)>) {
        let k = self.thresholds.len();
        let start = self.accepted.len();
        self.accepted.resize(start + k, 0);
        self.incorrect.resize(start + k, 0);
        for (score, bad) in frames {
            let first = self.thresholds.partition_point(|&t| t < score);
            for j in first..k {
                self.accepted[start + j] += 1;
                self.incorrect[start + j] += u32::from(bad);
            }
        }
        self.sequences += 1;
    }
    pub fn sequences(&self) -> usize {
        self.sequences
    }
    fn column(&self, j: usize) -> impl Iterator<Item = (u32, u32)> + '_ {
        let k = self.thresholds.len();
        (0..self.sequences).map(move |i| (self.accepted[i * k + j], self.incorrect[i * k + j]))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Targets {
    pub maximum_accepted_error_rate: f64,
    pub minimum_coverage: f64,
    pub delta: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateCertificate {
    pub threshold: f64,
    pub observed_coverage: f64,
    pub observed_conditional_error: Option<f64>,
    /// Hoeffding (legacy) or Chernoff–KL (Hoeffding–Bentkus) bound; `None`
    /// for tests that certify coverage without inverting a bound.
    pub coverage_lower_bound: Option<f64>,
    pub coverage_p_value: Option<f64>,
    pub risk_p_value: Option<f64>,
    /// Legacy ratio bound only.
    pub conditional_error_upper_bound: Option<f64>,
    pub coverage_certified: bool,
    pub risk_certified: bool,
    pub qualifies: bool,
}

/// Evaluate every candidate with one method. Sequence order matters only for
/// `Betting` and must not depend on scores or labels.
pub fn evaluate_candidates(
    table: &CountTable,
    targets: Targets,
    method: Method,
) -> Vec<CandidateCertificate> {
    let n = table.sequences();
    let frames = table.frames_per_sequence as f64;
    let k = table.thresholds.len();
    let alpha = targets.maximum_accepted_error_rate;
    let cmin = targets.minimum_coverage;
    let per_test = targets.delta / (2.0 * k as f64);
    let radius = ((2.0 * k as f64 / targets.delta).ln() / (2.0 * n as f64)).sqrt();
    let total = (n * table.frames_per_sequence) as f64;
    (0..k)
        .map(|j| {
            let (acc, bad) = table
                .column(j)
                .fold((0u64, 0u64), |(a, b), (x, y)| (a + x as u64, b + y as u64));
            let coverage = acc as f64 / total;
            let joint = bad as f64 / total;
            // Legacy conditions, also the numerical guard for dominance below.
            let legacy_lower = (coverage - radius).max(0.0);
            let legacy_upper = if legacy_lower > 0.0 {
                ((joint + radius).min(1.0) / legacy_lower).min(1.0)
            } else {
                1.0
            };
            let legacy_cov = legacy_lower >= cmin;
            let legacy_risk = legacy_upper <= alpha;
            let mut c = CandidateCertificate {
                threshold: table.thresholds[j],
                observed_coverage: coverage,
                observed_conditional_error: (acc > 0).then(|| bad as f64 / acc as f64),
                coverage_lower_bound: None,
                coverage_p_value: None,
                risk_p_value: None,
                conditional_error_upper_bound: None,
                coverage_certified: false,
                risk_certified: false,
                qualifies: false,
            };
            match method {
                Method::LegacyHoeffdingRatio => {
                    c.coverage_lower_bound = Some(legacy_lower);
                    c.conditional_error_upper_bound = Some(legacy_upper);
                    c.coverage_certified = legacy_cov;
                    c.risk_certified = legacy_risk;
                }
                Method::HoeffdingBentkus => {
                    let lower = kl_lower_bound(coverage, n, per_test).max(legacy_lower);
                    let z = joint - alpha * coverage + alpha;
                    let p = hoeffding_bentkus_p_value(z, n, alpha);
                    c.coverage_lower_bound = Some(lower);
                    c.risk_p_value = Some(p);
                    c.coverage_certified = lower >= cmin;
                    // The legacy condition implies p <= per_test exactly
                    // (Pinsker); OR-ing it removes floating-point edge cases.
                    c.risk_certified = p <= per_test || legacy_risk;
                }
                Method::Betting => {
                    let (zs, cs): (Vec<f64>, Vec<f64>) = table
                        .column(j)
                        .map(|(a, b)| {
                            let (a, b) = (a as f64 / frames, b as f64 / frames);
                            ((b - alpha * a + alpha).clamp(0.0, 1.0), a)
                        })
                        .unzip();
                    let pr = if alpha >= 1.0 {
                        0.0
                    } else {
                        betting_p_value(&zs, alpha, per_test, false)
                    };
                    let pc = betting_p_value(&cs, cmin, per_test, true);
                    c.risk_p_value = Some(pr);
                    c.coverage_p_value = Some(pc);
                    c.coverage_certified = pc <= per_test;
                    c.risk_certified = pr <= per_test;
                }
                Method::NaiveFrameBinomial => {
                    let pr = if alpha >= 1.0 {
                        0.0
                    } else if acc == 0 {
                        1.0
                    } else {
                        binomial_cdf(bad as i64, acc, alpha)
                    };
                    let pc = if coverage > cmin {
                        (-total * kl_bernoulli(coverage, cmin)).exp()
                    } else {
                        1.0
                    };
                    c.risk_p_value = Some(pr);
                    c.coverage_p_value = Some(pc);
                    c.coverage_certified = pc <= per_test;
                    c.risk_certified = pr <= per_test;
                }
            }
            c.qualifies = c.coverage_certified && c.risk_certified;
            c
        })
        .collect()
}

/// Index of the qualifying candidate with the largest observed coverage;
/// ties keep the smaller threshold, matching `abstention::fit`.
pub fn select(candidates: &[CandidateCertificate]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, c) in candidates.iter().enumerate() {
        if c.qualifies && best.is_none_or(|b| c.observed_coverage > candidates[b].observed_coverage)
        {
            best = Some(i);
        }
    }
    best
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub schema_version: String,
    pub status: PolicyStatus,
    pub method: Method,
    pub method_id: String,
    pub threshold: Option<f64>,
    pub independent_sequences: usize,
    pub frames_per_sequence: usize,
    pub delta: f64,
    pub per_test_delta: f64,
    pub maximum_accepted_error_rate: f64,
    pub minimum_coverage: f64,
    pub sequence_order: String,
    pub candidates: Vec<CandidateCertificate>,
    pub guarantee: String,
    pub warning: String,
}

impl Certificate {
    /// Fail closed exactly like `FittedPolicy::decide`.
    pub fn decide(&self, score: Option<f64>) -> Decision {
        let Some(score) = score.filter(|s| s.is_finite()) else {
            return Decision::WithholdInvalidScore;
        };
        let Some(t) = self.threshold.filter(|t| t.is_finite()) else {
            return Decision::WithholdUncalibrated;
        };
        if self.status != PolicyStatus::Ready
            || !self.method.valid_under_sequence_dependence()
            || !self
                .candidates
                .iter()
                .any(|c| c.qualifies && c.threshold == t)
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

/// Order sequences by SHA-256 of their identifiers: deterministic and
/// independent of scores and labels, as the betting test requires.
pub fn canonical_order(sequences: &[CalibrationSequence]) -> Vec<usize> {
    let mut keyed: Vec<_> = sequences
        .iter()
        .enumerate()
        .map(|(i, s)| (Sha256::digest(s.id.as_bytes()), i))
        .collect();
    keyed.sort();
    keyed.into_iter().map(|(_, i)| i).collect()
}

/// Certify a predeclared candidate set with a predeclared method.
pub fn certify(
    spec: &FitSpecification,
    sequences: &[CalibrationSequence],
    method: Method,
) -> Result<Certificate> {
    ensure!(
        method.valid_under_sequence_dependence(),
        "{} is a study comparator, not a deployable certification method",
        method.name()
    );
    let frames = crate::abstention::validate(spec, sequences)?;
    let mut table = CountTable::new(spec.candidate_thresholds.clone(), frames);
    for i in canonical_order(sequences) {
        table.push(sequences[i].frames.iter().map(|f| (f.score, f.incorrect)));
    }
    let targets = Targets {
        maximum_accepted_error_rate: spec.maximum_accepted_error_rate,
        minimum_coverage: spec.minimum_coverage,
        delta: spec.delta,
    };
    let candidates = evaluate_candidates(&table, targets, method);
    let chosen = select(&candidates);
    Ok(Certificate {
        schema_version: "triad-certificate-1".into(),
        status: if chosen.is_some() { PolicyStatus::Ready } else { PolicyStatus::Refused },
        method,
        method_id: spec.method_id.clone(),
        threshold: chosen.map(|i| candidates[i].threshold),
        independent_sequences: sequences.len(),
        frames_per_sequence: frames,
        delta: spec.delta,
        per_test_delta: spec.delta / (2.0 * spec.candidate_thresholds.len() as f64),
        maximum_accepted_error_rate: spec.maximum_accepted_error_rate,
        minimum_coverage: spec.minimum_coverage,
        sequence_order: "ascending SHA-256 of sequence id".into(),
        candidates,
        guarantee: "With probability at least 1 - delta over the draw of calibration sequences, every qualifying candidate simultaneously has conditional accepted-error rate <= maximum_accepted_error_rate and coverage >= minimum_coverage on the sampled population (Bonferroni over 2K one-sided tests).".into(),
        warning: "Holds only for independent identically distributed sequences of equal length from the deployment population, predeclared candidate thresholds and method, trustworthy labels and a frozen scoring model. No assurance under drift, hardware or site shift, numerical model mismatch, or reuse of calibration data as test evidence.".into(),
    })
}

/// Plug-in planning: the number of independent sequences at which a study
/// whose empirical coverage and conditional error equal the anticipated
/// values would certify. Not a power calculation and not a guarantee.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub anticipated_coverage: f64,
    pub anticipated_conditional_error: f64,
    pub maximum_accepted_error_rate: f64,
    pub minimum_coverage: f64,
    pub delta: f64,
    pub candidates: usize,
    pub legacy_hoeffding_ratio_sequences: Option<u64>,
    pub hoeffding_bentkus_sequences: Option<u64>,
    pub note: String,
}

pub fn plan(coverage: f64, risk: f64, targets: Targets, candidates: usize) -> Result<Plan> {
    ensure!(
        (0.0..=1.0).contains(&coverage) && (0.0..=1.0).contains(&risk),
        "anticipated coverage and conditional error must be in [0, 1]"
    );
    ensure!(
        (0.0..=1.0).contains(&targets.maximum_accepted_error_rate)
            && targets.minimum_coverage > 0.0
            && targets.minimum_coverage <= 1.0
            && targets.delta > 0.0
            && targets.delta < 1.0
            && (1..=1024).contains(&candidates),
        "invalid targets: alpha in [0,1], minimum coverage in (0,1], delta in (0,1), 1..1024 candidates"
    );
    let alpha = targets.maximum_accepted_error_rate;
    let cmin = targets.minimum_coverage;
    let joint = coverage * risk;
    let log_k = (2.0 * candidates as f64 / targets.delta).ln();
    let ceil = |x: f64| (x.is_finite() && x > 0.0).then(|| x.ceil().max(1.0) as u64);
    let slack = (coverage - cmin).min((alpha * coverage - joint) / (1.0 + alpha));
    let legacy = if slack > 0.0 {
        ceil(log_k / (2.0 * slack * slack))
    } else {
        None
    };
    let hb = if coverage > cmin && (alpha >= 1.0 || joint < alpha * coverage) {
        let risk_n = if alpha >= 1.0 {
            Some(1)
        } else {
            ceil(log_k / kl_bernoulli(joint - alpha * coverage + alpha, alpha))
        };
        risk_n
            .zip(ceil(log_k / kl_bernoulli(coverage, cmin)))
            .map(|(a, b)| a.max(b))
    } else {
        None
    };
    Ok(Plan {
        anticipated_coverage: coverage,
        anticipated_conditional_error: risk,
        maximum_accepted_error_rate: alpha,
        minimum_coverage: cmin,
        delta: targets.delta,
        candidates,
        legacy_hoeffding_ratio_sequences: legacy,
        hoeffding_bentkus_sequences: hb,
        note: "Plug-in sample sizes: the number of independent sequences at which a study whose empirical coverage and conditional error equal the anticipated values would certify. Hoeffding-Bentkus uses only its Chernoff-KL term (the Bentkus term can only lower it). Sampling variability means roughly half of such studies would fall short; plan with margin. The betting method adapts to sequence-level variance and has no closed form. null: the anticipated operating point cannot be certified at any sample size.".into(),
    })
}
