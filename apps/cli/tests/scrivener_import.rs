use sha2::Digest;
use std::{fs, process::Command};

#[test]
fn synthetic_scrivener_preview_then_atomic_both_mode_import() {
    // This fixture verifies the CLI's staging contract only. It does not
    // establish compatibility with actual Scrivener-produced projects.
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("Sample.scriv");
    let scene_dir = source.join("Files/Data/scene-1");
    std::fs::create_dir_all(&scene_dir).unwrap();
    std::fs::write(
        source.join("Sample.scrivx"),
        r#"<ScrivenerProject Version="2.0"><Binder>
          <BinderItem UUID="chapter-1" Type="Folder"><Title>Chapter One</Title><Children>
            <BinderItem UUID="scene-1" Type="Text"><Title>Opening</Title><Children/></BinderItem>
          </Children></BinderItem>
        </Binder></ScrivenerProject>"#,
    )
    .unwrap();
    std::fs::write(scene_dir.join("content.rtf"), r"{\rtf1 Hello, world.}").unwrap();

    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let inspected = Command::new(executable)
        .args(["inspect-scrivener", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(inspected.status.success());
    let mut preview: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(preview["inventory"]["items"].as_array().unwrap().len(), 2);
    let plan = preview["proposed_plan"]["decisions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|decision| decision["source_document_id"] == "chapter-1")
        .unwrap();
    plan["destination_kind"] = serde_json::json!("Chapter");
    let mapping = temp.path().join("mapping.json");
    std::fs::write(
        &mapping,
        serde_json::to_vec_pretty(&preview["proposed_plan"]).unwrap(),
    )
    .unwrap();
    let fingerprint = preview["inventory"]["source_fingerprint"]
        .as_str()
        .unwrap()
        .to_owned();

    let destination = temp.path().join("Imported Book");
    let imported = Command::new(executable)
        .args([
            "import-scrivener",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
            mapping.to_str().unwrap(),
            "Imported Book",
        ])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(destination.join("project.toml").is_file());
    assert!(destination.join("story.db").is_file());
    assert!(
        destination
            .join("exports/scrivener-import-report.json")
            .is_file()
    );
    assert_eq!(
        story_import::inspect_scrivener_project(&source)
            .unwrap()
            .source_fingerprint,
        fingerprint
    );
    let reference_root = destination.join("research/imported_sources");
    assert_eq!(std::fs::read_dir(reference_root).unwrap().count(), 1);
}

#[test]
fn real_scrivener_314_fixture_import_preserves_text_order_and_source() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scrivener-starter/test.scriv");
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("test.scriv");
    fs::create_dir(&source).unwrap();
    copy_tree(&fixture, &source);
    let original = story_import::inspect_scrivener_project(&source).unwrap();
    assert_eq!(original.format_version.as_deref(), Some("2.0"));
    assert_eq!(original.creator.as_deref(), Some("SCRMAC-3.1.4-12105"));
    assert_eq!(original.items.len(), 5);
    assert_eq!(
        original.items[1].content_markdown.as_deref(),
        Some("Test\n\nThis is an addition to the old file.")
    );
    assert_eq!(
        original.items[2].content_markdown.as_deref(),
        Some("And this is a new file test")
    );
    let fingerprint = original.source_fingerprint.clone();
    let mut plan = story_import::ScrivenerImportPlan::proposed(&original);
    for item in &original.items {
        let decision = plan
            .decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == item.source_document_id)
            .unwrap();
        match item.type_code.as_deref() {
            Some("DraftFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Chapter;
                decision.title_override = Some("Draft".into());
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
            other => panic!("unexpected binder item type: {other:?}"),
        }
    }
    let preview = plan.preview(&original).unwrap();
    assert!(preview.unresolved_source_ids.is_empty());
    assert_eq!(preview.total_items, 5);
    assert_eq!(preview.manuscript_items, 3); // Chapter plus two scenes.
    assert_eq!(preview.research_items, 1);
    assert_eq!(preview.skipped_items, 1);
    assert!(preview.unsupported.iter().any(|item| item.contains("RTF")));

    let destination = temp.path().join("Imported Book");
    let mut app = story_app::WriterApp::create_project(&destination, "Imported Book").unwrap();
    let (run, imported_scenes) = app
        .import_scrivener_inventory(&original, &plan, story_import::ScrivenerImportMode::Both)
        .unwrap();
    app.validate_project().unwrap();
    let scenes = app.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 2);
    assert_eq!(scenes[0].scene.title, "Opening");
    assert_eq!(scenes[1].scene.title, "new file");
    assert_eq!(imported_scenes.len(), 2);
    assert_eq!(
        app.load_scene_text(scenes[0].scene.id).unwrap().0,
        "Test\n\nThis is an addition to the old file."
    );
    assert_eq!(
        app.load_scene_text(scenes[1].scene.id).unwrap().0,
        "And this is a new file test"
    );
    assert_eq!(app.list_reading_documents().unwrap().len(), 1);
    let report = app.get_import_run_summary(run).unwrap();
    assert_eq!(report.status, "incomplete"); // Font/layout fidelity is explicitly unresolved.
    drop(app);

    let reference = destination.join("research/imported_sources");
    let retained = fs::read_dir(reference)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        story_import::inspect_scrivener_project(&retained)
            .unwrap()
            .source_fingerprint,
        fingerprint
    );
    assert_eq!(
        story_import::inspect_scrivener_project(&source)
            .unwrap()
            .source_fingerprint,
        fingerprint
    );
}

#[test]
fn real_scrivener_25_fixture_is_reported_without_claiming_content_support() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scrivener-starter/test Backup.scriv");
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("Legacy.scriv");
    fs::create_dir(&source).unwrap();
    copy_tree(&fixture, &source);
    let inventory = story_import::inspect_scrivener_project(source).unwrap();
    assert_eq!(inventory.creator.as_deref(), Some("SCRMAC-2.5-25239"));
    assert_eq!(inventory.format_version.as_deref(), Some("1.0"));
    assert!(
        inventory
            .items
            .iter()
            .all(|item| item.content_markdown.is_none())
    );
    assert!(
        inventory
            .unsupported
            .iter()
            .any(|warning| warning.contains("revision 1.0"))
    );
}

#[test]
fn sanitized_windows_311_regression_fixture_preserves_structure_and_formatting() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scrivener-windows-311/Windows Regression.scriv");
    let temp = tempfile::tempdir().unwrap();
    let before = hash_file_tree(&source);
    let inventory = story_import::inspect_scrivener_project(&source).unwrap();
    assert_eq!(inventory.creator.as_deref(), Some("SCRWIN-3.1.1.0"));
    assert_eq!(inventory.items.len(), 5);
    assert_eq!(inventory.items[1].position, 0);
    assert_eq!(inventory.items[2].position, 1);
    assert_eq!(inventory.items[3].position, 2);
    assert_eq!(inventory.items[1].include_in_compile, Some(true));
    assert_eq!(inventory.items[4].include_in_compile, Some(false));

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
            Some("Text") if item.area == "manuscript" => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Scene;
            }
            Some("ResearchFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Research;
            }
            Some("TrashFolder") | Some("Text") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Skip;
                decision.include_in_import = false;
            }
            other => panic!("unexpected sanitized binder type: {other:?}"),
        }
    }
    let preview = plan.preview(&inventory).unwrap();
    assert!(preview.unresolved_source_ids.is_empty());
    assert_eq!(preview.manuscript_items, 2);
    assert_eq!(preview.research_items, 1);
    assert_eq!(preview.skipped_items, 2);

    let destination = temp.path().join("Imported");
    let mut app = story_app::WriterApp::create_project(&destination, "Imported").unwrap();
    let (run, scenes) = app
        .import_scrivener_inventory(&inventory, &plan, story_import::ScrivenerImportMode::Both)
        .unwrap();
    assert_eq!(
        app.get_import_run_summary(run).unwrap().status,
        "incomplete"
    );
    assert_eq!(scenes.len(), 1);
    let text = app.load_scene_text(scenes[0]).unwrap().0;
    assert!(text.contains("A quiet, generic scene — with **bold** and *italic* words."));
    assert!(
        app.get_scene(scenes[0])
            .unwrap()
            .scene
            .synopsis
            .contains("generic opening scene")
    );
    assert_eq!(app.list_reading_documents().unwrap().len(), 1);
    app.validate_project().unwrap();
    drop(app);
    assert_eq!(hash_file_tree(&source), before);
}

#[test]
fn author_scrivener_project_import_and_series_book_three_smoke() {
    // The author-provided project is deliberately excluded from Git. This test
    // runs when that local fixture is available and never writes into it.
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-fixtures/scrivener/Adderand.scriv");
    if !source.is_dir() {
        eprintln!("author-provided Scrivener fixture unavailable; skipped local smoke");
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let source_file_hashes = hash_file_tree(&source);
    let original = story_import::inspect_scrivener_project(&source).unwrap();
    assert_eq!(original.format_version.as_deref(), Some("2.0"));
    assert_eq!(original.creator.as_deref(), Some("SCRWIN-3.1.1.0"));
    assert_eq!(original.items.len(), 5);
    assert!(original.items[0].parent_source_id.is_none());
    assert_eq!(original.items[0].position, 0);
    assert_eq!(original.items[2].position, 1);
    assert_eq!(original.items[3].position, 2);
    assert_eq!(
        original.items[1].parent_source_id.as_deref(),
        Some(original.items[0].source_document_id.as_str())
    );
    assert_eq!(
        original.items[4].parent_source_id.as_deref(),
        Some(original.items[3].source_document_id.as_str())
    );
    assert_eq!(original.items[1].position, 0);
    assert_eq!(original.items[4].position, 0);
    assert!(
        original.items[1]
            .content_markdown
            .as_ref()
            .is_some_and(|text| !text.is_empty())
    );
    assert_eq!(original.items[1].include_in_compile, Some(true));
    assert!(original.items[1].fields.contains_key("TextSelection"));

    let inspected = Command::new(env!("CARGO_BIN_EXE_writers-ide"))
        .args(["inspect-scrivener", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(inspected.status.success(), "Scrivener inventory failed");
    let mut preview: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    let decisions = preview["proposed_plan"]["decisions"]
        .as_array_mut()
        .unwrap();
    for item in &original.items {
        let decision = decisions
            .iter_mut()
            .find(|decision| decision["source_document_id"] == item.source_document_id)
            .unwrap();
        let (destination_kind, include) = match (item.area.as_str(), item.type_code.as_deref()) {
            ("trash", _) => ("Skip", false),
            (_, Some("DraftFolder")) => ("Chapter", true),
            ("manuscript", Some("Text")) => ("Scene", true),
            ("research", _) => ("Research", true),
            _ => panic!("unhandled fixture binder type"),
        };
        decision["destination_kind"] = serde_json::json!(destination_kind);
        decision["include_in_import"] = serde_json::json!(include);
    }
    let series_root = temp.path().join("Imported Series");
    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let created_series = Command::new(executable)
        .args([
            "series-create",
            series_root.to_str().unwrap(),
            "Imported Series",
        ])
        .output()
        .unwrap();
    assert!(created_series.status.success(), "series creation failed");

    let mapping = temp.path().join("scrivener-mapping.json");
    fs::write(
        &mapping,
        serde_json::to_vec_pretty(&preview["proposed_plan"]).unwrap(),
    )
    .unwrap();
    let destination = series_root.join("Book 1");
    let imported = Command::new(executable)
        .args([
            "import-scrivener",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
            mapping.to_str().unwrap(),
            "Imported Novel",
        ])
        .output()
        .unwrap();
    assert!(imported.status.success(), "Scrivener import failed");
    let added_to_series = Command::new(executable)
        .args([
            "series-add-book",
            series_root.to_str().unwrap(),
            destination.to_str().unwrap(),
            "Book 1",
            &original.source_fingerprint,
        ])
        .output()
        .unwrap();
    assert!(
        added_to_series.status.success(),
        "Book 1 series registration failed"
    );

    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(destination.join("exports/scrivener-import-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["inventory"]["items"].as_array().unwrap().len(), 5);
    assert_eq!(report["imported_scene_ids"].as_array().unwrap().len(), 1);
    assert_eq!(report["import_run"]["status"], "incomplete");
    assert_eq!(report["import_run"]["source_items"], 5);
    assert_eq!(report["import_run"]["provenance_records"], 3);
    assert!(
        report["inventory"]["items"][1]["unsupported"]
            .as_array()
            .is_some_and(|values| values
                .iter()
                .any(|value| { value.as_str().is_some_and(|text| text.contains("RTF")) }))
    );

    let scene_id: story_core::SceneId =
        serde_json::from_value(report["imported_scene_ids"][0].clone()).unwrap();
    let expected_source_text = original.items[1].content_markdown.as_deref().unwrap();
    let mut app = story_app::WriterApp::open_project(&destination).unwrap();
    let scenes = app.list_scenes(false).unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0].scene.id, scene_id);
    assert_eq!(scenes[0].scene.title, original.items[1].title);
    let structural_nodes = app.list_structural_nodes().unwrap();
    assert_eq!(structural_nodes.len(), 1);
    assert_eq!(structural_nodes[0].kind, "chapter");
    assert_eq!(structural_nodes[0].title, original.items[0].title);
    assert_eq!(app.list_reading_documents().unwrap().len(), 1);
    let (imported_text, expected_hash) = app.load_scene_text(scene_id).unwrap();
    assert!(
        imported_text == expected_source_text,
        "imported manuscript differs from inspected RTF text"
    );
    assert!(
        imported_text.contains("**") && imported_text.contains('*'),
        "bold and italic RTF runs were not represented"
    );
    assert_eq!(
        imported_text.chars().filter(|ch| !ch.is_ascii()).count(),
        366
    );

    // Exercise the WYSIWYM semantic contract with the locally supplied real
    // manuscript, without embedding its source text in this regression test.
    let semantic = story_document::SemanticDocument::parse(imported_text.clone());
    assert!(semantic.validate_ranges());
    assert_eq!(semantic.to_markdown().as_bytes(), imported_text.as_bytes());
    assert!(
        semantic
            .nodes
            .iter()
            .any(|node| matches!(node.kind, story_document::SemanticKind::Paragraph))
    );
    assert!(
        semantic
            .nodes
            .iter()
            .any(|node| matches!(node.kind, story_document::SemanticKind::Strong))
    );
    assert!(
        semantic
            .nodes
            .iter()
            .any(|node| matches!(node.kind, story_document::SemanticKind::Emphasis))
    );

    let markdown_artifact = app
        .compile_artifact(story_document::CompileFormat::Markdown, false)
        .unwrap();
    let compiled_markdown = String::from_utf8(markdown_artifact.bytes).unwrap();
    let compiled_semantic = story_document::SemanticDocument::parse(compiled_markdown.clone());
    assert!(compiled_semantic.validate_ranges());
    assert_eq!(
        compiled_semantic.to_markdown().as_bytes(),
        compiled_markdown.as_bytes()
    );
    assert!(markdown_artifact.source_map.iter().any(|entry| {
        entry.source_entity_id == scene_id.to_string()
            && entry.source_sha256 == semantic.source_sha256
            && entry.source.end <= imported_text.len()
            && imported_text.is_char_boundary(entry.source.start)
            && imported_text.is_char_boundary(entry.source.end)
    }));

    assert!(app.get_scene(scene_id).unwrap().scene.synopsis.is_empty());

    let marker = "\n\nWriter's IDE import regression edit.";
    app.save_scene_text(
        scene_id,
        Some(&expected_hash),
        &(imported_text.clone() + marker),
    )
    .unwrap();
    drop(app);

    let reopened = story_app::WriterApp::open_project(&destination).unwrap();
    let (edited_text, _) = reopened.load_scene_text(scene_id).unwrap();
    assert!(
        edited_text == format!("{imported_text}{marker}"),
        "scene edit did not survive reopening"
    );
    let search = reopened.search_scenes("regression edit").unwrap();
    assert!(
        search.contains(&scene_id),
        "search did not index edited imported prose"
    );
    let epub = reopened
        .compile_artifact(story_document::CompileFormat::Epub, false)
        .unwrap();
    assert!(!epub.bytes.is_empty());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(epub.bytes)).unwrap();
    let mut found_edit = false;
    let mut html_members = 0usize;
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        html_members +=
            usize::from(member.name().ends_with(".xhtml") || member.name().ends_with(".html"));
        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut member, &mut contents).unwrap();
        found_edit |= contents
            .windows(b"regression edit".len())
            .any(|window| window == b"regression edit");
    }
    assert!(
        found_edit,
        "compiled EPUB omitted the saved edit (members={}, HTML members={html_members})",
        archive.len()
    );
    reopened.validate_project().unwrap();
    drop(reopened);

    let retained = destination.join("research/imported_sources");
    let retained_project = fs::read_dir(retained)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        story_import::inspect_scrivener_project(&retained_project)
            .unwrap()
            .source_fingerprint,
        original.source_fingerprint,
    );
    assert_eq!(
        story_import::inspect_scrivener_project(&source)
            .unwrap()
            .source_fingerprint,
        original.source_fingerprint,
        "the Scrivener source changed during import or editing",
    );
    assert_eq!(hash_file_tree(&source), source_file_hashes);

    // Pair the supplied Windows project with a second, distinct authentic
    // Scrivener-produced project from the licensed small Mac fixture.
    let second_source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scrivener-starter/test.scriv");
    let second_source_hashes = hash_file_tree(&second_source);
    let second_inventory = story_import::inspect_scrivener_project(&second_source).unwrap();
    let mut second_plan = story_import::ScrivenerImportPlan::proposed(&second_inventory);
    for item in &second_inventory.items {
        let decision = second_plan
            .decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == item.source_document_id)
            .unwrap();
        match item.type_code.as_deref() {
            Some("DraftFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Chapter;
            }
            Some("Text") if item.area == "manuscript" => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Scene;
            }
            Some("ResearchFolder") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Research;
            }
            Some("TrashFolder") | Some("Text") => {
                decision.destination_kind = story_import::ScrivenerDestinationKind::Skip;
                decision.include_in_import = false;
            }
            other => panic!("unhandled second Scrivener fixture type: {other:?}"),
        }
    }
    assert!(
        second_plan
            .preview(&second_inventory)
            .unwrap()
            .unresolved_source_ids
            .is_empty()
    );
    let second_mapping = temp.path().join("second-scrivener-mapping.json");
    fs::write(
        &second_mapping,
        serde_json::to_vec_pretty(&second_plan).unwrap(),
    )
    .unwrap();
    let second_destination = series_root.join("Book 2");
    let second_import = Command::new(executable)
        .args([
            "import-scrivener",
            second_source.to_str().unwrap(),
            second_destination.to_str().unwrap(),
            second_mapping.to_str().unwrap(),
            "Book 2",
        ])
        .output()
        .unwrap();
    assert!(
        second_import.status.success(),
        "Book 2 Scrivener import failed"
    );
    let second_add = Command::new(executable)
        .args([
            "series-add-book",
            series_root.to_str().unwrap(),
            second_destination.to_str().unwrap(),
            "Book 2",
            &second_inventory.source_fingerprint,
        ])
        .output()
        .unwrap();
    assert!(
        second_add.status.success(),
        "Book 2 series registration failed"
    );

    let membership = story_series::SeriesStore::open(&series_root)
        .unwrap()
        .list_books()
        .unwrap();
    assert_eq!(membership.len(), 2);
    assert_ne!(membership[0].project_id, membership[1].project_id);
    assert_eq!(membership[0].position, 0);
    assert_eq!(membership[1].position, 1);
    let references = Command::new(executable)
        .args(["series-references", series_root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(references.status.success(), "series references failed");
    let references: serde_json::Value = serde_json::from_slice(&references.stdout).unwrap();
    assert!(
        references
            .as_array()
            .is_some_and(|documents| !documents.is_empty())
    );

    let second = story_app::WriterApp::open_project(&second_destination).unwrap();
    let second_scenes = second.list_scenes(false).unwrap();
    assert_eq!(second_scenes.len(), 2);
    let (second_text, _) = second.load_scene_text(second_scenes[0].scene.id).unwrap();
    assert!(
        second_text
            == second_inventory.items[1]
                .content_markdown
                .as_deref()
                .unwrap()
    );
    drop(second);

    let book_three = Command::new(executable)
        .args([
            "series-new-book",
            series_root.to_str().unwrap(),
            "Book 3",
            "Book 3",
        ])
        .output()
        .unwrap();
    assert!(book_three.status.success(), "Book 3 creation failed");
    let third_destination = series_root.join("Book 3");
    let mut third = story_app::WriterApp::open_project(&third_destination).unwrap();
    let chapter = third.create_chapter(None, "Chapter One", 0).unwrap();
    let prose = third
        .create_content_unit(
            story_core::ContentUnitKind::ChapterBody,
            "Chapter One",
            Some(chapter),
            0,
            None,
        )
        .unwrap();
    let (_, third_expected) = third.load_content_unit_text(prose).unwrap();
    third
        .save_content_unit_text(
            prose,
            Some(&third_expected),
            "Book 3 immediate drafting smoke text.",
        )
        .unwrap();
    drop(third);
    let third = story_app::WriterApp::open_project(&third_destination).unwrap();
    let (third_text, _) = third.load_content_unit_text(prose).unwrap();
    assert_eq!(third_text, "Book 3 immediate drafting smoke text.");
    assert!(third.list_scenes(false).unwrap().is_empty());
    let third_epub = third
        .compile_artifact(story_document::CompileFormat::Epub, false)
        .unwrap();
    assert!(!third_epub.bytes.is_empty());
    assert!(epub_contains(
        &third_epub.bytes,
        b"Book 3 immediate drafting smoke text."
    ));
    let (third_membership, _) =
        story_series::SeriesStore::open_for_book(&third_destination).unwrap();
    assert_eq!(third_membership.list_books().unwrap().len(), 3);
    assert_eq!(hash_file_tree(&source), source_file_hashes);
    assert_eq!(hash_file_tree(&second_source), second_source_hashes);
}

fn hash_file_tree(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, String> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        result: &mut std::collections::BTreeMap<std::path::PathBuf, String>,
    ) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, result);
            } else {
                let bytes = fs::read(&path).unwrap();
                result.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    format!("{:x}", sha2::Sha256::digest(bytes)),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn epub_contains(epub: &[u8], expected: &[u8]) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(epub)) else {
        return false;
    };
    for index in 0..archive.len() {
        let Ok(mut member) = archive.by_index(index) else {
            return false;
        };
        let mut contents = Vec::new();
        if std::io::Read::read_to_end(&mut member, &mut contents).is_err() {
            return false;
        }
        if contents
            .windows(expected.len())
            .any(|window| window == expected)
        {
            return true;
        }
    }
    false
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir(&destination_path).unwrap();
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).unwrap();
        }
    }
}
