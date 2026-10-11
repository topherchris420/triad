//! Ordered-stream benchmark. No independence, calibration or drift guarantee.
use crate::metrology::{self, Input, Record, Split};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, io::Write, path::Path};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub id: String,
    pub split: Split,
    pub start_day: usize,
    pub end_day: usize,
    pub covariate: f64,
    pub confidence: f64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub model_sha256: String,
    pub input: Input,
    pub windows: Vec<Window>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub provenance: Value,
    pub protocol_sha256: String,
    pub rows_per_day: usize,
    pub block_lengths: Vec<usize>,
    pub alert_quantile: f64,
    pub degradation_margin: f64,
    pub models: Vec<Model>,
}
fn hash_ok(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn validate(m: &Manifest) -> Result<()> {
    ensure!(
        m.schema_version == "triad-benchmark-1",
        "unsupported benchmark schema"
    );
    ensure!(hash_ok(&m.protocol_sha256), "invalid protocol hash");
    ensure!(m.provenance.is_object(), "provenance must be an object");
    for key in ["source_url", "sha256", "chronology", "license_as_reported"] {
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
        m.rows_per_day > 0 && m.rows_per_day <= 100_000,
        "invalid day size"
    );
    ensure!(
        !m.block_lengths.is_empty() && m.block_lengths.iter().all(|b| *b > 0 && *b <= 365),
        "invalid blocks"
    );
    ensure!(
        m.alert_quantile.is_finite() && m.alert_quantile > 0. && m.alert_quantile < 1.,
        "invalid alert quantile"
    );
    ensure!(
        m.degradation_margin.is_finite() && (0.0..=1.0).contains(&m.degradation_margin),
        "invalid degradation margin"
    );
    ensure!(
        !m.models.is_empty() && m.models.len() <= 16,
        "invalid model count"
    );
    let mut model_ids = BTreeSet::new();
    for model in &m.models {
        let input = &model.input;
        metrology::validate(input)?;
        ensure!(
            model_ids.insert(&input.model_id) && hash_ok(&model.model_sha256),
            "duplicate model or invalid hash"
        );
        ensure!(
            matches!(input.loss, metrology::Loss::Classification),
            "benchmark v1 requires classification"
        );
        ensure!(
            !input.evidence.numerical_required
                && !input.evidence.prospective_power_supported
                && !input.evidence.independent_replication,
            "unsupported assurance assertion"
        );
        ensure!(
            input
                .records
                .iter()
                .all(|r| matches!(r.split, Split::Calibration | Split::Test)),
            "export calibration/test records only"
        );
        for split in [Split::Calibration, Split::Test] {
            let sequences: BTreeSet<_> = input
                .records
                .iter()
                .filter(|r| r.split == split)
                .map(|r| &r.sequence)
                .collect();
            ensure!(sequences.len() == 1, "ordered stream must be one dependent sequence per split; use temporal blocks for uncertainty");
        }
        let mut last = None;
        for r in &input.records {
            let id: usize =
                r.id.parse()
                    .context("record id must be original ordered row index")?;
            ensure!(
                last.is_none_or(|v| id == v + 1),
                "records must be consecutive and ordered"
            );
            ensure!(
                r.error_probability == Some(r.uncertainty) && (0.0..=0.5).contains(&r.uncertainty),
                "binary error probability/score required"
            );
            ensure!(
                (r.prediction[0] == 0. || r.prediction[0] == 1.)
                    && r.truth.as_ref().is_none_or(|y| y[0] == 0. || y[0] == 1.),
                "binary labels required"
            );
            last = Some(id);
        }
        let mut covered = 0;
        let mut previous_end = None;
        let mut ids = BTreeSet::new();
        let mut seen_test = false;
        for w in &model.windows {
            ensure!(
                ids.insert(&w.id) && !w.id.is_empty(),
                "duplicate/empty window"
            );
            ensure!(
                w.start_day < w.end_day && w.end_day <= usize::MAX / m.rows_per_day,
                "invalid window bounds"
            );
            ensure!(
                previous_end.is_none_or(|e| e == w.start_day),
                "window gaps/overlap"
            );
            previous_end = Some(w.end_day);
            ensure!(
                w.covariate.is_finite()
                    && w.confidence.is_finite()
                    && w.covariate >= 0.
                    && w.confidence >= 0.,
                "invalid diagnostic score"
            );
            ensure!(
                matches!(w.split, Split::Calibration | Split::Test),
                "invalid window split"
            );
            ensure!(
                !(seen_test && w.split == Split::Calibration),
                "calibration after test"
            );
            seen_test |= w.split == Split::Test;
            let rows = window_rows(input, w, m.rows_per_day);
            ensure!(
                rows.len() == (w.end_day - w.start_day) * m.rows_per_day,
                "window incomplete"
            );
            ensure!(
                rows.iter().all(|r| r.split == w.split
                    && if w.split == Split::Test {
                        r.scenario == w.id
                    } else {
                        r.scenario == input.calibration_scenario
                    }),
                "window split/scenario mismatch"
            );
            covered += rows.len();
        }
        ensure!(
            covered == input.records.len()
                && model
                    .windows
                    .iter()
                    .filter(|w| w.split == Split::Calibration)
                    .count()
                    >= 2
                && seen_test,
            "incomplete window partition"
        );
        let first = &m.models[0];
        ensure!(
            input.target_coverage == first.input.target_coverage
                && input.tolerance == first.input.tolerance
                && input.bootstrap_seed == first.input.bootstrap_seed
                && input.bootstrap_replicates == first.input.bootstrap_replicates,
            "paired policies must share operating specification"
        );
        ensure!(
            input.records.len() == first.input.records.len()
                && input
                    .records
                    .iter()
                    .zip(&first.input.records)
                    .all(|(a, b)| a.id == b.id
                        && a.truth == b.truth
                        && a.split == b.split
                        && a.scenario == b.scenario),
            "models must share ordered rows and labels"
        );
        ensure!(
            model.windows.len() == first.windows.len()
                && model
                    .windows
                    .iter()
                    .zip(&first.windows)
                    .all(|(a, b)| a.id == b.id
                        && a.start_day == b.start_day
                        && a.end_day == b.end_day
                        && a.split == b.split),
            "paired windows differ"
        );
    }
    Ok(())
}
fn window_rows<'a>(input: &'a Input, w: &Window, size: usize) -> Vec<&'a Record> {
    input
        .records
        .iter()
        .filter(|r| {
            r.id.parse::<usize>()
                .is_ok_and(|i| i / size >= w.start_day && i / size < w.end_day)
        })
        .collect()
}
fn q(values: &[f64], p: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let x = p * (v.len() - 1) as f64;
    let lo = x.floor() as usize;
    v[lo] + (v[x.ceil() as usize] - v[lo]) * (x - lo as f64)
}
fn next(state: &mut u64, n: usize) -> usize {
    // SplitMix64 with rejection: portable uniform integer draws, no modulo bias.
    let bound = (n as u64).wrapping_neg() % n as u64;
    loop {
        *state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        if z >= bound {
            return (z % n as u64) as usize;
        }
    }
}
// Day totals: n, accepted A, errors A, accepted B, errors B, errors all.
pub fn block_intervals(
    days: &[[f64; 6]],
    block: usize,
    reps: usize,
    seed: u64,
    complete: bool,
) -> Value {
    if block == 0 || days.len() / block < 2 || reps < 100 {
        return json!({"status":"insufficient_blocks","ci95":null});
    }
    let total: Vec<f64> = (0..6).map(|j| days.iter().map(|d| d[j]).sum()).collect();
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); 7];
    let mut state = seed;
    let mut undefined = 0;
    for _ in 0..reps {
        let mut sum = [0.; 6];
        let mut count = 0;
        while count < days.len() {
            let start = next(&mut state, days.len());
            for k in 0..block.min(days.len() - count) {
                for (j, s) in sum.iter_mut().enumerate() {
                    *s += days[(start + k) % days.len()][j];
                }
                count += 1;
            }
        }
        let a = sum[2] / sum[1];
        let b = sum[4] / sum[3];
        undefined += usize::from(!a.is_finite() || !b.is_finite());
        for (v, x) in samples.iter_mut().zip([
            a,
            b,
            b - a,
            sum[1] / sum[0],
            sum[3] / sum[0],
            sum[2] / sum[0],
            sum[4] / sum[0],
        ]) {
            v.push(x);
        }
    }
    let names = [
        "risk_a",
        "risk_b",
        "risk_b_minus_a",
        "coverage_a",
        "coverage_b",
        "incorrect_acceptance_a",
        "incorrect_acceptance_b",
    ];
    let mut intervals = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        let rare_a = total[2] == 0. || total[2] == total[1];
        let rare_b = total[4] == 0. || total[4] == total[3];
        let unresolved = match i {
            0 | 5 => !complete || rare_a || samples[i].iter().any(|x| !x.is_finite()),
            1 | 6 => !complete || rare_b || samples[i].iter().any(|x| !x.is_finite()),
            2 => !complete || rare_a || rare_b || undefined > 0,
            _ => false,
        };
        intervals.insert(
            name.to_string(),
            if unresolved {
                Value::Null
            } else {
                json!([q(&samples[i], 0.025), q(&samples[i], 0.975)])
            },
        );
    }
    json!({"status":"descriptive_circular_moving_block","days":days.len(),"block_days":block,"replicates":reps,"seed":seed,"undefined_paired_replicates":undefined,"ci95":intervals})
}
fn totals(
    rows: &[&Record],
    a: &[bool],
    b: &[bool],
    other: Option<&[&Record]>,
    size: usize,
    input: &Input,
) -> Result<Vec<[f64; 6]>> {
    let mut days = vec![[0.; 6]; rows.len() / size];
    for (i, r) in rows.iter().enumerate() {
        let e = f64::from(metrology::event(r, input)? == Some(true));
        let f = f64::from(metrology::event(other.map_or(*r, |v| v[i]), input)? == Some(true));
        let d = &mut days[i / size];
        for (t, v) in d.iter_mut().zip([
            1.,
            f64::from(a[i]),
            e * f64::from(a[i]),
            f64::from(b[i]),
            f * f64::from(b[i]),
            e,
        ]) {
            *t += v;
        }
    }
    Ok(days)
}
pub fn evaluate(m: &Manifest) -> Result<Value> {
    validate(m)?;
    let mut reports = Vec::new();
    let mut paired_rows = Vec::new();
    let mut paired_masks = Vec::new();
    for model in &m.models {
        let input = &model.input;
        let tau = metrology::threshold(input);
        let test: Vec<_> = input
            .records
            .iter()
            .filter(|r| r.split == Split::Test)
            .collect();
        let cal: Vec<_> = input
            .records
            .iter()
            .filter(|r| r.split == Split::Calibration)
            .collect();
        let accept = |rows: &[&Record]| {
            rows.iter()
                .map(|r| tau.is_some_and(|t| r.uncertainty <= t))
                .collect::<Vec<_>>()
        };
        let base_mask = accept(&test);
        let cal_metric = metrology::metrics(&cal, &accept(&cal), input)?;
        let cal_risk = cal_metric.selective_error_risk;
        let score = |w: &Window, which: usize| match which {
            0 => w.covariate.max(w.confidence),
            1 => w.covariate,
            _ => w.confidence,
        };
        let thresholds: Vec<_> = (0..3)
            .map(|j| {
                q(
                    &model
                        .windows
                        .iter()
                        .filter(|w| w.split == Split::Calibration)
                        .map(|w| score(w, j))
                        .collect::<Vec<_>>(),
                    m.alert_quantile,
                )
            })
            .collect();
        let mut gate_masks = vec![Vec::new(); 3];
        let mut window_reports = Vec::new();
        let mut confusion = [[0usize; 5]; 3]; // TP, FP, FN, TN, unknown
        let mut alerts = [0usize; 3];
        let mut delay: Vec<Vec<Option<usize>>> = vec![Vec::new(); 3];
        let test_windows: Vec<_> = model
            .windows
            .iter()
            .filter(|w| w.split == Split::Test)
            .collect();
        let mut degraded_flags = Vec::new();
        let mut alert_flags = Vec::new();
        for w in &test_windows {
            let rows = window_rows(input, w, m.rows_per_day);
            let mask = accept(&rows);
            let metric = metrology::metrics(&rows, &mask, input)?;
            let degraded = cal_risk
                .zip(metric.selective_error_risk)
                .map(|(c, t)| t > c + m.degradation_margin);
            let flags: Vec<_> = (0..3).map(|j| score(w, j) > thresholds[j]).collect();
            for j in 0..3 {
                alerts[j] += usize::from(flags[j]);
                let k = match (degraded, flags[j]) {
                    (Some(true), true) => 0,
                    (Some(false), true) => 1,
                    (Some(true), false) => 2,
                    (Some(false), false) => 3,
                    (None, _) => 4,
                };
                confusion[j][k] += 1;
                gate_masks[j].extend(mask.iter().map(|a| *a && !flags[j]));
            }
            let days = totals(&rows, &mask, &mask, None, m.rows_per_day, input)?;
            window_reports.push(json!({"id":w.id,"start_day":w.start_day,"end_day":w.end_day,"selective":metric,"full":metrology::metrics(&rows,&vec![true;rows.len()],input)?,"degraded":degraded,"alerts":flags,"scores":[score(w,0),score(w,1),score(w,2)],"risk_uncertainty":m.block_lengths.iter().map(|b|block_intervals(&days,*b,input.bootstrap_replicates,input.bootstrap_seed,rows.iter().all(|r|r.truth.is_some()))).collect::<Vec<_>>()}));
            degraded_flags.push(degraded);
            alert_flags.push(flags);
        }
        // Delay only within contiguous degraded episodes; null = missed episode.
        for i in 0..test_windows.len() {
            if degraded_flags[i] == Some(true) && (i == 0 || degraded_flags[i - 1] != Some(true)) {
                let end = (i..test_windows.len())
                    .find(|k| degraded_flags[*k] != Some(true))
                    .unwrap_or(test_windows.len());
                for j in 0..3 {
                    delay[j].push((i..end).find(|k| alert_flags[*k][j]).map(|k| k - i));
                }
            }
        }
        let policies: Vec<_> = (0..3).map(|j| {
            let c=confusion[j];
            let ratio=|a:usize,b:usize|if b>0 {Some(a as f64/b as f64)} else {None};
            let days=totals(&test,&base_mask,&gate_masks[j],None,m.rows_per_day,input)?;
            Ok(json!({"name":(["combined","covariate_only","confidence_only"][j]),"threshold":thresholds[j],"calibration_alerts":model.windows.iter().filter(|w| w.split==Split::Calibration && score(w,j)>thresholds[j]).count(),"calibration_windows":model.windows.iter().filter(|w|w.split==Split::Calibration).count(),"test_alerts":alerts[j],"confusion":{"true_positive":c[0],"false_positive":c[1],"false_negative":c[2],"true_negative":c[3],"unknown":c[4]},"recall":ratio(c[0],c[0]+c[2]),"false_alarm_rate":ratio(c[1],c[1]+c[3]),"episode_delay_windows":delay[j],"delay_interpretation":"end-of-window warning, no prospective lead time; null episode is missed","gated":metrology::metrics(&test,&gate_masks[j],input)?,"paired_uncertainty":m.block_lengths.iter().map(|b|block_intervals(&days,*b,input.bootstrap_replicates,input.bootstrap_seed,test.iter().all(|r|r.truth.is_some()))).collect::<Vec<_>>(),"warning_utility_ci":null,"warning_utility_ci_reason":"few dependent windows; no independent alarm-trial interval asserted"}))
        }).collect::<Result<_>>()?;
        let mut conventional = metrology::evaluate(input)?;
        conventional.as_object_mut().unwrap().remove("input");
        reports.push(json!({"model_id":input.model_id,"calibration":cal_metric,"conventional_and_selective":conventional,"windows":window_reports,"policies":policies,"simple_warning_baseline":"exactly identical to combined policy; no added predictive value from validity metadata","validity":{"numerical":"not_applicable","statistical":"exploratory_dependence_unresolved","calibration":"retrospective_only; no calibration guarantee","distribution":"observable_window_diagnostics; not proof of concept drift","chronology":"conditional_published_row_order","independent_scientific_replication":false}}));
        paired_rows.push(test);
        paired_masks.push(base_mask);
    }
    let mut comparisons = Vec::new();
    for j in 1..m.models.len() {
        let rows = &paired_rows[0];
        let days = totals(
            rows,
            &paired_masks[0],
            &paired_masks[j],
            Some(&paired_rows[j]),
            m.rows_per_day,
            &m.models[0].input,
        )?;
        comparisons.push(json!({"a":m.models[0].input.model_id,"b":m.models[j].input.model_id,"interpretation":"difference of conditional risks on each model's own accepted set, shared sampled days; not matched accepted subsets","intervals":m.block_lengths.iter().map(|b|block_intervals(&days,*b,m.models[0].input.bootstrap_replicates,m.models[0].input.bootstrap_seed,rows.iter().all(|r|r.truth.is_some()))).collect::<Vec<_>>()}));
    }
    Ok(
        json!({"schema_version":"triad-benchmark-report-1","manifest":m,"manifest_sha256":crate::evaluation::hash(&serde_json::to_vec(m)?),"evaluator_source_sha256":crate::evaluation::hash(include_bytes!("metrology_benchmark.rs")),"models":reports,"paired_models":comparisons,"interpretation":"Exploratory external-data evaluation and computational cross-check, not independent scientific replication. Circular blocks require local stationarity; sensitivity is not a coverage guarantee. No NIST endorsement."}),
    )
}
pub fn validate_report(report: &Value) -> Result<()> {
    let m: Manifest =
        serde_json::from_value(report.get("manifest").context("missing manifest")?.clone())?;
    ensure!(
        &evaluate(&m)? == report,
        "benchmark differs from deterministic replay"
    );
    Ok(())
}
pub fn run(input: &Path, output: &Path) -> Result<()> {
    let m: Manifest = serde_json::from_slice(&fs::read(input)?)?;
    let report = evaluate(&m)?;
    fs::create_dir(output).context("benchmark output must be a new directory")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer(&mut file, &json!({"report":report}))?;
    file.write_all(b"\n")?;
    let mut md=String::from("# Electricity benchmark\n\nExploratory; chronology and temporal independence unresolved.\n\n| Model | Coverage | Selective risk | Brier | ECE |\n|---|---:|---:|---:|---:|\n");
    for r in report["models"].as_array().unwrap() {
        let s = &r["conventional_and_selective"]["overall"];
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r["model_id"],
            s["operational"]["coverage"],
            s["operational"]["selective_error_risk"],
            s["brier"],
            s["ece"]
        ));
    }
    md.push_str("\nFull curves, reliability, event counts, temporal windows, paired block intervals and ablations are in report.json. Null is insufficient evidence, never zero risk. Simple combined-warning and TRIAD gates are identical by construction. No scientific superiority or concept-drift detection claim follows.\n");
    fs::write(output.join("report.md"), md)?;
    Ok(())
}
