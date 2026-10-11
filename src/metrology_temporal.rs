//! Hourly temporal regression benchmark with prospective day-level trust
//! decisions (Milestone 7, benchmarks/bike-sharing).
//!
//! The evaluator recomputes elapsed time from local wall-clock labels, lag
//! eligibility, partitions and every metric from the manifest. Selective
//! metrics reuse `metrology::evaluate`; TRIAD day decisions use the generic
//! evidence engine. No independence, stationarity or drift guarantee follows.
use crate::evidence::{self, Assessment, Claim, Class, Dependence, Evidence, Family};
use crate::metrology::{self, Input, Loss, Record, Split};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::Write,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HourRow {
    pub date: String,
    pub hour: u32,
    pub holiday: u8,
    pub workingday: u8,
    pub weekday: u8,
    pub cnt: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prediction {
    pub date: String,
    pub hour: u32,
    pub prediction: f64,
    pub error_scale: f64,
    pub error_probability: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TemporalModel {
    pub model_id: String,
    pub model_sha256: String,
    pub predictions: Vec<Prediction>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Partitions {
    pub training: [String; 2],
    pub development: [String; 2],
    pub calibration: [String; 2],
    pub test: [String; 2],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    pub absolute: f64,
    pub relative: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub benchmark_id: String,
    pub provenance: Value,
    pub protocol_sha256: String,
    pub time_zone_rule: String,
    pub partitions: Partitions,
    pub lags: Vec<u32>,
    pub tolerance: Tolerance,
    pub tolerance_sensitivity: Vec<Tolerance>,
    pub target_coverage: f64,
    pub reliability_bins: usize,
    pub conformal_alpha: f64,
    pub degradation_margin: f64,
    pub min_accepted_per_day: usize,
    pub alert_quantile: f64,
    pub min_condition_days: u64,
    pub completeness_required: u64,
    pub abstention_costs: Vec<f64>,
    pub practical_threshold: f64,
    pub block_lengths: Vec<usize>,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: u64,
    pub family_alpha: f64,
    pub equivalence_agreement: f64,
    pub hours: Vec<HourRow>,
    pub models: Vec<TemporalModel>,
}

// ---------------------------------------------------------------------------
// Civil calendar and America/New_York elapsed hours (US rule since 2007).

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
pub fn date_string(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}
/// Strict ISO date; returns days since 1970-01-01.
pub fn parse_date(s: &str) -> Result<i64> {
    let b = s.as_bytes();
    ensure!(
        b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()),
        "invalid date {s}"
    );
    let y: i64 = s[0..4].parse()?;
    let m: u32 = s[5..7].parse()?;
    let d: u32 = s[8..10].parse()?;
    ensure!(
        (1..=12).contains(&m) && (1..=31).contains(&d),
        "invalid date {s}"
    );
    let days = days_from_civil(y, m, d);
    ensure!(civil_from_days(days) == (y, m, d), "invalid date {s}");
    Ok(days)
}
/// 0 = Sunday, as in the UCI weekday field.
pub fn weekday(days: i64) -> u8 {
    (days + 4).rem_euclid(7) as u8
}
fn nth_sunday(y: i64, month: u32, n: i64) -> i64 {
    let first = days_from_civil(y, month, 1);
    first + (7 - i64::from(weekday(first))) % 7 + 7 * (n - 1)
}
/// Elapsed hours since 2011-01-01 00:00 EST for a New York wall-clock label.
/// Ambiguous fall-back 01:00 maps to the first (EDT) occurrence; the
/// non-existent spring-forward 02:00 is rejected.
pub fn elapsed_hour(date: &str, hour: u32) -> Result<i64> {
    ensure!(hour <= 23, "hour must be 0..23");
    let days = parse_date(date)?;
    let (y, _, _) = civil_from_days(days);
    ensure!(
        (2007..=2099).contains(&y),
        "US 2007 DST rule applies to 2007-2099 only"
    );
    let (start, end) = (nth_sunday(y, 3, 2), nth_sunday(y, 11, 1));
    let dst = if days == start {
        ensure!(hour != 2, "non-existent local time {date} 02:00");
        hour >= 3
    } else if days == end {
        hour <= 1
    } else {
        days > start && days < end
    };
    Ok((days - days_from_civil(2011, 1, 1)) * 24 + i64::from(hour) - i64::from(dst))
}

// ---------------------------------------------------------------------------
// Validation

fn hash_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn probability(x: f64) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Part {
    Training,
    Development,
    Calibration,
    Test,
}
const PARTS: [(Part, &str); 4] = [
    (Part::Training, "training"),
    (Part::Development, "development"),
    (Part::Calibration, "calibration"),
    (Part::Test, "test"),
];
fn ranges(m: &Manifest) -> Result<[(i64, i64); 4]> {
    let p = &m.partitions;
    let mut out = [(0, 0); 4];
    for (i, r) in [&p.training, &p.development, &p.calibration, &p.test]
        .into_iter()
        .enumerate()
    {
        out[i] = (parse_date(&r[0])?, parse_date(&r[1])?);
        ensure!(out[i].0 <= out[i].1, "partition start after end");
        if i > 0 {
            ensure!(
                out[i].0 == out[i - 1].1 + 1,
                "partitions must be contiguous and chronological"
            );
        }
    }
    Ok(out)
}

struct Hour {
    elapsed: i64,
    days: i64,
    cnt: f64,
}
struct Row {
    elapsed: i64,
    day: i64,
    part: Part,
    y: f64,
    yhat: f64,
    s: f64,
    p: f64,
    lags: Vec<f64>,
}
struct Prepared {
    hours: Vec<Hour>,
    index: HashMap<i64, usize>,
    eligible: Vec<(i64, i64, Part)>,
    holiday_days: BTreeSet<i64>,
    ranges: [(i64, i64); 4],
    summary: Value,
}

pub fn validate(m: &Manifest) -> Result<()> {
    prepare(m).map(|_| ())
}

fn prepare(m: &Manifest) -> Result<Prepared> {
    ensure!(
        m.schema_version == "triad-temporal-regression-1",
        "unsupported temporal benchmark schema"
    );
    ensure!(!m.benchmark_id.trim().is_empty(), "benchmark_id required");
    ensure!(hash_ok(&m.protocol_sha256), "invalid protocol hash");
    ensure!(m.provenance.is_object(), "provenance must be an object");
    for key in ["source_url", "doi", "sha256", "license", "citation"] {
        ensure!(
            m.provenance[key]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty()),
            "missing provenance {key}"
        );
    }
    ensure!(
        hash_ok(m.provenance["sha256"].as_str().unwrap()),
        "invalid dataset hash"
    );
    ensure!(
        m.time_zone_rule == "America/New_York (US rule since 2007)",
        "unsupported time_zone_rule"
    );
    let ranges = ranges(m)?;
    ensure!(
        !m.lags.is_empty() && m.lags[0] == 1 && m.lags.windows(2).all(|w| w[0] < w[1]),
        "lags must be strictly increasing and start at 1 (the origin)"
    );
    for t in std::iter::once(&m.tolerance).chain(&m.tolerance_sensitivity) {
        ensure!(
            t.absolute.is_finite()
                && t.relative.is_finite()
                && t.absolute >= 0.0
                && t.relative >= 0.0
                && t.absolute + t.relative > 0.0,
            "invalid tolerance"
        );
    }
    ensure!(
        m.target_coverage > 0.0 && m.target_coverage <= 1.0,
        "target_coverage in (0,1]"
    );
    ensure!(
        (1..=1000).contains(&m.reliability_bins),
        "reliability_bins 1..1000"
    );
    ensure!(
        m.conformal_alpha > 0.0 && m.conformal_alpha < 1.0,
        "conformal_alpha in (0,1)"
    );
    ensure!(
        probability(m.degradation_margin),
        "degradation_margin in [0,1]"
    );
    ensure!(m.min_accepted_per_day >= 1, "min_accepted_per_day >= 1");
    ensure!(
        m.alert_quantile > 0.0 && m.alert_quantile < 1.0,
        "alert_quantile in (0,1)"
    );
    ensure!(m.completeness_required >= 1, "completeness_required >= 1");
    ensure!(
        !m.abstention_costs.is_empty()
            && m.abstention_costs
                .iter()
                .all(|c| c.is_finite() && *c > 0.0 && *c <= 1.0),
        "abstention costs in (0,1]"
    );
    ensure!(
        m.practical_threshold.is_finite() && m.practical_threshold > 0.0,
        "practical_threshold > 0"
    );
    ensure!(
        !m.block_lengths.is_empty() && m.block_lengths.iter().all(|b| (1..=365).contains(b)),
        "block lengths 1..365"
    );
    ensure!(
        (100..=100_000).contains(&m.bootstrap_replicates),
        "bootstrap_replicates 100..100000"
    );
    ensure!(
        m.family_alpha > 0.0 && m.family_alpha < 1.0,
        "family_alpha in (0,1)"
    );
    ensure!(
        m.equivalence_agreement > 0.0 && m.equivalence_agreement <= 1.0,
        "equivalence_agreement in (0,1]"
    );
    ensure!(
        !m.models.is_empty() && m.models.len() <= 8,
        "between one and eight models"
    );
    ensure!(!m.hours.is_empty(), "hours must not be empty");
    let mut hours = Vec::with_capacity(m.hours.len());
    let mut index = HashMap::new();
    let mut holiday_by_day: BTreeMap<i64, u8> = BTreeMap::new();
    for h in &m.hours {
        let days = parse_date(&h.date)?;
        let elapsed = elapsed_hour(&h.date, h.hour)?;
        ensure!(
            h.holiday <= 1 && h.workingday <= 1,
            "{} {}: binary calendar flags required",
            h.date,
            h.hour
        );
        ensure!(
            h.weekday == weekday(days),
            "{}: weekday disagrees with the calendar",
            h.date
        );
        ensure!(
            h.workingday == u8::from((1..=5).contains(&h.weekday) && h.holiday == 0),
            "{}: workingday disagrees with weekday and holiday",
            h.date
        );
        ensure!(
            *holiday_by_day.entry(days).or_insert(h.holiday) == h.holiday,
            "{}: holiday flag varies within a day",
            h.date
        );
        if let Some(last) = hours.last() {
            let last: &Hour = last;
            ensure!(
                elapsed > last.elapsed,
                "{} {:02}: hours must be strictly increasing in elapsed time (no duplicates)",
                h.date,
                h.hour
            );
        }
        index.insert(elapsed, hours.len());
        hours.push(Hour {
            elapsed,
            days,
            cnt: f64::from(h.cnt),
        });
    }
    let first_day = ranges[0].0;
    let last_day = ranges[3].1;
    ensure!(
        hours[0].days <= first_day && hours.last().unwrap().days >= last_day,
        "hours must cover every partition"
    );
    let mut eligible = Vec::new();
    let mut summary = serde_json::Map::new();
    for (i, (part, name)) in PARTS.iter().enumerate() {
        let (a, b) = ranges[i];
        let span = elapsed_hour(&date_string(b + 1), 0)? - elapsed_hour(&date_string(a), 0)?;
        let mut present = 0;
        let mut lag_missing = 0;
        let mut ok = 0;
        for h in hours.iter().filter(|h| h.days >= a && h.days <= b) {
            present += 1;
            if m.lags
                .iter()
                .all(|l| index.contains_key(&(h.elapsed - i64::from(*l))))
            {
                ok += 1;
                eligible.push((h.elapsed, h.days, *part));
            } else {
                lag_missing += 1;
            }
        }
        summary.insert(
            name.to_string(),
            json!({"first_date":date_string(a),"last_date":date_string(b),"days":b-a+1,"elapsed_hours":span,
            "present_target_hours":present,"missing_target_hours":span-present,
            "present_but_lag_missing":lag_missing,"eligible":ok}),
        );
    }
    let mut ids = BTreeSet::new();
    let expected: Vec<_> = eligible
        .iter()
        .filter(|e| e.2 != Part::Training)
        .map(|e| e.0)
        .collect();
    for model in &m.models {
        ensure!(
            !model.model_id.trim().is_empty() && ids.insert(&model.model_id),
            "empty or duplicate model id"
        );
        ensure!(hash_ok(&model.model_sha256), "invalid model hash");
        ensure!(
            model.predictions.len() == expected.len(),
            "{}: predictions must cover exactly the eligible development, calibration and test targets ({} expected, {} supplied)",
            model.model_id,
            expected.len(),
            model.predictions.len()
        );
        for (p, e) in model.predictions.iter().zip(&expected) {
            ensure!(
                elapsed_hour(&p.date, p.hour)? == *e,
                "{}: prediction {} {:02} is not the next eligible target",
                model.model_id,
                p.date,
                p.hour
            );
            ensure!(
                p.prediction.is_finite() && p.prediction >= 0.0,
                "{}: invalid prediction",
                model.model_id
            );
            ensure!(
                p.error_scale.is_finite() && p.error_scale >= 1.0,
                "{}: error_scale must be >= 1",
                model.model_id
            );
            ensure!(
                probability(p.error_probability),
                "{}: error_probability in [0,1]",
                model.model_id
            );
        }
    }
    let total_span = hours.last().unwrap().elapsed - hours[0].elapsed + 1;
    let summary = json!({"rows":hours.len(),"elapsed_span_hours":total_span,
        "missing_elapsed_hours":total_span-hours.len() as i64,"lags_hours":m.lags,"partitions":summary,
        "eligibility_rule":"target T eligible iff T and every T-lag elapsed hour are present; nothing imputed"});
    Ok(Prepared {
        hours,
        index,
        eligible,
        holiday_days: holiday_by_day
            .into_iter()
            .filter(|(_, h)| *h == 1)
            .map(|(d, _)| d)
            .collect(),
        ranges,
        summary,
    })
}

// ---------------------------------------------------------------------------
// Metrics helpers

fn material(yhat: f64, y: f64, t: Tolerance) -> bool {
    (yhat - y).abs() > t.absolute.max(t.relative * y)
}
fn month(day: i64) -> String {
    date_string(day)[..7].to_string()
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DayType {
    Holiday,
    Weekend,
    Working,
}
impl DayType {
    fn name(self) -> &'static str {
        match self {
            DayType::Holiday => "holiday",
            DayType::Weekend => "weekend",
            DayType::Working => "working day",
        }
    }
}
fn day_type(day: i64, holidays: &BTreeSet<i64>) -> DayType {
    if holidays.contains(&day) {
        DayType::Holiday
    } else if matches!(weekday(day), 0 | 6) {
        DayType::Weekend
    } else {
        DayType::Working
    }
}
fn errors(rows: &[&Row], t: Tolerance) -> Value {
    if rows.is_empty() {
        return json!({"n":0});
    }
    let n = rows.len() as f64;
    let abs: f64 = rows.iter().map(|r| (r.yhat - r.y).abs()).sum();
    let sq: f64 = rows.iter().map(|r| (r.yhat - r.y).powi(2)).sum();
    let bias: f64 = rows.iter().map(|r| r.yhat - r.y).sum();
    let events = rows.iter().filter(|r| material(r.yhat, r.y, t)).count();
    json!({"n":rows.len(),"mae":abs/n,"rmse":(sq/n).sqrt(),"bias":bias/n,"material_errors":events,"material_error_rate":events as f64/n})
}
fn reference(rows: &[&Row], lag_pos: usize, t: Tolerance) -> Value {
    let shifted: Vec<Row> = rows
        .iter()
        .map(|r| Row {
            elapsed: r.elapsed,
            day: r.day,
            part: r.part,
            y: r.y,
            yhat: r.lags[lag_pos],
            s: r.s,
            p: r.p,
            lags: Vec::new(),
        })
        .collect();
    errors(&shifted.iter().collect::<Vec<_>>(), t)
}
/// Two-sample Kolmogorov-Smirnov statistic sup|F_a - F_b| (inputs sorted).
pub fn ks_statistic(a: &[f64], b: &[f64]) -> f64 {
    let (mut i, mut j, mut d) = (0usize, 0usize, 0.0_f64);
    while i < a.len() && j < b.len() {
        let v = a[i].min(b[j]);
        while i < a.len() && a[i] <= v {
            i += 1;
        }
        while j < b.len() && b[j] <= v {
            j += 1;
        }
        d = d.max((i as f64 / a.len() as f64 - j as f64 / b.len() as f64).abs());
    }
    d
}
fn quantile(values: &[f64], p: f64) -> Option<f64> {
    (!values.is_empty()).then(|| crate::evaluation::quantile(values, p))
}

/// SplitMix64 with rejection, identical to the Milestone 6 draws (that function
/// is private to an archived, hash-pinned source file and cannot be reused).
fn draw(state: &mut u64, n: usize) -> usize {
    let bound = (n as u64).wrapping_neg() % n as u64;
    loop {
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        if z >= bound {
            return (z % n as u64) as usize;
        }
    }
}
/// Circular moving-block resamples of day indices (common to all comparisons).
fn block_indices(days: usize, block: usize, reps: usize, seed: u64) -> Option<Vec<Vec<usize>>> {
    if block == 0 || days / block < 2 {
        return None;
    }
    let mut state = seed;
    Some(
        (0..reps)
            .map(|_| {
                let mut idx = Vec::with_capacity(days);
                while idx.len() < days {
                    let start = draw(&mut state, days);
                    for k in 0..block.min(days - idx.len()) {
                        idx.push((start + k) % days);
                    }
                }
                idx
            })
            .collect(),
    )
}
/// Percentile interval for sum(diff)/sum(n) over resampled days.
fn ratio_interval(diff: &[f64], n: &[f64], idx: &Option<Vec<Vec<usize>>>, level: f64) -> Value {
    let Some(idx) = idx else {
        return Value::Null;
    };
    let mut stats = Vec::with_capacity(idx.len());
    for rep in idx {
        let (d, w) = rep
            .iter()
            .fold((0.0, 0.0), |(d, w), i| (d + diff[*i], w + n[*i]));
        if w <= 0.0 {
            return Value::Null;
        }
        stats.push(d / w);
    }
    let a = (1.0 - level) / 2.0;
    json!([
        crate::evaluation::quantile(&stats, a),
        crate::evaluation::quantile(&stats, 1.0 - a)
    ])
}

// ---------------------------------------------------------------------------
// Day-level monitoring

#[derive(Clone)]
struct Day {
    day: i64,
    kind: DayType,
    rows: Vec<usize>,
    accepted: usize,
    accepted_errors: usize,
    ks: Option<f64>,
    mean_p: Option<f64>,
}
struct Scores {
    cov: Option<f64>,
    conf: Option<f64>,
    perf: f64,
    available: u64,
    recent: Vec<bool>,
}

const POLICY_SCORES: [&str; 4] = ["cov", "conf", "perf", "std"];
const ABLATIONS: [(&str, Family); 4] = [
    (
        "triad_without_r1_calibration_applicability",
        Family::CalibrationApplicability,
    ),
    (
        "triad_without_r2_representativeness",
        Family::Representativeness,
    ),
    ("triad_without_r3_completeness", Family::Completeness),
    ("triad_without_r4_recent_outcomes", Family::Statistical),
];

#[allow(clippy::too_many_arguments)]
fn triad_decision(
    date: &str,
    kind: DayType,
    s: &Scores,
    counts: &BTreeMap<String, u64>,
    calibration_days: u64,
    support_max: f64,
    deff: f64,
    bound: f64,
    m: &Manifest,
    disabled: &BTreeSet<Family>,
) -> Result<(Class, Vec<String>)> {
    let mut ev = vec![
        Evidence::Calibration {
            id: "calibration".into(),
            population: "calibration days".into(),
            conditions: counts.keys().cloned().collect(),
            condition_counts: counts.clone(),
            n: calibration_days,
            unit: "day".into(),
            dependence: Dependence::Temporal,
            method: "calibration-quantile selective threshold".into(),
        },
        Evidence::Completeness {
            id: "completeness".into(),
            required: m.completeness_required,
            available: s.available,
        },
        Evidence::OutcomeRecords {
            id: "recent".into(),
            metric: "accepted_material_error".into(),
            population: "previous day accepted predictions".into(),
            conditions: vec!["previous day".into()],
            unit: "hour".into(),
            dependence: Dependence::Temporal,
            outcomes: s.recent.clone(),
            design_effect: Some(deff),
        },
    ];
    if let Some(v) = s.cov {
        ev.push(Evidence::SupportCheck {
            id: "representativeness".into(),
            statistic: "max KS of history features, previous day versus development".into(),
            value: v,
            support_max,
        });
    }
    let ids = ev.iter().map(|e| e.id().to_string()).collect();
    let a = Assessment {
        schema_version: "triad-evidence-1".into(),
        assessment_id: format!("day-{date}"),
        evidence: ev,
        claims: vec![Claim::OperationalRisk {
            id: date.into(),
            statement: format!("Accepted material-error risk on {date} is at most {bound}"),
            evidence: ids,
            population: format!("accepted predictions on {date}"),
            recent_population: "previous day accepted predictions".into(),
            condition: kind.name().into(),
            bound,
            confidence: 0.95,
            min_condition_support: m.min_condition_days,
        }],
    };
    let f = evidence::assess(&a, None, disabled)?.remove(0);
    let rules = f
        .reasons
        .iter()
        .filter(|r| {
            matches!(
                r.outcome,
                evidence::Outcome::Insufficient | evidence::Outcome::Contradict
            )
        })
        .map(|r| r.rule.clone())
        .collect();
    Ok((f.classification, rules))
}

fn std_score(s: &Scores, scale: (f64, f64)) -> f64 {
    (s.cov.unwrap_or(0.0) / scale.0).max(s.perf / scale.1)
}
fn score_of(name: &str, s: &Scores, scale: (f64, f64)) -> Option<f64> {
    match name {
        "cov" => s.cov,
        "conf" => s.conf,
        "perf" => Some(s.perf),
        _ => Some(std_score(s, scale)),
    }
}
/// Smallest calibration score v whose alerts (score > v, plus extra warnings) number at most k.
fn matched_threshold(scores: &[Option<f64>], extra: &[bool], k: usize) -> (f64, usize) {
    let mut sorted: Vec<f64> = scores.iter().flatten().copied().collect();
    sorted.sort_by(f64::total_cmp);
    let alerts = |v: f64| {
        scores
            .iter()
            .zip(extra)
            .filter(|(s, e)| **e || s.is_some_and(|x| x > v))
            .count()
    };
    for v in &sorted {
        if alerts(*v) <= k {
            return (*v, alerts(*v));
        }
    }
    let v = sorted.last().copied().unwrap_or(f64::INFINITY);
    (v, alerts(v))
}

#[allow(clippy::too_many_arguments)]
fn monitor(
    m: &Manifest,
    prep: &Prepared,
    rows: &[Row],
    accepted: &[bool],
    t: Tolerance,
    detail: bool,
) -> Result<Value> {
    let event: Vec<bool> = rows.iter().map(|r| material(r.yhat, r.y, t)).collect();
    let (cal_acc, cal_err) = rows
        .iter()
        .zip(accepted)
        .zip(&event)
        .filter(|((r, a), _)| r.part == Part::Calibration && **a)
        .fold((0usize, 0usize), |(n, k), (_, e)| {
            (n + 1, k + usize::from(*e))
        });
    ensure!(cal_acc > 0, "calibration accepted no predictions");
    let r_cal = cal_err as f64 / cal_acc as f64;
    let bound = (r_cal + m.degradation_margin).min(1.0);
    // Lag-1 autocorrelation of the hourly event indicator over consecutive eligible calibration hours.
    let pos: HashMap<i64, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.elapsed, i))
        .collect();
    let pairs: Vec<(f64, f64)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.part == Part::Calibration)
        .filter_map(|(i, r)| {
            pos.get(&(r.elapsed - 1))
                .filter(|j| rows[**j].part == Part::Calibration)
                .map(|j| {
                    (
                        f64::from(u8::from(event[i])),
                        f64::from(u8::from(event[*j])),
                    )
                })
        })
        .collect();
    let (ma, mb) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / pairs.len() as f64,
        pairs.iter().map(|p| p.1).sum::<f64>() / pairs.len() as f64,
    );
    let num: f64 = pairs.iter().map(|(a, b)| (a - ma) * (b - mb)).sum();
    let den = (pairs.iter().map(|(a, _)| (a - ma).powi(2)).sum::<f64>()
        * pairs.iter().map(|(_, b)| (b - mb).powi(2)).sum::<f64>())
    .sqrt();
    let rho = if !pairs.is_empty() && den > 0.0 {
        num / den
    } else {
        0.0
    };
    let rho_used = rho.clamp(0.0, 0.95);
    let deff = (1.0 + rho_used) / (1.0 - rho_used);
    // Development reference for KS (counts; KS is invariant to log1p).
    let mut reference: Vec<Vec<f64>> = vec![Vec::new(); m.lags.len()];
    for r in rows.iter().filter(|r| r.part == Part::Development) {
        for (k, v) in r.lags.iter().enumerate() {
            reference[k].push(*v);
        }
    }
    for v in &mut reference {
        v.sort_by(f64::total_cmp);
    }
    // Day records from development start to test end.
    let (dev0, test1) = (prep.ranges[1].0, prep.ranges[3].1);
    let mut days: Vec<Day> = (dev0..=test1)
        .map(|d| Day {
            day: d,
            kind: day_type(d, &prep.holiday_days),
            rows: Vec::new(),
            accepted: 0,
            accepted_errors: 0,
            ks: None,
            mean_p: None,
        })
        .collect();
    for (i, r) in rows.iter().enumerate() {
        let d = &mut days[(r.day - dev0) as usize];
        d.rows.push(i);
        d.accepted += usize::from(accepted[i]);
        d.accepted_errors += usize::from(accepted[i] && event[i]);
    }
    for d in &mut days {
        if d.rows.is_empty() {
            continue;
        }
        d.mean_p = Some(d.rows.iter().map(|i| rows[*i].p).sum::<f64>() / d.rows.len() as f64);
        let mut ks: f64 = 0.0;
        for (k, refv) in reference.iter().enumerate() {
            let mut v: Vec<f64> = d.rows.iter().map(|i| rows[*i].lags[k]).collect();
            v.sort_by(f64::total_cmp);
            ks = ks.max(ks_statistic(&v, refv));
        }
        d.ks = Some(ks);
    }
    // Scores for day index i use day i-1 only.
    let scores = |i: usize| -> Scores {
        let prev = &days[i - 1];
        Scores {
            cov: prev.ks,
            conf: prev.mean_p,
            perf: if prev.accepted > 0 {
                prev.accepted_errors as f64 / prev.accepted as f64
            } else {
                0.0
            },
            available: prev.rows.len() as u64,
            recent: prev
                .rows
                .iter()
                .filter(|j| accepted[**j])
                .map(|j| event[*j])
                .collect(),
        }
    };
    let idx_in = |part: Part| -> Vec<usize> {
        let (a, b) = prep.ranges[part as usize];
        ((a - dev0) as usize..=(b - dev0) as usize)
            .filter(|i| *i >= 1)
            .collect()
    };
    let dev_idx = idx_in(Part::Development);
    let cal_idx = idx_in(Part::Calibration);
    let test_idx = idx_in(Part::Test);
    let dev_scores: Vec<Scores> = dev_idx.iter().map(|i| scores(*i)).collect();
    let floor = |x: Option<f64>| x.unwrap_or(0.0).max(1e-12);
    let scale = (
        floor(quantile(
            &dev_scores.iter().filter_map(|s| s.cov).collect::<Vec<_>>(),
            0.8,
        )),
        floor(quantile(
            &dev_scores.iter().map(|s| s.perf).collect::<Vec<_>>(),
            0.8,
        )),
    );
    let cal_scores: Vec<Scores> = cal_idx.iter().map(|i| scores(*i)).collect();
    let test_scores: Vec<Scores> = test_idx.iter().map(|i| scores(*i)).collect();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for i in &cal_idx {
        *counts.entry(days[*i].kind.name().into()).or_default() += 1;
    }
    let support_max = cal_scores
        .iter()
        .filter_map(|s| s.cov)
        .fold(f64::NEG_INFINITY, f64::max);
    let decide = |i: usize, s: &Scores, disabled: &BTreeSet<Family>| {
        triad_decision(
            &date_string(days[i].day),
            days[i].kind,
            s,
            &counts,
            cal_idx.len() as u64,
            support_max,
            deff,
            bound,
            m,
            disabled,
        )
    };
    let none = BTreeSet::new();
    let mut triad_cal = Vec::new();
    for (i, s) in cal_idx.iter().zip(&cal_scores) {
        triad_cal.push(decide(*i, s, &none)?.0 != Class::SupportedWithinScope);
    }
    let k_cal = triad_cal.iter().filter(|a| **a).count();
    let incomplete = |ss: &[Scores]| -> Vec<bool> {
        ss.iter()
            .map(|s| s.available < m.completeness_required)
            .collect()
    };
    let mut policies: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    let mut calibration_alerts: BTreeMap<String, Value> = BTreeMap::new();
    let mut thresholds = serde_json::Map::new();
    policies.insert("none".into(), vec![false; test_idx.len()]);
    calibration_alerts.insert("none".into(), json!(0));
    let mut triad_test = Vec::new();
    let mut triad_rules = Vec::new();
    for (i, s) in test_idx.iter().zip(&test_scores) {
        let (c, rules) = decide(*i, s, &none)?;
        triad_test.push(c != Class::SupportedWithinScope);
        triad_rules.push((c, rules));
    }
    let k_test = triad_test.iter().filter(|a| **a).count();
    policies.insert("triad".into(), triad_test.clone());
    calibration_alerts.insert("triad".into(), json!(k_cal));
    let mut k_outcome_free = 0;
    for (name, fam) in ABLATIONS {
        let disabled = BTreeSet::from([fam]);
        let mut test_alerts = Vec::new();
        for (i, s) in test_idx.iter().zip(&test_scores) {
            test_alerts.push(decide(*i, s, &disabled)?.0 != Class::SupportedWithinScope);
        }
        let mut cal_alerts = 0;
        for (i, s) in cal_idx.iter().zip(&cal_scores) {
            cal_alerts += usize::from(decide(*i, s, &disabled)?.0 != Class::SupportedWithinScope);
        }
        policies.insert(name.into(), test_alerts);
        calibration_alerts.insert(name.into(), json!(cal_alerts));
        if fam == Family::Statistical {
            k_outcome_free = cal_alerts;
        }
    }
    for name in POLICY_SCORES {
        let cal: Vec<Option<f64>> = cal_scores
            .iter()
            .map(|s| score_of(name, s, scale))
            .collect();
        let test: Vec<Option<f64>> = test_scores
            .iter()
            .map(|s| score_of(name, s, scale))
            .collect();
        let is_std = name == "std";
        let cal_extra = if is_std {
            incomplete(&cal_scores)
        } else {
            vec![false; cal.len()]
        };
        let test_extra = if is_std {
            incomplete(&test_scores)
        } else {
            vec![false; test.len()]
        };
        let defined: Vec<f64> = cal.iter().flatten().copied().collect();
        let native = quantile(&defined, m.alert_quantile).context("no calibration scores")?;
        let apply = |v: f64, sc: &[Option<f64>], extra: &[bool]| -> Vec<bool> {
            sc.iter()
                .zip(extra)
                .map(|(s, e)| *e || s.is_some_and(|x| x > v))
                .collect()
        };
        let (matched, matched_cal) = matched_threshold(&cal, &cal_extra, k_cal);
        let mut order: Vec<usize> = (0..test.len()).collect();
        order.sort_by(|a, b| {
            test[*b]
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&test[*a].unwrap_or(f64::NEG_INFINITY))
                .then(a.cmp(b))
        });
        let mut topk = vec![false; test.len()];
        for i in order.into_iter().take(k_test) {
            topk[i] = true;
        }
        policies.insert(format!("{name}_native"), apply(native, &test, &test_extra));
        policies.insert(
            format!("{name}_matched_calibration"),
            apply(matched, &test, &test_extra),
        );
        policies.insert(format!("{name}_matched_test_topk"), topk);
        if matches!(name, "cov" | "conf") {
            // Protocol amendment 1: outcome-free comparators matched to TRIAD without R4.
            let (v, c) = matched_threshold(&cal, &cal_extra, k_outcome_free);
            policies.insert(
                format!("{name}_matched_outcome_free"),
                apply(v, &test, &test_extra),
            );
            calibration_alerts.insert(format!("{name}_matched_outcome_free"), json!(c));
        }
        calibration_alerts.insert(
            format!("{name}_native"),
            json!(apply(native, &cal, &cal_extra)
                .iter()
                .filter(|a| **a)
                .count()),
        );
        calibration_alerts.insert(format!("{name}_matched_calibration"), json!(matched_cal));
        calibration_alerts.insert(format!("{name}_matched_test_topk"), Value::Null);
        thresholds.insert(name.into(), json!({"native":native,"matched_calibration":matched,"matched_calibration_alerts":matched_cal,"budget_exceeded":matched_cal>k_cal}));
    }
    // Retrospective day outcomes (labels used only here).
    let degraded: Vec<Option<bool>> = test_idx
        .iter()
        .map(|i| {
            let d = &days[*i];
            (d.accepted >= m.min_accepted_per_day)
                .then(|| d.accepted_errors as f64 / d.accepted as f64 > bound)
        })
        .collect();
    let test_rows: Vec<usize> = test_idx
        .iter()
        .flat_map(|i| days[*i].rows.clone())
        .collect();
    let n_test = test_rows.len() as f64;
    let day_n: Vec<f64> = test_idx
        .iter()
        .map(|i| days[*i].rows.len() as f64)
        .collect();
    let cost_by_day = |alerts: &[bool], c: f64| -> Vec<f64> {
        test_idx
            .iter()
            .zip(alerts)
            .map(|(i, a)| {
                days[*i]
                    .rows
                    .iter()
                    .map(|j| {
                        if accepted[*j] && !a {
                            f64::from(u8::from(event[*j]))
                        } else {
                            c
                        }
                    })
                    .sum()
            })
            .collect()
    };
    let block_sets: Vec<(usize, Option<Vec<Vec<usize>>>)> = m
        .block_lengths
        .iter()
        .map(|b| {
            (
                *b,
                block_indices(test_idx.len(), *b, m.bootstrap_replicates, m.bootstrap_seed),
            )
        })
        .collect();
    let mut policy_reports = serde_json::Map::new();
    for (name, alerts) in &policies {
        let (mut acc, mut acc_err) = (0usize, 0usize);
        for (i, a) in test_idx.iter().zip(alerts) {
            if !a {
                acc += days[*i].accepted;
                acc_err += days[*i].accepted_errors;
            }
        }
        let mut conf = [0usize; 6]; // TP FP FN TN unknown_alerted unknown_not_alerted
        for (d, a) in degraded.iter().zip(alerts) {
            conf[match (d, a) {
                (Some(true), true) => 0,
                (Some(false), true) => 1,
                (Some(true), false) => 2,
                (Some(false), false) => 3,
                (None, true) => 4,
                (None, false) => 5,
            }] += 1;
        }
        let ratio = |a: usize, b: usize| (b > 0).then(|| a as f64 / b as f64);
        // Episodes: maximal runs of consecutive degraded days; null delay = missed.
        let mut delays = Vec::new();
        let mut i = 0;
        while i < degraded.len() {
            if degraded[i] == Some(true) {
                let mut j = i;
                while j < degraded.len() && degraded[j] == Some(true) {
                    j += 1;
                }
                delays.push((i..j).find(|k| alerts[*k]).map(|k| k - i));
                i = j;
            } else {
                i += 1;
            }
        }
        // Per-hour threshold tightening to the same test coverage (retrospective comparator).
        let k = acc;
        let mut ranked = test_rows.clone();
        ranked.sort_by(|a, b| {
            rows[*a]
                .p
                .total_cmp(&rows[*b].p)
                .then(rows[*a].elapsed.cmp(&rows[*b].elapsed))
        });
        let matched_err = ranked.iter().take(k).filter(|j| event[**j]).count();
        let costs: Vec<Value> = m
            .abstention_costs
            .iter()
            .map(|c| json!({"abstention_cost":c,"mean_cost":cost_by_day(alerts,*c).iter().sum::<f64>()/n_test}))
            .collect();
        policy_reports.insert(name.clone(), json!({
            "test_alert_days":alerts.iter().filter(|a| **a).count(),"test_days":alerts.len(),
            "calibration_alert_days":calibration_alerts[name],
            "gated":{"accepted":acc,"accepted_errors":acc_err,"coverage":acc as f64/n_test,"selective_risk":ratio(acc_err,acc)},
            "per_hour_threshold_at_same_coverage":{"accepted":k,"accepted_errors":matched_err,"selective_risk":ratio(matched_err,k)},
            "degraded_day_confusion":{"true_positive":conf[0],"false_positive":conf[1],"false_negative":conf[2],"true_negative":conf[3],"unknown_alerted":conf[4],"unknown_not_alerted":conf[5]},
            "recall":ratio(conf[0],conf[0]+conf[2]),"false_alarm_rate":ratio(conf[1],conf[1]+conf[3]),
            "episode_delay_days":delays,
            "costs":costs,
        }));
    }
    let comparisons = |a: &str,
                       b: &str,
                       c: f64,
                       levels: &[f64],
                       blocks: &[(usize, Option<Vec<Vec<usize>>>)]|
     -> Value {
        let ca = cost_by_day(&policies[a], c);
        let cb = cost_by_day(&policies[b], c);
        let diff: Vec<f64> = ca.iter().zip(&cb).map(|(x, y)| x - y).collect();
        let delta = diff.iter().sum::<f64>() / n_test;
        let intervals: Vec<Value> = blocks
            .iter()
            .map(|(bl, idx)| {
                json!({"block_days":bl,"intervals":levels.iter().map(|l| json!({"level":l,"interval":ratio_interval(&diff,&day_n,idx,*l)})).collect::<Vec<_>>()})
            })
            .collect();
        let same = policies[a]
            .iter()
            .zip(&policies[b])
            .filter(|(x, y)| x == y)
            .count();
        json!({"a":a,"b":b,"abstention_cost":c,"delta_mean_cost_a_minus_b":delta,"identical_decision_days":same,"bootstrap":intervals})
    };
    let adjusted = 1.0 - m.family_alpha / m.models.len() as f64;
    let primary_cost = m.abstention_costs[0];
    let primary = comparisons(
        "triad",
        "std_matched_calibration",
        primary_cost,
        &[adjusted, 0.95],
        &block_sets,
    );
    let primary_block = &block_sets[..1];
    let mut secondary = Vec::new();
    for name in policies.keys().filter(|n| *n != "triad") {
        for c in &m.abstention_costs {
            secondary.push(comparisons("triad", name, *c, &[0.95], primary_block));
        }
    }
    let outcome_free: Vec<Value> = [
        "cov_matched_outcome_free",
        "conf_matched_outcome_free",
        "none",
    ]
    .iter()
    .flat_map(|b| {
        m.abstention_costs
            .iter()
            .map(|c| comparisons(ABLATIONS[3].0, b, *c, &[0.95], primary_block))
    })
    .collect();
    let interval = primary["bootstrap"][0]["intervals"][0]["interval"].clone();
    let delta = primary["delta_mean_cost_a_minus_b"].as_f64().unwrap();
    let identical = primary["identical_decision_days"].as_u64().unwrap() as usize == test_idx.len();
    let lo = interval[0].as_f64();
    let hi = interval[1].as_f64();
    let decision = if identical {
        "identical_decisions"
    } else if delta <= -m.practical_threshold && hi.is_some_and(|h| h < 0.0) {
        "advantage_established"
    } else if delta >= m.practical_threshold && lo.is_some_and(|l| l > 0.0) {
        "disadvantage_established"
    } else {
        "evidence_insufficient_to_establish_a_practical_advantage"
    };
    let agreement = |a: &str, b: &str| -> Value {
        let (x, y) = (&policies[a], &policies[b]);
        let n = x.len() as f64;
        let po = x.iter().zip(y).filter(|(p, q)| p == q).count() as f64 / n;
        let (pa, pb) = (
            x.iter().filter(|v| **v).count() as f64 / n,
            y.iter().filter(|v| **v).count() as f64 / n,
        );
        let pe = pa * pb + (1.0 - pa) * (1.0 - pb);
        json!({"a":a,"b":b,"agreement":po,"cohen_kappa":(pe<1.0).then(|| (po-pe)/(1.0-pe)),
            "operationally_equivalent":po>=m.equivalence_agreement})
    };
    let outcome_free_agreement: Vec<Value> =
        ["cov_matched_outcome_free", "conf_matched_outcome_free"]
            .iter()
            .map(|b| agreement(ABLATIONS[3].0, b))
            .collect();
    let agreements: Vec<Value> = policies
        .keys()
        .filter(|n| *n != "triad")
        .map(|n| agreement("triad", n))
        .collect();
    let known = degraded.iter().flatten().count();
    let n_degraded = degraded.iter().filter(|d| **d == Some(true)).count();
    let mut out = json!({
        "tolerance":t,"calibration_accepted":cal_acc,"calibration_accepted_errors":cal_err,"r_cal":r_cal,
        "claim_bound":bound,"lag1_event_autocorrelation_calibration":rho,"rho_used":rho_used,"design_effect":deff,
        "calibration_pairs":pairs.len(),"score_scales_development_p80":{"cov":scale.0,"perf":scale.1},
        "thresholds":thresholds,"triad_calibration_alert_days_k":k_cal,"triad_test_alert_days":k_test,
        "calibration_condition_counts":counts,"cov_support_max_calibration":support_max,
        "degraded_days":{"test_days":test_idx.len(),"known":known,"degraded":n_degraded,"unknown":test_idx.len()-known,
            "descriptive_only":n_degraded<10},
        "policies":policy_reports,
        "primary":{"comparison":primary,"interval_level":adjusted,"decision":decision},
        "secondary_comparisons":secondary,
        "amendment_1_outcome_free":{"k_calibration":k_outcome_free,"comparisons":outcome_free,
            "agreement":outcome_free_agreement},
        "agreement":agreements,
    });
    if detail {
        let day_rows: Vec<Value> = test_idx
            .iter()
            .enumerate()
            .map(|(k, i)| {
                let d = &days[*i];
                let s = &test_scores[k];
                json!({"date":date_string(d.day),"day_type":d.kind.name(),"eligible":d.rows.len(),"accepted":d.accepted,
                    "accepted_errors":d.accepted_errors,"degraded":degraded[k],
                    "scores":{"cov":s.cov,"conf":s.conf,"perf":s.perf,"std":std_score(s,scale),"previous_day_eligible":s.available},
                    "triad_classification":triad_rules[k].0,"triad_rules_fired":triad_rules[k].1,
                    "alerts":policies.iter().map(|(n,v)| (n.clone(),json!(v[k]))).collect::<serde_json::Map<_,_>>()})
            })
            .collect();
        out["test_days"] = json!(day_rows);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Evaluation

fn generic_input(m: &Manifest, model: &str, rows: &[Row], t: Tolerance) -> Input {
    let records = rows
        .iter()
        .filter(|r| matches!(r.part, Part::Calibration | Part::Test))
        .map(|r| {
            let cal = r.part == Part::Calibration;
            Record {
                id: format!("{:06}", r.elapsed),
                prediction: vec![(r.yhat - r.y) / t.absolute.max(t.relative * r.y)],
                truth: Some(vec![0.0]),
                uncertainty: r.p,
                error_probability: Some(r.p),
                sequence: if cal {
                    "calibration-dependent-stream"
                } else {
                    "test-dependent-stream"
                }
                .into(),
                spatial_cluster: None,
                split: if cal { Split::Calibration } else { Split::Test },
                scenario: if cal {
                    "calibration".into()
                } else {
                    format!("test-{}", month(r.day))
                },
            }
        })
        .collect();
    Input {
        schema_version: "triad-metrology-1".into(),
        model_id: model.into(),
        experiment_id: m.benchmark_id.clone(),
        loss: Loss::Euclidean,
        tolerance: 1.0,
        target_coverage: m.target_coverage,
        calibration_scenario: "calibration".into(),
        bootstrap_replicates: 100,
        bootstrap_seed: m.bootstrap_seed,
        reliability_bins: m.reliability_bins,
        evidence: metrology::Evidence {
            numerical_required: false,
            checks: BTreeMap::new(),
            evidence_resources: vec!["benchmarks/bike-sharing/protocol.json".into()],
            evaluated_scenarios: Vec::new(),
            prospective_power_supported: false,
            independent_replication: false,
        },
        records,
    }
}

pub fn evaluate(m: &Manifest) -> Result<Value> {
    let prep = prepare(m)?;
    let cnt = |e: i64| prep.hours[prep.index[&e]].cnt;
    let mut model_reports = Vec::new();
    let mut confirmatory = Vec::new();
    for model in &m.models {
        let rows: Vec<Row> = prep
            .eligible
            .iter()
            .filter(|e| e.2 != Part::Training)
            .zip(&model.predictions)
            .map(|((elapsed, day, part), p)| Row {
                elapsed: *elapsed,
                day: *day,
                part: *part,
                y: cnt(*elapsed),
                yhat: p.prediction,
                s: p.error_scale,
                p: p.error_probability,
                lags: m
                    .lags
                    .iter()
                    .map(|l| cnt(elapsed - i64::from(*l)))
                    .collect(),
            })
            .collect();
        let t = m.tolerance;
        let input = generic_input(m, &model.model_id, &rows, t);
        let tau = metrology::threshold(&input);
        let accepted: Vec<bool> = rows.iter().map(|r| tau.is_some_and(|x| r.p <= x)).collect();
        let mut generic = metrology::evaluate(&input)?;
        generic.as_object_mut().unwrap().remove("input");
        // Internal consistency: the generic evaluator's events equal the material-error events.
        let test_rows: Vec<&Row> = rows.iter().filter(|r| r.part == Part::Test).collect();
        let own = test_rows
            .iter()
            .filter(|r| material(r.yhat, r.y, t))
            .count();
        ensure!(
            generic["overall"]["operational"]["errors"] == json!(own),
            "generic event encoding disagrees with the material-error rule"
        );
        let by = |part: Part| rows.iter().filter(|r| r.part == part).collect::<Vec<_>>();
        let mut months: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
        let mut kinds: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
        for r in &test_rows {
            months.entry(month(r.day)).or_default().push(r);
            kinds
                .entry(day_type(r.day, &prep.holiday_days).name())
                .or_default()
                .push(r);
        }
        let conventional = json!({
            "development":errors(&by(Part::Development),t),"calibration":errors(&by(Part::Calibration),t),
            "test":errors(&test_rows,t),
            "test_by_month":months.iter().map(|(k,v)| (k.clone(),errors(v,t))).collect::<serde_json::Map<_,_>>(),
            "test_by_day_type":kinds.iter().map(|(k,v)| (k.to_string(),errors(v,t))).collect::<serde_json::Map<_,_>>(),
        });
        let refs: serde_json::Map<String, Value> = m
            .lags
            .iter()
            .enumerate()
            .filter(|(_, l)| matches!(**l, 1 | 24 | 168))
            .map(|(k, l)| (format!("cnt(T-{l})"), reference(&test_rows, k, t)))
            .collect();
        // Normalised split conformal on calibration.
        let mut scores: Vec<f64> = rows
            .iter()
            .filter(|r| r.part == Part::Calibration)
            .map(|r| (r.y - r.yhat).abs() / r.s)
            .collect();
        scores.sort_by(f64::total_cmp);
        let kq = (((scores.len() + 1) as f64) * (1.0 - m.conformal_alpha)).ceil() as usize;
        let q = (kq >= 1 && kq <= scores.len()).then(|| scores[kq - 1]);
        let cover = |rs: &[&Row]| -> Value {
            match q {
                None => json!({"n":rs.len(),"coverage":null}),
                Some(q) => {
                    let c = rs
                        .iter()
                        .filter(|r| (r.y - r.yhat).abs() <= q * r.s)
                        .count();
                    json!({"n":rs.len(),"covered":c,"coverage":(!rs.is_empty()).then(|| c as f64/rs.len() as f64),
                        "mean_width_bikes":(!rs.is_empty()).then(|| rs.iter().map(|r| 2.0*q*r.s).sum::<f64>()/rs.len() as f64)})
                }
            }
        };
        let conformal = json!({"alpha":m.conformal_alpha,"calibration_n":scores.len(),"rank":kq,"quantile":q,
            "calibration":cover(&by(Part::Calibration)),"test":cover(&test_rows),
            "test_by_month":months.iter().map(|(k,v)| (k.clone(),cover(v))).collect::<serde_json::Map<_,_>>(),
            "test_by_day_type":kinds.iter().map(|(k,v)| (k.to_string(),cover(v))).collect::<serde_json::Map<_,_>>(),
            "assumption":"Exchangeability of calibration (May-Aug) and test (Sep-Dec) hours is not assumed to hold; coverage is measured, not guaranteed."});
        let monitoring = monitor(m, &prep, &rows, &accepted, t, true)?;
        let mut sensitivity = Vec::new();
        for st in &m.tolerance_sensitivity {
            let gi = generic_input(m, &model.model_id, &rows, *st);
            let g = metrology::evaluate(&gi)?;
            let mon = monitor(m, &prep, &rows, &accepted, *st, false)?;
            sensitivity.push(json!({"tolerance":st,"test_errors":errors(&test_rows,*st),
                "selective":g["overall"]["operational"],"aurc":g["overall"]["aurc"],
                "r_cal":mon["r_cal"],"triad_test_alert_days":mon["triad_test_alert_days"],
                "degraded_days":mon["degraded_days"],"primary":mon["primary"]}));
        }
        confirmatory.push(json!({"model_id":model.model_id,"delta":monitoring["primary"]["comparison"]["delta_mean_cost_a_minus_b"],
            "interval":monitoring["primary"]["comparison"]["bootstrap"][0]["intervals"][0]["interval"],
            "decision":monitoring["primary"]["decision"]}));
        model_reports.push(json!({
            "model_id":model.model_id,"model_sha256":model.model_sha256,
            "selective_threshold":tau,"conventional":conventional,"reference_forecasts_test":refs,
            "selective_generic":generic,"conformal":conformal,"monitoring":monitoring,
            "tolerance_sensitivity":sensitivity,
        }));
    }
    let established = confirmatory
        .iter()
        .all(|c| c["decision"] == "advantage_established");
    Ok(json!({
        "schema_version":"triad-temporal-regression-report-1",
        "benchmark_id":m.benchmark_id,
        "manifest":m,
        "manifest_sha256":crate::evaluation::hash(&serde_json::to_vec(m)?),
        "protocol_sha256":m.protocol_sha256,
        "evaluator_source_sha256":{
            "metrology_temporal.rs":crate::evaluation::hash(include_bytes!("metrology_temporal.rs")),
            "evidence.rs":crate::evaluation::hash(include_bytes!("evidence.rs")),
            "metrology.rs":crate::evaluation::hash(include_bytes!("metrology.rs")),
        },
        "data":prep.summary,
        "models":model_reports,
        "confirmatory":{"endpoint":"mean decision cost per eligible test hour, triad minus std at matched calibration budget",
            "abstention_cost":m.abstention_costs[0],"practical_threshold":m.practical_threshold,
            "interval":"paired circular moving-block bootstrap of test days, primary block length, Bonferroni-adjusted two-sided level",
            "per_model":confirmatory,"added_predictive_utility_established":established},
        "interpretation":"Exploratory external-data evaluation with a frozen local protocol and computational cross-check; not independent scientific replication. Day blocks assume approximate local stationarity; intervals condition on fitted models and thresholds. No concept-drift identification, no guarantee under unseen conditions, no NIST endorsement."
    }))
}

pub fn validate_report(report: &Value) -> Result<()> {
    let m: Manifest =
        serde_json::from_value(report.get("manifest").context("missing manifest")?.clone())?;
    if &evaluate(&m)? != report {
        bail!("temporal benchmark report differs from deterministic replay");
    }
    Ok(())
}

pub fn markdown(report: &Value) -> String {
    let mut md = String::from("# Bike Sharing temporal benchmark\n\nExploratory; hours and days are dependent. See report.json for every count, curve, interval and day.\n\n| Model | Test MAE | Material-error rate | Selective coverage | Selective risk | Conformal 90% coverage | Primary delta (triad - std) | Decision |\n|---|---:|---:|---:|---:|---:|---:|---|\n");
    for r in report["models"].as_array().unwrap() {
        let op = &r["selective_generic"]["overall"]["operational"];
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r["model_id"].as_str().unwrap_or(""),
            r["conventional"]["test"]["mae"],
            r["conventional"]["test"]["material_error_rate"],
            op["coverage"],
            op["selective_error_risk"],
            r["conformal"]["test"]["coverage"],
            r["monitoring"]["primary"]["comparison"]["delta_mean_cost_a_minus_b"],
            r["monitoring"]["primary"]["decision"]
                .as_str()
                .unwrap_or("")
        ));
    }
    md.push_str(&format!(
        "\nAdded predictive utility established (both models): {}.\nNull values mean insufficient information, never zero risk.\n",
        report["confirmatory"]["added_predictive_utility_established"]
    ));
    md
}

pub fn run(input: &Path, output: &Path) -> Result<()> {
    let m: Manifest = serde_json::from_slice(&fs::read(input)?)
        .context("invalid triad-temporal-regression-1 manifest")?;
    let report = evaluate(&m)?;
    fs::create_dir(output).context("benchmark output must be a new directory")?;
    // Compact JSON: the report embeds the full manifest.
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer(&mut f, &json!({"report":report}))?;
    f.write_all(b"\n")?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.md"))?;
    f.write_all(markdown(&report).as_bytes())?;
    Ok(())
}
