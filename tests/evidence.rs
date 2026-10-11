//! Rule-level and adversarial tests of the claim-relative evidence engine.
//! These follow docs/metrology/evidence-sufficiency-method.md, not the suite.
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use triad::evidence::*;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
fn parse(v: serde_json::Value) -> Assessment {
    serde_json::from_value(v).unwrap()
}
fn class(v: serde_json::Value) -> Class {
    assess(&parse(v), Some(root()), &BTreeSet::new()).unwrap()[0].classification
}
fn metric(n: u64, events: u64, dep: &str, conditions: &[&str]) -> serde_json::Value {
    json!({"id":"m","kind":"metric_report","metric":"error_rate","population":"p","conditions":conditions,"unit":"u","n":n,"events":events,"value":null,"dependence":dep,"interval":null,"design_effect":null,"source":"test"})
}
fn bound_claim(bound: f64, conditions: &[&str]) -> serde_json::Value {
    json!({"type":"metric_bound","id":"c","statement":"s","evidence":["m"],"metric":"error_rate","population":"p","conditions":conditions,"comparator":"at_most","bound":bound,"confidence":0.95,"claimed_value":null,"tolerance":0.0})
}
fn doc(evidence: Vec<serde_json::Value>, claim: serde_json::Value) -> serde_json::Value {
    json!({"schema_version":"triad-evidence-1","assessment_id":"t","evidence":evidence,"claims":[claim]})
}

#[test]
fn clopper_pearson_matches_closed_forms_and_tables() {
    let (lo, hi) = clopper_pearson(0.0, 40.0, 0.95);
    assert_eq!(lo, 0.0);
    assert!((hi - (1.0 - 0.05f64.powf(1.0 / 40.0))).abs() < 1e-12);
    // Two-sided 95% exact interval for 5/10 is [0.187086, 0.812914].
    let (lo, hi) = clopper_pearson(5.0, 10.0, 0.975);
    assert!((lo - 0.187_086_028).abs() < 1e-8 && (hi - 0.812_913_972).abs() < 1e-8);
    assert!((beta_quantile(0.5, 2.0, 2.0) - 0.5).abs() < 1e-12);
    assert_eq!(clopper_pearson(10.0, 10.0, 0.95).1, 1.0);
    assert!((mcnemar_exact(0, 6) - 0.03125).abs() < 1e-12);
    let w = wilson(5, 10).unwrap();
    assert!((w[0] - 0.236_593).abs() < 1e-5 && (w[1] - 0.763_407).abs() < 1e-5);
}

#[test]
fn unknown_null_or_attested_evidence_never_supports() {
    // F3: missing evidence.
    assert_eq!(
        class(doc(
            vec![],
            json!({"type":"no_leakage","id":"c","statement":"s","evidence":[],"task":"forecast"})
        )),
        Class::InsufficientEvidence
    );
    // Attestation only.
    let att = json!({"id":"a","kind":"attestation","statement":"fine","value":true,"attester":"x"});
    for claim in [
        json!({"type":"no_leakage","id":"c","statement":"s","evidence":["a"],"task":"forecast"}),
        json!({"type":"independence","id":"c","statement":"s","evidence":["a"],"unit":"hour"}),
    ] {
        assert_eq!(
            class(doc(vec![att.clone()], claim)),
            Class::InsufficientEvidence
        );
    }
    // Null numerical check.
    let checks = json!({"id":"n","kind":"numerical_checks","component":"f","checks":{"mesh":true,"solver":null}});
    let claim = json!({"type":"numerical_adequacy","id":"c","statement":"s","evidence":["n"],"component":"f","required_checks":["mesh","solver"]});
    assert_eq!(class(doc(vec![checks], claim)), Class::InsufficientEvidence);
    // Unknown dependence.
    assert_eq!(
        class(doc(
            vec![metric(10_000, 0, "unknown", &["a"])],
            bound_claim(0.01, &["a"])
        )),
        Class::InsufficientEvidence
    );
    // A claim with no rule able to support it (all families ablated) stays insufficient.
    let all: BTreeSet<_> = FAMILIES.into_iter().collect();
    let a = parse(doc(
        vec![att],
        json!({"type":"independence","id":"c","statement":"s","evidence":["a"],"unit":"hour"}),
    ));
    let mut disabled = all.clone();
    disabled.remove(&Family::AttestationPolicy);
    assert_eq!(
        assess(&a, None, &disabled).unwrap()[0].classification,
        Class::InsufficientEvidence
    );
}

#[test]
fn claim_relative_scope_and_rare_events() {
    let ev = vec![metric(400, 0, "independent", &["night"])];
    assert_eq!(
        class(doc(ev.clone(), bound_claim(0.01, &["night"]))),
        Class::SupportedWithinScope
    );
    assert_eq!(
        class(doc(ev.clone(), bound_claim(0.01, &["2013"]))),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(doc(
            vec![metric(40, 0, "independent", &["night"])],
            bound_claim(0.01, &["night"])
        )),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(doc(
            vec![metric(400, 300, "independent", &["night"])],
            bound_claim(0.01, &["night"])
        )),
        Class::Contradicted
    );
    // Temporal units need a design effect or dependence-aware interval.
    assert_eq!(
        class(doc(
            vec![metric(400, 0, "temporal", &["night"])],
            bound_claim(0.01, &["night"])
        )),
        Class::InsufficientEvidence
    );
    let mut m = metric(2000, 2, "temporal", &["night"]);
    m["design_effect"] = json!(2.0);
    assert_eq!(
        class(doc(vec![m], bound_claim(0.01, &["night"]))),
        Class::SupportedWithinScope
    );
    // Universal claims are never supported.
    let u = json!({"type":"universal_reliability","id":"c","statement":"s","evidence":["m"],"metric":"error_rate","reliability_bound":0.5,"confidence":0.95});
    assert_eq!(class(doc(ev, u)), Class::InsufficientEvidence);
}

#[test]
fn hashes_establish_consistency_and_authority_only() {
    let path = "benchmarks/electricity/protocol.json";
    let good = triad::evaluation::hash(&std::fs::read(root().join(path)).unwrap());
    let mk = |sha: &str, authority: &str, required: &str| {
        doc(
            vec![
                json!({"id":"h","kind":"file_hash","path":path,"expected_sha256":sha,"authority":authority}),
            ],
            json!({"type":"provenance","id":"c","statement":"s","evidence":["h"],"artifact":path,"authority_required":required}),
        )
    };
    assert_eq!(
        class(mk(&good, "repository_record", "repository_record")),
        Class::SupportedWithinScope
    );
    assert_eq!(
        class(mk(&good, "self_reported", "external_source")),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(mk(&"0".repeat(64), "external_source", "external_source")),
        Class::Contradicted
    );
    // Broken integrity makes dependent claims insufficient in either direction.
    let mut ev = vec![metric(100, 90, "independent", &["a"])];
    ev.push(json!({"id":"h","kind":"file_hash","path":path,"expected_sha256":"0".repeat(64),"authority":"repository_record"}));
    let mut claim = bound_claim(0.01, &["a"]);
    claim["evidence"] = json!(["m", "h"]);
    assert_eq!(class(doc(ev, claim)), Class::InsufficientEvidence);
    // Paths may not escape the root.
    let bad = mk(&good, "repository_record", "repository_record");
    let mut bad = bad;
    bad["evidence"][0]["path"] = json!("../outside.json");
    assert!(validate(&parse(bad)).is_err());
}

#[test]
fn strict_schema_and_linkage() {
    let mut v = doc(
        vec![metric(10, 1, "independent", &["a"])],
        bound_claim(0.5, &["a"]),
    );
    v["evidence"][0]["certified"] = json!(true);
    assert!(serde_json::from_value::<Assessment>(v).is_err());
    let mut v = doc(
        vec![metric(10, 1, "independent", &["a"])],
        bound_claim(0.5, &["a"]),
    );
    v["claims"][0]["evidence"] = json!(["missing"]);
    assert!(validate(&parse(v)).is_err());
    let mut v = doc(
        vec![metric(10, 1, "independent", &["a"])],
        bound_claim(0.5, &["a"]),
    );
    v["claims"][0]["confidence"] = json!(1.0);
    assert!(validate(&parse(v)).is_err());
    let mut v = doc(
        vec![metric(10, 1, "independent", &["a"])],
        bound_claim(0.5, &["a"]),
    );
    v["schema_version"] = json!("triad-evidence-0");
    assert!(validate(&parse(v)).is_err());
}

#[test]
fn not_applicable_leakage_and_standard_baseline_differences() {
    let sys =
        json!({"id":"s","kind":"system_description","component":"ridge","numerical_solver":false});
    let claim = json!({"type":"numerical_adequacy","id":"c","statement":"s","evidence":["s"],"component":"ridge","required_checks":["mesh"]});
    let a = parse(doc(vec![sys], claim));
    assert_eq!(
        assess(&a, None, &BTreeSet::new()).unwrap()[0].classification,
        Class::NotApplicable
    );
    assert_eq!(
        standard_checks(&a, None, false).unwrap()[0].classification,
        Class::InsufficientEvidence
    );
    // Target reconstruction is contradicted even when an attestation says otherwise.
    let ev = vec![
        json!({"id":"t","kind":"feature_timing","origin":"o","features":[{"name":"casual","available_offset_hours":0.0},{"name":"registered","available_offset_hours":0.0}]}),
        json!({"id":"p","kind":"partition","ordering":"temporal","splits":[{"name":"training","start":"2011-01-01","end":"2011-12-31"},{"name":"test","start":"2012-01-01","end":"2012-12-31"}],"training_split":"training","evaluation_split":"test","preprocessing_fit_splits":["training"]}),
        json!({"id":"k","kind":"target_construction","target":"cnt","identities":[{"features":["casual","registered"],"relation":"sum","rows":10,"max_abs_residual":0.0}]}),
        json!({"id":"a","kind":"attestation","statement":"no leakage","value":true,"attester":"x"}),
    ];
    let a = parse(doc(
        ev,
        json!({"type":"no_leakage","id":"c","statement":"s","evidence":["t","p","k","a"],"task":"forecast"}),
    ));
    let f = &assess(&a, None, &BTreeSet::new()).unwrap()[0];
    assert_eq!(f.classification, Class::Contradicted);
    assert_eq!(f.attestations_not_relied_upon, vec!["a".to_string()]);
    assert_eq!(
        standard_checks(&a, None, false).unwrap()[0].classification,
        Class::SupportedWithinScope
    );
}

#[test]
fn operational_risk_rules_and_report_replay() {
    let mk = |condition: &str, value: f64, available: u64, outcomes: Vec<bool>| {
        doc(
            vec![
                json!({"id":"cal","kind":"calibration","population":"cal","conditions":["working day","holiday"],"condition_counts":{"working day":80,"holiday":2},"n":82,"unit":"day","dependence":"temporal","method":"q"}),
                json!({"id":"s","kind":"support_check","statistic":"ks","value":value,"support_max":0.5}),
                json!({"id":"c","kind":"completeness","required":22,"available":available}),
                json!({"id":"r","kind":"outcome_records","metric":"accepted_risk","population":"previous day","conditions":["previous day"],"unit":"hour","dependence":"temporal","outcomes":outcomes,"design_effect":1.5}),
            ],
            json!({"type":"operational_risk","id":"k","statement":"s","evidence":["cal","s","c","r"],"population":"today","recent_population":"previous day","condition":condition,"bound":0.25,"confidence":0.95,"min_condition_support":5}),
        )
    };
    let calm = vec![false; 18]
        .into_iter()
        .chain([true; 2])
        .collect::<Vec<_>>();
    assert_eq!(
        class(mk("working day", 0.2, 24, calm.clone())),
        Class::SupportedWithinScope
    );
    assert_eq!(
        class(mk("holiday", 0.2, 24, calm.clone())),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(mk("working day", 0.9, 24, calm.clone())),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(mk("working day", 0.2, 10, calm.clone())),
        Class::InsufficientEvidence
    );
    assert_eq!(
        class(mk("working day", 0.2, 24, vec![true; 20])),
        Class::Contradicted
    );
    // Empty recent outcomes cannot contradict; support still comes from calibration.
    assert_eq!(
        class(mk("working day", 0.2, 24, vec![])),
        Class::SupportedWithinScope
    );
    let a = parse(mk("working day", 0.2, 24, calm));
    let mut r = report(&a, Some(root())).unwrap();
    validate_report(&r, Some(root())).unwrap();
    r["findings"][0]["classification"] = json!("contradicted");
    assert!(validate_report(&r, Some(root())).is_err());
}
