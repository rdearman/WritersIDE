use std::process::Command;
use story_app::WriterApp;
use story_core::{
    BookType, ContentUnitKind, CustomFieldDefinition, FieldId, FieldType, FieldValue,
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_writers-ide"))
}

#[test]
fn saved_custom_template_recreates_structure_without_project_content() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source");
    let template_path = directory.path().join("templates/field-guide.toml");
    let destination = directory.path().join("new-book");
    let mut source = WriterApp::create_project_with_type(
        &source_path,
        "Private Source Book",
        BookType::Nonfiction,
    )
    .unwrap();
    let section = source.create_section("Part One", 0).unwrap();
    let subsection = source.create_section("Background", 1).unwrap();
    source
        .move_structural_node(subsection, Some(section), 0)
        .unwrap();
    let chapter = source
        .create_chapter(Some(subsection), "Chapter One", 0)
        .unwrap();
    let body = source
        .create_content_unit(
            ContentUnitKind::ChapterBody,
            "Chapter One prose",
            Some(chapter),
            0,
            Some(1200),
        )
        .unwrap();
    let (_, hash) = source.load_content_unit_text(body).unwrap();
    source
        .save_content_unit_text(body, Some(&hash), "PRIVATE PROSE MUST NOT ENTER TEMPLATE")
        .unwrap();
    source
        .define_custom_field(&CustomFieldDefinition {
            id: FieldId::new(),
            name: "Review state".into(),
            field_type: FieldType::Enum,
            required: false,
            default_value: Some(FieldValue::Enum("draft".into())),
            enum_values: vec!["draft".into(), "review".into()],
            reference_kind: None,
        })
        .unwrap();
    let source_id = source.get_project().unwrap().id;
    drop(source);

    std::fs::create_dir_all(template_path.parent().unwrap()).unwrap();
    let saved = cli()
        .args([
            "template-save",
            source_path.to_str().unwrap(),
            "field-guide",
            "Field Guide",
            template_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let encoded = std::fs::read_to_string(&template_path).unwrap();
    assert!(!encoded.contains("PRIVATE PROSE"));
    assert!(!encoded.contains("Private Source Book"));
    assert!(!encoded.contains(&source_id.to_string()));
    assert!(String::from_utf8_lossy(&saved.stdout).contains("prose"));

    let checked = cli()
        .args(["template-check", template_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );

    let created = cli()
        .args([
            "create-from-template",
            destination.to_str().unwrap(),
            template_path.to_str().unwrap(),
            "New Field Guide",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let restored = WriterApp::open_project(&destination).unwrap();
    assert_eq!(
        restored.get_project().unwrap().book_type,
        BookType::Nonfiction
    );
    assert_ne!(restored.get_project().unwrap().id, source_id);
    let nodes = restored.list_structural_nodes().unwrap();
    assert_eq!(nodes.len(), 3);
    let part = nodes.iter().find(|node| node.title == "Part One").unwrap();
    let subsection = nodes
        .iter()
        .find(|node| node.title == "Background")
        .unwrap();
    let chapter = nodes
        .iter()
        .find(|node| node.title == "Chapter One")
        .unwrap();
    assert_eq!(subsection.parent_id, Some(part.id));
    assert_eq!(chapter.parent_id, Some(subsection.id));
    assert!(restored.list_scenes(false).unwrap().is_empty());
    let units = restored.list_content_units(false).unwrap();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].title, "Chapter One prose");
    assert_eq!(units[0].target_words, Some(1200));
    assert!(
        restored
            .load_content_unit_text(units[0].id)
            .unwrap()
            .0
            .is_empty()
    );
    let fields = restored.list_custom_field_definitions().unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Review state");
    assert_eq!(
        fields[0].default_value,
        Some(FieldValue::Enum("draft".into()))
    );

    let existing_output = directory.path().join("must-not-overwrite.toml");
    std::fs::write(&existing_output, "author-owned data").unwrap();
    let refused = cli()
        .args([
            "template-save",
            source_path.to_str().unwrap(),
            "field-guide",
            "Field Guide",
            existing_output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        std::fs::read_to_string(existing_output).unwrap(),
        "author-owned data"
    );
}

#[test]
fn template_cli_rejects_invalid_template_without_creating_a_project() {
    let directory = tempfile::tempdir().unwrap();
    let template = directory.path().join("invalid.toml");
    let destination = directory.path().join("must-not-exist");
    std::fs::write(
        &template,
        "schema_version = 1\ntemplate_revision = 1\nid = 'bad'\nname = 'Bad'\nbook_type = 'nonfiction'\nunmapped = 'data'\n",
    )
    .unwrap();
    let checked = cli()
        .args(["template-check", template.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let created = cli()
        .args([
            "create-from-template",
            destination.to_str().unwrap(),
            template.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!created.status.success());
    assert!(!destination.exists());
}
