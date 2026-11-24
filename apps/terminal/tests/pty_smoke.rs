#[cfg(target_os = "linux")]
#[test]
fn full_screen_terminal_opens_menu_resizes_and_exits_in_a_real_pty() {
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Pty Project");
    let mut app = WriterApp::create_project(&project, "Pty Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (prose, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(
        scene,
        Some(&hash),
        &format!("{prose}The opening paragraph remains readable."),
    )
    .unwrap();
    drop(app);

    let python = Command::new("python3").arg("--version").output();
    if python.is_err() {
        eprintln!("skipping Linux PTY smoke: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .output()
        .expect("run the Linux PTY smoke harness");
    assert!(
        result.status.success(),
        "PTY smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn full_screen_terminal_creates_a_fiction_project_and_saves_a_scene_in_a_pty() {
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::BookType;
    use story_document::CompileFormat;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let python = Command::new("python3").arg("--version").output();
    if python.is_err() {
        eprintln!("skipping Linux PTY workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg("-")
        .arg(&home)
        .arg("create-fiction")
        .output()
        .expect("run the Linux PTY project-creation workflow");
    assert!(
        result.status.success(),
        "PTY fiction workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let project_path = home.join("My Novel");
    let mut app = WriterApp::open_project(&project_path).unwrap();
    assert_eq!(app.get_project().unwrap().book_type, BookType::Fiction);
    let scenes = app.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0].scene.title, "Opening");
    assert_eq!(
        app.load_scene_text(scenes[0].scene.id).unwrap().0,
        "First passage."
    );

    // Complete the same project-service path used by the UI after the PTY
    // writing step: place the independent scene, reopen it and compile it.
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    app.move_scene(scenes[0].scene.id, chapter, 0).unwrap();
    let compiled = app
        .compile_artifact(CompileFormat::Markdown, false)
        .unwrap();
    let manuscript = String::from_utf8(compiled.bytes).unwrap();
    assert!(manuscript.contains("Chapter One"));
    assert!(manuscript.contains("First passage."));
}

#[cfg(target_os = "linux")]
#[test]
fn empty_previous_position_from_navigate_menu_keeps_the_terminal_running() {
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Pty Project");
    let mut app = WriterApp::create_project(&project, "Pty Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (prose, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), &format!("{prose}Opening prose."))
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY navigation regression: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("previous-position")
        .output()
        .expect("run the Linux PTY navigation regression");
    assert!(
        result.status.success(),
        "empty Previous Position PTY flow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
