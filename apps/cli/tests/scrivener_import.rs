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
