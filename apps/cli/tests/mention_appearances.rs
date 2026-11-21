use std::process::Command;

use story_app::WriterApp;
use story_core::MentionEntityKind;

#[test]
fn entity_mentions_lists_confirmed_prose_references_across_scenes() {
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Novel");
    let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let first = app.create_scene(chapter, "First", 0).unwrap();
    let second = app.create_scene(chapter, "Second", 1).unwrap();
    let object = app
        .create_story_object("Silver Key", "A small key.")
        .unwrap();
    for (scene, text) in [
        (first, "Mara found the Silver Key."),
        (second, "The Silver Key was missing."),
    ] {
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), text).unwrap();
        let mention = app
            .list_scene_mentions(scene)
            .unwrap()
            .into_iter()
            .find(|row| row.quote == "Silver Key")
            .unwrap();
        app.confirm_scene_mention(mention.id, MentionEntityKind::Object, &object.to_string())
            .unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_writers-ide"))
        .args([
            "entity-mentions",
            project_path.to_str().unwrap(),
            "object",
            &object.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("First"));
    assert!(stdout.contains("Second"));
    assert!(stdout.contains("Silver Key"));
}
