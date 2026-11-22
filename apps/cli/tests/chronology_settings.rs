use std::process::Command;

#[test]
fn cli_configures_optional_chronology_and_updates_scene_time() {
    let directory = tempfile::tempdir().unwrap();
    let project_path = directory.path().join("Book");
    let mut app = story_app::WriterApp::create_project(&project_path, "Book").unwrap();
    let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
    let scene = app.create_scene(chapter, "Opening", 0).unwrap();
    let (_, original_hash) = app.load_scene_text(scene).unwrap();
    app.save_scene_text(
        scene,
        Some(&original_hash),
        "The manuscript remains intact.",
    )
    .unwrap();
    drop(app);

    let executable = env!("CARGO_BIN_EXE_writers-ide");
    let enabled = Command::new(executable)
        .args([
            "chronology-preferences",
            project_path.to_str().unwrap(),
            "on",
        ])
        .output()
        .unwrap();
    assert!(enabled.status.success());

    let timed = Command::new(executable)
        .args([
            "scene-time-set",
            project_path.to_str().unwrap(),
            &scene.to_string(),
            "-86400",
            "0",
        ])
        .output()
        .unwrap();
    assert!(timed.status.success());

    let boundary = Command::new(executable)
        .args([
            "scene-time-boundaries",
            project_path.to_str().unwrap(),
            &scene.to_string(),
            "exclusive",
            "inclusive",
        ])
        .output()
        .unwrap();
    assert!(boundary.status.success());

    let reopened = story_app::WriterApp::open_project(&project_path).unwrap();
    assert!(reopened.chronology_preferences().unwrap().enabled);
    let record = reopened.get_scene(scene).unwrap();
    assert_eq!(
        record.scene.start_time,
        story_core::StoryTime::exact(-86_400)
    );
    assert_eq!(record.scene.end_time, story_core::StoryTime::exact(0));
    assert_eq!(
        reopened.scene_story_interval(scene).unwrap().start_boundary,
        story_core::IntervalBoundary::Exclusive
    );
    assert_eq!(
        reopened.load_scene_text(scene).unwrap().0,
        "The manuscript remains intact."
    );
}
