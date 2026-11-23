use story_app::WriterApp;
use story_core::{BookType, ContentUnitKind};
use story_document::CompileFormat;

#[test]
fn fiction_book_workflow_creates_orders_excludes_edits_reopens_and_compiles() {
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Fiction");
    let mut app =
        WriterApp::create_project_with_type(&project_path, "Fiction", BookType::Fiction).unwrap();
    let first_chapter = app.create_chapter(None, "Opening", 0).unwrap();
    let second_chapter = app.create_chapter(None, "Later", 1).unwrap();
    let mut scenes = Vec::new();
    for index in 0..60 {
        let (chapter, position) = if index < 30 {
            (first_chapter, index)
        } else {
            (second_chapter, index - 30)
        };
        scenes.push(
            app.create_scene(chapter, &format!("Scene {index:02}"), position)
                .unwrap(),
        );
    }
    let first = scenes[0];
    let excluded = scenes[1];
    let (_, old_hash) = app.load_scene_text(first).unwrap();
    app.save_scene_text(first, Some(&old_hash), "The story opens here.")
        .unwrap();
    app.set_scene_included(excluded, false).unwrap();
    app.move_scene(first, second_chapter, 0).unwrap();
    app.reorder_scene(scenes[2], 0).unwrap();

    drop(app);
    let reopened = WriterApp::open_project(&project_path).unwrap();
    assert_eq!(reopened.list_scenes(false).unwrap().len(), 60);
    assert_eq!(
        reopened.load_scene_text(first).unwrap().0,
        "The story opens here."
    );
    assert!(!reopened.scene_is_included(excluded).unwrap());
    assert_eq!(
        reopened.get_scene(first).unwrap().chapter_id,
        Some(second_chapter)
    );
    let markdown = reopened
        .compile_artifact(CompileFormat::Markdown, false)
        .unwrap();
    let manuscript = String::from_utf8(markdown.bytes).unwrap();
    assert!(manuscript.contains("The story opens here."));
    assert!(!manuscript.contains("Scene 01"));
    assert!(manuscript.find("Scene 02").unwrap() < manuscript.find("Scene 00").unwrap());
    if pandoc_available() {
        let epub = reopened
            .compile_artifact(CompileFormat::Epub, false)
            .unwrap();
        assert_epub(&epub.bytes);
    }
}

#[test]
fn nonfiction_book_workflow_writes_chapter_prose_without_scenes_and_compiles() {
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Nonfiction");
    let mut app =
        WriterApp::create_project_with_type(&project_path, "Nonfiction", BookType::Nonfiction)
            .unwrap();
    let section = app.create_section("Part One", 0).unwrap();
    let first = app.create_chapter(Some(section), "Foundations", 0).unwrap();
    let second = app.create_chapter(Some(section), "Practice", 1).unwrap();
    let first_body = app
        .create_content_unit(
            ContentUnitKind::ChapterBody,
            "Foundations prose",
            Some(first),
            0,
            None,
        )
        .unwrap();
    let second_body = app
        .create_content_unit(
            ContentUnitKind::ChapterBody,
            "Practice prose",
            Some(second),
            0,
            None,
        )
        .unwrap();
    let (_, first_hash) = app.load_content_unit_text(first_body).unwrap();
    app.save_content_unit_text(first_body, Some(&first_hash), "Foundational evidence.")
        .unwrap();
    let (_, second_hash) = app.load_content_unit_text(second_body).unwrap();
    app.save_content_unit_text(
        second_body,
        Some(&second_hash),
        "Practice follows evidence.",
    )
    .unwrap();
    app.move_structural_node(second, Some(section), 0).unwrap();
    drop(app);

    let reopened = WriterApp::open_project(&project_path).unwrap();
    assert!(reopened.list_scenes(false).unwrap().is_empty());
    let markdown = reopened
        .compile_artifact(CompileFormat::Markdown, false)
        .unwrap();
    let manuscript = String::from_utf8(markdown.bytes).unwrap();
    assert!(manuscript.contains("Foundational evidence."));
    assert!(manuscript.contains("Practice follows evidence."));
    assert!(
        manuscript.find("Practice follows evidence.").unwrap()
            < manuscript.find("Foundational evidence.").unwrap()
    );

    if pandoc_available() {
        let docx = reopened
            .compile_artifact(CompileFormat::Docx, false)
            .unwrap();
        assert!(docx.bytes.starts_with(b"PK"));
        let epub = reopened
            .compile_artifact(CompileFormat::Epub, false)
            .unwrap();
        assert_epub(&epub.bytes);
    }
}

#[test]
fn scrivener_manuscript_workflow_imports_edits_reopens_and_compiles_standalone() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cli/tests/fixtures/scrivener-starter/test.scriv");
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Source.scriv");
    copy_tree(&fixture, &source);
    let original_fingerprint = story_import::inspect_scrivener_project(&source)
        .unwrap()
        .source_fingerprint;
    let inventory = story_import::inspect_scrivener_project(&source).unwrap();
    let mut plan = story_import::ScrivenerImportPlan::proposed(&inventory);
    for item in &inventory.items {
        let decision = plan
            .decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == item.source_document_id)
            .unwrap();
        match item.type_code.as_deref() {
            Some("DraftFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Chapter;
            }
            Some("Text") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Scene;
                if item.title.is_empty() {
                    decision.title_override = Some("Opening".into());
                }
            }
            Some("ResearchFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Research;
            }
            Some("TrashFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Skip;
                decision.include_in_import = false;
            }
            other => panic!("unexpected fixture binder type: {other:?}"),
        }
    }

    let destination = root.path().join("Imported Fiction");
    let mut app =
        WriterApp::create_project_with_type(&destination, "Imported Fiction", BookType::Fiction)
            .unwrap();
    let (_, imported) = app
        .import_scrivener_inventory(&inventory, &plan, story_import::ScrivenerImportMode::Both)
        .unwrap();
    assert_eq!(imported.len(), 2);
    let scenes = app.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 2);
    assert_eq!(
        app.load_scene_text(scenes[0].scene.id).unwrap().0,
        "Test\n\nThis is an addition to the old file."
    );
    let first = scenes[0].scene.id;
    let (_, hash) = app.load_scene_text(first).unwrap();
    app.save_scene_text(first, Some(&hash), "Continued imported scene.")
        .unwrap();
    let last_chapter = app
        .list_structural_nodes()
        .unwrap()
        .into_iter()
        .find(|node| node.kind == "chapter")
        .unwrap()
        .id;
    app.move_scene(first, last_chapter, 1).unwrap();
    let compiled = app
        .compile_artifact(CompileFormat::Markdown, false)
        .unwrap();
    let manuscript = String::from_utf8(compiled.bytes).unwrap();
    assert!(manuscript.contains("Continued imported scene."));
    drop(app);

    let reopened = WriterApp::open_project(&destination).unwrap();
    assert_eq!(reopened.list_scenes(false).unwrap().len(), 2);
    assert_eq!(
        reopened.load_scene_text(first).unwrap().0,
        "Continued imported scene."
    );
    assert_eq!(
        story_import::inspect_scrivener_project(&source)
            .unwrap()
            .source_fingerprint,
        original_fingerprint
    );
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn pandoc_available() -> bool {
    std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
}

fn assert_epub(bytes: &[u8]) {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut mimetype = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("mimetype").unwrap(), &mut mimetype)
        .unwrap();
    assert_eq!(mimetype, "application/epub+zip");
    assert!(archive.file_names().any(|name| name.ends_with(".opf")));
}
