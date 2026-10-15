//! Milestone 9: pre-registered simulation of abstention certification methods.
//! Protocol: `experiments/certify-v1.json` (frozen before this code existed).
use crate::certify::{self, evaluate_candidates, select, CountTable, Method, Targets};
use crate::research_integrity::write_new_json;
use anyhow::{bail, ensure, Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};
use std::{fs, path::Path};

pub const METHODS: [Method; 4] = [
    Method::LegacyHoeffdingRatio,
    Method::HoeffdingBentkus,
    Method::Betting,
    Method::NaiveFrameBinomial,
];

/// Every executable setting is checked; prose fields are not an API.
pub fn validate_protocol(p: &Value) -> Result<()> {
    let expected = json!({
        "protocol_version": "certify-v1",
        "methods": {
            "baseline": "legacy-hoeffding-ratio",
            "primary": "hoeffding-bentkus",
            "secondary": "betting",
            "comparator_invalid_under_dependence": "naive-frame-binomial"
        },
        "targets": {"maximum_accepted_error_rate": 0.05, "minimum_coverage": 0.5, "delta": 0.05},
        "candidate_thresholds": [0.05,0.1,0.15,0.2,0.25,0.3,0.35,0.4,0.45,0.5,0.55,0.6,0.65,0.7,0.75,0.8,0.85,0.9,0.95,1.0],
        "sequence_counts": [100,250,500,1000,2500,5000,10000,25000],
        "replicates_per_cell": 1000,
        "master_seed": 20261015,
        "no_hardware_claims": true
    });
    for (key, value) in expected.as_object().unwrap() {
        ensure!(
            p.get(key) == Some(value),
            "source and frozen protocol disagree at '{key}'; create a new versioned study to change executable settings"
        );
    }
    for (name, a0, a3) in [("graded", 0.01, 0.3), ("null", 0.055, 0.0)] {
        let m = &p["error_models"][name];
        ensure!(
            m["a0"] == json!(a0) && m["a3"] == json!(a3),
            "error model '{name}' differs from the implemented study"
        );
    }
    for (name, frames, effect) in [
        ("independent_frames", 1, false),
        ("dependent_frames", 10, true),
    ] {
        let r = &p["regimes"][name];
        ensure!(
            r["frames_per_sequence"] == json!(frames) && r["sequence_effect"] == json!(effect),
            "regime '{name}' differs from the implemented study"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Cell {
    index: usize,
    model: (&'static str, f64, f64),
    regime: (&'static str, usize, bool),
    sequences: usize,
}

fn true_risk(model: (&str, f64, f64), t: f64) -> f64 {
    model.1 + model.2 * t.powi(3) / 4.0
}

/// Exact one-sided Clopper–Pearson bound by bisection on the binomial CDF.
pub fn clopper_pearson(k: u64, n: u64, level: f64, upper: bool) -> f64 {
    if (upper && k >= n) || (!upper && k == 0) {
        return if upper { 1.0 } else { 0.0 };
    }
    let tail = 1.0 - level;
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        // Upper: P(X <= k | mid) decreases in mid. Lower: P(X >= k | mid) increases.
        let too_low = if upper {
            certify::binomial_cdf(k as i64, n, mid) > tail
        } else {
            1.0 - certify::binomial_cdf(k as i64 - 1, n, mid) < tail
        };
        if too_low {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    if upper {
        hi
    } else {
        lo
    }
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn run_cell(cell: Cell, thresholds: &[f64], targets: Targets, reps: usize, seed: u64) -> Value {
    let (_, frames, effect) = cell.regime;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut certified = [0u64; 4];
    let mut false_cert = [0u64; 4];
    let mut false_risk = [0u64; 4];
    let mut false_cov = [0u64; 4];
    let mut coverage_sum = [0.0f64; 4];
    let mut dominance_violations = 0u64;
    let mut frame_buf = Vec::with_capacity(frames);
    for _ in 0..reps {
        let mut table = CountTable::new(thresholds.to_vec(), frames);
        for _ in 0..cell.sequences {
            let multiplier = if effect { 2.0 * rng.gen::<f64>() } else { 1.0 };
            frame_buf.clear();
            for _ in 0..frames {
                let s: f64 = rng.gen();
                let q = (cell.model.1 + cell.model.2 * s.powi(3)) * multiplier;
                frame_buf.push((s, rng.gen::<f64>() < q));
            }
            table.push(frame_buf.iter().copied());
        }
        let results: Vec<_> = METHODS
            .iter()
            .map(|&m| evaluate_candidates(&table, targets, m))
            .collect();
        let (legacy, hb) = (&results[0], &results[1]);
        if legacy.iter().zip(hb).any(|(l, h)| {
            (l.qualifies && !h.qualifies) || h.coverage_lower_bound < l.coverage_lower_bound
        }) {
            dominance_violations += 1;
        }
        for (m, candidates) in results.iter().enumerate() {
            if let Some(i) = select(candidates) {
                let t = candidates[i].threshold;
                let bad_risk = true_risk(cell.model, t) > targets.maximum_accepted_error_rate;
                let bad_cov = t < targets.minimum_coverage;
                certified[m] += 1;
                coverage_sum[m] += t;
                false_risk[m] += u64::from(bad_risk);
                false_cov[m] += u64::from(bad_cov);
                false_cert[m] += u64::from(bad_risk || bad_cov);
            }
        }
    }
    let n = reps as u64;
    let methods: serde_json::Map<_, _> = METHODS
        .iter()
        .enumerate()
        .map(|(m, method)| {
            (
                method.name().to_string(),
                json!({
                    "certified": certified[m],
                    "false_certified": false_cert[m],
                    "false_risk": false_risk[m],
                    "false_coverage": false_cov[m],
                    "certification_rate": certified[m] as f64 / n as f64,
                    "false_certification_rate": false_cert[m] as f64 / n as f64,
                    "false_certification_lower_99": round6(clopper_pearson(false_cert[m], n, 0.99, false)),
                    "false_certification_upper_99": round6(clopper_pearson(false_cert[m], n, 0.99, true)),
                    "mean_selected_true_coverage": round6(coverage_sum[m] / n as f64),
                }),
            )
        })
        .collect();
    json!({
        "cell_index": cell.index,
        "error_model": cell.model.0,
        "regime": cell.regime.0,
        "frames_per_sequence": frames,
        "sequences": cell.sequences,
        "replicates": reps,
        "seed": seed,
        "dominance_violations": dominance_violations,
        "methods": methods,
    })
}

fn n80(cells: &[Value], model: &str, regime: &str, method: &str) -> Option<u64> {
    cells
        .iter()
        .filter(|c| c["error_model"] == model && c["regime"] == regime)
        .find(|c| c["methods"][method]["certification_rate"].as_f64().unwrap() >= 0.8)
        .map(|c| c["sequences"].as_u64().unwrap())
}

fn efficiency(cells: &[Value], method: &str) -> Value {
    let base = n80(
        cells,
        "graded",
        "dependent_frames",
        "legacy-hoeffding-ratio",
    );
    let new = n80(cells, "graded", "dependent_frames", method);
    let supported = match (new, base) {
        (Some(a), Some(b)) => 2 * a <= b,
        (Some(a), None) => a <= 12_500,
        _ => false,
    };
    json!({
        "method": method,
        "n80": new,
        "legacy_n80": base,
        "legacy_n80_note": if base.is_none() {"legacy never reached 0.80 certification; treated as exceeding 25000"} else {"reached within grid"},
        "supported": supported,
    })
}

fn hypotheses(cells: &[Value], delta: f64) -> Value {
    let valid_methods = ["legacy-hoeffding-ratio", "hoeffding-bentkus", "betting"];
    let exceed: Vec<_> = cells
        .iter()
        .flat_map(|c| {
            valid_methods.iter().filter_map(move |m| {
                let lower = c["methods"][m]["false_certification_lower_99"]
                    .as_f64()
                    .unwrap();
                (lower > delta).then(|| json!({"cell_index": c["cell_index"], "method": m}))
            })
        })
        .collect();
    let violations: u64 = cells
        .iter()
        .map(|c| c["dominance_violations"].as_u64().unwrap())
        .sum();
    let naive: Vec<_> = cells
        .iter()
        .filter(|c| c["error_model"] == "null" && c["regime"] == "dependent_frames")
        .map(|c| {
            json!({"sequences": c["sequences"],
                   "false_certification_rate": c["methods"]["naive-frame-binomial"]["false_certification_rate"],
                   "false_certification_lower_99": c["methods"]["naive-frame-binomial"]["false_certification_lower_99"]})
        })
        .collect();
    let naive_exceeds = naive
        .iter()
        .any(|c| c["false_certification_lower_99"].as_f64().unwrap() > delta);
    json!({
        "H1_validity": {"supported": exceed.is_empty(), "exceedances": exceed},
        "H2_dominance": {"supported": violations == 0, "violating_datasets": violations},
        "H3_primary_efficiency": efficiency(cells, "hoeffding-bentkus"),
        "H4_secondary_efficiency": efficiency(cells, "betting"),
        "E1_exploratory": {"naive_exceeds_delta": naive_exceeds, "null_dependent_cells": naive},
    })
}

/// Run the frozen study into a new directory.
pub fn run(protocol_path: &Path, output: &Path) -> Result<Value> {
    let bytes = fs::read(protocol_path)
        .with_context(|| format!("read protocol {}", protocol_path.display()))?;
    let protocol: Value = serde_json::from_slice(&bytes).context("protocol must be JSON")?;
    validate_protocol(&protocol)?;
    let report = simulate(&protocol)?;
    fs::create_dir(output).context("certify-study requires a new output directory")?;
    write_new_json(&output.join("report.json"), &json!({"report": report}))?;
    let mut md = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.md"))?;
    std::io::Write::write_all(&mut md, markdown(&report).as_bytes())?;
    Ok(report)
}

pub fn simulate(protocol: &Value) -> Result<Value> {
    validate_protocol(protocol)?;
    let thresholds: Vec<f64> = serde_json::from_value(protocol["candidate_thresholds"].clone())?;
    let counts: Vec<usize> = serde_json::from_value(protocol["sequence_counts"].clone())?;
    let reps = protocol["replicates_per_cell"].as_u64().unwrap() as usize;
    let master = protocol["master_seed"].as_u64().unwrap();
    let t = &protocol["targets"];
    let targets = Targets {
        maximum_accepted_error_rate: t["maximum_accepted_error_rate"].as_f64().unwrap(),
        minimum_coverage: t["minimum_coverage"].as_f64().unwrap(),
        delta: t["delta"].as_f64().unwrap(),
    };
    let mut cells = Vec::new();
    for model in [("graded", 0.01, 0.3), ("null", 0.055, 0.0)] {
        for regime in [
            ("independent_frames", 1, false),
            ("dependent_frames", 10, true),
        ] {
            for &sequences in &counts {
                cells.push(Cell {
                    index: cells.len(),
                    model,
                    regime,
                    sequences,
                });
            }
        }
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(cells.len());
    // Largest cells first; each cell owns its seed, so scheduling cannot change results.
    let mut order: Vec<usize> = (0..cells.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(cells[i].sequences * cells[i].regime.1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let shared = std::sync::Mutex::new(vec![None::<Value>; cells.len()]);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&i) = order.get(k) else { break };
                let seed = master + 1000 * i as u64;
                let value = run_cell(cells[i], &thresholds, targets, reps, seed);
                shared.lock().unwrap()[i] = Some(value);
            });
        }
    });
    let cells: Vec<Value> = shared
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|c| c.expect("every cell ran"))
        .collect();
    let hypotheses = hypotheses(&cells, targets.delta);
    Ok(json!({
        "schema_version": "triad-certify-study-report-1",
        "software_version": env!("CARGO_PKG_VERSION"),
        "protocol": protocol,
        "protocol_sha256": crate::evaluation::hash(&serde_json::to_vec(protocol)?),
        "source_sha256": {
            "certify.rs": crate::evaluation::hash(include_bytes!("certify.rs")),
            "certify_study.rs": crate::evaluation::hash(include_bytes!("certify_study.rs")),
        },
        "cells": cells,
        "hypotheses": hypotheses,
        "interpretation": "Simulation of statistical procedures under their stated assumptions (independent sequences, analytic truth). Not a sensor, hardware or real-data result.",
    }))
}

/// Recompute the study from its embedded protocol; the report must match exactly.
pub fn validate_report(report: &Value) -> Result<()> {
    let expected = simulate(report.get("protocol").context("report missing protocol")?)?;
    ensure!(
        &expected == report,
        "report differs from recomputed simulation; do not edit derived evidence"
    );
    Ok(())
}

pub fn markdown(report: &Value) -> String {
    let mut s = String::from("# Milestone 9: certification of abstention policies\n\nSimulation under the frozen protocol `experiments/certify-v1.json`. Certification rate / false-certification rate / mean true coverage of the selected threshold (refusal counts as zero coverage).\n\n| Error model | Regime | Sequences | Legacy Hoeffding ratio | Hoeffding–Bentkus | Betting | Naive frame binomial |\n|---|---|---:|---|---|---|---|\n");
    for c in report["cells"].as_array().unwrap() {
        s += &format!(
            "| {} | {} | {} |",
            c["error_model"].as_str().unwrap(),
            c["regime"].as_str().unwrap(),
            c["sequences"]
        );
        for m in METHODS {
            let r = &c["methods"][m.name()];
            s += &format!(
                " {:.3} / {:.3} / {:.3} |",
                r["certification_rate"].as_f64().unwrap(),
                r["false_certification_rate"].as_f64().unwrap(),
                r["mean_selected_true_coverage"].as_f64().unwrap()
            );
        }
        s += "\n";
    }
    s += &format!(
        "\n## Pre-registered hypotheses\n\n```json\n{}\n```\n\nThis is a simulation of statistical procedures. It does not validate a sensor, a scoring model or any deployment.\n",
        serde_json::to_string_pretty(&report["hypotheses"]).unwrap()
    );
    s
}

const HELP: &str = "TRIAD certification (Milestone 9)

  certify-abstention --input REQUEST.json [--method hoeffding-bentkus|betting]
      Certify a predeclared abstention policy (same input as calibrate-abstention).
      Default method: hoeffding-bentkus, which provably qualifies every threshold
      the legacy controller qualifies. Declare the method before seeing data.
  plan-certification --coverage C --risk R --alpha A --min-coverage M --delta D --candidates K
      Plug-in number of independent sequences needed to certify, legacy versus
      Hoeffding-Bentkus. A planning aid, not a guarantee.
  certify-study --output NEW_DIRECTORY [--protocol experiments/certify-v1.json]
      Run the frozen Milestone 9 simulation; refuses existing outputs.
  certify-study --validate REPORT.json
      Recompute the archived simulation and require an exact match.

Method: docs/certification.md";

pub fn cli(args: &[String]) -> Result<()> {
    let command = args.first().map(String::as_str).unwrap_or("--help");
    if args.len() < 2 || matches!(args.get(1).map(String::as_str), Some("--help" | "-h")) {
        println!("{HELP}");
        return Ok(());
    }
    let mut opts = std::collections::BTreeMap::new();
    for pair in args[1..].chunks(2) {
        ensure!(pair.len() == 2, "{} requires a value; use --help", pair[0]);
        ensure!(
            opts.insert(pair[0].as_str(), pair[1].as_str()).is_none(),
            "duplicate {}",
            pair[0]
        );
    }
    let allowed: &[&str] = match command {
        "certify-abstention" => &["--input", "--method"],
        "plan-certification" => &[
            "--coverage",
            "--risk",
            "--alpha",
            "--min-coverage",
            "--delta",
            "--candidates",
        ],
        "certify-study" => &["--output", "--protocol", "--validate"],
        other => bail!("unknown certification command {other}"),
    };
    if let Some(bad) = opts.keys().find(|k| !allowed.contains(k)) {
        bail!("{bad} is not valid for {command}; use --help");
    }
    match command {
        "certify-abstention" => {
            let path = opts
                .get("--input")
                .context("certify-abstention requires --input FILE")?;
            let name = opts.get("--method").copied().unwrap_or("hoeffding-bentkus");
            let method = Method::parse(name)
                .filter(|m| matches!(m, Method::HoeffdingBentkus | Method::Betting))
                .with_context(|| {
                    format!("--method must be hoeffding-bentkus or betting, not {name}")
                })?;
            let bytes = fs::read(path).with_context(|| format!("read {path}"))?;
            let request: crate::abstention::FitRequest =
                serde_json::from_slice(&bytes).context("parse abstention calibration JSON")?;
            let cert = certify::certify(&request.spec, &request.sequences, method)?;
            println!("{}", serde_json::to_string_pretty(&cert)?);
        }
        "plan-certification" => {
            let num = |k: &str| -> Result<f64> {
                opts.get(k)
                    .with_context(|| format!("plan-certification requires {k}"))?
                    .parse::<f64>()
                    .with_context(|| format!("{k} must be a number"))
            };
            let plan = certify::plan(
                num("--coverage")?,
                num("--risk")?,
                Targets {
                    maximum_accepted_error_rate: num("--alpha")?,
                    minimum_coverage: num("--min-coverage")?,
                    delta: num("--delta")?,
                },
                opts.get("--candidates")
                    .context("plan-certification requires --candidates")?
                    .parse()
                    .context("--candidates must be a positive integer")?,
            )?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        "certify-study" => {
            if let Some(path) = opts.get("--validate") {
                ensure!(opts.len() == 1, "--validate takes no other options");
                let envelope: Value = serde_json::from_slice(&fs::read(path)?)?;
                validate_report(envelope.get("report").context("missing report payload")?)?;
                println!("Simulation reproduces exactly; this certifies computation, not scientific validity.");
            } else {
                let output = opts
                    .get("--output")
                    .context("certify-study requires --output NEW_DIRECTORY")?;
                let protocol = opts
                    .get("--protocol")
                    .copied()
                    .unwrap_or("experiments/certify-v1.json");
                let report = run(Path::new(protocol), Path::new(output))?;
                print!("{}", markdown(&report));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}
