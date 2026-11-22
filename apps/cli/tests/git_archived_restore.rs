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

#[test]
fn cli_previews_then_requires_confirmation_for_transactional_project_restore() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Novel");
    let mut app = story_app::WriterApp::create_project(&project, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Original prose.")
        .unwrap();
    let first = app.git_snapshot("First draft").unwrap().unwrap();
    app.create_scene(chapter, "Later", 1).unwrap();
    app.git_snapshot("Second draft").unwrap().unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Uncommitted current prose.")
        .unwrap();
    drop(app);

    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let preview = Command::new(executable)
        .args(["git-restore-preview", project.to_str().unwrap(), &first])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["commit_id"], first);
    assert!(!preview["changes"].as_array().unwrap().is_empty());
    assert!(
        !preview["working_tree_changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let refused = Command::new(executable)
        .args(["git-restore-project", project.to_str().unwrap(), &first])
        .output()
        .unwrap();
    assert!(!refused.status.success());

    let restored = Command::new(executable)
        .args([
            "git-restore-project",
            project.to_str().unwrap(),
            &first,
            "--confirm",
        ])
        .output()
        .unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let reopened = story_app::WriterApp::open_project(project).unwrap();
    let scenes = reopened.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0].scene.id, scene);
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "Original prose."
    );
    reopened.validate_project().unwrap();
}

#[test]
fn cli_configures_and_pushes_a_project_git_remote_without_force() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Novel");
    let bare = temp.path().join("backup.git");
    drop(story_app::WriterApp::create_project(&project, "Novel").unwrap());
    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let snapshot = Command::new(executable)
        .args(["git-snapshot", project.to_str().unwrap(), "Initial"])
        .output()
        .unwrap();
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let branch = Command::new("git")
        .arg("-C")
        .arg(&project)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .unwrap();
    assert!(branch.status.success());
    let branch = String::from_utf8(branch.stdout).unwrap().trim().to_owned();
    assert!(
        Command::new("git")
            .args(["init", "--bare", bare.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success()
    );

    let add = Command::new(executable)
        .args([
            "git-remote-add",
            project.to_str().unwrap(),
            "backup",
            bare.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let remotes = Command::new(executable)
        .args(["git-remotes", project.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(remotes.status.success());
    assert_eq!(String::from_utf8(remotes.stdout).unwrap().trim(), "backup");
    let pushed = Command::new(executable)
        .args(["git-back-up", project.to_str().unwrap(), "backup", &branch])
        .output()
        .unwrap();
    assert!(
        pushed.status.success(),
        "{}",
        String::from_utf8_lossy(&pushed.stderr)
    );
}
