use std::{fs, path::Path};
use triad::evaluation::hash;

#[test]
fn recorded_development_diagnostic_preserves_baseline_and_failed_readiness() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("results/numerical-components-v4.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["kind"], "numerical_components_v4_development_only");
    assert_eq!(report["development_probe_count"].as_u64(), Some(10));
    assert_eq!(report["rows"].as_array().unwrap().len(), 10);
    assert_eq!(report["readiness_for_new_selective_claims"], false);
    let hash_original = hash(&fs::read(root.join("experiments/config.json")).unwrap());
    assert_eq!(report["baseline_config_sha256"], hash_original);
    for row in report["rows"].as_array().unwrap() {
        let mesh = row["mesh_1mm_vs_0_5mm_noise_units"].as_f64().unwrap();
        let interp = row["local_patch_vs_exact_1mm_noise_units"].as_f64().unwrap();
        let total = row["local_patch_vs_exact_0_5mm_noise_units"].as_f64().unwrap();
        assert!(mesh.is_finite() && interp.is_finite() && total.is_finite());
        assert!(mesh >= 0.0 && interp >= 0.0 && total >= 0.0);
        assert!(total <= mesh + interp + 1e-8);
    }
}
