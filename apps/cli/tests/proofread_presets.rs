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

#[test]
fn cli_compile_flags_promote_or_suppress_only_authoring_findings() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("Book");
    let mut app = story_app::WriterApp::create_project(&project, "Book").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Mara said said.")
        .unwrap();
    app.create_builtin_compile_configuration("plain", "plain")
        .unwrap();
    drop(app);
    let executable = env!("CARGO_BIN_EXE_writers-ide");

    let promoted = Command::new(executable)
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "run",
            "plain",
            "-Werror=repeated-word",
        ])
        .output()
        .unwrap();
    assert!(!promoted.status.success());
    let output = String::from_utf8_lossy(&promoted.stdout);
    assert!(output.contains("\"status\": \"failed\""));
    assert!(output.contains("\"effective_level\": \"error\""));
    assert!(
        !project
            .join("exports/compile/plain/manuscript.txt")
            .exists()
    );

    let quiet = Command::new(executable)
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "run",
            "plain",
            "-w",
        ])
        .output()
        .unwrap();
    assert!(
        quiet.status.success(),
        "{}",
        String::from_utf8_lossy(&quiet.stderr)
    );
    assert!(
        project
            .join("exports/compile/plain/manuscript.txt")
            .exists()
    );

    let recipe_path = project.join(".writers-ide/compile-configurations/plain.toml");
    let original = std::fs::read_to_string(&recipe_path).unwrap();
    std::fs::write(&recipe_path, "not a valid recipe").unwrap();
    let invalid_config = Command::new(executable)
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "run",
            "plain",
            "-w",
        ])
        .output()
        .unwrap();
    assert!(!invalid_config.status.success());
    std::fs::write(recipe_path, original).unwrap();
}
