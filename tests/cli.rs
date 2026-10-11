use std::process::Command;

#[test]
fn help_and_version_are_successful_without_data_access() {
    for flag in ["--help", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_triad"))
            .args(["--root", "/does-not-exist", flag])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn malformed_commands_fail_before_running_experiments() {
    for args in [vec!["--root"], vec!["selective", "typo"], vec!["unknown"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_triad"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn metrology_cli_roundtrip_and_refuses_overwrite() {
    let root = std::env::temp_dir().join(format!("triad-metrology-cli-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let output = root.join("report.json");
    let example =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/predictions.json");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_triad"))
            .args(["metrology", "evaluate", "--input"])
            .arg(&example)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    let original = std::fs::read(&output).unwrap();
    assert!(!run().status.success());
    assert_eq!(std::fs::read(&output).unwrap(), original);
    assert!(Command::new(env!("CARGO_BIN_EXE_triad"))
        .args(["metrology", "validate", "--input"])
        .arg(&output)
        .status()
        .unwrap()
        .success());
    std::fs::remove_dir_all(root).unwrap();
}
