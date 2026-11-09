use crate::{
    metrology::{self, Input},
    research_integrity::write_new_json,
};
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

const HELP: &str="TRIAD metrology (exploratory measurement)\n\n  metrology evidence --input assessment.json --output report.json [--root DIR]\n  metrology evidence-suite --input suite.json --output NEW_DIRECTORY [--root DIR]\n  metrology evidence-batch --input cases.json --output NEW_DIRECTORY\n  metrology benchmark --input manifest.json --output NEW_DIRECTORY\n  metrology evaluate --input predictions.json --output report.json\n  metrology validate --input report.json [--root DIR]\n  metrology demo --output NEW_DIRECTORY\n  metrology numerical --output NEW_DIRECTORY\n  metrology adaptive --output NEW_DIRECTORY\n  metrology plan --output planning.json\n\nOutputs are never overwritten. Numerical runs are expensive; demo does not run FEM.";

fn write_report(input: &Input, output: &Path, raw_hash: &str) -> Result<()> {
    let markdown = output.with_extension("md");
    ensure!(
        !output.exists() && !markdown.exists(),
        "output JSON or Markdown already exists"
    );
    let start = Instant::now();
    let report = metrology::evaluate(input)?;
    let elapsed = start.elapsed().as_secs_f64();
    let envelope = json!({"report":report,"execution":{"evaluation_seconds":elapsed,"input_bytes_sha256":raw_hash,"os":std::env::consts::OS,"architecture":std::env::consts::ARCH}});
    write_new_json(output, &envelope)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(markdown)?;
    writeln!(file,"# Selective-risk evaluation\n\nStatus: descriptive / exploratory.\n\nModel: `{}`\n\nCalibration-fixed threshold: {}\n\n| Scenario | Coverage | Selective risk | Abstention |\n| --- | ---: | ---: | ---: |",input.model_id,report["threshold"])?;
    for (name, s) in report["scenarios"].as_object().unwrap() {
        let m = &s["operational"];
        writeln!(
            file,
            "| {name} | {} | {} | {} |",
            m["coverage"], m["selective_error_risk"], m["abstention_frequency"]
        )?;
    }
    writeln!(file,"\n`null` means insufficient information, never zero risk.\n\nValidity:\n```json\n{}\n```\n\nEvaluation seconds: {elapsed}\n\nBootstrap intervals condition on fitted model and calibration threshold, assume independent sampled clusters, and do not cover model discrepancy or unseen shift. Sequence and spatial intervals are separate sensitivity analyses, not a crossed-cluster guarantee. Individual uncertainty scores are distinct from population metric uncertainty.",serde_json::to_string_pretty(&report["validity"])?)?;
    Ok(())
}
// Only two frozen benchmark manifests predate flexplan's addition to
// Cargo.lock. Their reports identify the original lockfile; the re-evaluated
// metrics and all source hashes must still agree exactly. Keep this exception
// in the CLI, outside the source-hashed scientific evaluators.
fn validate_pinned_benchmark_replay(
    archived: &Value,
    mut replay: Value,
    pinned_manifest_sha256: &str,
    generic_section: &str,
) -> Result<()> {
    if &replay == archived {
        return Ok(());
    }
    const OLD_LOCK_SHA256: &str =
        "3dd3d6ed4f97b166bb0510731d2d634ad1dc14d2b80757754a442c278db6d97a";
    ensure!(
        archived["manifest_sha256"].as_str() == Some(pinned_manifest_sha256)
            && replay["manifest_sha256"].as_str() == Some(pinned_manifest_sha256),
        "historical lock compatibility is restricted to a pinned frozen manifest"
    );
    let old_models = archived["models"]
        .as_array()
        .context("archived benchmark has no models")?;
    let new_models = replay["models"]
        .as_array_mut()
        .context("replayed benchmark has no models")?;
    ensure!(old_models.len() == new_models.len(), "benchmark model count changed");
    let current_lock = crate::evaluation::hash(include_bytes!("../Cargo.lock"));
    for (old, new) in old_models.iter().zip(new_models.iter_mut()) {
        ensure!(
            old[generic_section]["dependency_lock_sha256"].as_str() == Some(OLD_LOCK_SHA256),
            "unrecognized archived dependency lock"
        );
        ensure!(
            new[generic_section]["dependency_lock_sha256"].as_str()
                == Some(current_lock.as_str()),
            "replay dependency hash does not match compiled lockfile"
        );
        new[generic_section]["dependency_lock_sha256"] = json!(OLD_LOCK_SHA256);
    }
    ensure!(
        &replay == archived,
        "frozen benchmark differs from deterministic replay beyond its pinned dependency provenance"
    );
    Ok(())
}

#[cfg(test)]
mod historical_replay_tests {
    use super::*;

    #[test]
    fn permits_only_known_lock_provenance_and_preserves_all_metrics() {
        const OLD: &str =
            "3dd3d6ed4f97b166bb0510731d2d634ad1dc14d2b80757754a442c278db6d97a";
        let current = crate::evaluation::hash(include_bytes!("../Cargo.lock"));
        let archived = json!({"manifest_sha256":"pinned","models":[
            {"selective_generic":{"dependency_lock_sha256":OLD,"measured_risk":0.2}}
        ]});
        let replay = json!({"manifest_sha256":"pinned","models":[
            {"selective_generic":{"dependency_lock_sha256":current,"measured_risk":0.2}}
        ]});
        assert!(
            validate_pinned_benchmark_replay(&archived, replay.clone(), "pinned", "selective_generic")
                .is_ok()
        );
        let mut altered = archived.clone();
        altered["models"][0]["selective_generic"]["measured_risk"] = json!(0.5);
        assert!(
            validate_pinned_benchmark_replay(&altered, replay.clone(), "pinned", "selective_generic")
                .is_err()
        );
        let mut unknown = archived.clone();
        unknown["models"][0]["selective_generic"]["dependency_lock_sha256"] = json!("unknown");
        assert!(
            validate_pinned_benchmark_replay(&unknown, replay.clone(), "pinned", "selective_generic")
                .is_err()
        );
        assert!(
            validate_pinned_benchmark_replay(&archived, replay, "other", "selective_generic")
                .is_err()
        );
    }
}

pub fn run(args: &[String]) -> Result<()> {
    if args == ["benchmark", "--help"] {
        println!("metrology benchmark --input MANIFEST --output NEW_DIRECTORY\n\nThe manifest schema selects the benchmark:\n  triad-benchmark-1            ordered binary-classification stream (benchmarks/electricity/manifest.json)\n  triad-temporal-regression-1  hourly regression with prospective day-level trust decisions\n                               (benchmarks/bike-sharing/manifest.json)\nCalibration-only thresholds, paired temporal block bootstrap, deterministic replay.\nOutputs report.json and report.md. Run metrology validate --input NEW_DIRECTORY/report.json to replay.");
        return Ok(());
    }
    if args == ["evidence", "--help"] {
        println!("metrology evidence --input ASSESSMENT.json --output NEW_REPORT.json [--root DIR]\n\nClassifies each typed claim (schema triad-evidence-1) as supported_within_scope,\ninsufficient_evidence, contradicted or not_applicable using the explicit rules of\ndocs/metrology/evidence-sufficiency-method.md, and reports the standard-checks\nbaseline beside it. file_hash paths resolve against --root (default: current\ndirectory) and may not leave it. Unknown evidence never passes; attestations are\nreported but never relied upon; a matching hash shows byte consistency only.\nNo confidence score is produced. Example: examples/evidence-assessment.json");
        return Ok(());
    }
    if args == ["evaluate", "--help"] {
        println!("metrology evaluate --input PREDICTIONS.json --output NEW_REPORT.json\n\nSelective risk at target coverage (schema triad-metrology-1; example: examples/predictions.json).\nThe acceptance threshold is the ceil(n*c)-th smallest calibration-split score (ties accepted;\ncalibration labels unused). On the test split it reports realised coverage, selective risk\n(accepted excess-loss events / accepted), both counts, the retrospective risk-coverage curve\nand AURC (descriptive only), event Brier/ECE and whole-sequence or whole-cluster bootstrap\nintervals. Definition: docs/metrology/selective-risk-quickstart.md");
        return Ok(());
    }
    if args == ["evidence-batch", "--help"] {
        println!("metrology evidence-batch --input CASES.json --output NEW_DIRECTORY\n\nClassifies every case of a triad-evidence-outcomes-cases-1 file (Milestone 8,\nbenchmarks/evidence-outcomes) with TRIAD, standard checks C, C+scope and one\nablation per rule family. Writes NEW_DIRECTORY/classifications.json. Cases carry\nno ground truth; scoring is done by scripts/m8-score.py.");
        return Ok(());
    }
    if args == ["evidence-suite", "--help"] {
        println!("metrology evidence-suite --input SUITE.json --output NEW_DIRECTORY [--root DIR]\n\nRuns TRIAD, standard checks C, C+scope and per-family ablations against frozen\nground truth (benchmarks/evidence-validation/suite.json). Results describe an\nauthor-constructed suite only.");
        return Ok(());
    }
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        println!("{HELP}");
        return Ok(());
    }
    let mut input = None;
    let mut output = None;
    let mut root: Option<PathBuf> = None;
    for pair in args[1..].chunks(2) {
        ensure!(pair.len() == 2, "{} requires a value", pair[0]);
        match pair[0].as_str() {
            "--input" => {
                ensure!(input.is_none(), "duplicate --input");
                input = Some(PathBuf::from(&pair[1]));
            }
            "--output" => {
                ensure!(output.is_none(), "duplicate --output");
                output = Some(PathBuf::from(&pair[1]));
            }
            "--root" => {
                ensure!(root.is_none(), "duplicate --root");
                root = Some(PathBuf::from(&pair[1]));
            }
            other => bail!("unknown option {other}"),
        }
    }
    ensure!(
        root.is_none() || matches!(args[0].as_str(), "evidence" | "evidence-suite" | "validate"),
        "--root is only valid for evidence, evidence-suite and validate"
    );
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    match args[0].as_str() {
        "benchmark" => {
            let path = input.context("benchmark requires --input")?;
            let output = output.context("benchmark requires --output")?;
            let head: Value = serde_json::from_slice(&fs::read(&path)?)
                .context("benchmark manifest must be JSON")?;
            if head["schema_version"] == "triad-temporal-regression-1" {
                crate::metrology_temporal::run(&path, &output)?;
            } else {
                crate::metrology_benchmark::run(&path, &output)?;
            }
        }
        "evidence" => {
            let bytes = fs::read(input.context("evidence requires --input")?)?;
            let output = output.context("evidence requires --output")?;
            let a: crate::evidence::Assessment = serde_json::from_slice(&bytes)
                .context("invalid assessment JSON; see evidence --help")?;
            let report = crate::evidence::report(&a, Some(&root))?;
            write_new_json(&output, &json!({"report":report}))?;
            for f in report["findings"].as_array().unwrap() {
                println!(
                    "{}: {}",
                    f["claim_id"].as_str().unwrap(),
                    f["classification"].as_str().unwrap()
                );
            }
        }
        "evidence-batch" => {
            crate::evidence_batch::run(
                &input.context("evidence-batch requires --input")?,
                &output.context("evidence-batch requires --output")?,
            )?;
        }
        "evidence-suite" => {
            let bytes = fs::read(input.context("evidence-suite requires --input")?)?;
            let dir = output.context("evidence-suite requires --output")?;
            let suite: crate::evidence_suite::Suite =
                serde_json::from_slice(&bytes).context("invalid suite JSON")?;
            let report = crate::evidence_suite::evaluate(&suite, &root)?;
            fs::create_dir(&dir).context("evidence-suite requires a new output directory")?;
            write_new_json(&dir.join("report.json"), &json!({"report":report}))?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join("report.md"))?;
            file.write_all(crate::evidence_suite::markdown(&report).as_bytes())?;
            print!("{}", crate::evidence_suite::markdown(&report));
        }
        "adaptive" => {
            ensure!(
                input.is_none(),
                "adaptive uses frozen development cases, not --input"
            );
            crate::metrology_adaptive_study::run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &output.context("adaptive requires --output")?,
            )?;
        }
        "evaluate" => {
            let bytes = fs::read(input.context("evaluate requires --input")?)?;
            let data: Input = serde_json::from_slice(&bytes)
                .context("invalid predictions JSON; see docs/metrology/input-schema.json")?;
            write_report(
                &data,
                &output.context("evaluate requires --output")?,
                &crate::evaluation::hash(&bytes),
            )?;
        }
        "validate" => {
            ensure!(output.is_none(), "validate does not accept --output");
            let envelope: Value =
                serde_json::from_slice(&fs::read(input.context("validate requires --input")?)?)?;
            let report = envelope.get("report").context("missing report payload")?;
            match report["schema_version"].as_str() {
                Some("triad-benchmark-report-1") => {
                    let manifest: crate::metrology_benchmark::Manifest = serde_json::from_value(
                        report.get("manifest").context("missing benchmark manifest")?.clone(),
                    )?;
                    validate_pinned_benchmark_replay(
                        report,
                        crate::metrology_benchmark::evaluate(&manifest)?,
                        "2f82b63e07e251c19e3c4aa1ab5ced8a474222fd99b62a9c854ddf974c6aea31",
                        "conventional_and_selective",
                    )?;
                }
                Some("triad-temporal-regression-report-1") => {
                    let manifest: crate::metrology_temporal::Manifest = serde_json::from_value(
                        report.get("manifest").context("missing temporal manifest")?.clone(),
                    )?;
                    validate_pinned_benchmark_replay(
                        report,
                        crate::metrology_temporal::evaluate(&manifest)?,
                        "7c51d830c1dd81c6cf4bb841fa690f46f34bd6c1f8e85ebd123133e7de94f50a",
                        "selective_generic",
                    )?;
                }
                Some("triad-evidence-report-1") => {
                    crate::evidence::validate_report(report, Some(&root))?
                }
                Some("triad-evidence-suite-report-1") => {
                    crate::evidence_suite::validate_report(report, &root)?
                }
                _ => metrology::validate_report(report)?,
            }
            println!("Report metrics and configuration hash reproduce; scientific validity is not certified.");
        }
        "demo" => {
            ensure!(input.is_none(), "demo does not accept --input");
            let dir = output.context("demo requires --output")?;
            fs::create_dir(&dir).context("demo requires a new output directory")?;
            let (data, model) = crate::metrology_demo::generate();
            write_new_json(&dir.join("model.json"), &model)?;
            write_new_json(&dir.join("predictions.json"), &data)?;
            let raw = fs::read(dir.join("predictions.json"))?;
            write_report(
                &data,
                &dir.join("report.json"),
                &crate::evaluation::hash(&raw),
            )?;
        }
        "numerical" => {
            ensure!(
                input.is_none(),
                "numerical uses frozen v5 probes, not --input"
            );
            crate::metrology_numerical::run(&output.context("numerical requires --output")?)?;
        }
        "plan" => {
            let pilot = if let Some(path) = input {
                let data: Input = serde_json::from_slice(&fs::read(path)?)?;
                Some(crate::metrology_planning::pilot_summary(&data)?)
            } else {
                None
            };
            let mut result = crate::metrology_planning::run()?;
            result["independent_pilot"] = json!(pilot);
            write_new_json(&output.context("plan requires --output")?, &result)?;
        }
        other => bail!("unknown metrology command {other}; use metrology --help"),
    }
    Ok(())
}
