use std::{env, process::ExitCode};
use story_app::WriterApp;
use story_core::{
    EntityRef, LinkId, LinkKind, NodeId, QueryId, SavedSceneQuery, SceneId, SceneQueryFilter,
    SnapshotId,
};

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
        "character" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing character name")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_character(&name)?);
        }
        "plotline" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing plotline name")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_plotline(&name)?);
        }
        "search" => {
            let path = args.next().ok_or("missing project path")?;
            let terms = args.collect::<Vec<_>>().join(" ");
            let app = WriterApp::open_project(path)?;
            for scene in app.search_scenes(&terms)? {
                println!("{scene}");
            }
        }
        "query-save" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing query name")?;
            let mut filters = Vec::new();
            for arg in args {
                let (key, value) = arg
                    .split_once('=')
                    .ok_or("filters must use key=value syntax")?;
                filters.push(match key {
                    "status" => SceneQueryFilter::Status(parse_status(value)?),
                    "character" => {
                        SceneQueryFilter::Character(story_core::CharacterId(value.parse()?))
                    }
                    "plotline" => {
                        SceneQueryFilter::Plotline(story_core::PlotlineId(value.parse()?))
                    }
                    "before" => SceneQueryFilter::TimeBefore(value.parse()?),
                    "after" => SceneQueryFilter::TimeAfter(value.parse()?),
                    "issue" if value == "unresolved" => SceneQueryFilter::UnresolvedIssue,
                    _ => return Err(format!("unsupported query filter: {arg}").into()),
                });
            }
            let query = SavedSceneQuery {
                id: QueryId::new(),
                name,
                description: String::new(),
                filters,
            };
            let mut app = WriterApp::open_project(path)?;
            app.save_scene_query(&query)?;
            println!("{}", query.id);
        }
        "query-run" => {
            let path = args.next().ok_or("missing project path")?;
            let id = args.next().ok_or("missing query id")?.parse()?;
            let app = WriterApp::open_project(path)?;
            for scene in app.run_scene_query(QueryId(id))? {
                println!("{scene}");
            }
        }
        "query-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for query in app.list_saved_queries()? {
                println!("{}\t{}", query.id, query.name);
            }
        }
        "link" => {
            let path = args.next().ok_or("missing project path")?;
            let source_kind = args.next().ok_or("missing source kind")?;
            let source_id = args.next().ok_or("missing source id")?;
            let target_kind = args.next().ok_or("missing target kind")?;
            let target_id = args.next().ok_or("missing target id")?;
            let link_kind = args.next().ok_or("missing link kind")?;
            let label = args.collect::<Vec<_>>().join(" ");
            let link = story_core::StoryLink {
                id: LinkId::new(),
                source: parse_entity(&source_kind, &source_id)?,
                source_anchor: None,
                target: parse_entity(&target_kind, &target_id)?,
                target_anchor: None,
                kind: parse_link_kind(&link_kind)?,
                label,
            };
            let mut app = WriterApp::open_project(path)?;
            app.create_story_link(&link)?;
            println!("{}", link.id);
        }
        "backlinks" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = args.next().ok_or("missing entity kind")?;
            let id = args.next().ok_or("missing entity id")?;
            let app = WriterApp::open_project(path)?;
            for link in app.backlinks(parse_entity(&kind, &id)?)? {
                println!("{}\t{}\t{}", link.id, link.kind.as_str(), link.label);
            }
        }
        "broken-links" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for link in app.broken_links()? {
                println!("{}\t{}", link.link_id, link.reason);
            }
        }
        "snapshot" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let label = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_snapshot(scene, &label)?);
        }
        "history" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            for item in app.list_snapshots(scene)? {
                println!("{}\t{}\t{}", item.id, item.created_at, item.label);
            }
        }
        "diff" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let snapshot = SnapshotId(args.next().ok_or("missing snapshot UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            for line in app.diff_snapshot(scene, snapshot)? {
                let prefix = match line.kind {
                    story_storage::SnapshotLineKind::Equal => ' ',
                    story_storage::SnapshotLineKind::Added => '+',
                    story_storage::SnapshotLineKind::Removed => '-',
                };
                print!("{prefix}{}", line.text);
            }
        }
        "restore" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let snapshot = SnapshotId(args.next().ok_or("missing snapshot UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            let (_, hash) = app.load_scene_text(scene)?;
            app.restore_snapshot(scene, snapshot, &hash)?;
            println!("Restored; current text was snapshotted first.");
        }
        "analytics" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            let data = app.analytics_dataset()?;
            let project = app.get_project()?;
            let words = data.iter().map(|s| u64::from(s.actual_words)).sum::<u64>();
            println!("{} scenes · {words} words", data.len());
            for pair in story_analysis::character_interactions(&data) {
                println!(
                    "interaction\t{}\t{}\t{} scenes",
                    pair.first_character, pair.second_character, pair.shared_scenes
                );
            }
            for gap in story_analysis::subplot_gaps(&data, 3) {
                println!(
                    "subplot-gap\t{}\t{} scenes\t{} → {}",
                    gap.plotline_name, gap.intervening_scenes, gap.from_scene_id, gap.to_scene_id
                );
            }
            for finding in story_analysis::chronology_ambiguities(&data) {
                println!(
                    "chronology-uncertain\t{}\t{}",
                    finding.first_scene_id, finding.second_scene_id
                );
            }
            if let Some(target) = project.target_words {
                let preview = story_analysis::preview_budget_redistribution(target, &data);
                println!(
                    "budget-preview\t{} target\t{} fixed\t{} unallocated",
                    preview.target_words, preview.fixed_words, preview.unallocated_words
                );
                for (scene, words) in preview.suggested_scene_targets {
                    println!("suggested-scene-target\t{scene}\t{words}");
                }
            }
        }
        "process" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            let reports = app.run_processing()?;
            let incomplete = reports.iter().any(|r| {
                matches!(
                    r.status,
                    story_pipeline::StageStatus::Failed | story_pipeline::StageStatus::Skipped
                )
            });
            for report in reports {
                println!(
                    "{:?}\t{:?}\t{}",
                    report.stage, report.status, report.message
                );
            }
            if incomplete {
                return Err("processing pipeline incomplete; inspect failed/skipped stages".into());
            }
        }
        "extract" => {
            let path = std::path::PathBuf::from(args.next().ok_or("missing project path")?);
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let model = args.next().ok_or("missing Ollama model name")?;
            let endpoint = args
                .next()
                .unwrap_or_else(|| "http://localhost:11434".into());
            let app = WriterApp::open_project(&path)?;
            let (source, _) = app.load_scene_text(scene)?;
            let output = story_ollama::OllamaClient::new(&endpoint, &model)?.extract(&source)?;
            let proposal = story_ollama::attach_provenance(
                "scene",
                &scene.to_string(),
                &source,
                &model,
                output,
            );
            let folder = path.join(".writers-ide").join("proposals");
            std::fs::create_dir_all(&folder)?;
            let target = folder.join(format!(
                "scene-{scene}-{}-{}.json",
                &proposal.source_fingerprint[..16],
                QueryId::new()
            ));
            std::fs::write(&target, serde_json::to_vec_pretty(&proposal)?)?;
            println!(
                "Saved provisional, unapproved extraction to {}",
                target.display()
            );
        }
        "study" => {
            let path = args.next().ok_or("missing project path")?;
            let source_kind = args.next().ok_or("missing source kind: scene|reading")?;
            let source_id = args.next().ok_or("missing source UUID")?;
            let model = args.next().ok_or("missing Ollama model name")?;
            let options = args.collect::<Vec<_>>();
            let endpoint = options
                .iter()
                .find(|arg| !arg.starts_with("--"))
                .map(String::as_str)
                .unwrap_or("http://localhost:11434");
            let include_questions = options.iter().any(|arg| arg == "--questions");
            let app = WriterApp::open_project(path)?;
            let source = match source_kind.as_str() {
                "scene" => app.load_scene_text(SceneId(source_id.parse()?))?.0,
                "reading" => {
                    app.get_reading_document(story_core::ReadingId(source_id.parse()?))?
                        .extracted_text
                }
                _ => return Err("source kind must be scene or reading".into()),
            };
            let guide = story_ollama::OllamaClient::new(endpoint, &model)?
                .study_guide(&source, include_questions)?;
            println!(
                "Summary\n{}\n\nExplanation\n{}",
                guide.summary, guide.explanation
            );
            for question in guide.questions {
                println!("\nQuestion: {question}");
            }
        }
        "approve-extraction" => {
            let file = std::path::PathBuf::from(args.next().ok_or("missing proposal JSON path")?);
            let mut proposal: story_ollama::ProvisionalExtraction =
                serde_json::from_slice(&std::fs::read(&file)?)?;
            for claim in &mut proposal.claims {
                claim.approved = true;
            }
            let bytes = serde_json::to_vec_pretty(&proposal)?;
            let temp = file.with_extension(format!("{}.tmp", QueryId::new()));
            use std::io::Write;
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            out.write_all(&bytes)?;
            out.sync_all()?;
            std::fs::rename(&temp, &file)?;
            if let Some(parent) = file.parent() {
                std::fs::File::open(parent)?.sync_all()?;
            }
            println!(
                "Marked proposal approved. This records review only; it does not create canonical story facts."
            );
        }
        _ => usage(),
    }
    Ok(())
}
fn usage() {
    eprintln!(
        "writers-ide create <path> [title]\nwriters-ide inspect <path>\nwriters-ide chapter <path> <title>\nwriters-ide scene <path> <chapter-uuid> <title>\nwriters-ide show <path> <scene-uuid>\nwriters-ide write <path> <scene-uuid> <text>\nwriters-ide search <path> <terms>\nwriters-ide analytics <path>\nwriters-ide process <path>\nwriters-ide extract <path> <scene-uuid> <model> [endpoint]\nwriters-ide study <path> <scene|reading> <source-uuid> <model> [endpoint] [--questions]\nwriters-ide approve-extraction <proposal-json-file>\nwriters-ide query-save <path> <name> [status=...|character=UUID|plotline=UUID|before=TICK|after=TICK|issue=unresolved]...\nwriters-ide query-list <path>\nwriters-ide query-run <path> <query-uuid>\nwriters-ide link <path> <source-kind> <source-id> <target-kind> <target-id> <related|cites|transcludes|includes|navigates> [label]\nwriters-ide backlinks <path> <entity-kind> <entity-id>\nwriters-ide broken-links <path>\nwriters-ide snapshot <path> <scene-uuid> <label>\nwriters-ide history <path> <scene-uuid>\nwriters-ide diff <path> <scene-uuid> <snapshot-uuid>\nwriters-ide restore <path> <scene-uuid> <snapshot-uuid>\nwriters-ide resolve <path> <transaction-uuid> external|prepared\nwriters-ide backup <path> <destination>"
    );
}
fn parse_status(value: &str) -> Result<story_core::SceneStatus, Box<dyn std::error::Error>> {
    use story_core::SceneStatus::*;
    Ok(match value {
        "idea" => Idea,
        "outlined" => Outlined,
        "ready_to_draft" => ReadyToDraft,
        "drafting" => Drafting,
        "draft_complete" => DraftComplete,
        "revising" => Revising,
        "complete" => Complete,
        "archived" => Archived,
        _ => return Err(format!("unknown scene status: {value}").into()),
    })
}
fn parse_entity(kind: &str, id: &str) -> Result<EntityRef, Box<dyn std::error::Error>> {
    Ok(match kind {
        "scene" => EntityRef::Scene(SceneId(id.parse()?)),
        "character" => EntityRef::Character(story_core::CharacterId(id.parse()?)),
        "plotline" => EntityRef::Plotline(story_core::PlotlineId(id.parse()?)),
        "research" => EntityRef::Research(story_core::ResearchId(id.parse()?)),
        _ => return Err(format!("unknown entity kind: {kind}").into()),
    })
}
fn parse_link_kind(kind: &str) -> Result<LinkKind, Box<dyn std::error::Error>> {
    Ok(match kind {
        "related" => LinkKind::Related,
        "cites" => LinkKind::Cites,
        "transcludes" => LinkKind::Transcludes,
        "includes" => LinkKind::Includes,
        "navigates" => LinkKind::Navigates,
        _ => return Err(format!("unknown link kind: {kind}").into()),
    })
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
