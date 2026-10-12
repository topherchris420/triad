//! Milestone 8: batch classification of outcome-grounded claim cases.
//!
//! Runs the unchanged Milestone 7 rules (`evidence::assess`), baseline C and
//! C+scope, and one ablation per rule family, on every case of a cases file
//! (benchmarks/evidence-outcomes). Ground truth never enters this module: the
//! cases file carries no truth, and scoring happens elsewhere.
use crate::evidence::{self, Assessment, Family, Finding, Outcome, FAMILIES};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case_id: String,
    pub assessment: Assessment,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cases {
    pub schema_version: String,
    pub cases: Vec<Case>,
}

pub fn validate(c: &Cases) -> Result<()> {
    ensure!(
        c.schema_version == "triad-evidence-outcomes-cases-1",
        "unsupported cases schema"
    );
    ensure!(!c.cases.is_empty(), "no cases");
    let mut ids = BTreeSet::new();
    for case in &c.cases {
        ensure!(ids.insert(&case.case_id), "duplicate case {}", case.case_id);
        ensure!(
            case.assessment.claims.len() == 1,
            "case {} must contain exactly one claim",
            case.case_id
        );
        evidence::validate(&case.assessment).with_context(|| format!("case {}", case.case_id))?;
    }
    Ok(())
}

fn class(f: &Finding) -> Value {
    serde_json::to_value(f.classification).unwrap()
}

/// Families whose rules returned insufficient, contradict or not_applicable.
fn blocking(f: &Finding) -> Vec<Value> {
    let set: BTreeSet<(Family, String, String)> = f
        .reasons
        .iter()
        .filter(|r| {
            matches!(
                r.outcome,
                Outcome::Insufficient | Outcome::Contradict | Outcome::NotApplicable
            )
        })
        .map(|r| {
            (
                r.family,
                r.dimension.clone(),
                serde_json::to_value(r.outcome)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
        })
        .collect();
    set.into_iter()
        .map(|(family, dimension, outcome)| json!({"family":family,"dimension":dimension,"outcome":outcome}))
        .collect()
}

pub fn evaluate(cases: &Cases, input_sha256: &str) -> Result<Value> {
    validate(cases)?;
    let none = BTreeSet::new();
    let mut results = Vec::new();
    let mut counts: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    for case in &cases.cases {
        let a = &case.assessment;
        let triad = &evidence::assess(a, None, &none)?[0];
        let c = &evidence::standard_checks(a, None, false)?[0];
        let cs = &evidence::standard_checks(a, None, true)?[0];
        let mut ablations = BTreeMap::new();
        for f in FAMILIES {
            let off = BTreeSet::from([f]);
            let finding = &evidence::assess(a, None, &off)?[0];
            ablations.insert(
                serde_json::to_value(f)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
                class(finding),
            );
        }
        // Distinctness (F4): only the scope, dependence and statistical families (plus
        // the always-on evidence-presence rules) remain enabled.
        let core_off: BTreeSet<Family> = FAMILIES
            .into_iter()
            .filter(|f| !matches!(f, Family::Scope | Family::Dependence | Family::Statistical))
            .collect();
        let core = &evidence::assess(a, None, &core_off)?[0];
        for (name, f) in [("triad", triad), ("c", c), ("c_scope", cs)] {
            *counts
                .entry(name)
                .or_default()
                .entry(class(f).as_str().unwrap().to_string())
                .or_default() += 1;
        }
        results.push(json!({
            "case_id":case.case_id,
            "triad":class(triad),
            "c":class(c),
            "c_scope":class(cs),
            "triad_core":class(core),
            "triad_blocking":blocking(triad),
            "ablations":ablations,
        }));
    }
    Ok(json!({
        "schema_version":"triad-evidence-batch-report-1",
        "software_version":env!("CARGO_PKG_VERSION"),
        "evaluator_source_sha256":{
            "evidence.rs":crate::evaluation::hash(include_bytes!("evidence.rs")),
            "evidence_batch.rs":crate::evaluation::hash(include_bytes!("evidence_batch.rs")),
        },
        "input_sha256":input_sha256,
        "cases":cases.cases.len(),
        "classification_counts":counts,
        "results":results,
        "interpretation":"Classifications only. Ground truth is not an input to this report; see the Milestone 8 scorer. Supported within scope is not proof that a claim is true.",
    }))
}

pub fn run(input: &Path, output: &Path) -> Result<()> {
    let bytes = fs::read(input).with_context(|| format!("cannot read {}", input.display()))?;
    let cases: Cases = serde_json::from_slice(&bytes)
        .context("invalid cases JSON (schema triad-evidence-outcomes-cases-1)")?;
    let report = evaluate(&cases, &crate::evaluation::hash(&bytes))?;
    fs::create_dir(output).context("evidence-batch requires a new output directory")?;
    let mut file = fs::File::create_new(output.join("classifications.json"))?;
    file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
    file.write_all(b"\n")?;
    let counts = &report["classification_counts"];
    println!(
        "{} cases classified\n{}",
        report["cases"],
        serde_json::to_string_pretty(counts)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str, dependence: &str, conditions: &[&str]) -> Case {
        let a: Assessment = serde_json::from_value(json!({
            "schema_version":"triad-evidence-1","assessment_id":id,
            "evidence":[{"kind":"outcome_records","id":"s","metric":"material_error_rate",
                "population":"p","conditions":["m1","m2"],"unit":"hour","dependence":dependence,
                "outcomes":vec![false;400],"design_effect":null}],
            "claims":[{"type":"metric_bound","id":"claim","statement":"x","evidence":["s"],
                "metric":"material_error_rate","population":"p","conditions":conditions,
                "comparator":"at_most","bound":0.02,"confidence":0.95,"claimed_value":null,"tolerance":0.0}]
        }))
        .unwrap();
        Case {
            case_id: id.into(),
            assessment: a,
        }
    }

    #[test]
    fn batch_runs_triad_baselines_and_ablations() {
        let cases = Cases {
            schema_version: "triad-evidence-outcomes-cases-1".into(),
            cases: vec![
                case("iid", "independent", &["m1", "m2"]),
                case("clustered", "clustered", &["m1", "m2"]),
                case("scope", "independent", &["m3"]),
            ],
        };
        let r = evaluate(&cases, "x").unwrap();
        let res = r["results"].as_array().unwrap();
        // 0/400 i.i.d.: one-sided 95% upper bound 0.0075 <= 0.02.
        assert_eq!(res[0]["triad"], "supported_within_scope");
        assert_eq!(res[0]["c"], "supported_within_scope");
        // Clustered without a design effect: TRIAD fails closed, C does not.
        assert_eq!(res[1]["triad"], "insufficient_evidence");
        assert_eq!(res[1]["c"], "supported_within_scope");
        assert_eq!(res[1]["ablations"]["dependence"], "supported_within_scope");
        // Unevaluated stratum: scope-aware methods withhold support.
        assert_eq!(res[2]["triad"], "insufficient_evidence");
        assert_eq!(res[2]["c_scope"], "insufficient_evidence");
        assert_eq!(res[2]["c"], "supported_within_scope");
        assert_eq!(res[2]["triad_blocking"][0]["family"], "scope");
        assert_eq!(evaluate(&cases, "x").unwrap(), r);
    }

    #[test]
    fn rejects_multi_claim_and_duplicate_cases() {
        let mut c = case("a", "independent", &["m1"]);
        let mut cases = Cases {
            schema_version: "triad-evidence-outcomes-cases-1".into(),
            cases: vec![c.clone(), c.clone()],
        };
        assert!(validate(&cases).is_err());
        let claim = c.assessment.claims[0].clone();
        c.assessment.claims.push(claim);
        cases.cases = vec![c];
        assert!(validate(&cases).is_err());
    }
}
