use std::process::Command;

use story_app::WriterApp;

#[test]
fn replace_preview_emits_proposals_and_does_not_write_manuscript_files() {
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Novel");
    let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "A cat waits. A cat leaves.")
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_writers-ide"))
        .args([
            "replace-preview",
            project_path.to_str().unwrap(),
            "normal",
            "all",
            "cat",
            "=>",
            "fox",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(preview[0]["scene_id"], scene.to_string());
    assert_eq!(preview[0]["replacements"], 2);
    assert_eq!(preview[0]["original"], "A cat waits. A cat leaves.");
    assert_eq!(preview[0]["proposed"], "A fox waits. A fox leaves.");
    assert_eq!(
        app.load_scene_text(scene).unwrap().0,
        "A cat waits. A cat leaves."
    );
}
