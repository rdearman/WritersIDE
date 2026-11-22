use std::process::Command;

#[test]
fn cli_writes_named_diagnostic_presets_without_overwriting_project_rules() {
    let directory = tempfile::tempdir().unwrap();
    let drafting = directory.path().join("Drafting");
    story_app::WriterApp::create_project(&drafting, "Drafting").unwrap();
    let executable = env!("CARGO_BIN_EXE_writers-ide");

    let created = Command::new(executable)
        .args(["proofread-config", drafting.to_str().unwrap(), "copyedit"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let config_path = drafting.join(".writers-ide/diagnostics.toml");
    let source = std::fs::read_to_string(&config_path).unwrap();
    assert!(source.contains("severity = \"warning\""));

    let original = source.clone();
    let duplicate = Command::new(executable)
        .args([
            "proofread-config",
            drafting.to_str().unwrap(),
            "publication",
        ])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert_eq!(std::fs::read_to_string(config_path).unwrap(), original);

    let invalid = Command::new(executable)
        .args(["proofread-config", drafting.to_str().unwrap(), "unknown"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
}
