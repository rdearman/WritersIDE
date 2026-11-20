use std::{env, process::ExitCode};
use story_app::WriterApp;
use story_core::{NodeId, SceneId};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        usage();
        return Ok(());
    };
    match cmd.as_str() {
        "create" => {
            let path = args.next().ok_or("missing project path")?;
            let title = args.next().unwrap_or_else(|| "Untitled Novel".into());
            drop(WriterApp::create_project(path, &title)?);
            println!("Created project: {title}");
        }
        "inspect" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for conflict in app.list_recovery_conflicts()? {
                println!(
                    "Unresolved manuscript conflict {} for scene {} at {}",
                    conflict.transaction_id,
                    conflict.scene_id,
                    conflict.recovery_path.display()
                );
            }
            app.validate_project()?;
            println!("Project is valid.");
        }
        "chapter" => {
            let path = args.next().ok_or("missing project path")?;
            let title = args.next().ok_or("missing chapter title")?;
            let mut app = WriterApp::open_project(path)?;
            let id = app.create_chapter(None, &title, 0)?;
            println!("{id}");
        }
        "scene" => {
            let path = args.next().ok_or("missing project path")?;
            let ch = args.next().ok_or("missing chapter id")?.parse()?;
            let title = args.next().ok_or("missing scene title")?;
            let mut app = WriterApp::open_project(path)?;
            let id = app.create_scene(NodeId(ch), &title, 0)?;
            println!("{id}");
        }
        "show" => {
            let path = args.next().ok_or("missing project path")?;
            let id = args.next().ok_or("missing scene id")?.parse()?;
            let app = WriterApp::open_project(path)?;
            let row = app.get_scene(SceneId(id))?;
            println!(
                "{} — {} words — {}",
                row.scene.title, row.actual_words, row.content_hash
            );
        }
        "write" => {
            let path = args.next().ok_or("missing project path")?;
            let id = args.next().ok_or("missing scene id")?.parse()?;
            let text = args.next().ok_or("missing manuscript text")?;
            let mut app = WriterApp::open_project(path)?;
            let (_, hash) = app.load_scene_text(SceneId(id))?;
            let new = app.save_scene_text(SceneId(id), Some(&hash), &text)?;
            println!("Saved {new}");
        }
        "resolve" => {
            let path = args.next().ok_or("missing project path")?;
            let txid = args.next().ok_or("missing transaction id")?;
            let choice = args.next().ok_or("missing choice: external|prepared")?;
            let resolution = match choice.as_str() {
                "external" => story_storage::ConflictResolution::KeepExternal,
                "prepared" => story_storage::ConflictResolution::UsePrepared,
                _ => return Err("choice must be external or prepared".into()),
            };
            let mut app = WriterApp::open_project(path)?;
            app.resolve_conflict(&txid, resolution)?;
            println!("Conflict resolved.");
        }
        "backup" => {
            let path = args.next().ok_or("missing project path")?;
            let destination = args.next().ok_or("missing backup destination")?;
            let mut app = WriterApp::open_project(path)?;
            app.backup_project(destination)?;
            println!("Backup created.");
        }
        _ => usage(),
    }
    Ok(())
}
fn usage() {
    eprintln!(
        "writers-ide create <path> [title]\nwriters-ide inspect <path>\nwriters-ide chapter <path> <title>\nwriters-ide scene <path> <chapter-uuid> <title>\nwriters-ide show <path> <scene-uuid>\nwriters-ide write <path> <scene-uuid> <text>\nwriters-ide resolve <path> <transaction-uuid> external|prepared\nwriters-ide backup <path> <destination>"
    );
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
