use std::{io::Cursor, process::Command};

fn synthetic_source(
    root: &std::path::Path,
    name: &str,
    prose: &str,
) -> (std::path::PathBuf, String) {
    let source = root.join(format!("{name}.scriv"));
    let content_dir = source.join("Files/Data/scene-1");
    std::fs::create_dir_all(&content_dir).unwrap();
    std::fs::write(
        source.join(format!("{name}.scrivx")),
        r#"<ScrivenerProject Version="2.0"><Binder>
          <BinderItem UUID="draft-root" Type="DraftFolder"><Title>Draft</Title><Children>
            <BinderItem UUID="chapter-1" Type="Folder"><Title>Chapter One</Title><Children>
              <BinderItem UUID="scene-1" Type="Text"><Title>Opening</Title><IncludeInCompile>Yes</IncludeInCompile><Children/></BinderItem>
            </Children></BinderItem>
          </Children></BinderItem>
        </Binder></ScrivenerProject>"#,
    )
    .unwrap();
    std::fs::write(
        content_dir.join("content.rtf"),
        format!(r"{{\rtf1 {prose}}}"),
    )
    .unwrap();
    let fingerprint = story_import::inspect_scrivener_project(&source)
        .unwrap()
        .source_fingerprint;
    (source, fingerprint)
}

fn import_into_series(
    executable: &str,
    temp: &std::path::Path,
    series: &std::path::Path,
    number: u8,
    name: &str,
    prose: &str,
) -> (std::path::PathBuf, String) {
    let (source, source_fingerprint) = synthetic_source(temp, name, prose);
    let inspect = Command::new(executable)
        .args(["inspect-scrivener", source.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(inspect.status.success());
    let mut preview: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    {
        let draft_root = preview["proposed_plan"]["decisions"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|decision| decision["source_document_id"] == "draft-root")
            .unwrap();
        draft_root["destination_kind"] = serde_json::json!("Section");
    }
    let folder = preview["proposed_plan"]["decisions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|decision| decision["source_document_id"] == "chapter-1")
        .unwrap();
    folder["destination_kind"] = serde_json::json!("Chapter");
    let mapping = temp.join(format!("{name}-mapping.json"));
    std::fs::write(
        &mapping,
        serde_json::to_vec_pretty(&preview["proposed_plan"]).unwrap(),
    )
    .unwrap();
    let book = series.join(format!("Book {number}"));
    let imported = Command::new(executable)
        .args([
            "import-scrivener",
            source.to_str().unwrap(),
            book.to_str().unwrap(),
            mapping.to_str().unwrap(),
            &format!("Book {number}"),
        ])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let added = Command::new(executable)
        .args([
            "series-add-book",
            series.to_str().unwrap(),
            book.to_str().unwrap(),
            &format!("Book {number}"),
            &source_fingerprint,
        ])
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    (book, source_fingerprint)
}

#[test]
fn synthetic_two_book_scrivener_series_creates_writable_book_three_and_epub() {
    // Synthetic workflow integration only; these files are not Scrivener
    // compatibility fixtures and must not be used to claim supported versions.
    let temp = tempfile::tempdir().unwrap();
    let series_root = temp.path().join("Chronicles");
    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let created = Command::new(executable)
        .args(["series-create", series_root.to_str().unwrap(), "Chronicles"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let (book_one, source_one_hash) = import_into_series(
        executable,
        temp.path(),
        &series_root,
        1,
        "Source One",
        "Book one prose.",
    );
    let (book_two, source_two_hash) = import_into_series(
        executable,
        temp.path(),
        &series_root,
        2,
        "Source Two",
        "Book two prose.",
    );
    let series = story_series::SeriesStore::open(&series_root).unwrap();
    let membership = series.list_books().unwrap();
    assert_eq!(membership.len(), 2);
    assert_ne!(membership[0].project_id, membership[1].project_id);
    assert_eq!(membership[0].position, 0);
    assert_eq!(membership[1].position, 1);
    drop(series);

    // Editing Book 1 remains isolated from Book 2 and from both source folders.
    let mut first = story_app::WriterApp::open_project(&book_one).unwrap();
    let first_scene = first.list_scenes(false).unwrap()[0].scene.id;
    let (_, expected) = first.load_scene_text(first_scene).unwrap();
    first
        .save_scene_text(first_scene, Some(&expected), "Edited Book 1 prose.")
        .unwrap();
    drop(first);
    let second = story_app::WriterApp::open_project(&book_two).unwrap();
    assert_eq!(
        second
            .load_scene_text(second.list_scenes(false).unwrap()[0].scene.id)
            .unwrap()
            .0,
        "Book two prose."
    );
    assert_eq!(
        story_import::inspect_scrivener_project(temp.path().join("Source One.scriv"))
            .unwrap()
            .source_fingerprint,
        source_one_hash
    );
    assert_eq!(
        story_import::inspect_scrivener_project(temp.path().join("Source Two.scriv"))
            .unwrap()
            .source_fingerprint,
        source_two_hash
    );

    let third = Command::new(executable)
        .args([
            "series-new-book",
            series_root.to_str().unwrap(),
            "Book 3",
            "Book Three",
        ])
        .output()
        .unwrap();
    assert!(
        third.status.success(),
        "{}",
        String::from_utf8_lossy(&third.stderr)
    );
    let third_path = series_root.join("Book 3");
    let mut third_app = story_app::WriterApp::open_project(&third_path).unwrap();
    let chapter = third_app.create_chapter(None, "Chapter One", 0).unwrap();
    let body = third_app
        .create_content_unit(
            story_core::ContentUnitKind::ChapterBody,
            "Chapter One",
            Some(chapter),
            0,
            None,
        )
        .unwrap();
    let (_, expected) = third_app.load_content_unit_text(body).unwrap();
    third_app
        .save_content_unit_text(body, Some(&expected), "Book 3 starts immediately.")
        .unwrap();
    drop(third_app);
    let third_app = story_app::WriterApp::open_project(&third_path).unwrap();
    assert!(
        third_app
            .load_content_unit_text(body)
            .unwrap()
            .0
            .contains("Book 3 starts immediately.")
    );
    let epub = third_app
        .compile_artifact(story_document::CompileFormat::Epub, false)
        .unwrap();
    assert_eq!(&epub.bytes[..2], b"PK");
    let mut archive = zip::ZipArchive::new(Cursor::new(epub.bytes)).unwrap();
    let mut found_prose = false;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if file.name().ends_with(".xhtml") || file.name().ends_with(".html") {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut file, &mut text).unwrap();
            found_prose |= text.contains("Book 3 starts immediately.");
        }
    }
    assert!(found_prose, "Book 3 EPUB did not contain the saved prose");
}
