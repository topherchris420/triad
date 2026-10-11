use anyhow::{bail, Context, Result};
use std::path::PathBuf;

const HELP: &str = "TRIAD — when should a sensor refuse to answer?

Usage: triad [--root PATH] COMMAND

Inspect the instrument and evidence:
  metrology --help Generic selective-risk evaluation and v5 development tools
  --help, -h       Show this guide
  --version, -V    Show package version
  audit           Describe frozen optimizer outcomes
  information     Compute local nuisance-adjusted information
  numerical-components  Diagnose FEM versus dictionary error on development probes

Frozen baseline (writes baseline artifacts; preserve originals first):
  validate        Run numerical physics gates
  baseline        Generate and estimate the locked baseline
  all             Run validate, baseline, and audit

Completed v3 research (archived; refuses existing outputs):
  numerical       Quantify mesh, interpolation, and optimizer mismatch
  selective       Evaluate C2 versus D on independent sequences
  geometry-screen Screen equal-width four-electrode layouts

Milestone 4 development diagnostic (does not evaluate new selectors):
  numerical-components  Test separate mesh/interpolation errors and local stencil

--root selects a repository/data root containing experiments/ and results/.
The default is the directory from which this binary was built.
Audit commands write derived reports; they do not modify frozen evidence.
New research commands do not establish scientific findings merely by running.";

fn main() -> Result<()> {
    let early: Vec<_> = std::env::args().skip(1).collect();
    if early.first().is_some_and(|a| a == "metrology") {
        return triad::metrology_cli::run(&early[1..]);
    }
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut command = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().context("--root requires a path")?),
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("triad {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if command.is_none() => command = Some(arg),
            _ => bail!("unexpected argument '{arg}'; use --help"),
        }
    }
    match command.as_deref() {
        None => println!("{HELP}"),
        Some("validate") => {
            triad::validation::run(&root)?;
        }
        Some("baseline") => triad::experiment::run(&root)?,
        Some("information") => triad::observability::run_information_audit(&root)?,
        Some("numerical-components") => triad::numerical_components::run(&root)?,
        Some("numerical") => triad::numerical_audit::run(&root)?,
        Some("selective") => triad::selective::run(&root)?,
        Some("geometry-screen") => triad::geometry_search::run(&root)?,
        Some("audit") => {
            triad::diagnostics::run(&root)?;
        }
        Some("all") => {
            triad::validation::run(&root)?;
            triad::experiment::run(&root)?;
            triad::diagnostics::run(&root)?;
        }
        Some(other) => bail!("unknown command '{other}'; use --help"),
    }
    Ok(())
}
