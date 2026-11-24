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
