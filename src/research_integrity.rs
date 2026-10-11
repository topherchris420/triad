//! Fail-closed protocol checks and append-only research artifacts.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::Write, path::Path};

/// Verify every executable setting, including seeds and the decision rule.
/// Changes require an explicitly versioned study, not a silent protocol edit.
pub fn validate_selective_protocol(cfg: &Value) -> Result<()> {
    let expected = json!({
        "protocol_version": "3.0-independent-selective",
        "primary_scenario": "strong_drift",
        "error_threshold_mm": 5.0,
        "target_coverage": 0.8,
        "sequence_counts": {"development":64,"calibration":64,"test":300},
        "frames_per_sequence":12,
        "calibration_frames":24,
        "positions_per_split":24,
        "development_seed":9301101,
        "calibration_seed":9301102,
        "test_seed":9301103,
        "independent_positions_per_split":true,
        "observation_fem_spacing_m":0.0005,
        "dictionary_fem_spacing_m":0.001,
        "posterior_temperature":2.0,
        "d_physics_weight":0.25,
        "bootstrap":{"paired":true,"unit":"complete strong-drift sequence","replicates":500,"seed":901773},
        "practical_risk_improvement_absolute":0.02,
        "minimum_test_failures":30,
        "no_hardware_claims":true
    });
    for (key, value) in expected.as_object().unwrap() {
        ensure!(cfg.get(key) == Some(value),
            "source and frozen protocol disagree at '{key}'; create a new versioned study to change executable settings");
    }
    Ok(())
}

/// Exclusive creation prevents concurrent writers and accidental replacement.
/// A failed write remains visible; never silently erase incomplete evidence.
pub fn write_new_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("refuse to replace research artifact: {}", path.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}
