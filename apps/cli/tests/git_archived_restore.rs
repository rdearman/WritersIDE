use std::process::Command;

#[test]
fn cli_reactivates_archived_scene_with_original_uuid() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Novel");
    let mut app = story_app::WriterApp::create_project(&project, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Original prose.")
        .unwrap();
    let commit = app.git_snapshot("Before archive").unwrap().unwrap();
    app.archive_scene(scene).unwrap();
    drop(app);

    let output = Command::new(env!("CARGO_BIN_EXE_writers-ide"))
        .args([
            "git-restore-scene-archived",
            project.to_str().unwrap(),
            &commit,
            &scene.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let reopened = story_app::WriterApp::open_project(project).unwrap();
    let active = reopened.list_scenes(false).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].scene.id, scene);
    assert_eq!(active[0].chapter_id, Some(chapter));
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "Original prose."
    );
}
