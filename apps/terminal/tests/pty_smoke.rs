static PTY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(target_os = "linux")]
fn lock_pty_test() -> std::sync::MutexGuard<'static, ()> {
    PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(target_os = "linux")]
#[test]
fn full_screen_terminal_opens_menu_resizes_and_exits_in_a_real_pty() {
    let _guard = lock_pty_test();
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
        &format!(
            "{prose}The opening paragraph remains readable.\nMaqrara opened the old bok and found a note inside.\nÉlodie waved beside 한글 🧭."
        ),
    )
    .unwrap();
    let reference_path = root.path().join("reference notes.txt");
    std::fs::write(&reference_path, "Reference only stays clear.").unwrap();
    let reference = story_reading::import_reference(&reference_path).unwrap();
    app.import_reading_document(&reference, reference_path.to_str().unwrap())
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
    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The opening paragraph remains readable.\nMaqrara opened the old bok and found a note inside.\nÉlodie waved beside 한글 🧭."
    );
    assert_eq!(
        std::fs::read_to_string(reference_path).unwrap(),
        "Reference only stays clear."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn view_menu_switches_between_unified_outline_presentations_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Outline View PTY");
    let mut app = WriterApp::create_project(&project, "Outline View PTY").unwrap();
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The harbour was quiet at dawn.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY unified-outline workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("outline-view")
        .output()
        .expect("run the unified-outline Linux PTY workflow");
    assert!(
        result.status.success(),
        "unified-outline PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The harbour was quiet at dawn."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn removed_scenes_empty_state_returns_to_cards_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Removed Scenes PTY");
    let mut app = WriterApp::create_project(&project, "Removed Scenes PTY").unwrap();
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    app.move_scene(scene, chapter, 0).unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY Removed Scenes workflow: python3 is unavailable");
        return;
    }
    let home = root.path().join("home");
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(&home)
        .arg("removed-scenes")
        .output()
        .expect("run the Removed Scenes Linux PTY workflow");
    assert!(
        result.status.success(),
        "Removed Scenes PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn full_screen_terminal_creates_a_fiction_project_and_saves_a_scene_in_a_pty() {
    let _guard = lock_pty_test();
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
fn publish_menu_compiles_a_docx_and_reports_success_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Publish Project");
    let mut app = WriterApp::create_project(&project, "Publish Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The manuscript reaches the reader.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY compile workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("compile-docx")
        .output()
        .expect("run the Linux PTY DOCX compile workflow");
    assert!(
        result.status.success(),
        "PTY DOCX compile workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let output = project.join("Publish Project.docx");
    assert!(output.is_file());
    assert!(std::fs::metadata(&output).unwrap().len() > 0);
    let mut archive = zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
    let mut document = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("word/document.xml").unwrap(),
        &mut document,
    )
    .unwrap();
    assert!(document.contains("The manuscript reaches the reader."));
}

#[cfg(target_os = "linux")]
#[test]
fn insert_date_time_and_file_work_through_the_real_insert_menu() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let source = home.join("imported.txt");
    let source_text = "Inserted file text remains read-only.";
    std::fs::write(&source, source_text).unwrap();
    let original_source = std::fs::read(&source).unwrap();
    let project = root.path().join("Writing Project");
    let mut app = WriterApp::create_project(&project, "Writing Project").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Scene", 0).unwrap();
    let hash = app.load_scene_text(scene).unwrap().1;
    app.save_scene_text(scene, Some(&hash), "Opening paragraph.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY insertion workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(&home)
        .arg("insert-edit-actions")
        .output()
        .expect("run the Linux PTY edit insertion workflow");
    assert!(
        result.status.success(),
        "PTY insertion workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    let text = reopened.load_scene_text(scene).unwrap().0;
    assert!(
        text.contains("UTC"),
        "date/time not saved in scene: {text:?}"
    );
    assert!(
        text.contains(source_text),
        "file text not saved in scene: {text:?}"
    );
    assert_eq!(std::fs::read(source).unwrap(), original_source);
}

#[cfg(target_os = "linux")]
#[test]
fn special_character_picker_inserts_and_saves_a_unicode_codepoint_through_the_real_menu() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Special Character Project");
    let mut app = WriterApp::create_project(&project, "Special Character Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The opening paragraph.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY special-character workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("special-character")
        .output()
        .expect("run the Linux PTY special-character workflow");
    assert!(
        result.status.success(),
        "PTY special-character workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "—The opening paragraph."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn named_window_layouts_save_and_load_through_the_real_view_menu() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Layout Project");
    let mut app = WriterApp::create_project(&project, "Layout Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The saved view remains intact.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY named-layout workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("named-layouts")
        .output()
        .expect("run the Linux PTY named-layout workflow");
    assert!(
        result.status.success(),
        "PTY named-layout workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let layout_path = project.join(".writers-ide/window-layouts.json");
    let layouts: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout_path).unwrap()).unwrap();
    assert_eq!(layouts["layouts"][0]["name"], "UAT Layout");
    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The saved view remains intact."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn edit_menu_is_visible_and_selection_commands_dispatch_from_writing_mode_in_a_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Edit Menu Project");
    let mut app = WriterApp::create_project(&project, "Edit Menu Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(
        scene,
        Some(&hash),
        "An author can find the editing actions.\n\nA second paragraph waits.",
    )
    .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY Edit-menu workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("edit-menu")
        .output()
        .expect("run the Linux PTY Edit-menu workflow");
    assert!(
        result.status.success(),
        "PTY Edit-menu workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "An author can find the editing actions. A second paragraph waits."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn empty_previous_position_from_navigate_menu_keeps_the_terminal_running() {
    let _guard = lock_pty_test();
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

#[cfg(target_os = "linux")]
#[test]
fn outline_enter_opens_a_section_descendant_and_returns_to_the_same_scene() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Outline PTY");
    let mut app = WriterApp::create_project(&project, "Outline PTY").unwrap();
    let section = app.create_section("Part I", 0).unwrap();
    let chapter = app.create_chapter(Some(section), "Chapter 1", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let expected = "Opening paragraph remains editable.";
    let expected_hash = app.load_scene_text(scene).unwrap().1;
    app.save_scene_text(scene, Some(&expected_hash), expected)
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux unified Outline PTY workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("outline-enter")
        .output()
        .expect("run the Linux PTY unified Outline workflow");
    assert!(
        result.status.success(),
        "unified Outline PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(reopened.load_scene_text(scene).unwrap().0, expected);
}

#[cfg(target_os = "linux")]
#[test]
fn scene_and_reference_removal_dialogues_are_cancellable_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Removal Project");
    let source = root.path().join("Field Notes.txt");
    std::fs::write(&source, "A read-only field reference.").unwrap();
    let mut app = WriterApp::create_project(&project, "Removal Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The traveler arrives.")
        .unwrap();
    let reference = story_reading::import_reference(&source).unwrap();
    let reference_id = app
        .import_reading_document(&reference, source.to_str().unwrap())
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY removal workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("removal-dialogs")
        .output()
        .expect("run the Linux PTY removal-dialog workflow");
    assert!(
        result.status.success(),
        "PTY removal workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert!(!reopened.get_scene(scene).unwrap().scene.archived);
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    let active_references = reopened.list_reading_documents().unwrap();
    assert_eq!(active_references.len(), 1);
    assert_eq!(active_references[0].id, reference_id);
    assert_eq!(
        std::fs::read_to_string(source).unwrap(),
        "A read-only field reference."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn confirmed_reference_removal_never_modifies_its_source_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Reference Removal Project");
    let source = root.path().join("Field Notes.txt");
    std::fs::write(&source, "A read-only field reference.").unwrap();
    let mut app = WriterApp::create_project(&project, "Reference Removal Project").unwrap();
    let reference = story_reading::import_reference(&source).unwrap();
    let reference_id = app
        .import_reading_document(&reference, source.to_str().unwrap())
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY reference-removal workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("reference-removal")
        .output()
        .expect("run the Linux PTY reference-removal workflow");
    assert!(
        result.status.success(),
        "PTY reference-removal workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    assert!(reopened.list_reading_documents().unwrap().is_empty());
    let removed = reopened.list_removed_reading_documents().unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].id, reference_id);
    assert_eq!(
        std::fs::read_to_string(&source).unwrap(),
        "A read-only field reference."
    );
    reopened.restore_reading_document(reference_id).unwrap();
    assert_eq!(
        reopened.list_reading_documents().unwrap()[0].id,
        reference_id
    );
}

#[cfg(target_os = "linux")]
#[test]
fn populated_chapter_removal_retains_scenes_through_the_real_terminal_workflow() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Chapter Removal Project");
    let mut app = WriterApp::create_project(&project, "Chapter Removal Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The traveler arrives.")
        .unwrap();
    let prose = app
        .create_content_unit(
            story_core::ContentUnitKind::ChapterBody,
            "Chapter prose",
            Some(chapter),
            0,
            None,
        )
        .unwrap();
    let (_, prose_hash) = app.load_content_unit_text(prose).unwrap();
    app.save_content_unit_text(
        prose,
        Some(&prose_hash),
        "Direct chapter prose remains recoverable.",
    )
    .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY chapter-removal workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-removal")
        .output()
        .expect("run the Linux PTY populated-chapter removal workflow");
    assert!(
        result.status.success(),
        "PTY chapter-removal workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    let moved_scene = reopened.get_scene(scene).unwrap();
    assert!(!moved_scene.scene.archived);
    assert_eq!(moved_scene.chapter_id, None);
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    assert!(reopened.get_content_unit(prose).unwrap().archived);
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "Direct chapter prose remains recoverable."
    );
    assert!(
        reopened
            .list_archived_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == chapter)
    );
    let compiled = reopened
        .compile_artifact(story_document::CompileFormat::Markdown, false)
        .unwrap();
    assert!(
        !String::from_utf8(compiled.bytes)
            .unwrap()
            .contains("The traveler arrives.")
    );
    assert!(
        !String::from_utf8(
            reopened
                .compile_artifact(story_document::CompileFormat::Markdown, false)
                .unwrap()
                .bytes
        )
        .unwrap()
        .contains("Direct chapter prose remains recoverable.")
    );

    reopened.restore_latest_structural_removal(chapter).unwrap();
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(chapter));
    assert!(!reopened.get_content_unit(prose).unwrap().archived);
    assert!(!reopened.get_content_unit(prose).unwrap().archived);
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "Direct chapter prose remains recoverable."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn empty_chapters_created_from_the_scene_outliner_persist_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Chapter Creation Project");
    WriterApp::create_project(&project, "Chapter Creation Project").unwrap();

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY chapter-creation workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-creation")
        .output()
        .expect("run the Linux PTY chapter-creation workflow");
    assert!(
        result.status.success(),
        "PTY chapter-creation workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    let chapters = reopened
        .list_structural_nodes()
        .unwrap()
        .into_iter()
        .filter(|node| node.kind == "chapter")
        .map(|node| node.title)
        .collect::<Vec<_>>();
    // New chapters are inserted at the selected chapter boundary, so the
    // second chapter is placed immediately before the still-selected Alpha.
    assert_eq!(chapters, ["Beta", "Alpha"]);
}

#[cfg(target_os = "linux")]
#[test]
fn structural_undo_and_redo_work_from_outline_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Structural Undo Project");
    let mut app = WriterApp::create_project(&project, "Structural Undo Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY structural undo/redo workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-removal-undo-redo")
        .output()
        .expect("run the structural undo/redo Linux PTY workflow");
    assert!(
        result.status.success(),
        "structural undo/redo PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    let removed = reopened.get_scene(scene).unwrap();
    assert!(removed.scene.archived);
    assert_eq!(removed.chapter_id, Some(chapter));
    assert!(reopened.list_structural_nodes().unwrap().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn populated_chapter_removal_can_rehome_scenes_to_a_selected_chapter_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Chapter Rehome Project");
    let mut app = WriterApp::create_project(&project, "Chapter Rehome Project").unwrap();
    let source = app.create_chapter(None, "Opening", 0).unwrap();
    let target = app.create_chapter(None, "Destination", 1).unwrap();
    let scene = app.create_scene(source, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The traveler arrives.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY chapter-rehome workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-removal-target")
        .output()
        .expect("run the Linux PTY chapter-rehome workflow");
    assert!(
        result.status.success(),
        "PTY chapter-rehome workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(target));
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    let compiled = String::from_utf8(
        reopened
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert!(compiled.contains("Destination"));
    assert!(compiled.contains("The traveler arrives."));

    reopened.restore_latest_structural_removal(source).unwrap();
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(source));
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cancelling_populated_chapter_removal_preserves_hierarchy_and_prose_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::ContentUnitKind;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Chapter Cancellation Project");
    let mut app = WriterApp::create_project(&project, "Chapter Cancellation Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, scene_hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&scene_hash), "The traveler arrives.")
        .unwrap();
    let prose = app
        .create_content_unit(
            ContentUnitKind::ChapterBody,
            "Chapter prose",
            Some(chapter),
            0,
            None,
        )
        .unwrap();
    let (_, prose_hash) = app.load_content_unit_text(prose).unwrap();
    app.save_content_unit_text(prose, Some(&prose_hash), "The road was quiet.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY chapter-removal cancellation: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-removal-cancel")
        .output()
        .expect("run the Linux PTY chapter-removal cancellation workflow");
    assert!(
        result.status.success(),
        "PTY chapter-removal cancellation failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(chapter));
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    assert!(!reopened.get_content_unit(prose).unwrap().archived);
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "The road was quiet."
    );
    assert!(
        reopened
            .list_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == chapter && node.title == "Opening")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn populated_chapter_subtree_deletion_is_recoverable_through_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::{
        ContentUnitKind, EntityRef, LinkKind, ParticipationRole, ScenePlotline, StoryLink,
        WordAttributionPolicy,
    };
    use story_document::CompileFormat;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Chapter Removal Project");
    let mut app = WriterApp::create_project(&project, "Chapter Removal Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The traveler arrives.")
        .unwrap();
    let snapshot = app
        .create_snapshot(scene, "Before chapter removal")
        .unwrap();
    let anchor = app
        .create_passage_anchor(scene, 0, 3, "The", "", " traveler")
        .unwrap();
    let character = app.create_character("Mara").unwrap();
    app.link_character(scene, character, ParticipationRole::Major)
        .unwrap();
    let plotline = app.create_plotline("The journey").unwrap();
    app.set_scene_plotlines(
        scene,
        &[ScenePlotline {
            plotline_id: plotline,
            role: "advances".into(),
            share_basis_points: None,
        }],
        WordAttributionPolicy::Overlap,
    )
    .unwrap();
    let event = app.add_event("Arrival", "day 1").unwrap();
    app.link_event(scene, event, "depicts").unwrap();
    let link = StoryLink {
        id: story_core::LinkId::new(),
        source: EntityRef::Scene(scene),
        source_anchor: Some(anchor),
        target: EntityRef::Character(character),
        target_anchor: None,
        kind: LinkKind::Related,
        label: "Mara's arrival".into(),
    };
    app.create_story_link(&link).unwrap();
    let prose = app
        .create_content_unit(
            ContentUnitKind::ChapterBody,
            "Opening prose",
            Some(chapter),
            0,
            None,
        )
        .unwrap();
    let (_, prose_hash) = app.load_content_unit_text(prose).unwrap();
    app.save_content_unit_text(prose, Some(&prose_hash), "A chapter introduction.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY subtree-deletion workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("chapter-removal-delete")
        .output()
        .expect("run the Linux PTY chapter-subtree deletion workflow");
    assert!(
        result.status.success(),
        "PTY chapter-subtree deletion workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    assert!(reopened.list_scenes(false).unwrap().is_empty());
    assert!(
        reopened
            .list_archived_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == chapter)
    );
    assert!(reopened.get_content_unit(prose).unwrap().archived);
    let compiled = String::from_utf8(
        reopened
            .compile_artifact(CompileFormat::Markdown, false)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert!(!compiled.contains("The traveler arrives."));
    assert!(!compiled.contains("A chapter introduction."));

    reopened.restore_latest_structural_removal(chapter).unwrap();
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(chapter));
    assert!(!reopened.get_content_unit(prose).unwrap().archived);
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The traveler arrives."
    );
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "A chapter introduction."
    );
    assert_eq!(
        reopened.scene_character_analytics(scene).unwrap(),
        vec![(character.to_string(), "Mara".into(), "major".into())]
    );
    assert_eq!(
        reopened.scene_plotline_links(scene).unwrap()[0].plotline_id,
        plotline
    );
    assert_eq!(
        reopened.scene_event_links(scene).unwrap(),
        vec![(event, "depicts".into())]
    );
    assert_eq!(reopened.list_snapshots(scene).unwrap()[0].id, snapshot);
    assert_eq!(reopened.list_passage_anchors(scene).unwrap()[0].id, anchor);
    assert_eq!(
        reopened.outgoing_links(EntityRef::Scene(scene)).unwrap(),
        vec![link]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn nonfiction_section_subtree_deletion_preserves_owned_prose_for_recovery_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::{BookType, ContentUnitKind, NodeKind};
    use story_document::CompileFormat;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Non-fiction Removal Project");
    let mut app =
        WriterApp::create_project_with_type(&project, "Field Guide", BookType::Nonfiction).unwrap();
    let parent = app
        .create_structural_node(None, NodeKind::Section, "Part One", 0)
        .unwrap();
    let section = app
        .create_structural_node(Some(parent), NodeKind::Section, "Old Heading", 0)
        .unwrap();
    let child = app
        .create_structural_node(Some(section), NodeKind::Section, "Subsection", 0)
        .unwrap();
    let prose = app
        .create_content_unit(
            ContentUnitKind::SectionBody,
            "Old Heading prose",
            Some(section),
            0,
            None,
        )
        .unwrap();
    let (_, hash) = app.load_content_unit_text(prose).unwrap();
    app.save_content_unit_text(prose, Some(&hash), "Field notes under this heading.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!(
            "skipping Linux PTY non-fiction section-removal workflow: python3 is unavailable"
        );
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("section-removal-delete")
        .output()
        .expect("run the Linux PTY non-fiction section-removal workflow");
    assert!(
        result.status.success(),
        "PTY non-fiction section-removal workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    assert!(
        reopened
            .list_archived_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == section)
    );
    assert!(
        !reopened
            .list_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == child)
    );
    assert!(reopened.get_content_unit(prose).unwrap().archived);
    let compiled = String::from_utf8(
        reopened
            .compile_artifact(CompileFormat::Markdown, false)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert!(!compiled.contains("Field notes under this heading."));

    reopened.restore_latest_structural_removal(section).unwrap();
    let restored_nodes = reopened.list_structural_nodes().unwrap();
    assert!(restored_nodes.iter().any(|node| node.id == section));
    assert_eq!(
        restored_nodes
            .iter()
            .find(|node| node.id == child)
            .unwrap()
            .parent_id,
        Some(section)
    );
    assert!(!reopened.get_content_unit(prose).unwrap().archived);
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "Field notes under this heading."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn nonfiction_section_owned_prose_rehomes_through_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::{BookType, ContentUnitKind, NodeKind};
    use story_document::CompileFormat;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Non-fiction Rehome Project");
    let mut app =
        WriterApp::create_project_with_type(&project, "Field Guide", BookType::Nonfiction).unwrap();
    let parent = app
        .create_structural_node(None, NodeKind::Section, "Part One", 0)
        .unwrap();
    let section = app
        .create_structural_node(Some(parent), NodeKind::Section, "Old Heading", 0)
        .unwrap();
    let child = app
        .create_structural_node(Some(section), NodeKind::Section, "Subsection", 0)
        .unwrap();
    let prose = app
        .create_content_unit(
            ContentUnitKind::SectionBody,
            "Old Heading prose",
            Some(section),
            0,
            None,
        )
        .unwrap();
    let (_, hash) = app.load_content_unit_text(prose).unwrap();
    app.save_content_unit_text(prose, Some(&hash), "Field notes stay in the manuscript.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY non-fiction prose rehome workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("section-removal-rehome")
        .output()
        .expect("run the Linux PTY non-fiction prose rehome workflow");
    assert!(
        result.status.success(),
        "PTY non-fiction prose rehome failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let mut reopened = WriterApp::open_project(&project).unwrap();
    assert!(
        reopened
            .list_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.id == child && node.parent_id == Some(parent))
    );
    let active_prose = reopened.get_content_unit(prose).unwrap();
    assert!(!active_prose.archived);
    assert_eq!(active_prose.parent_id, Some(parent));
    assert_eq!(active_prose.position, 0);
    let compiled = String::from_utf8(
        reopened
            .compile_artifact(CompileFormat::Markdown, false)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert!(compiled.contains("Field notes stay in the manuscript."));
    reopened.restore_latest_structural_removal(section).unwrap();
    let restored = reopened.get_content_unit(prose).unwrap();
    assert_eq!(restored.parent_id, Some(section));
    assert_eq!(restored.position, 0);
    assert_eq!(
        reopened.load_content_unit_text(prose).unwrap().0,
        "Field notes stay in the manuscript."
    );
}
