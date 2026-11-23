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

    if std::process::Command::new("pandoc")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
    {
        let docx = reopened
            .compile_artifact(CompileFormat::Docx, false)
            .unwrap();
        assert!(docx.bytes.starts_with(b"PK"));
    }
}
