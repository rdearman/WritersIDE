use std::process::Command;

#[test]
fn create_series_new_book_reopen_and_write_without_series_database() {
    let temp = tempfile::tempdir().unwrap();
    let series_root = temp.path().join("Chronicles");
    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let created = Command::new(executable)
        .args([
            "series-create",
            series_root.to_str().unwrap(),
            "The Chronicles",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );

    let new_book = Command::new(executable)
        .args([
            "series-new-book",
            series_root.to_str().unwrap(),
            "Book 3",
            "The Third Book",
        ])
        .output()
        .unwrap();
    assert!(
        new_book.status.success(),
        "{}",
        String::from_utf8_lossy(&new_book.stderr)
    );
    let book_root = series_root.join("Book 3");
    let membership: story_series::BookMembership = toml::from_str(
        &std::fs::read_to_string(book_root.join(".writers-ide/series-membership.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(membership.format_version, 1);

    let series = story_series::SeriesStore::open(&series_root).unwrap();
    let books = series.list_books().unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, membership.book_id);
    assert_eq!(books[0].relative_path, "Book 3");
    let project_id = books[0].project_id;
    drop(series);

    let mut app = story_app::WriterApp::open_project(&book_root).unwrap();
    assert_eq!(app.get_project().unwrap().id, project_id);
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(scene, Some(&hash), "Book 3 begins here.")
        .unwrap();
    drop(app);

    std::fs::rename(&series_root, temp.path().join("Relocated Series")).unwrap();
    let relocated_book = temp.path().join("Relocated Series/Book 3");
    std::fs::rename(
        temp.path().join("Relocated Series/series.db"),
        temp.path().join("Relocated Series/series.db.offline"),
    )
    .unwrap();
    let app = story_app::WriterApp::open_project(&relocated_book).unwrap();
    assert_eq!(app.load_scene_text(scene).unwrap().0, "Book 3 begins here.");
}
