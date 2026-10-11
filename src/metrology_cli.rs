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

const HELP: &str="TRIAD metrology (exploratory measurement)\n\n  metrology evaluate --input predictions.json --output report.json\n  metrology validate --input report.json\n  metrology demo --output NEW_DIRECTORY\n  metrology numerical --output NEW_DIRECTORY\n  metrology adaptive --output NEW_DIRECTORY\n  metrology plan --output planning.json\n\nOutputs are never overwritten. Numerical runs are expensive; demo does not run FEM.";

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
pub fn run(args: &[String]) -> Result<()> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        println!("{HELP}");
        return Ok(());
    }
    let mut input = None;
    let mut output = None;
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
            other => bail!("unknown option {other}"),
        }
    }
    match args[0].as_str() {
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
            metrology::validate_report(envelope.get("report").context("missing report payload")?)?;
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
