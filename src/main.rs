use anyhow::{bail, Result};

fn main() -> Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match std::env::args().nth(1).as_deref() {
        Some("validate") => {
            triad::validation::run(&root)?;
        }
        Some("baseline") => triad::experiment::run(&root)?,
        Some("audit") => {
            triad::diagnostics::run(&root)?;
        }
        Some("all") => {
            triad::validation::run(&root)?;
            triad::experiment::run(&root)?;
            triad::diagnostics::run(&root)?;
        }
        _ => bail!("Usage: triad validate | baseline | audit | all"),
    }
    Ok(())
}
