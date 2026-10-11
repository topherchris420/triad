//! Experiment B: claim-validity suite runner (benchmarks/evidence-validation).
//!
//! Runs TRIAD, baseline C and the pre-declared C+scope variant on identical
//! inputs, compares them with frozen ground truth, and reports per-family
//! ablations. Suite statistics describe this author-constructed suite only.
use crate::evidence::{self, Assessment, Class, Family, Finding, Outcome, FAMILIES};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub case_id: String,
    pub category: String,
    pub source: String,
    pub ground_truth: Class,
    pub ground_truth_dimension: Option<String>,
    pub rationale: String,
    pub assessment: Assessment,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub schema_version: String,
    pub suite_id: String,
    pub status: String,
    pub author_disclosure: String,
    pub cases: Vec<Case>,
}

pub const PRACTICAL_THRESHOLD: f64 = 0.2;
pub const FALSE_REJECTION_MARGIN: f64 = 0.05;

fn unsupported(c: Class) -> bool {
    matches!(c, Class::InsufficientEvidence | Class::Contradicted)
}

pub fn validate(s: &Suite) -> Result<()> {
    ensure!(
        s.schema_version == "triad-evidence-suite-1",
        "unsupported suite schema"
    );
    ensure!(!s.cases.is_empty(), "empty suite");
    let mut ids = BTreeSet::new();
    for c in &s.cases {
        ensure!(ids.insert(&c.case_id), "duplicate case {}", c.case_id);
        ensure!(
            c.assessment.claims.len() == 1,
            "case {} must contain exactly one claim",
            c.case_id
        );
        ensure!(
            unsupported(c.ground_truth) == c.ground_truth_dimension.is_some(),
            "case {}: unsupported ground truth requires a dimension and only then",
            c.case_id
        );
        evidence::validate(&c.assessment).with_context(|| format!("case {}", c.case_id))?;
    }
    Ok(())
}

fn rate(num: usize, den: usize) -> Value {
    if den == 0 {
        Value::Null
    } else {
        json!(num as f64 / den as f64)
    }
}

fn dims(f: &Finding) -> BTreeSet<String> {
    f.reasons
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Insufficient | Outcome::Contradict))
        .map(|r| r.dimension.clone())
        .collect()
}

fn score(cases: &[Case], predictions: &[Finding]) -> Value {
    let mut unsupported_n = 0;
    let mut detected = 0;
    let mut supported_n = 0;
    let mut false_rejections = 0;
    let mut correct = 0;
    let mut indeterminate = 0;
    let mut audit_complete = 0;
    let mut confusion: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (c, f) in cases.iter().zip(predictions) {
        let truth = serde_json::to_value(c.ground_truth).unwrap();
        let pred = serde_json::to_value(f.classification).unwrap();
        *confusion
            .entry(truth.as_str().unwrap().into())
            .or_default()
            .entry(pred.as_str().unwrap().into())
            .or_default() += 1;
        correct += usize::from(c.ground_truth == f.classification);
        indeterminate += usize::from(f.classification == Class::InsufficientEvidence);
        if unsupported(c.ground_truth) {
            unsupported_n += 1;
            if unsupported(f.classification) {
                detected += 1;
                if c.ground_truth_dimension
                    .as_ref()
                    .is_some_and(|d| dims(f).contains(d))
                {
                    audit_complete += 1;
                }
            }
        }
        if c.ground_truth == Class::SupportedWithinScope {
            supported_n += 1;
            false_rejections += usize::from(unsupported(f.classification));
        }
    }
    json!({
        "cases":cases.len(),
        "unsupported_cases":unsupported_n,"detected":detected,"detection_rate":rate(detected,unsupported_n),
        "valid_cases":supported_n,"false_rejections":false_rejections,"false_rejection_rate":rate(false_rejections,supported_n),
        "four_class_correct":correct,"four_class_accuracy":rate(correct,cases.len()),
        "four_class_accuracy_wilson95_suite_internal":evidence::wilson(correct,cases.len()),
        "indeterminate":indeterminate,"indeterminate_frequency":rate(indeterminate,cases.len()),
        "audit_complete":audit_complete,"audit_completeness":rate(audit_complete,detected),
        "confusion_truth_by_prediction":confusion,
    })
}

fn subsets(cases: &[Case], predictions: &[Finding], key: impl Fn(&Case) -> &str) -> Value {
    let mut groups: BTreeMap<&str, (Vec<Case>, Vec<Finding>)> = BTreeMap::new();
    for (c, f) in cases.iter().zip(predictions) {
        let g = groups.entry(key(c)).or_default();
        g.0.push(c.clone());
        g.1.push(f.clone());
    }
    groups
        .into_iter()
        .map(|(k, (c, f))| (k.to_string(), score(&c, &f)))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

pub fn evaluate(s: &Suite, root: &Path) -> Result<Value> {
    validate(s)?;
    let none = BTreeSet::new();
    let mut triad = Vec::new();
    let mut standard = Vec::new();
    let mut scoped = Vec::new();
    for c in &s.cases {
        triad.push(evidence::assess(&c.assessment, Some(root), &none)?.remove(0));
        standard.push(evidence::standard_checks(&c.assessment, Some(root), false)?.remove(0));
        scoped.push(evidence::standard_checks(&c.assessment, Some(root), true)?.remove(0));
    }
    let mut ablations = serde_json::Map::new();
    let mut ablation_classes: BTreeMap<Family, Vec<Class>> = BTreeMap::new();
    for fam in FAMILIES {
        let disabled = BTreeSet::from([fam]);
        let mut preds = Vec::new();
        for c in &s.cases {
            preds.push(evidence::assess(&c.assessment, Some(root), &disabled)?.remove(0));
        }
        ablation_classes.insert(fam, preds.iter().map(|f| f.classification).collect());
        let name = serde_json::to_value(fam)?.as_str().unwrap().to_string();
        ablations.insert(name, score(&s.cases, &preds));
    }
    let t = score(&s.cases, &triad);
    let c = score(&s.cases, &standard);
    let diff = t["detection_rate"].as_f64().unwrap() - c["detection_rate"].as_f64().unwrap();
    let frr_t = t["false_rejection_rate"].as_f64().unwrap();
    let frr_c = c["false_rejection_rate"].as_f64().unwrap();
    let met =
        diff >= PRACTICAL_THRESHOLD - 1e-12 && frr_t <= frr_c + FALSE_REJECTION_MARGIN + 1e-12;
    let mut only_triad = 0;
    let mut only_c = 0;
    let mut per_case = Vec::new();
    let mut discordant = Vec::new();
    for (i, case) in s.cases.iter().enumerate() {
        let td = unsupported(triad[i].classification);
        let cd = unsupported(standard[i].classification);
        if unsupported(case.ground_truth) {
            only_triad += usize::from(td && !cd);
            only_c += usize::from(cd && !td);
        }
        let responsible: Vec<_> = FAMILIES
            .iter()
            .filter(|f| ablation_classes[f][i] != triad[i].classification)
            .map(|f| serde_json::to_value(f).unwrap())
            .collect();
        if triad[i].classification != standard[i].classification {
            discordant.push(json!({"case_id":case.case_id,"ground_truth":case.ground_truth,"triad":triad[i].classification,"standard":standard[i].classification,"families_whose_removal_changes_triad":responsible}));
        }
        per_case.push(json!({
            "case_id":case.case_id,"category":case.category,"source":case.source,
            "ground_truth":case.ground_truth,"ground_truth_dimension":case.ground_truth_dimension,
            "triad":triad[i],"standard":standard[i],"standard_plus_scope":scoped[i].classification,
            "ablations":FAMILIES.iter().map(|f| (serde_json::to_value(f).unwrap().as_str().unwrap().to_string(), json!(ablation_classes[f][i]))).collect::<serde_json::Map<_,_>>(),
        }));
    }
    let supplied_dependent = triad
        .iter()
        .filter(|f| {
            f.classification == Class::SupportedWithinScope
                && f.reasons
                    .iter()
                    .any(|r| r.basis == evidence::Basis::Supplied && r.outcome == Outcome::Support)
        })
        .count();
    Ok(json!({
        "schema_version":"triad-evidence-suite-report-1",
        "suite_id":s.suite_id,
        "suite_sha256":crate::evaluation::hash(&serde_json::to_vec(s)?),
        "evaluator_source_sha256":{
            "evidence.rs":crate::evaluation::hash(include_bytes!("evidence.rs")),
            "evidence_suite.rs":crate::evaluation::hash(include_bytes!("evidence_suite.rs")),
        },
        "suite":s,
        "methods":{"triad":t,"standard_checks_c":c,"standard_checks_c_plus_scope":score(&s.cases,&scoped)},
        "primary":{
            "endpoint":"detection rate (TRIAD) minus detection rate (C) over unsupported cases",
            "difference":diff,"practical_threshold":PRACTICAL_THRESHOLD,
            "false_rejection_triad":frr_t,"false_rejection_c":frr_c,"false_rejection_margin":FALSE_REJECTION_MARGIN,
            "criterion_met":met,
            "discordant_detection_pairs":{"triad_only":only_triad,"c_only":only_c},
            "mcnemar_exact_two_sided_suite_internal":evidence::mcnemar_exact(only_triad,only_c),
            "interpretation":"Engineering detection capability on an author-constructed suite; not general real-world scientific validity and not a population inference."
        },
        "by_source":{"triad":subsets(&s.cases,&triad,|c|&c.source),"standard_checks_c":subsets(&s.cases,&standard,|c|&c.source)},
        "by_category":{"triad":subsets(&s.cases,&triad,|c|&c.category),"standard_checks_c":subsets(&s.cases,&standard,|c|&c.category)},
        "ablations_triad_without_family":ablations,
        "discordant_cases":discordant,
        "reviewer_effort":{"measured":false,"proxy_supported_findings_relying_on_supplied_data":supplied_dependent},
        "cases":per_case,
    }))
}

pub fn validate_report(report: &Value, root: &Path) -> Result<()> {
    let s: Suite = serde_json::from_value(report.get("suite").context("missing suite")?.clone())?;
    ensure!(
        &evaluate(&s, root)? == report,
        "suite report differs from deterministic replay"
    );
    Ok(())
}

pub fn markdown(report: &Value) -> String {
    let m = &report["methods"];
    let p = &report["primary"];
    let mut md = String::from("# Claim-validity suite (Experiment B)\n\nAuthor-constructed suite; engineering detection capability only.\n\n| Method | Detection of unsupported claims | False rejection of valid claims | Four-class accuracy | Indeterminate | Audit completeness |\n|---|---:|---:|---:|---:|---:|\n");
    for (name, key) in [
        ("TRIAD", "triad"),
        ("Standard checks C", "standard_checks_c"),
        ("C + scope rule", "standard_checks_c_plus_scope"),
    ] {
        let s = &m[key];
        md.push_str(&format!(
            "| {name} | {}/{} | {}/{} | {}/{} | {} | {}/{} |\n",
            s["detected"],
            s["unsupported_cases"],
            s["false_rejections"],
            s["valid_cases"],
            s["four_class_correct"],
            s["cases"],
            s["indeterminate"],
            s["audit_complete"],
            s["detected"]
        ));
    }
    md.push_str(&format!(
        "\nPrimary difference {} (threshold {}), criterion met: {}. Exact McNemar (suite-internal) p = {}.\n\nSee report.json for every case, reason, ablation and discordant-case attribution.\n",
        p["difference"], p["practical_threshold"], p["criterion_met"], p["mcnemar_exact_two_sided_suite_internal"]
    ));
    md
}
