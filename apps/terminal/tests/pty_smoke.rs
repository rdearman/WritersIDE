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
        .arg(&harness)
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
fn scrivener_import_to_edit_reopen_and_compile_works_through_the_production_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("isolated-home");
    std::fs::create_dir_all(&home).unwrap();
    let source = home.join("0 Import.scriv");
    let files = source.join("Files/Data");
    std::fs::create_dir_all(files.join("scene-id")).unwrap();
    std::fs::create_dir_all(files.join("chapter-id")).unwrap();
    std::fs::create_dir_all(files.join("research-id")).unwrap();
    std::fs::create_dir_all(source.join("Settings")).unwrap();
    std::fs::write(
        source.join("0 Import.scrivx"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <ScrivenerProject Version="2.0"><Binder>
          <BinderItem UUID="draft-id" Type="DraftFolder"><Title>Manuscript</Title><Children>
            <BinderItem UUID="part-id" Type="Folder"><Title>Part One</Title><Children>
              <BinderItem UUID="chapter-id" Type="Folder"><Title>Chapter One</Title><Children>
                <BinderItem UUID="scene-id" Type="Text"><Title>Opening Scene</Title><Children/></BinderItem>
              </Children></BinderItem>
            </Children></BinderItem>
          </Children></BinderItem>
          <BinderItem UUID="research-folder-id" Type="ResearchFolder"><Title>Research</Title><Children>
            <BinderItem UUID="research-id" Type="Text"><Title>Private Note</Title><Children/></BinderItem>
          </Children></BinderItem>
        </Binder></ScrivenerProject>"#,
    )
    .unwrap();
    std::fs::write(
        source.join("Settings/compile.xml"),
        r#"<CompileSettings><FormatSettings><Format ID="DEFAULT"/></FormatSettings></CompileSettings>"#,
    )
    .unwrap();
    std::fs::write(
        files.join("chapter-id/content.rtf"),
        r"{\rtf1\ansi Chapter prose survives direct editing and scene navigation.}",
    )
    .unwrap();
    std::fs::write(
        files.join("scene-id/content.rtf"),
        r"{\rtf1\ansi The patient blacksmith studied the arrowhead.}",
    )
    .unwrap();
    std::fs::write(
        files.join("research-id/content.rtf"),
        r"{\rtf1\ansi PRIVATE RESEARCH MUST NOT BE PUBLISHED.}",
    )
    .unwrap();

    let initial = root.path().join("Initial Project");
    WriterApp::create_project(&initial, "Initial Project").unwrap();
    let source_before = story_import::inspect_scrivener_project(&source).unwrap();
    assert!(
        source_before.items.iter().any(|item| {
            item.source_document_id == "scene-id"
                && item
                    .content_markdown
                    .as_deref()
                    .is_some_and(|text| text.contains("The patient blacksmith"))
        }),
        "the synthetic source fixture must contain readable manuscript text: {:?}",
        source_before.items
    );
    let source_fingerprint = source_before.source_fingerprint;
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Scrivener PTY workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&initial)
        .arg(&home)
        .arg("scrivener-import")
        .env(
            "WIDE_PTY_COMPILE_PROFILE",
            std::env::var("WIDE_PTY_COMPILE_PROFILE").unwrap_or_else(|_| "1".into()),
        )
        .env("WIDE_PTY_TEST_DIRECT_PROSE", "1")
        .output()
        .expect("run Scrivener import through Linux PTY");
    assert!(
        result.status.success(),
        "Scrivener author workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let inventory = story_import::inspect_scrivener_project(&source).unwrap();
    assert_eq!(inventory.source_fingerprint, source_fingerprint);
    let expected_title = inventory.project_title().unwrap_or(&inventory.title);
    let destination = std::fs::read_dir(&home)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.join("project.toml").is_file()
                && WriterApp::open_project(path)
                    .ok()
                    .and_then(|app| app.get_project().ok())
                    .is_some_and(|project| project.title == expected_title)
        })
        .expect("first Save should promote the staged import into the selected permanent location");
    let reopened = WriterApp::open_project(&destination).unwrap();
    let nodes = reopened.list_structural_nodes().unwrap();
    for expected in ["Part One", "Chapter One"] {
        assert!(
            nodes.iter().any(|node| node.title == expected),
            "missing {expected}"
        );
    }
    let scenes = reopened.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 1);
    let prose = reopened.load_scene_text(scenes[0].scene.id).unwrap().0;
    assert!(prose.contains("The patient blacksmith"));
    assert!(prose.contains("Additional UAT sentence."));
    let chapter_prose = reopened
        .list_content_units(false)
        .unwrap()
        .into_iter()
        .find(|unit| {
            unit.title == "Chapter One" && unit.kind == story_core::ContentUnitKind::ChapterBody
        })
        .expect("chapter-level Scrivener prose should remain an editable content unit");
    assert_eq!(
        reopened.load_content_unit_text(chapter_prose.id).unwrap().0,
        "Edited Chapter prose survives direct editing and scene navigation."
    );
    let artifact = destination.join(format!("{} - Default.docx", inventory.title));
    assert!(
        artifact.is_file(),
        "the visible compile workflow did not create {artifact:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn staged_working_project_recovers_after_an_abrupt_pty_exit() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("isolated-home");
    let staged = home.join("data/writers-ide/staging").join("working-novel");
    std::fs::create_dir_all(&staged).unwrap();
    let mut app = WriterApp::create_project(&staged, "Working Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Recovery Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Recovery checkpoint.")
        .unwrap();
    std::fs::create_dir_all(staged.join(".writers-ide")).unwrap();
    std::fs::write(
        staged.join(".writers-ide/staging.json"),
        serde_json::json!({"version": 1, "created_at": 1}).to_string(),
    )
    .unwrap();
    drop(app);

    let harness =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_staging_recovery.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&home)
        .output()
        .expect("run staged-work recovery through Linux PTY");
    assert!(
        result.status.success(),
        "staged recovery PTY failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !staged.exists(),
        "the explicit discard after recovery should remove only the staged project"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires WIDE_PRIVATE_ARROWMAKING_PROJECT to point at the protected local corpus"]
fn protected_arrowmaking_project_imports_through_the_production_pty_without_source_changes() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let source: std::path::PathBuf = std::env::var_os("WIDE_PRIVATE_ARROWMAKING_PROJECT")
        .expect("set WIDE_PRIVATE_ARROWMAKING_PROJECT to the protected .scriv project")
        .into();
    let inventory = story_import::inspect_scrivener_project(&source).unwrap();
    let fingerprint = inventory.source_fingerprint.clone();
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("isolated-home");
    std::fs::create_dir_all(&home).unwrap();
    let selected_source = home.join("0 Import.scriv");
    std::os::unix::fs::symlink(&source, &selected_source).unwrap();
    let initial = root.path().join("Initial Project");
    WriterApp::create_project(&initial, "Initial Project").unwrap();
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping private Scrivener PTY workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&initial)
        .arg(&home)
        .arg("scrivener-import")
        .env("WIDE_PTY_IMPORT_TITLES", "")
        .env("WIDE_PTY_IMPORT_TEXT", "")
        .env("WIDE_PTY_COMPILE_PROFILE", "1")
        .env("WIDE_PTY_TEST_DIRECT_PROSE", "1")
        .env("WIDE_PTY_DIRECT_PROSE_MARKER", "")
        .env("WIDE_PTY_DIRECT_PROSE_SAVED_MARKER", "Edited ")
        .env(
            "WIDE_PTY_DIRECT_PROSE_ROW_MARKER",
            std::env::var("WIDE_PTY_DIRECT_PROSE_ROW_MARKER")
                .unwrap_or_else(|_| "chapter_body".into()),
        )
        .output()
        .expect("run protected Scrivener project through Linux PTY");
    assert!(
        result.status.success(),
        "protected Scrivener PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let expected_title = inventory.project_title().unwrap_or(&inventory.title);
    let destination = std::fs::read_dir(&home)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.join("project.toml").is_file()
                && WriterApp::open_project(path)
                    .ok()
                    .and_then(|app| app.get_project().ok())
                    .is_some_and(|project| project.title == expected_title)
        })
        .expect("protected import should have a promoted permanent project");
    let reopened = WriterApp::open_project(&destination).unwrap();
    assert!(
        !reopened.list_structural_nodes().unwrap().is_empty(),
        "imported project should retain its binder hierarchy"
    );
    let proposed = story_import::ScrivenerImportPlan::proposed(&inventory);
    let expected_scenes = proposed
        .decisions
        .iter()
        .filter(|decision| {
            decision.destination_kind == story_import::ScrivenerDestinationKind::Scene
                && decision.include_in_import
        })
        .count();
    assert_eq!(
        reopened.list_scenes(false).unwrap().len(),
        expected_scenes,
        "all automatically mapped scenes must remain visible after direct-prose editing and reopen"
    );
    let content_units = reopened.list_content_units(false).unwrap();
    assert!(
        content_units.iter().any(|unit| {
            reopened
                .load_content_unit_text(unit.id)
                .is_ok_and(|(text, _)| text.contains("Edited "))
        }),
        "the direct-prose edit must remain persisted after closing and reopening the imported project"
    );
    let actual = story_import::inspect_scrivener_project(&source)
        .unwrap()
        .source_fingerprint;
    assert_eq!(
        actual, fingerprint,
        "the protected source must remain unchanged"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn first_run_tutorial_navigation_and_resume_work_through_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Author's Book");
    WriterApp::create_project(&project, "Author's Book").unwrap();

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY tutorial workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("tutorial-navigation")
        .output()
        .expect("run the first-run tutorial Linux PTY workflow");
    assert!(
        result.status.success(),
        "tutorial PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let demo_path = root.path().join("data/writers-ide/tutorial/sample-project");
    let demo = WriterApp::open_project(&demo_path).unwrap();
    let scenes = demo.list_scenes(false).unwrap();
    assert_eq!(
        scenes.len(),
        1,
        "tutorial should use its isolated sample scene"
    );
    let saved = demo.load_scene_text(scenes[0].scene.id).unwrap().0;
    assert!(saved.contains("First tutorial sentence."), "{saved}");
    let original = WriterApp::open_project(project).unwrap();
    assert!(original.list_scenes(false).unwrap().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn scene_rename_from_the_context_menu_is_editable_and_persistent_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Rename Book");
    let mut app = WriterApp::create_project(&project, "Rename Book").unwrap();
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Original Scene", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Original scene body.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY scene rename workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("scene-rename")
        .output()
        .expect("run the scene rename Linux PTY workflow");
    assert!(
        result.status.success(),
        "scene rename PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(project).unwrap();
    assert_eq!(
        reopened.get_scene(scene).unwrap().scene.title,
        "The Returning Light"
    );
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "Original scene body."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn outline_heading_rename_from_the_edit_menu_is_persistent_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Heading Book");
    let mut app = WriterApp::create_project(&project, "Heading Book").unwrap();
    let section = app.create_section("Part One", 0).unwrap();
    let chapter = app.create_chapter(Some(section), "Old Chapter", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Original scene body.")
        .unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY heading rename workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("heading-rename")
        .output()
        .expect("run the outline heading rename Linux PTY workflow");
    assert!(
        result.status.success(),
        "heading rename PTY workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    let node = reopened
        .list_structural_nodes()
        .unwrap()
        .into_iter()
        .find(|node| node.id == chapter)
        .unwrap();
    assert_eq!(node.title, "Part One · The Returning Light");
    assert_eq!(node.parent_id, Some(section));
    assert_eq!(reopened.get_scene(scene).unwrap().chapter_id, Some(chapter));
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "Original scene body."
    );
}

#[cfg(target_os = "linux")]
#[test]
fn date_time_insertion_dispatches_from_the_production_menu_and_persists_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Date Time PTY");
    let mut app = WriterApp::create_project(&project, "Date Time PTY").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The clock struck noon.")
        .unwrap();
    drop(app);

    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("date-time-insertion")
        .output()
        .expect("run date/time insertion through the production Linux PTY");
    assert!(
        result.status.success(),
        "date/time insertion PTY failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    let text = reopened.load_scene_text(scene).unwrap().0;
    assert!(text.contains("The clock struck noon."));
    assert_ne!(
        text, "The clock struck noon.",
        "menu action must insert text"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn date_time_insertion_without_an_active_document_is_rejected_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Empty Project");
    WriterApp::create_project(&project, "Empty Project").unwrap();
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("date-time-unavailable")
        .output()
        .expect("run unavailable date/time action through the production Linux PTY");
    assert!(
        result.status.success(),
        "unavailable date/time PTY failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        WriterApp::open_project(&project)
            .unwrap()
            .list_scenes(false)
            .unwrap()
            .is_empty()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn find_replace_fields_show_focus_caret_and_remain_editable_before_search_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Find Replace PTY");
    let mut app = WriterApp::create_project(&project, "Find Replace PTY").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "The red fox waits.")
        .unwrap();
    drop(app);

    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("find-replace-focus")
        .output()
        .expect("run Find and Replace focus workflow through the production Linux PTY");
    assert!(
        result.status.success(),
        "Find and Replace focus PTY failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The red fox waits.",
        "editing the search fields without running a replacement must not alter the manuscript"
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
    let starting_project = home.join("Starting Project");
    WriterApp::create_project(&starting_project, "Starting Project").unwrap();
    let python = Command::new("python3").arg("--version").output();
    if python.is_err() {
        eprintln!("skipping Linux PTY workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&starting_project)
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

    let resumed = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project_path)
        .arg(&home)
        .arg("continue-fiction")
        .output()
        .expect("resume the saved fiction project through a second PTY session");
    assert!(
        resumed.status.success(),
        "PTY reopen/continue workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    app = WriterApp::open_project(&project_path).unwrap();
    assert_eq!(
        app.load_scene_text(scenes[0].scene.id).unwrap().0,
        "First passage. Continued."
    );

    // The PTY created both the chapter and its scene through the author UI.
    // Verify that the persisted structure is used by the production compiler.
    assert!(
        app.list_structural_nodes()
            .unwrap()
            .iter()
            .any(|node| node.title == "Opening Chapter")
    );
    let compiled = app
        .compile_artifact(CompileFormat::Markdown, false)
        .unwrap();
    let manuscript = String::from_utf8(compiled.bytes).unwrap();
    assert!(manuscript.contains("Opening Chapter"));
    assert!(manuscript.contains("First passage."));
}

#[cfg(target_os = "linux")]
#[test]
fn full_screen_terminal_creates_and_writes_a_nonfiction_chapter_without_scenes() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;
    use story_core::BookType;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let starting_project = home.join("Starting Project");
    WriterApp::create_project(&starting_project, "Starting Project").unwrap();
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(&harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&starting_project)
        .arg(&home)
        .arg("create-nonfiction")
        .output()
        .expect("run the Linux PTY non-fiction lifecycle");
    assert!(
        result.status.success(),
        "PTY non-fiction workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let project_path = home.join("My Book");
    let app = WriterApp::open_project(&project_path).unwrap();
    assert_eq!(app.get_project().unwrap().book_type, BookType::Nonfiction);
    assert!(app.list_scenes(false).unwrap().is_empty());
    let chapters = app.list_structural_nodes().unwrap();
    assert!(chapters.iter().any(|node| node.title == "Opening Chapter"));
    assert!(chapters.iter().all(|node| node.kind != "scene"));
    let chapter_id = chapters
        .iter()
        .find(|node| node.title == "Opening Chapter")
        .unwrap()
        .id;
    let content = app.list_content_units(false).unwrap();
    let chapter_text = content
        .iter()
        .find(|unit| {
            unit.kind == story_core::ContentUnitKind::ChapterBody
                && unit.parent_id == Some(chapter_id)
        })
        .map(|unit| app.load_content_unit_text(unit.id).unwrap().0)
        .unwrap();
    assert!(chapter_text.contains("A chapter written without compulsory scenes."));
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
    std::fs::create_dir_all(project.join(".writers-ide")).unwrap();
    std::fs::write(
        project.join(".writers-ide/editor-preferences.toml"),
        "[editor]\ntime_zone = \"utc\"\n",
    )
    .unwrap();

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
fn review_menu_exposes_spellcheck_and_grammar_actions_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Review Menu Project");
    let mut app = WriterApp::create_project(&project, "Review Menu Project").unwrap();
    let chapter = app.create_chapter(None, "Opening", 0).unwrap();
    app.create_scene(chapter, "Arrival", 0).unwrap();
    drop(app);

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY Review-menu workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("review-menu")
        .output()
        .expect("run the Linux PTY Review-menu workflow");
    assert!(
        result.status.success(),
        "PTY Review-menu workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn offline_thesaurus_runs_from_the_review_menu_in_a_real_pty() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Thesaurus Project");
    WriterApp::create_project(&project, "Thesaurus Project").unwrap();
    let dictionary = root.path().join("test-mythes.dat");
    std::fs::write(&dictionary, "UTF-8\n1\ncolour|1\n(noun)|hue|pigment\n").unwrap();

    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping Linux PTY thesaurus workflow: python3 is unavailable");
        return;
    }
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("review-thesaurus")
        .env("WIDE_THESAURUS_DATA", &dictionary)
        .output()
        .expect("run the Linux PTY thesaurus workflow");
    assert!(
        result.status.success(),
        "PTY thesaurus workflow failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
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
fn production_menu_authoring_workflow_creates_edits_and_links_manuscript_documents() {
    let _guard = lock_pty_test();
    use std::process::Command;
    use story_app::WriterApp;

    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("Authoring Audit Project");
    WriterApp::create_project(&project, "Authoring Audit Project").unwrap();
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pty_smoke.py");
    let result = Command::new("python3")
        .arg(harness)
        .arg(env!("CARGO_BIN_EXE_writers-ide-terminal"))
        .arg(&project)
        .arg(root.path())
        .arg("authoring-audit")
        .output()
        .expect("run the production authoring-audit PTY workflow");
    assert!(
        result.status.success(),
        "production authoring audit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let reopened = WriterApp::open_project(&project).unwrap();
    assert_eq!(
        reopened.get_project().unwrap().book_type,
        story_core::BookType::Nonfiction
    );
    let nodes = reopened.list_structural_nodes().unwrap();
    let section = nodes.iter().find(|node| node.title == "Part One").unwrap();
    let chapter = nodes
        .iter()
        .find(|node| node.title == "Chapter One")
        .unwrap();
    assert_eq!(chapter.parent_id, Some(section.id));
    let scenes = reopened.list_scenes(false).unwrap();
    let opening = scenes
        .iter()
        .find(|scene| scene.scene.title == "Opening")
        .unwrap();
    let discovery = scenes
        .iter()
        .find(|scene| scene.scene.title == "Discovery")
        .unwrap();
    assert!(reopened.scene_is_included(opening.scene.id).unwrap());
    assert_eq!(opening.chapter_id, Some(chapter.id));
    assert_eq!(
        reopened.load_scene_text(opening.scene.id).unwrap().0,
        "The map revealed a hidden pass."
    );
    assert_eq!(
        reopened
            .outgoing_links(story_core::EntityRef::Scene(opening.scene.id))
            .unwrap()[0]
            .target,
        story_core::EntityRef::Scene(discovery.scene.id)
    );
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
