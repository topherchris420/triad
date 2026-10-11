use serde_json::{json, Value};
use triad::research_integrity::{validate_selective_protocol, write_new_json};

fn protocol() -> Value {
    serde_json::from_str(include_str!("../experiments/selective-v3.json")).unwrap()
}

#[test]
fn committed_protocol_matches_executable_study() {
    validate_selective_protocol(&protocol()).unwrap();
}

#[test]
fn every_executable_setting_rejects_mutation_and_omission() {
    let pointers = [
        "/protocol_version",
        "/primary_scenario",
        "/error_threshold_mm",
        "/target_coverage",
        "/sequence_counts/development",
        "/sequence_counts/calibration",
        "/sequence_counts/test",
        "/frames_per_sequence",
        "/calibration_frames",
        "/positions_per_split",
        "/development_seed",
        "/calibration_seed",
        "/test_seed",
        "/independent_positions_per_split",
        "/observation_fem_spacing_m",
        "/dictionary_fem_spacing_m",
        "/posterior_temperature",
        "/d_physics_weight",
        "/bootstrap/paired",
        "/bootstrap/unit",
        "/bootstrap/replicates",
        "/bootstrap/seed",
        "/practical_risk_improvement_absolute",
        "/minimum_test_failures",
        "/no_hardware_claims",
    ];
    for pointer in pointers {
        let mut changed = protocol();
        *changed.pointer_mut(pointer).unwrap() = json!("changed");
        assert!(validate_selective_protocol(&changed).is_err(), "{pointer}");
        let mut missing = protocol();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        missing
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(validate_selective_protocol(&missing).is_err(), "{pointer}");
    }
}

#[test]
fn artifact_writer_never_replaces_existing_evidence() {
    let path = std::env::temp_dir().join(format!("triad-integrity-{}.json", std::process::id()));
    write_new_json(&path, &json!({"evidence":"original"})).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(write_new_json(&path, &json!({"evidence":"replacement"})).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
}

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("triad-{name}-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("experiments")).unwrap();
        std::fs::create_dir(root.join("results")).unwrap();
        std::fs::write(
            root.join("experiments/selective-v3.json"),
            include_bytes!("../experiments/selective-v3.json"),
        )
        .unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn incomplete_selective_run_is_preserved_without_regeneration() {
    let fixture = Fixture::new("interrupted");
    let output = fixture.0.join("results/selective-v3");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("test-scores.json"), b"exposed evidence").unwrap();
    let error = triad::selective::run(&fixture.0).unwrap_err().to_string();
    assert!(error.contains("possibly incomplete"), "{error}");
    assert_eq!(
        std::fs::read(output.join("test-scores.json")).unwrap(),
        b"exposed evidence"
    );
    assert!(!output.join("summary.json").exists());
}

#[test]
fn numerical_audit_refuses_existing_report_before_computation() {
    let fixture = Fixture::new("numerical");
    let output = fixture.0.join("results/numerical-audit-v3.json");
    std::fs::write(&output, b"previous evidence").unwrap();
    assert!(triad::numerical_audit::run(&fixture.0)
        .unwrap_err()
        .to_string()
        .contains("refuse overwrite"));
    assert_eq!(std::fs::read(output).unwrap(), b"previous evidence");
}

#[test]
fn geometry_rejects_tampered_selective_evidence_before_computation() {
    let fixture = Fixture::new("tampered");
    let output = fixture.0.join("results/selective-v3");
    std::fs::create_dir(&output).unwrap();
    let mut hashes = json!({});
    for name in ["run.json", "test-scores.json", "evaluation.json"] {
        std::fs::write(output.join(name), b"{}").unwrap();
        hashes[name] = json!(triad::evaluation::hash(b"{}"));
    }
    write_new_json(&output.join("summary.json"), &json!({
        "experiment":"selective-v3",
        "protocol_sha256":triad::evaluation::hash(include_bytes!("../experiments/selective-v3.json")),
        "artifact_sha256":hashes
    })).unwrap();
    std::fs::write(output.join("test-scores.json"), b"changed").unwrap();
    let error = triad::geometry_search::run(&fixture.0)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("artifact missing or changed: test-scores.json"),
        "{error}"
    );
    assert!(!fixture.0.join("results/geometry-screen-v1.json").exists());
}

#[test]
fn pre_milestone7_artifacts_are_byte_identical() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: Value = serde_json::from_str(include_str!(
        "../benchmarks/bike-sharing/frozen-before-v7.json"
    ))
    .unwrap();
    let files = manifest["sha256"].as_object().unwrap();
    assert!(files.len() > 100);
    for (name, digest) in files {
        assert_eq!(
            triad::evaluation::hash(&std::fs::read(root.join(name)).unwrap()),
            digest.as_str().unwrap(),
            "frozen artifact changed: {name}"
        );
    }
}
