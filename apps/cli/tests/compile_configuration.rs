use std::process::Command;
use story_app::WriterApp;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_writers-ide"))
}

#[test]
fn named_compile_configuration_is_portable_and_clean_only_removes_manifested_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Novel");
    let mut app = WriterApp::create_project(&project, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, initial_hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&initial_hash), "A portable Compile test.")
        .unwrap();
    let expected = app.load_scene_text(scene).unwrap();
    drop(app);

    let create = cli()
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "create",
            "markdown",
            "plain",
        ])
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    let list = cli()
        .args(["compile-config", project.to_str().unwrap(), "list"])
        .output()
        .unwrap();
    assert!(list.status.success());
    assert_eq!(String::from_utf8(list.stdout).unwrap().trim(), "plain");

    let output = project.join("exports/compile/plain/manuscript.md");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(&output, "unmanaged user file").unwrap();
    let refused = cli()
        .args(["compile-config", project.to_str().unwrap(), "run", "plain"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "unmanaged user file"
    );

    std::fs::remove_file(&output).unwrap();
    let compiled = cli()
        .args(["compile-config", project.to_str().unwrap(), "run", "plain"])
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(
        std::fs::read_to_string(&output)
            .unwrap()
            .contains("A portable Compile test.")
    );
    let results = cli()
        .args(["compile-config", project.to_str().unwrap(), "results"])
        .output()
        .unwrap();
    assert!(results.status.success());
    let records: serde_json::Value = serde_json::from_slice(&results.stdout).unwrap();
    assert!(records.as_array().unwrap().iter().any(|record| {
        record["succeeded"] == false
            && record["error"]
                .as_str()
                .is_some_and(|error| error.contains("not managed"))
    }));
    assert!(records.as_array().unwrap().iter().any(|record| {
        record["succeeded"] == true
            && record["outputs"].as_array().is_some_and(|outputs| {
                outputs
                    .iter()
                    .any(|value| value == "exports/compile/plain/manuscript.md")
            })
    }));
    let source_map = std::path::PathBuf::from(format!("{}.source-map.json", output.display()));
    assert!(source_map.is_file());
    std::fs::write(output.parent().unwrap().join("author-note.txt"), "keep me").unwrap();

    let refused_clean = cli()
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "clean",
            "plain",
        ])
        .output()
        .unwrap();
    assert!(!refused_clean.status.success());
    let cleaned = cli()
        .args([
            "compile-config",
            project.to_str().unwrap(),
            "clean",
            "plain",
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        cleaned.status.success(),
        "{}",
        String::from_utf8_lossy(&cleaned.stderr)
    );
    assert!(!output.exists());
    assert!(!source_map.exists());
    assert!(output.parent().unwrap().join("author-note.txt").exists());

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(reopened.load_scene_text(scene).unwrap(), expected);
}
