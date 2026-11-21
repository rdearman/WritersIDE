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
        "create-kind" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = story_core::BookType::parse(&args.next().ok_or("missing book type")?)
                .ok_or("book type must be fiction, nonfiction, or custom")?;
            let title = args.next().unwrap_or_else(|| "Untitled Book".into());
            drop(WriterApp::create_project_with_type(path, &title, kind)?);
            println!("Created {kind:?} project: {title}");
        }
        "create-template" => {
            let path = args.next().ok_or("missing project path")?;
            let template_id = args.next().ok_or("missing template ID")?;
            let template=story_core::BuiltinTemplate::parse(&template_id).ok_or("template must be blank-fiction, novel, short-story, blank-nonfiction, or nonfiction-book")?;
            let title = args.next().unwrap_or_else(|| "Untitled Book".into());
            drop(WriterApp::create_project_from_template(
                path, &title, template,
            )?);
            println!("Created {template_id} project: {title}");
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
            for conflict in app.list_content_unit_recovery_conflicts()? {
                println!(
                    "Unresolved manuscript conflict {} for content unit {} at {}",
                    conflict.transaction_id,
                    conflict.content_unit_id,
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
        "content" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = match args.next().as_deref().ok_or("missing content kind")? {
                "chapter" => story_core::ContentUnitKind::ChapterBody,
                "section" => story_core::ContentUnitKind::SectionBody,
                "front" => story_core::ContentUnitKind::FrontMatter,
                "back" => story_core::ContentUnitKind::BackMatter,
                _ => return Err("content kind must be chapter, section, front, or back".into()),
            };
            let parent = args.next().ok_or("missing parent node UUID or -")?;
            let parent = if parent == "-" {
                None
            } else {
                Some(NodeId(parent.parse()?))
            };
            let title = args.next().ok_or("missing content-unit title")?;
            let mut app = WriterApp::open_project(path)?;
            println!(
                "{}",
                app.create_content_unit(kind, &title, parent, 0, None)?
            );
        }
        "read-content" => {
            let path = args.next().ok_or("missing project path")?;
            let id =
                story_core::ContentUnitId(args.next().ok_or("missing content unit UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            print!("{}", app.load_content_unit_text(id)?.0);
        }
        "write-content" => {
            let path = args.next().ok_or("missing project path")?;
            let id =
                story_core::ContentUnitId(args.next().ok_or("missing content unit UUID")?.parse()?);
            let text = args.next().ok_or("missing content text")?;
            let mut app = WriterApp::open_project(path)?;
            let (_, hash) = app.load_content_unit_text(id)?;
            println!(
                "Saved {}",
                app.save_content_unit_text(id, Some(&hash), &text)?
            );
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
        "resolve-content" => {
            let path = args.next().ok_or("missing project path")?;
            let txid = args.next().ok_or("missing transaction ID")?;
            let choice = args.next().ok_or("missing choice: external|prepared")?;
            let use_prepared = match choice.as_str() {
                "prepared" => true,
                "external" => false,
                _ => return Err("choice must be external or prepared".into()),
            };
            let mut app = WriterApp::open_project(path)?;
            app.resolve_content_unit_conflict(&txid, use_prepared)?;
            println!("Content conflict resolved; recovery versions retained.");
        }
        "backup" => {
            let path = args.next().ok_or("missing project path")?;
            let destination = args.next().ok_or("missing backup destination")?;
            let mut app = WriterApp::open_project(path)?;
            app.backup_project(destination)?;
            println!("Backup created.");
        }
        "git-snapshot" => {
            let path = args.next().ok_or("missing project path")?;
            let label = args.next().ok_or("missing snapshot label")?;
            let mut app = WriterApp::open_project(path)?;
            match app.git_snapshot(&label)? {
                Some(commit) => println!("Snapshot {commit}"),
                None => println!("No changes; snapshot not created."),
            }
        }
        "git-history" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for commit in app.git_history(100)? {
                println!("{}\t{}\t{}", commit.id, commit.date, commit.subject);
            }
        }
        "git-diff" => {
            let path = args.next().ok_or("missing project path")?;
            let from = args.next().ok_or("missing first commit")?;
            let to = args.next().ok_or("missing second commit")?;
            let file = args.next();
            let app = WriterApp::open_project(path)?;
            print!("{}", app.git_diff(&from, &to, file.as_deref())?);
        }
        "character" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing character name")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_character(&name)?);
        }
        "character-profile" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::CharacterId(args.next().ok_or("missing character UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            let character = app.get_character(id)?;
            println!(
                "ID: {}\nName: {}\nPreferred name: {}\nAliases: {}\nStatus: {}\nDescription: {}\nBackground: {}\nGoals: {}\nMotivations: {}\nSecrets: {}\nInternal conflict: {}\nExternal conflict: {}",
                character.id,
                character.full_name,
                character.preferred_name,
                character.aliases.join(", "),
                character.status,
                character.description,
                character.background,
                character.goals,
                character.motivations,
                character.secrets,
                character.internal_conflict,
                character.external_conflict
            );
        }
        "character-set" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::CharacterId(args.next().ok_or("missing character UUID")?.parse()?);
            let field = args.next().ok_or("missing character field")?;
            let value = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            app.set_character_field(id, &field, &value)?;
            println!("Updated character {id} field {field}");
        }
        "character-alias" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::CharacterId(args.next().ok_or("missing character UUID")?.parse()?);
            let alias = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            app.add_character_alias(id, &alias)?;
            println!("Added alias for character {id}");
        }
        "character-un-alias" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::CharacterId(args.next().ok_or("missing character UUID")?.parse()?);
            let alias = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            app.remove_character_alias(id, &alias)?;
            println!("Removed alias from character {id}");
        }
        "plotline" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing plotline name")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_plotline(&name)?);
        }
        "issue-status" => {
            let path = args.next().ok_or("missing project path")?;
            let issue = story_core::IssueId(args.next().ok_or("missing issue UUID")?.parse()?);
            let status = args.next().ok_or("missing issue status")?;
            let mut app = WriterApp::open_project(path)?;
            app.set_issue_status(issue, &status)?;
            println!("Updated issue {issue} to {status}");
        }
        "search" => {
            let path = args.next().ok_or("missing project path")?;
            let terms = args.collect::<Vec<_>>().join(" ");
            let app = WriterApp::open_project(path)?;
            for scene in app.search_scenes(&terms)? {
                println!("{scene}");
            }
        }
        "search-all" => {
            let path = args.next().ok_or("missing project path")?;
            let terms = args.collect::<Vec<_>>().join(" ");
            let app = WriterApp::open_project(path)?;
            for hit in app.search_project(&terms, 100)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    hit.entity_kind, hit.entity_id, hit.title, hit.excerpt
                );
            }
        }
        "rebuild-search" => {
            let path = args.next().ok_or("missing project path")?;
            let mut app = WriterApp::open_project(path)?;
            app.rebuild_project_search_index()?;
            println!("Project search index rebuilt.");
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
        "outgoing" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = args.next().ok_or("missing entity kind")?;
            let id = args.next().ok_or("missing entity UUID")?;
            let app = WriterApp::open_project(path)?;
            for link in app.outgoing_links(parse_entity(&kind, &id)?)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    link.id,
                    link.kind.as_str(),
                    link.target.kind(),
                    link.target.id()
                );
            }
        }
        "broken-links" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for link in app.broken_links()? {
                println!("{}\t{}", link.link_id, link.reason);
            }
        }
        "anchors" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            for anchor in app.list_passage_anchors(scene)? {
                println!(
                    "{}\t{}..{}\t{}\t{}",
                    anchor.id, anchor.start_byte, anchor.end_byte, anchor.state, anchor.quote
                );
            }
        }
        "anchor-move" => {
            let path = args.next().ok_or("missing project path")?;
            let anchor = story_core::AnchorId(args.next().ok_or("missing anchor UUID")?.parse()?);
            let start = args.next().ok_or("missing start byte")?.parse()?;
            let end = args.next().ok_or("missing end byte")?.parse()?;
            let mut app = WriterApp::open_project(path)?;
            app.reanchor_passage(anchor, start, end)?;
            println!("Anchor {anchor} reattached");
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
        "knowledge" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for claim in app.list_approved_knowledge_claims()? {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}:{}..{}\t{}",
                    claim.id,
                    claim.kind,
                    claim.name,
                    claim.claim,
                    claim.approximate_story_time.as_deref().unwrap_or("?"),
                    claim.source_kind,
                    claim.source_id,
                    claim.evidence_start_byte,
                    claim.evidence_end_byte,
                    claim.evidence_quote
                );
            }
        }
        "process" => {
            let path = args.next().ok_or("missing project path")?;
            let mut expand_transclusions = false;
            let mut selected = None;
            while let Some(option) = args.next() {
                match option.as_str() {
                    "--expand-transclusions" => expand_transclusions = true,
                    "--only" => {
                        let names = args
                            .next()
                            .ok_or("--only needs a comma-separated stage list")?;
                        let stages = names
                            .split(',')
                            .map(|name| {
                                let name = name.trim();
                                story_pipeline::Stage::parse_name(name)
                                    .ok_or_else(|| format!("unknown processing stage '{name}'"))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        if stages.is_empty() {
                            return Err("--only requires at least one processing stage".into());
                        }
                        selected = Some(stages);
                    }
                    _ => return Err(format!("unknown process option: {option}").into()),
                }
            }
            let app = WriterApp::open_project(path)?;
            let stages = selected.as_deref().unwrap_or(&story_pipeline::Stage::ALL);
            let reports = app.run_processing_stages(stages, expand_transclusions)?;
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
            let source_kind = args.next().ok_or("missing source kind: scene|reading")?;
            let source_id = args.next().ok_or("missing source UUID")?;
            let model = args.next().ok_or("missing Ollama model name")?;
            let endpoint = args
                .next()
                .unwrap_or_else(|| "http://localhost:11434".into());
            let app = WriterApp::open_project(&path)?;
            let proposal = app.extract_with_ollama(&source_kind, &source_id, &model, &endpoint)?;
            let filename = format!(
                "{source_kind}-{source_id}-{}-{}.json",
                &proposal.source_fingerprint[..16],
                QueryId::new()
            );
            let target =
                app.save_extraction_proposal(&filename, &serde_json::to_vec_pretty(&proposal)?)?;
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
            let project = args.next().ok_or("missing project path")?;
            let file = std::path::PathBuf::from(args.next().ok_or("missing proposal JSON path")?);
            let claim_index: usize = args
                .next()
                .ok_or("missing claim index")?
                .parse()
                .map_err(|_| "claim index must be a zero-based integer")?;
            if args.next().is_some() {
                return Err(
                    "approve-extraction accepts <project> <proposal-json-file> <claim-index>"
                        .into(),
                );
            }
            let mut app = WriterApp::open_project(project)?;
            let (original, fingerprint) = app.load_extraction_proposal_file(&file)?;
            let mut proposal: story_ollama::ProvisionalExtraction =
                serde_json::from_slice(&original)?;
            proposal
                .claims
                .get(claim_index)
                .ok_or("claim index is out of range")?;
            let approved_id = app.approve_extraction_claim(&proposal, claim_index)?;
            proposal.claims[claim_index].approved = true;
            app.update_extraction_proposal_file(
                &file,
                &fingerprint,
                &serde_json::to_vec_pretty(&proposal)?,
            )?;
            println!(
                "Promoted claim {claim_index} to approved knowledge record {approved_id} with its source evidence."
            );
        }
        "dismiss-extraction" => {
            let project = args.next().ok_or("missing project path")?;
            let file = std::path::PathBuf::from(args.next().ok_or("missing proposal JSON path")?);
            let claim_index: usize = args
                .next()
                .ok_or("missing claim index")?
                .parse()
                .map_err(|_| "claim index must be a zero-based integer")?;
            if args.next().is_some() {
                return Err(
                    "dismiss-extraction accepts <project> <proposal-json-file> <claim-index>"
                        .into(),
                );
            }
            let app = WriterApp::open_project(project)?;
            let (original, fingerprint) = app.load_extraction_proposal_file(&file)?;
            let mut proposal: story_ollama::ProvisionalExtraction =
                serde_json::from_slice(&original)?;
            app.dismiss_extraction_claim(&mut proposal, claim_index)?;
            app.update_extraction_proposal_file(
                &file,
                &fingerprint,
                &serde_json::to_vec_pretty(&proposal)?,
            )?;
            println!("Dismissed provisional claim {claim_index}; source evidence was retained.");
        }
        "proposals" => {
            let project = args.next().ok_or("missing project path")?;
            if args.next().is_some() {
                return Err("proposals accepts <project>".into());
            }
            let app = WriterApp::open_project(project)?;
            let files = app.list_extraction_proposal_files()?;
            if files.is_empty() {
                println!("No extraction proposals.");
            }
            for file in files {
                let (bytes, _) = app.load_extraction_proposal_file(&file)?;
                let proposal: story_ollama::ProvisionalExtraction = serde_json::from_slice(&bytes)?;
                println!(
                    "{} · {} {} · model {}",
                    file.file_name().unwrap_or_default().to_string_lossy(),
                    proposal.source_kind,
                    proposal.source_id,
                    proposal.model
                );
                for (index, claim) in proposal.claims.iter().enumerate() {
                    let status = if claim.approved {
                        "approved"
                    } else if claim.dismissed {
                        "dismissed"
                    } else {
                        "provisional"
                    };
                    println!(
                        "  [{index}] {status}: {} · {} — {} | evidence: {}",
                        claim.claim.kind,
                        claim.claim.name,
                        claim.claim.claim,
                        claim.claim.evidence_quote
                    );
                }
            }
        }
        _ => usage(),
    }
    Ok(())
}
fn usage() {
    eprintln!(
        "writers-ide create <path> [title]\nwriters-ide create-kind <path> <fiction|nonfiction|custom> [title]\nwriters-ide inspect <path>\nwriters-ide chapter <path> <title>\nwriters-ide scene <path> <chapter-uuid> <title>\nwriters-ide show <path> <scene-uuid>\nwriters-ide write <path> <scene-uuid> <text>\nwriters-ide search <path> <terms>\nwriters-ide analytics <path>\nwriters-ide process <path> [--expand-transclusions] [--only stage,stage]\nwriters-ide extract <path> <scene|reading> <source-uuid> <model> [endpoint]\nwriters-ide study <path> <scene|reading> <source-uuid> <model> [endpoint] [--questions]\nwriters-ide proposals <project>\nwriters-ide approve-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide dismiss-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide knowledge <project>\nwriters-ide query-save <path> <name> [status=...|character=UUID|plotline=UUID|before=TICK|after=TICK|issue=unresolved]...\nwriters-ide query-list <path>\nwriters-ide query-run <path> <query-uuid>\nwriters-ide link <path> <source-kind> <source-id> <target-kind> <target-id> <related|cites|transcludes|includes|navigates> [label]\nwriters-ide outgoing <path> <entity-kind> <entity-id>\nwriters-ide backlinks <path> <entity-kind> <entity-id>\nwriters-ide broken-links <path>\nwriters-ide snapshot <path> <scene-uuid> <label>\nwriters-ide history <path> <scene-uuid>\nwriters-ide diff <path> <scene-uuid> <snapshot-uuid>\nwriters-ide restore <path> <scene-uuid> <snapshot-uuid>\nwriters-ide resolve <path> <transaction-uuid> external|prepared\nwriters-ide backup <path> <destination>\nwriters-ide character-profile <path> <character-uuid>\nwriters-ide character-set <path> <character-uuid> <field> <value>\nwriters-ide character-alias <path> <character-uuid> <alias>\nwriters-ide character-un-alias <path> <character-uuid> <alias>"
    );
    eprintln!(
        "writers-ide anchors <path> <scene-uuid>\nwriters-ide anchor-move <path> <anchor-uuid> <start-byte> <end-byte>"
    );
    eprintln!("writers-ide search-all <path> <terms>\nwriters-ide rebuild-search <path>");
    eprintln!(
        "writers-ide create-template <path> <blank-fiction|novel|short-story|blank-nonfiction|nonfiction-book> [title]\nwriters-ide content <path> <chapter|section|front|back> <parent-uuid|-> <title>\nwriters-ide read-content <path> <content-uuid>\nwriters-ide write-content <path> <content-uuid> <text>\nwriters-ide resolve-content <path> <transaction-id> external|prepared\nwriters-ide git-snapshot <path> <label>\nwriters-ide git-history <path>\nwriters-ide git-diff <path> <commit-a> <commit-b> [project-relative-path]"
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
        "reading" => EntityRef::Reading(story_core::ReadingId(id.parse()?)),
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
