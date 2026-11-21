use std::process::Command;

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
