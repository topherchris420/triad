//! Post-hoc audit of the *frozen* always-answer baseline.
//! Reads saved evaluated predictions. It is not a rejection algorithm,
//! does not re-fit the model, and must not tune thresholds on test labels.
use crate::{evaluation::hash, experiment::EvaluationRow};
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

fn statistics(rows: &[&EvaluationRow]) -> Value {
    let n = rows.len();
    if n == 0 {
        return json!({
            "count": 0,
            "over_5mm_count": 0,
            "over_5mm_rate": null,
            "mean_error_mm": null,
            "mean_normalized_residual": null
        });
    }
    let over = rows.iter().filter(|r| r.error_mm > 5.0).count();
    json!({
        "count": n,
        "over_5mm_count": over,
        "over_5mm_rate": over as f64 / n as f64,
        "mean_error_mm": rows.iter().map(|r| r.error_mm).sum::<f64>() / n as f64,
        "mean_normalized_residual": rows.iter().map(|r| r.residual).sum::<f64>() / n as f64
    })
}

fn partition(rows: &[&EvaluationRow]) -> Value {
    let flagged: Vec<_> = rows.iter().copied().filter(|r| !r.converged).collect();
    let unflagged: Vec<_> = rows.iter().copied().filter(|r| r.converged).collect();
    json!({
        "total": statistics(rows),
        "nonconverged": statistics(&flagged),
        "converged": statistics(&unflagged)
    })
}

/// Descriptive retrospective diagnostics, never a claim of calibrated reliability.
pub fn audit(rows: &[EvaluationRow], evaluation_sha256: &str) -> Value {
    let all: Vec<_> = rows.iter().collect();
    let scenarios: BTreeSet<_> = rows.iter().map(|r| r.scenario.as_str()).collect();
    let mut by_scenario = serde_json::Map::new();
    for scenario in scenarios {
        let subset: Vec<_> = all
            .iter()
            .copied()
            .filter(|r| r.scenario == scenario)
            .collect();
        by_scenario.insert(scenario.to_owned(), partition(&subset));
    }
    let missed_by_flag = rows.iter().filter(|r| r.error_mm > 5.0 && r.converged).count();
    json!({
        "analysis_kind": "retrospective_exploratory_optimizer_flag_audit",
        "source": "results/evaluation.json",
        "source_sha256": evaluation_sha256,
        "error_tolerance_mm": 5.0,
        "all": partition(&all),
        "large_errors_with_converged_flag": missed_by_flag,
        "scenarios": by_scenario,
        "limitations": [
            "Uses saved test labels only AFTER the baseline predictions were frozen.",
            "Convergence is an algorithmic stopping indicator, not calibrated physical reliability.",
            "A low residual or converged fit does not prove correct localization or global optimality.",
            "This report does not select an abstention threshold or establish selective-prediction improvement.",
            "Any optimizer modifications must be evaluated on newly frozen, untouched sequences."
        ]
    })
}

pub fn run(root: &Path) -> Result<Value> {
    let bytes = std::fs::read(root.join("results/evaluation.json"))?;
    let rows: Vec<EvaluationRow> = serde_json::from_slice(&bytes)?;
    ensure!(!rows.is_empty(), "no frozen evaluation rows available");
    ensure!(
        rows.iter()
            .all(|r| r.error_mm.is_finite() && r.residual.is_finite() && r.error_mm >= 0.0),
        "non-finite or invalid evaluation rows"
    );
    let report = audit(&rows, &hash(&bytes));
    let out = root.join("results/optimizer-audit.json");
    std::fs::write(out, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(report)
}
