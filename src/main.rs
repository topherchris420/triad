use anyhow::{bail, Result};

fn main() -> Result<()> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match std::env::args().nth(1).as_deref() {
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
        _ => bail!("Usage: triad validate | baseline | audit | information | numerical | selective | geometry-screen | all"),
    }
    Ok(())
}
