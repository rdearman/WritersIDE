use std::process::Command;

use story_app::WriterApp;

#[test]
fn cli_board_svg_export_includes_cards_and_connectors_without_mutating_board() {
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Novel");
    let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
    let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
    let first_scene = app.create_scene(chapter, "First", 0).unwrap();
    let second_scene = app.create_scene(chapter, "Second", 1).unwrap();
    let board = app.create_planning_board("Plan <A>").unwrap();
    let source = app
        .add_planning_board_item(board, "scene", &first_scene.to_string(), 0.0, 0.0, "Start")
        .unwrap();
    let target = app
        .add_planning_board_item(
            board,
            "scene",
            &second_scene.to_string(),
            40.0,
            0.0,
            "Finish",
        )
        .unwrap();
    app.connect_planning_board_items(board, source, target, "leads to")
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_writers-ide"))
        .args([
            "board-export-svg",
            project_path.to_str().unwrap(),
            &board.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout).unwrap();
    assert!(svg.contains("Plan &lt;A&gt;"));
    assert!(svg.contains(&first_scene.to_string()));
    assert!(svg.contains(&second_scene.to_string()));
    assert!(svg.contains("leads to"));
    assert_eq!(app.list_planning_board_items(board).unwrap().len(), 2);
    assert_eq!(app.list_planning_board_connectors(board).unwrap().len(), 1);
}
