use anyhow::{bail, Context, Result};
use std::path::PathBuf;

const HELP: &str = "TRIAD — when should a sensor refuse to answer?

Usage: triad [--root PATH] COMMAND

Inspect the instrument and evidence:
  --help, -h       Show this guide
  --version, -V    Show package version
  audit           Describe frozen optimizer outcomes
  information     Compute local nuisance-adjusted information

Frozen baseline (writes baseline artifacts; preserve originals first):
  validate        Run numerical physics gates
  baseline        Generate and estimate the locked baseline
  all             Run validate, baseline, and audit

Versioned research (run in this order; refuses existing outputs):
  numerical       Quantify mesh, interpolation, and optimizer mismatch
  selective       Evaluate C2 versus D on independent sequences
  geometry-screen Screen equal-width four-electrode layouts

--root selects a repository/data root containing experiments/ and results/.
The default is the directory from which this binary was built.
Audit commands write derived reports; they do not modify frozen evidence.
New research commands do not establish scientific findings merely by running.";

fn main() -> Result<()> {
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
