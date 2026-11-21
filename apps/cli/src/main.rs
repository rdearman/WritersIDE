use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::Duration,
};
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
        "backup-archive" => {
            let path = args.next().ok_or("missing project path")?;
            let archive = args.next().ok_or("missing destination archive path")?;
            let mut app = WriterApp::open_project(path)?;
            app.backup_project_archive(archive)?;
            println!("Project backup archive created and verified.");
        }
        "restore-archive" => {
            let archive = args.next().ok_or("missing source archive path")?;
            let destination = args.next().ok_or("missing new project directory")?;
            WriterApp::restore_project_archive(archive, destination)?;
            println!("Project archive restored and validated.");
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
        "language" => {
            let path = args.next().ok_or("missing project path")?;
            let code = args.next();
            let mut app = WriterApp::open_project(path)?;
            if let Some(code) = code {
                app.set_project_language(&code)?;
            }
            println!("{}", app.get_project()?.language_code);
        }
        "find" => {
            let path = args.next().ok_or("missing project path")?;
            let mode = parse_find_mode(&args.next().ok_or("missing mode")?)?;
            let scope = args.next().ok_or("missing scope: all or scene UUID")?;
            validate_scene_scope(&scope)?;
            let pattern = args.collect::<Vec<_>>().join(" ");
            if pattern.is_empty() {
                return Err("missing search pattern".into());
            }
            let app = WriterApp::open_project(path)?;
            let scenes = app.list_scenes(false)?;
            for scene in scenes
                .into_iter()
                .filter(|s| scene_matches_scope(s, &scope))
            {
                let (text, _) = app.load_scene_text(scene.scene.id)?;
                for range in story_core::find_matches(&text, &pattern, mode)? {
                    println!(
                        "{}\t{}\t{}..{}",
                        scene.scene.id, scene.scene.title, range.start, range.end
                    );
                }
            }
        }
        "replace" => {
            let path = args.next().ok_or("missing project path")?;
            let mode = parse_find_mode(&args.next().ok_or("missing mode")?)?;
            let scope = args.next().ok_or("missing scope: all or scene UUID")?;
            validate_scene_scope(&scope)?;
            let rest = args.collect::<Vec<_>>().join(" ");
            let (pattern, replacement) = rest
                .split_once(" => ")
                .ok_or("use: replace PROJECT MODE all|SCENE_UUID PATTERN => REPLACEMENT")?;
            let mut app = WriterApp::open_project(path)?;
            let scenes = app.list_scenes(false)?;
            let mut changed = 0;
            for scene in scenes
                .into_iter()
                .filter(|s| scene_matches_scope(s, &scope))
            {
                let (text, hash) = app.load_scene_text(scene.scene.id)?;
                let (updated, count) =
                    story_core::replace_matches(&text, pattern, replacement, mode)?;
                if count > 0 {
                    app.save_scene_text(scene.scene.id, Some(&hash), &updated)?;
                    println!("{}\t{} replacements", scene.scene.id, count);
                    changed += count;
                }
            }
            println!("Total: {changed}");
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
        "scratchpad-create" => {
            let path = args.next().ok_or("missing project path")?;
            let title = args.next().ok_or("missing note title")?;
            let body = args.next().unwrap_or_default();
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_scratchpad_note(&title, &body)?);
        }
        "scratchpad-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for note in app.list_scratchpad_notes(true)? {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    note.id,
                    note.state,
                    if note.pinned { "pinned" } else { "" },
                    note.title,
                    note.body
                );
            }
        }
        "scratchpad-promote" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::ScratchpadId(args.next().ok_or("missing note UUID")?.parse()?);
            let kind = args.next().ok_or("missing destination kind")?;
            let destination = args.next().ok_or("missing destination UUID")?;
            let mut app = WriterApp::open_project(path)?;
            app.promote_scratchpad_note(id, &kind, &destination)?;
            println!("Promoted {id}; note content is retained.");
        }
        "mentions" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            let (text, hash) = app.load_scene_text(scene)?;
            app.reindex_scene_mentions(scene, &text, &hash)?;
            for mention in app.list_scene_mentions(scene)? {
                let candidates = mention
                    .candidates
                    .iter()
                    .map(|candidate| {
                        format!(
                            "{}:{} ({})",
                            candidate.entity_kind, candidate.entity_id, candidate.matched_name
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "{}\t{}\t{}..{}\t{}\t{}",
                    mention.id,
                    mention.state,
                    mention.start_byte,
                    mention.end_byte,
                    mention.quote,
                    candidates
                );
            }
        }
        "mention-confirm" => {
            let path = args.next().ok_or("missing project path")?;
            let mention =
                story_core::MentionId(args.next().ok_or("missing mention UUID")?.parse()?);
            let kind = match args.next().as_deref().ok_or("missing entity kind")? {
                "character" => story_core::MentionEntityKind::Character,
                "location" => story_core::MentionEntityKind::Location,
                "object" => story_core::MentionEntityKind::Object,
                _ => return Err("entity kind must be character, location, or object".into()),
            };
            let entity = args.next().ok_or("missing entity UUID")?;
            let mut app = WriterApp::open_project(path)?;
            app.confirm_scene_mention(mention, kind, &entity)?;
        }
        "mention-dismiss" => {
            let path = args.next().ok_or("missing project path")?;
            let mention =
                story_core::MentionId(args.next().ok_or("missing mention UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.dismiss_scene_mention(mention)?;
        }
        "object-create" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing object name")?;
            let description = args.next().unwrap_or_default();
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_story_object(&name, &description)?);
        }
        "assembly-create" => {
            let path = args.next().ok_or("missing project path")?;
            let title = args.next().ok_or("missing assembly title")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_assembly(&title)?);
        }
        "assembly-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for assembly in app.list_assemblies()? {
                println!("{}\t{}", assembly.id, assembly.title);
            }
        }
        "assembly-add-scene" => {
            let path = args.next().ok_or("missing project path")?;
            let assembly =
                story_core::AssemblyId(args.next().ok_or("missing assembly UUID")?.parse()?);
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let position = args.next().ok_or("missing position")?.parse()?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.add_assembly_scene(assembly, scene, position)?);
        }
        "assembly-move" => {
            let path = args.next().ok_or("missing project path")?;
            let assembly =
                story_core::AssemblyId(args.next().ok_or("missing assembly UUID")?.parse()?);
            let item = story_core::AssemblyItemId(args.next().ok_or("missing item UUID")?.parse()?);
            let position = args.next().ok_or("missing position")?.parse()?;
            let mut app = WriterApp::open_project(path)?;
            app.move_assembly_item(assembly, item, position)?;
        }
        "assembly-include" => {
            let path = args.next().ok_or("missing project path")?;
            let item = story_core::AssemblyItemId(args.next().ok_or("missing item UUID")?.parse()?);
            let value = match args.next().as_deref().ok_or("use on or off")? {
                "on" => true,
                "off" => false,
                _ => return Err("assembly inclusion must be on or off".into()),
            };
            let mut app = WriterApp::open_project(path)?;
            app.set_assembly_item_included(item, value)?;
        }
        "assembly-compile" => {
            let path = args.next().ok_or("missing project path")?;
            let assembly =
                story_core::AssemblyId(args.next().ok_or("missing assembly UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            print!("{}", app.compile_assembly_markdown(assembly)?);
        }
        "assembly-draft-create" => {
            let path = args.next().ok_or("missing project path")?;
            let assembly =
                story_core::AssemblyId(args.next().ok_or("missing assembly UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            let draft = app.create_assembly_draft(assembly)?;
            println!("{}", draft.id);
        }
        "assembly-draft-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for draft in app.list_assembly_drafts()? {
                println!(
                    "{}\t{}\t{} scene files",
                    draft.id,
                    draft.assembly_id,
                    draft.items.len()
                );
            }
        }
        "assembly-draft-set" => {
            let path = args.next().ok_or("missing project path")?;
            let draft =
                story_core::AssemblyDraftId(args.next().ok_or("missing draft UUID")?.parse()?);
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let source = args.next().ok_or("missing UTF-8 text file")?;
            let text = std::fs::read_to_string(source)?;
            let mut app = WriterApp::open_project(path)?;
            app.update_assembly_draft_item(draft, scene, &text)?;
        }
        "assembly-draft-commit" => {
            let path = args.next().ok_or("missing project path")?;
            let draft =
                story_core::AssemblyDraftId(args.next().ok_or("missing draft UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.commit_assembly_draft(draft)?;
        }
        "board-create" => {
            let path = args.next().ok_or("missing project path")?;
            let title = args.next().ok_or("missing board title")?;
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.create_planning_board(&title)?);
        }
        "story-template-import" => {
            let path = args.next().ok_or("missing project path")?;
            let source = args.next().ok_or("missing template TOML path")?;
            let template: story_core::StoryBibleTemplate =
                toml::from_str(&std::fs::read_to_string(source)?)?;
            let app = WriterApp::open_project(path)?;
            app.save_story_bible_template(&template)?;
            println!("{}", template.id);
        }
        "story-template-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for template in app.list_story_bible_templates()? {
                println!(
                    "{}\t{}\t{}\t{} fields",
                    template.id,
                    template.entity_kind.as_str(),
                    template.name,
                    template.fields.len()
                );
            }
        }
        "story-profile-import" => {
            let path = args.next().ok_or("missing project path")?;
            let source = args.next().ok_or("missing profile JSON path")?;
            let profile: story_core::StoryBibleProfile =
                serde_json::from_slice(&std::fs::read(source)?)?;
            let mut app = WriterApp::open_project(path)?;
            app.save_story_bible_profile(&profile)?;
            println!(
                "Stored profile for {} {}",
                profile.entity_kind.as_str(),
                profile.entity_id
            );
        }
        "story-profile-show" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = parse_story_entity_kind(&args.next().ok_or("missing entity kind")?)?;
            let id = args.next().ok_or("missing entity UUID")?;
            let app = WriterApp::open_project(path)?;
            let profile = app.load_story_bible_profile(kind, &id)?;
            println!("{}", serde_json::to_string_pretty(&profile)?);
        }
        "board-add-scene" => {
            let path = args.next().ok_or("missing project path")?;
            let board =
                story_core::PlanningBoardId(args.next().ok_or("missing board UUID")?.parse()?);
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let x = args.next().ok_or("missing x coordinate")?.parse()?;
            let y = args.next().ok_or("missing y coordinate")?.parse()?;
            let note = args.next().unwrap_or_default();
            let mut app = WriterApp::open_project(path)?;
            println!(
                "{}",
                app.add_planning_board_item(board, "scene", &scene.to_string(), x, y, &note)?
            );
        }
        "board-list" => {
            let path = args.next().ok_or("missing project path")?;
            let board =
                story_core::PlanningBoardId(args.next().ok_or("missing board UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            for item in app.list_planning_board_items(board)? {
                println!(
                    "{}\t{}\t{}\t{},{}\t{}",
                    item.id, item.entity_kind, item.entity_id, item.x, item.y, item.note
                );
            }
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
        "git-search-history" => {
            let path = args.next().ok_or("missing project path")?;
            let text = args.next().ok_or("missing literal text to search")?;
            let file = args.next();
            let app = WriterApp::open_project(path)?;
            for change in app.git_search_history(&text, file.as_deref(), 100)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    change.commit.id, change.commit.date, change.path, change.commit.subject
                );
                print!("{}", change.evidence_diff);
            }
        }
        "git-blame" => {
            let path = args.next().ok_or("missing project path")?;
            let file = args.next().ok_or("missing project-relative file")?;
            let first = args.next().ok_or("missing first line")?.parse::<usize>()?;
            let last = args.next().ok_or("missing last line")?.parse::<usize>()?;
            let app = WriterApp::open_project(path)?;
            for line in app.git_blame(&file, first, last)? {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    line.line, line.commit, line.author, line.date, line.content
                );
            }
        }
        "git-restore-new" => {
            let path = args.next().ok_or("missing source project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let destination = args.next().ok_or("missing new project destination")?;
            let files = WriterApp::restore_git_snapshot(path, &commit, &destination)?;
            println!(
                "Restored {commit} into {destination} ({} project files).",
                files.len()
            );
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
                filters.push(parse_scene_query_filter(&arg)?);
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
        "character-absence" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = story_core::SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let character =
                story_core::CharacterId(args.next().ok_or("missing character UUID")?.parse()?);
            let notes = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            app.record_character_absence(scene, character, &notes)?;
            println!("Explicit character absence recorded.");
        }
        "query-run" => {
            let path = args.next().ok_or("missing project path")?;
            let id = args.next().ok_or("missing query id")?.parse()?;
            let app = WriterApp::open_project(path)?;
            for scene in app.run_scene_query(QueryId(id))? {
                println!("{scene}");
            }
        }
        "query-expression-set" => {
            let path = args.next().ok_or("missing project path")?;
            let id = QueryId(args.next().ok_or("missing query id")?.parse()?);
            let source = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            if source == "clear" {
                app.set_scene_query_expression(id, None)?;
            } else {
                let expression: story_core::QueryExpression = serde_json::from_str(&source)?;
                app.set_scene_query_expression(id, Some(&expression))?;
            }
            println!("Updated expression for query {id}");
        }
        "query-assert" => {
            let path = args.next().ok_or("missing project path")?;
            let id = QueryId(args.next().ok_or("missing query ID")?.parse()?);
            let rule = args
                .next()
                .ok_or("missing assertion: non-empty|empty|at-least:N|at-most:N|exactly:N")?;
            let assertion = parse_query_assertion(&rule)?;
            let app = WriterApp::open_project(path)?;
            let count = app.run_scene_query(id)?.len();
            let report = story_analysis::evaluate_query_assertion(count, &assertion);
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.passed {
                return Err("saved query assertion failed".into());
            }
        }
        "query-assert-save" => {
            let path = args.next().ok_or("missing project path")?;
            let id = QueryId(args.next().ok_or("missing query ID")?.parse()?);
            let assertions = args
                .map(|value| parse_query_assertion(&value))
                .collect::<Result<Vec<_>, _>>()?;
            let mut app = WriterApp::open_project(path)?;
            app.set_query_assertions(id, &assertions)?;
            println!("Saved {} assertion(s) for query {id}", assertions.len());
        }
        "query-assert-check" => {
            let path = args.next().ok_or("missing project path")?;
            let id = QueryId(args.next().ok_or("missing query ID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            let assertions = app.get_query_assertions(id)?;
            if assertions.is_empty() {
                return Err("query has no saved assertions".into());
            }
            let count = app.run_scene_query(id)?.len();
            let reports = assertions
                .iter()
                .map(|assertion| story_analysis::evaluate_query_assertion(count, assertion))
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&reports)?);
            if reports.iter().any(|report| !report.passed) {
                return Err("one or more saved query assertions failed".into());
            }
        }
        "coverage" => {
            let path = args.next().ok_or("missing project path")?;
            let dimension =
                match args
                    .next()
                    .as_deref()
                    .ok_or("missing character|plotline|pov|location|event dimension")?
                {
                    "character" => story_analysis::CoverageDimension::Character,
                    "plotline" => story_analysis::CoverageDimension::Plotline,
                    "pov" => story_analysis::CoverageDimension::Pov,
                    "location" => story_analysis::CoverageDimension::Location,
                    "event" => story_analysis::CoverageDimension::Event,
                    _ => return Err(
                        "coverage dimension must be character, plotline, pov, location or event"
                            .into(),
                    ),
                };
            let entity_id = args.next().ok_or("missing entity UUID")?;
            let minimum_scenes = args
                .next()
                .ok_or("missing minimum scene count")?
                .parse::<usize>()?;
            let app = WriterApp::open_project(path)?;
            let report = story_analysis::evaluate_coverage(
                &app.analytics_dataset()?,
                &[story_analysis::CoverageRequirement {
                    id: "cli-coverage".into(),
                    label: format!(
                        "{} appears in at least {minimum_scenes} scenes",
                        dimension_label(dimension)
                    ),
                    entity_id,
                    dimension,
                    minimum_scenes,
                }],
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report[0].coverage_met {
                return Err("coverage requirement not met".into());
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
        "proofread" => {
            let path = args.next().ok_or("missing project path")?;
            let scope = args.next().unwrap_or_else(|| "all".into());
            let severities = args.next().map(|value| {
                value
                    .strip_prefix("severity=")
                    .unwrap_or(&value)
                    .split(',')
                    .map(str::to_owned)
                    .collect::<std::collections::BTreeSet<_>>()
            });
            if let Some(values) = &severities
                && (values.is_empty()
                    || values.iter().any(|value| {
                        !matches!(value.as_str(), "error" | "warning" | "information")
                    }))
            {
                return Err("severity filter must contain error, warning, or information".into());
            }
            let app = WriterApp::open_project(path)?;
            for scene in app
                .list_scenes(false)?
                .into_iter()
                .filter(|scene| scope == "all" || scene.scene.id.to_string() == scope)
            {
                let (text, _) = app.load_scene_text(scene.scene.id)?;
                for finding in app.proofread_text(&text)? {
                    if severities
                        .as_ref()
                        .is_some_and(|values| !values.contains(&finding.severity))
                    {
                        continue;
                    }
                    println!(
                        "{}\t{}\t{}..{}\t{}\t{}: {}",
                        scene.scene.id,
                        scene.scene.title,
                        finding.byte_range.start,
                        finding.byte_range.end,
                        finding.category,
                        finding.severity,
                        finding.message
                    );
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
        "proofread-config" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "Created diagnostics configuration at {}",
                app.write_default_proofreading_config()?.display()
            );
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
        "process-recipe" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let recipe = PathBuf::from(args.next().ok_or("missing processing recipe TOML")?);
            run_processing_recipe(&project, &recipe)?;
        }
        "watch-process" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let recipe = PathBuf::from(args.next().ok_or("missing processing recipe TOML")?);
            let poll_ms = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(1000)
                .clamp(250, 60_000);
            let mut previous = String::new();
            println!("Watching processing recipe; Ctrl-C stops the watcher.");
            loop {
                let recipe_config =
                    story_pipeline::ProcessingRecipe::from_toml(&fs::read_to_string(&recipe)?)?;
                let current = processing_watch_fingerprint(&project, &recipe)?;
                if current != previous {
                    if recipe_config.debounce_ms > 0 {
                        thread::sleep(Duration::from_millis(recipe_config.debounce_ms));
                    }
                    previous = processing_watch_fingerprint(&project, &recipe)?;
                    if let Err(error) = run_processing_recipe(&project, &recipe) {
                        eprintln!("Processing run incomplete; inspect stage failures: {error}");
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        }
        "compile" => {
            let project = args.next().ok_or("missing project path")?;
            let format_name = args
                .next()
                .ok_or("missing format (markdown, latex, html, epub, docx, pdf)")?;
            let format = story_document::CompileFormat::parse(&format_name)
                .ok_or("format must be markdown, latex, html, epub, docx, or pdf")?;
            let remaining = args.collect::<Vec<_>>();
            let expand = remaining
                .iter()
                .any(|argument| argument == "--expand-transclusions");
            let design = remaining
                .iter()
                .find_map(|argument| argument.strip_prefix("--design="))
                .map(|name| {
                    story_document::BookDesign::parse(name)
                        .ok_or("design must be trade-paperback, hardback, manuscript, nonfiction, or technical-academic")
                })
                .transpose()?
                .unwrap_or_default();
            let output = remaining
                .iter()
                .find(|argument| !argument.starts_with("--"));
            let app = WriterApp::open_project(&project)?;
            let artifact = app.compile_artifact_with_design(format, expand, design)?;
            let destination = output.map(PathBuf::from).unwrap_or_else(|| {
                app.project_root()
                    .join("exports")
                    .join(format!("compiled.{}", format.extension()))
            });
            atomic_write(&destination, &artifact.bytes)?;
            write_compile_auxiliary(&destination, &artifact.auxiliary_files)?;
            let sidecar = PathBuf::from(format!("{}.source-map.json", destination.display()));
            let source_map = serde_json::json!({
                "schema_version": 1,
                "output": destination.file_name().and_then(|name| name.to_str()).unwrap_or("compiled"),
                "output_format": format,
                "offset_space": "compile-ir-markdown-utf8",
                "visual_layout_mapping": false,
                "entries": artifact.source_map,
            });
            atomic_write(&sidecar, &serde_json::to_vec_pretty(&source_map)?)?;
            println!(
                "Compiled {} bytes to {}",
                artifact.bytes.len(),
                destination.display()
            );
            println!("Source map: {}", sidecar.display());
        }
        "compile-check" => {
            let project = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(project)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.compile_diagnostics()?)?
            );
        }
        "challenge-start" => {
            let path = args.next().ok_or("missing project path")?;
            let target = args
                .next()
                .ok_or("missing target word count")?
                .parse::<u32>()?;
            let seconds = args
                .next()
                .ok_or("missing duration seconds")?
                .parse::<u32>()?;
            let mode = args.next().unwrap_or_else(|| "steady".into());
            let mut app = WriterApp::open_project(path)?;
            println!("{}", app.start_writing_challenge(target, seconds, &mode)?);
        }
        "challenge-finish" => {
            let path = args.next().ok_or("missing project path")?;
            let id = args.next().ok_or("missing challenge ID")?;
            let words = args.next().ok_or("missing words written")?.parse::<u32>()?;
            let abandoned = args.next().as_deref() == Some("abandoned");
            let mut app = WriterApp::open_project(path)?;
            app.finish_writing_challenge_with_activity(
                &id,
                words,
                0,
                0,
                if abandoned { "quit" } else { "timeout" },
                None,
            )?;
            println!("Challenge {id} recorded.");
        }
        "challenge-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.list_writing_challenges()?)?
            );
        }
        "goal-create" => {
            let path = args.next().ok_or("missing project path")?;
            let start_date = args.next().ok_or("missing start date YYYY-MM-DD")?;
            let end_date = args.next().ok_or("missing inclusive end date YYYY-MM-DD")?;
            let target_words = args
                .next()
                .ok_or("missing target word count")?
                .parse::<i64>()?;
            let baseline_words = args
                .next()
                .ok_or("missing starting word count")?
                .parse::<i64>()?;
            let time_zone = args
                .next()
                .ok_or("missing timezone (UTC or fixed offset)")?;
            let success_metric = args.next().ok_or("missing raw|real success metric")?;
            let scope = args.next().ok_or("missing project or scene scope")?;
            let scope_scene_id = if scope == "project" {
                None
            } else {
                Some(SceneId(scope.parse()?))
            };
            let title = args.next().ok_or("missing goal title")?;
            let mut app = WriterApp::open_project(path)?;
            let id = app.create_writing_goal(&story_storage::NewWritingGoal {
                title,
                scope_scene_id,
                start_date,
                end_date,
                target_words,
                baseline_words,
                time_zone,
                word_count_policy: "unicode-word-boundaries".into(),
                success_metric,
            })?;
            println!("{id}");
        }
        "goal-list" => {
            let path = args.next().ok_or("missing project path")?;
            let include_archived = args.next().as_deref() == Some("--all");
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.list_writing_goals(include_archived)?)?
            );
        }
        "goal-record" => {
            let path = args.next().ok_or("missing project path")?;
            let goal_id = args.next().ok_or("missing goal ID")?;
            let occurred_at = args
                .next()
                .ok_or("missing Unix timestamp")?
                .parse::<i64>()?;
            let origin = args.next().ok_or("missing event origin")?;
            let source_identifier = args.next().ok_or("missing source identifier")?;
            let source_fingerprint = args.next().ok_or("missing source fingerprint")?;
            let scene = args.next().ok_or("missing scene UUID or none")?;
            let scene_id = if scene == "none" {
                None
            } else {
                Some(SceneId(scene.parse()?))
            };
            let raw_delta = args
                .next()
                .ok_or("missing raw word delta")?
                .parse::<i64>()?;
            let real_delta = args
                .next()
                .ok_or("missing real word delta")?
                .parse::<i64>()?;
            let detail = args.next().unwrap_or_default();
            let mut app = WriterApp::open_project(path)?;
            let id = app.record_writing_goal_event(&story_storage::NewWritingGoalEvent {
                goal_id,
                occurred_at,
                origin,
                source_identifier,
                source_fingerprint,
                scene_id,
                raw_delta,
                real_delta,
                detail,
            })?;
            println!("{id}");
        }
        "goal-events" => {
            let path = args.next().ok_or("missing project path")?;
            let goal_id = args.next().ok_or("missing goal ID")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.list_writing_goal_events(&goal_id)?)?
            );
        }
        "goal-progress" | "goal-weekly" => {
            let path = args.next().ok_or("missing project path")?;
            let goal_id = args.next().ok_or("missing goal ID")?;
            let app = WriterApp::open_project(path)?;
            let as_of = time::OffsetDateTime::now_utc().unix_timestamp();
            let points = app.writing_goal_progress(&goal_id, as_of)?;
            if cmd == "goal-progress" {
                println!("{}", serde_json::to_string_pretty(&points)?);
            } else {
                let goal = app
                    .list_writing_goals(true)?
                    .into_iter()
                    .find(|goal| goal.id == goal_id)
                    .ok_or("writing goal not found")?;
                let mut prior_target = goal.baseline_words;
                let mut prior_raw = goal.baseline_words;
                let mut prior_real = goal.baseline_words;
                let rows = points.chunks(7).map(|week| {
                    let last = week.last().expect("week chunks are non-empty");
                    let target_delta = last.target_words.saturating_sub(prior_target);
                    prior_target = last.target_words;
                    let raw_delta = last.raw_words.map(|raw| { let delta = raw.saturating_sub(prior_raw); prior_raw = raw; delta });
                    let real_delta = last.real_words.map(|real| { let delta = real.saturating_sub(prior_real); prior_real = real; delta });
                    serde_json::json!({"start":week[0].date,"end":last.date,"target_pace":target_delta,"raw":raw_delta,"real":real_delta,"projected":last.projected})
                }).collect::<Vec<_>>();
                println!("{}", serde_json::to_string_pretty(&rows)?);
            }
        }
        "goal-archive" => {
            let path = args.next().ok_or("missing project path")?;
            let goal_id = args.next().ok_or("missing goal ID")?;
            WriterApp::open_project(path)?.archive_writing_goal(&goal_id)?;
        }
        "goal-reconcile" => {
            let path = args.next().ok_or("missing project path")?;
            let event_id = args.next().ok_or("missing event ID")?;
            let fingerprint = args.next().ok_or("missing expected fingerprint")?;
            let raw = args
                .next()
                .ok_or("missing corrected raw count")?
                .parse::<i64>()?;
            let real = args
                .next()
                .ok_or("missing corrected real count")?
                .parse::<i64>()?;
            let reason = args.next().ok_or("missing correction reason")?;
            let mut app = WriterApp::open_project(path)?;
            println!(
                "{}",
                app.reconcile_writing_goal_event(&event_id, &fingerprint, raw, real, &reason)?
            );
        }
        "compile-watch" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let format_name = args.next().ok_or("missing format")?;
            let format = story_document::CompileFormat::parse(&format_name)
                .ok_or("format must be markdown, latex, html, epub, docx, or pdf")?;
            let output = PathBuf::from(args.next().ok_or("missing output path")?);
            let poll_ms = args
                .next()
                .map(|v| v.parse::<u64>())
                .transpose()?
                .unwrap_or(1000)
                .clamp(250, 60_000);
            let mut previous = String::new();
            println!("Watching project for changes; Ctrl-C stops the watcher.");
            loop {
                let fingerprint = watch_fingerprint(&project)?;
                if fingerprint != previous {
                    previous = fingerprint;
                    match WriterApp::open_project(&project)
                        .and_then(|app| app.compile_artifact(format, false))
                    {
                        Ok(artifact) => {
                            atomic_write(&output, &artifact.bytes)?;
                            write_compile_auxiliary(&output, &artifact.auxiliary_files)?;
                            let map = serde_json::to_vec_pretty(
                                &serde_json::json!({"schema_version":1,"entries":artifact.source_map}),
                            )?;
                            atomic_write(
                                &PathBuf::from(format!("{}.source-map.json", output.display())),
                                &map,
                            )?;
                            println!(
                                "Compiled {} bytes to {}",
                                artifact.bytes.len(),
                                output.display()
                            );
                        }
                        Err(error) => {
                            eprintln!("Compile failed; previous output preserved: {error}")
                        }
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        }
        "compile-edition" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let recipe = PathBuf::from(args.next().ok_or("missing edition recipe TOML")?);
            compile_edition(&project, &recipe)?;
        }
        "spellcheck" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let language = args.next().ok_or("missing Aspell language tag")?;
            let dictionary = args.next();
            let findings =
                spellcheck_project(&project, &language, dictionary.as_deref(), "aspell")?;
            println!("{}", serde_json::to_string_pretty(&findings)?);
            if !findings.is_empty() {
                return Err(
                    format!("{} scene(s) contain spelling candidates", findings.len()).into(),
                );
            }
        }
        "watch-edition" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let recipe = PathBuf::from(args.next().ok_or("missing edition recipe TOML")?);
            let poll_ms = args
                .next()
                .map(|v| v.parse::<u64>())
                .transpose()?
                .unwrap_or(1000)
                .clamp(250, 60_000);
            let mut previous = String::new();
            println!("Watching edition recipe; Ctrl-C stops the watcher.");
            loop {
                let mut fingerprint = Sha256::new();
                fingerprint.update(watch_fingerprint(&project)?);
                let recipe_bytes = fs::read(&recipe)?;
                fingerprint.update(&recipe_bytes);
                let recipe_config =
                    story_document::EditionRecipe::from_toml(std::str::from_utf8(&recipe_bytes)?)?;
                if let Some(dictionary) = recipe_config.dictionary {
                    fingerprint.update(fs::read(project.join(dictionary))?);
                }
                let current = format!("{:x}", fingerprint.finalize());
                if current != previous {
                    previous = current;
                    if let Err(error) = compile_edition(&project, &recipe) {
                        eprintln!("Edition compile failed; previous output preserved: {error}");
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        }
        "inspect-manuscript" => {
            let source = args
                .next()
                .ok_or("missing DOCX, RTF, ODT, or EPUB source path")?;
            let staging = story_import::inspect_manuscript(&source)?;
            println!("{}", serde_json::to_string_pretty(&staging)?);
        }
        "review-inspect" => {
            let source = args.next().ok_or("missing DOCX file")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&story_import::inspect_docx_review(source)?)?
            );
        }
        "import-manuscript" => {
            let source = args
                .next()
                .ok_or("missing DOCX, RTF, ODT, or EPUB source path")?;
            let destination = PathBuf::from(args.next().ok_or("missing new project directory")?);
            let requested_title = args.next();
            if destination.exists() {
                return Err("manuscript import requires a new project directory that does not already exist".into());
            }
            let staging = story_import::inspect_manuscript(&source)?;
            let title = requested_title.as_deref().unwrap_or(&staging.title);
            let mut app = WriterApp::create_project(&destination, title)?;
            let result = (|| -> Result<(story_core::ImportRunId, Vec<SceneId>), Box<dyn std::error::Error>> {
                let (run, scenes) = app.import_staged_manuscript(&staging)?;
                let report = serde_json::json!({"staging": staging, "import_run": app.get_import_run_summary(run)?, "imported_scene_ids": scenes});
                atomic_write(
                    &destination.join("exports/import-report.json"),
                    &serde_json::to_vec_pretty(&report)?,
                )?;
                Ok((run, scenes))
            })();
            match result {
                Ok((_, scenes)) => println!(
                    "Imported {} manuscript units into {}",
                    scenes.len(),
                    destination.display()
                ),
                Err(error) => {
                    drop(app);
                    let _ = fs::remove_dir_all(&destination);
                    return Err(error);
                }
            }
        }
        "review-merge" => {
            let project = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let base = fs::read_to_string(args.next().ok_or("missing base manuscript file")?)?;
            let reviewed =
                fs::read_to_string(args.next().ok_or("missing reviewed manuscript file")?)?;
            let mut app = WriterApp::open_project(project)?;
            let result = app.merge_review_text(scene, &base, &reviewed)?;
            if result.merged_text.is_some() {
                println!(
                    "Editorial revision merged and saved through the manuscript write journal."
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&result.conflicts)?);
                return Err("overlapping editorial changes require author resolution".into());
            }
        }
        "review-docx" => {
            let project = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let base = fs::read_to_string(args.next().ok_or("missing base manuscript file")?)?;
            let docx = args.next().ok_or("missing reviewed DOCX file")?;
            let disposition = args.next().ok_or("choose accept or reject")?;
            let accept = match disposition.as_str() {
                "accept" => true,
                "reject" => false,
                _ => return Err("review disposition must be accept or reject".into()),
            };
            let mut app = WriterApp::open_project(project)?;
            let result = app.merge_docx_review(scene, &base, docx, accept)?;
            if result.merged_text.is_some() {
                println!(
                    "DOCX revisions {disposition}ed and merged through the durable save journal."
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&result.conflicts)?);
                return Err("overlapping editorial changes require author resolution".into());
            }
        }
        "browse" => {
            let project = args.next().ok_or("missing project path")?;
            let port = args
                .next()
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(7878);
            let mut app = WriterApp::open_project(project)?;
            let server = story_web::BrowserServer::bind(port)?;
            println!(
                "Read-only Writer's IDE browser view at http://{} (Ctrl-C to stop)",
                server.address()
            );
            server.serve(&mut app)?;
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

fn parse_find_mode(value: &str) -> Result<story_core::FindMode, Box<dyn std::error::Error>> {
    Ok(match value.to_ascii_lowercase().as_str() {
        "normal" => story_core::FindMode::Normal,
        "extended" => story_core::FindMode::Extended,
        "regex" => story_core::FindMode::Regex,
        _ => return Err("mode must be normal, extended, or regex".into()),
    })
}

fn dimension_label(dimension: story_analysis::CoverageDimension) -> &'static str {
    match dimension {
        story_analysis::CoverageDimension::Character => "character",
        story_analysis::CoverageDimension::Plotline => "plotline",
        story_analysis::CoverageDimension::Pov => "pov",
        story_analysis::CoverageDimension::Location => "location",
        story_analysis::CoverageDimension::Event => "event",
    }
}

fn parse_query_assertion(
    value: &str,
) -> Result<story_core::QueryAssertion, Box<dyn std::error::Error>> {
    match value {
        "non-empty" => Ok(story_core::QueryAssertion::NonEmpty),
        "empty" => Ok(story_core::QueryAssertion::Empty),
        _ => {
            let (kind, threshold) = value
                .split_once(':')
                .ok_or("numeric assertion must be kind:threshold")?;
            let threshold = threshold.parse::<u64>()?;
            Ok(match kind {
                "at-least" => story_core::QueryAssertion::AtLeast(threshold),
                "at-most" => story_core::QueryAssertion::AtMost(threshold),
                "exactly" => story_core::QueryAssertion::Exactly(threshold),
                _ => {
                    return Err(
                        "assertion must be at-least, at-most, exactly, non-empty, or empty".into(),
                    );
                }
            })
        }
    }
}

fn validate_scene_scope(scope: &str) -> Result<(), Box<dyn std::error::Error>> {
    if scope == "all" {
        return Ok(());
    }
    if let Some(chapter) = scope.strip_prefix("chapter:") {
        let _: uuid::Uuid = chapter.parse()?;
    } else {
        let _: uuid::Uuid = scope.parse()?;
    }
    Ok(())
}

fn scene_matches_scope(scene: &story_storage::SceneRecord, scope: &str) -> bool {
    if scope == "all" {
        return true;
    }
    if let Some(chapter) = scope.strip_prefix("chapter:") {
        scene.chapter_id.is_some_and(|id| id.to_string() == chapter)
    } else {
        scene.scene.id.to_string() == scope
    }
}
fn usage() {
    eprintln!(
        "writers-ide compile <project> <markdown|latex|html|epub|docx|pdf> [output] [--expand-transclusions] [--design=preset]"
    );
    eprintln!("writers-ide compile-check <project>");
    eprintln!("writers-ide inspect-manuscript <DOCX|RTF|ODT|EPUB-source>");
    eprintln!("writers-ide import-manuscript <source-file> <new-project-directory> [title]");
    eprintln!("writers-ide browse <project> [loopback-port]");
    eprintln!(
        "writers-ide compile-edition <project> <recipe.toml>\nwriters-ide watch-edition <project> <recipe.toml> [poll-ms]\nwriters-ide spellcheck <project> <language-tag> [project-relative-wordlist]"
    );
    eprintln!(
        "writers-ide query-assert-save <project> <query-uuid> <non-empty|empty|at-least:N|at-most:N|exactly:N>...\nwriters-ide query-assert-check <project> <query-uuid>"
    );
    eprintln!(
        "writers-ide proofread-config <project> (creates editable .writers-ide/diagnostics.toml)"
    );
    eprintln!(
        "writers-ide find <path> <normal|extended|regex> <all|scene-uuid> <pattern>\nwriters-ide replace <path> <normal|extended|regex> <all|scene-uuid> <pattern> => <replacement>\nwriters-ide language <path> [language-tag]"
    );
    eprintln!(
        "writers-ide create <path> [title]\nwriters-ide create-kind <path> <fiction|nonfiction|custom> [title]\nwriters-ide inspect <path>\nwriters-ide chapter <path> <title>\nwriters-ide scene <path> <chapter-uuid> <title>\nwriters-ide show <path> <scene-uuid>\nwriters-ide write <path> <scene-uuid> <text>\nwriters-ide search <path> <terms>\nwriters-ide analytics <path>\nwriters-ide process <path> [--expand-transclusions] [--only stage,stage]\nwriters-ide compile <path> <markdown|html|epub|docx|pdf> [output-path] [--expand-transclusions]\nwriters-ide extract <path> <scene|reading> <source-uuid> <model> [endpoint]\nwriters-ide study <path> <scene|reading> <source-uuid> <model> [endpoint] [--questions]\nwriters-ide proposals <project>\nwriters-ide approve-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide dismiss-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide knowledge <project>\nwriters-ide query-save <path> <name> [status=...|character=UUID|present=UUID|mentioned=UUID|absent=UUID|location=UUID|event=UUID|chapter=UUID|text=TERM|plotline=UUID|assembly=UUID|issue-status=STATUS|meta=FIELD:VALUE|meta-contains=FIELD:KIND:ENTITY|before-definitely=TICK|before-possibly=TICK|after-definitely=TICK|after-possibly=TICK|before=TICK|after=TICK]...\nwriters-ide character-absence <path> <scene-uuid> <character-uuid> [notes]\nwriters-ide query-list <path>\nwriters-ide query-run <path> <query-uuid>\nwriters-ide link <path> <source-kind> <source-id> <target-kind> <target-id> <related|cites|transcludes|includes|navigates> [label]\nwriters-ide outgoing <path> <entity-kind> <entity-id>\nwriters-ide backlinks <path> <entity-kind> <entity-id>\nwriters-ide broken-links <path>\nwriters-ide snapshot <path> <scene-uuid> <label>\nwriters-ide history <path> <scene-uuid>\nwriters-ide diff <path> <scene-uuid> <snapshot-uuid>\nwriters-ide restore <path> <scene-uuid> <snapshot-uuid>\nwriters-ide resolve <path> <transaction-uuid> external|prepared\nwriters-ide backup <path> <destination>\nwriters-ide character-profile <path> <character-uuid>\nwriters-ide character-set <path> <character-uuid> <field> <value>\nwriters-ide character-alias <path> <character-uuid> <alias>\nwriters-ide character-un-alias <path> <character-uuid> <alias>"
    );
    eprintln!(
        "writers-ide anchors <path> <scene-uuid>\nwriters-ide anchor-move <path> <anchor-uuid> <start-byte> <end-byte>"
    );
    eprintln!("writers-ide search-all <path> <terms>\nwriters-ide rebuild-search <path>");
    eprintln!("writers-ide query-expression-set <path> <query-uuid> <JSON|clear>");
    eprintln!(
        "writers-ide git-search-history <path> <literal-text> [project-relative-path]\nwriters-ide git-blame <path> <project-relative-file> <first-line> <last-line>"
    );
    eprintln!(
        "writers-ide create-template <path> <blank-fiction|novel|short-story|blank-nonfiction|nonfiction-book> [title]\nwriters-ide content <path> <chapter|section|front|back> <parent-uuid|-> <title>\nwriters-ide read-content <path> <content-uuid>\nwriters-ide write-content <path> <content-uuid> <text>\nwriters-ide resolve-content <path> <transaction-id> external|prepared\nwriters-ide git-snapshot <path> <label>\nwriters-ide git-history <path>\nwriters-ide git-diff <path> <commit-a> <commit-b> [project-relative-path]\nwriters-ide git-restore-new <source-project> <commit> <new-project-directory>"
    );
    eprintln!(
        "writers-ide scratchpad-create <path> <title> [body]\nwriters-ide scratchpad-list <path>\nwriters-ide scratchpad-promote <path> <note-uuid> <scene|character|location|object|research> <entity-uuid>\nwriters-ide mentions <path> <scene-uuid>\nwriters-ide mention-confirm <path> <mention-uuid> <character|location|object> <entity-uuid>\nwriters-ide mention-dismiss <path> <mention-uuid>\nwriters-ide object-create <path> <name> [description]\nwriters-ide assembly-draft-create <path> <assembly-uuid>\nwriters-ide assembly-draft-list <path>\nwriters-ide assembly-draft-set <path> <draft-uuid> <scene-uuid> <text.md>\nwriters-ide assembly-draft-commit <path> <draft-uuid>\nwriters-ide board-create <path> <title>\nwriters-ide board-add-scene <path> <board-uuid> <scene-uuid> <x> <y> [note]\nwriters-ide board-list <path> <board-uuid>\nwriters-ide backup-archive <project-path> <archive.zip>\nwriters-ide restore-archive <archive.zip> <new-project-directory>"
    );
    eprintln!(
        "writers-ide compile-watch <path> <format> <output> [poll-ms]\nwriters-ide process-recipe <project> <recipe.toml>\nwriters-ide watch-process <project> <recipe.toml> [poll-ms]\nwriters-ide query-assert <path> <query-uuid> <empty|non-empty|at-least:N|at-most:N|exactly:N>\nwriters-ide coverage <path> <character|plotline> <entity-uuid> <minimum-scenes>\nwriters-ide challenge-start <path> <target-words> <duration-seconds> [steady|kamikaze]\nwriters-ide challenge-finish <path> <challenge-id> <words-written> [abandoned]\nwriters-ide challenge-list <path>\nwriters-ide goal-create <project> <start> <end> <target> <baseline> <UTC|±HH:MM> <raw|real> <project|scene-uuid> <title>\nwriters-ide goal-list <project> [--all]\nwriters-ide goal-record <project> <goal> <unix-time> <origin> <source-id> <fingerprint> <scene-uuid|none> <raw> <real> [detail]\nwriters-ide goal-events <project> <goal>\nwriters-ide goal-progress <project> <goal>\nwriters-ide goal-weekly <project> <goal>\nwriters-ide goal-archive <project> <goal>\nwriters-ide goal-reconcile <project> <event> <fingerprint> <raw> <real> <reason>"
    );
}

fn run_processing_recipe(
    project: &Path,
    recipe_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(recipe_path)?;
    let recipe = story_pipeline::ProcessingRecipe::from_toml(&source)?;
    let app = WriterApp::open_project(project)?;
    let reports = app.run_processing_recipe(&recipe)?;
    let incomplete = reports.iter().any(|report| {
        matches!(
            report.status,
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
        return Err("processing recipe incomplete; inspect failed/skipped stages".into());
    }
    Ok(())
}

fn processing_watch_fingerprint(
    project: &Path,
    recipe_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut fingerprint = Sha256::new();
    fingerprint.update(watch_fingerprint(project)?);
    fingerprint.update(fs::read(recipe_path)?);
    Ok(format!("{:x}", fingerprint.finalize()))
}

fn watch_fingerprint(project: &Path) -> Result<String, Box<dyn std::error::Error>> {
    fn visit(
        path: &Path,
        manuscript_root: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if entry.file_name() == "exports" || entry.file_name() == ".writers-ide" {
                    continue;
                }
                visit(&entry.path(), manuscript_root, files)?;
            }
        } else if path.file_name().is_some_and(|name| {
            name == "project.toml" || name.to_string_lossy().starts_with("story.db")
        }) || path.starts_with(manuscript_root)
        {
            files.push(path.to_path_buf());
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(project, &project.join("manuscript"), &mut files)?;
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        hash.update(path.strip_prefix(project)?.to_string_lossy().as_bytes());
        hash.update(fs::read(path)?);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn compile_edition(project: &Path, recipe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    compile_edition_with_aspell(project, recipe_path, "aspell")
}

fn compile_edition_with_aspell(
    project: &Path,
    recipe_path: &Path,
    aspell: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(recipe_path)?;
    let recipe = story_document::EditionRecipe::from_toml(&source)?;
    let output_relative = Path::new(&recipe.output);
    ensure_project_path_has_no_symlink(project, output_relative)?;
    let spelling = if recipe.spellcheck {
        Some(spellcheck_project(
            project,
            recipe
                .language
                .as_deref()
                .ok_or("spellcheck language missing")?,
            recipe.dictionary.as_deref(),
            aspell,
        )?)
    } else {
        None
    };
    let app = WriterApp::open_project(project)?;
    let artifact = app.compile_artifact_with_design(
        recipe.format,
        recipe.expand_transclusions,
        recipe.design,
    )?;
    let output = project.join(output_relative);
    atomic_write(&output, &artifact.bytes)?;
    write_compile_auxiliary(&output, &artifact.auxiliary_files)?;
    let sidecar = PathBuf::from(format!("{}.source-map.json", output.display()));
    let map = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 1,
        "edition": recipe.name,
        "output_format": recipe.format,
        "offset_space": "compile-ir-markdown-utf8",
        "visual_layout_mapping": false,
        "entries": artifact.source_map,
    }))?;
    atomic_write(&sidecar, &map)?;
    if let Some(findings) = spelling {
        let report = PathBuf::from(format!("{}.spellcheck.json", output.display()));
        atomic_write(&report, &serde_json::to_vec_pretty(&findings)?)?;
        if !findings.is_empty() {
            eprintln!(
                "Edition spelling check found candidate words in {} scene(s): {}",
                findings.len(),
                report.display()
            );
        }
    }
    println!(
        "Compiled edition {} · {} bytes · {}",
        recipe.name,
        artifact.bytes.len(),
        output.display()
    );
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize)]
struct SpellingFinding {
    entity_kind: String,
    entity_id: String,
    title: String,
    candidates: std::collections::BTreeMap<String, usize>,
}

fn spellcheck_project(
    project: &Path,
    language: &str,
    dictionary: Option<&str>,
    executable: &str,
) -> Result<Vec<SpellingFinding>, Box<dyn std::error::Error>> {
    if language.trim().is_empty() {
        return Err("language tag cannot be empty".into());
    }
    let allowed = if let Some(relative) = dictionary {
        let relative = Path::new(relative);
        ensure_project_path_has_no_symlink(project, relative)?;
        let path = project.join(relative);
        if !fs::symlink_metadata(&path)?.is_file() {
            return Err("personal dictionary must be a regular file".into());
        }
        story_document::parse_personal_dictionary(&fs::read_to_string(path)?)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
    } else {
        std::collections::BTreeSet::new()
    };
    let app = WriterApp::open_project(project)?;
    let mut findings = Vec::new();
    let input = app.processing_input()?;
    let prose = input
        .scenes
        .into_iter()
        .map(|scene| ("scene".to_owned(), scene.id, scene.title, scene.text))
        .chain(
            input
                .content_units
                .into_iter()
                .map(|unit| ("content-unit".to_owned(), unit.id, unit.title, unit.text)),
        );
    for (entity_kind, entity_id, title, text) in prose {
        let mut child = Command::new(executable)
            .arg(format!("--lang={language}"))
            .arg("list")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or("Aspell stdin unavailable")?
            .write_all(text.as_bytes())?;
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(format!(
                "Aspell failed for {entity_kind} {title}: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let mut candidates = std::collections::BTreeMap::new();
        for word in String::from_utf8(output.stdout)?
            .lines()
            .map(str::trim)
            .filter(|word| !word.is_empty())
        {
            let normalized = word.to_lowercase();
            if !allowed.contains(&normalized) {
                *candidates.entry(normalized).or_insert(0) += 1;
            }
        }
        if !candidates.is_empty() {
            findings.push(SpellingFinding {
                entity_kind,
                entity_id,
                title,
                candidates,
            });
        }
    }
    Ok(findings)
}

fn ensure_project_path_has_no_symlink(
    root: &Path,
    relative: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err("project output path contains traversal".into());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "project output path crosses a symlink: {}",
                    current.display()
                )
                .into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn write_compile_auxiliary(
    output: &Path,
    files: &[story_document::CompileAuxiliaryFile],
) -> Result<(), Box<dyn std::error::Error>> {
    for file in files {
        let extension = Path::new(&file.name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("log");
        atomic_write(&output.with_extension(extension), &file.bytes)?;
    }
    Ok(())
}

fn parse_story_entity_kind(
    value: &str,
) -> Result<story_core::StoryBibleEntityKind, Box<dyn std::error::Error>> {
    match value {
        "character" => Ok(story_core::StoryBibleEntityKind::Character),
        "location" => Ok(story_core::StoryBibleEntityKind::Location),
        "object" => Ok(story_core::StoryBibleEntityKind::Object),
        "plotline" => Ok(story_core::StoryBibleEntityKind::Plotline),
        "event" => Ok(story_core::StoryBibleEntityKind::Event),
        _ => Err("entity kind must be character, location, object, plotline, or event".into()),
    }
}
fn parse_scene_query_filter(
    argument: &str,
) -> Result<SceneQueryFilter, Box<dyn std::error::Error>> {
    let (key, value) = argument
        .split_once('=')
        .ok_or("filters must use key=value syntax")?;
    let parse_ids = |value: &str| {
        value
            .split(',')
            .map(str::parse::<uuid::Uuid>)
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(match key {
        "status" => SceneQueryFilter::Status(parse_status(value)?),
        "not-status" => SceneQueryFilter::ExcludeStatus(parse_status(value)?),
        "character" => SceneQueryFilter::Character(story_core::CharacterId(value.parse()?)),
        "present" => SceneQueryFilter::CharacterPresent(story_core::CharacterId(value.parse()?)),
        "mentioned" => {
            SceneQueryFilter::CharacterMentioned(story_core::CharacterId(value.parse()?))
        }
        "absent" => SceneQueryFilter::CharacterAbsent(story_core::CharacterId(value.parse()?)),
        "character-any" => SceneQueryFilter::AnyCharacter(
            parse_ids(value)?
                .into_iter()
                .map(story_core::CharacterId)
                .collect(),
        ),
        "plotline" => SceneQueryFilter::Plotline(story_core::PlotlineId(value.parse()?)),
        "location" => SceneQueryFilter::Location(story_core::LocationId(value.parse()?)),
        "event" => SceneQueryFilter::Event(story_core::EventId(value.parse()?)),
        "assembly" => SceneQueryFilter::Assembly(story_core::AssemblyId(value.parse()?)),
        "issue-status" => SceneQueryFilter::IssueStatus(value.to_owned()),
        "before-definitely" => SceneQueryFilter::TimeBeforeDefinitely(value.parse()?),
        "before-possibly" => SceneQueryFilter::TimeBeforePossibly(value.parse()?),
        "after-definitely" => SceneQueryFilter::TimeAfterDefinitely(value.parse()?),
        "after-possibly" => SceneQueryFilter::TimeAfterPossibly(value.parse()?),
        "chapter" => SceneQueryFilter::Chapter(value.to_owned()),
        "text" => SceneQueryFilter::TextContains(value.to_owned()),
        "meta" => {
            let (field_id, raw_value) = value
                .split_once(':')
                .ok_or("meta filter uses meta=FIELD-UUID:VALUE")?;
            let field_id = story_core::FieldId(field_id.parse()?);
            let value = if let Some(value) = raw_value.strip_prefix("int:") {
                story_core::FieldValue::Integer(value.parse()?)
            } else if let Some(value) = raw_value.strip_prefix("decimal:") {
                story_core::FieldValue::Decimal(value.into())
            } else if let Some(value) = raw_value.strip_prefix("bool:") {
                story_core::FieldValue::Boolean(value.parse()?)
            } else if let Some(value) = raw_value.strip_prefix("datetime:") {
                story_core::FieldValue::DateTime(value.into())
            } else if let Some(value) = raw_value.strip_prefix("enum:") {
                story_core::FieldValue::Enum(value.into())
            } else if let Some(value) = raw_value.strip_prefix("entity:") {
                story_core::FieldValue::EntityRef(value.into())
            } else {
                story_core::FieldValue::Text(raw_value.into())
            };
            SceneQueryFilter::CustomMetadataEquals { field_id, value }
        }
        "meta-contains" => {
            let mut parts = value.splitn(3, ':');
            let field_id = story_core::FieldId(
                parts
                    .next()
                    .ok_or("meta-contains uses FIELD-UUID:KIND:ENTITY-UUID")?
                    .parse()?,
            );
            let target_kind = parts
                .next()
                .ok_or("meta-contains uses FIELD-UUID:KIND:ENTITY-UUID")?
                .to_owned();
            let target_id = parts
                .next()
                .ok_or("meta-contains uses FIELD-UUID:KIND:ENTITY-UUID")?
                .to_owned();
            SceneQueryFilter::CustomMetadataContainsEntity {
                field_id,
                target_kind,
                target_id,
            }
        }
        "plotline-any" => SceneQueryFilter::AnyPlotline(
            parse_ids(value)?
                .into_iter()
                .map(story_core::PlotlineId)
                .collect(),
        ),
        "before" => SceneQueryFilter::TimeBefore(value.parse()?),
        "after" => SceneQueryFilter::TimeAfter(value.parse()?),
        "issue" if value == "unresolved" => SceneQueryFilter::UnresolvedIssue,
        _ => return Err(format!("unsupported query filter: {argument}").into()),
    })
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

#[cfg(test)]
mod watch_tests {
    use super::{
        compile_edition, compile_edition_with_aspell, parse_scene_query_filter,
        run_processing_recipe, spellcheck_project, watch_fingerprint,
    };
    use std::fs;
    use story_app::WriterApp;
    #[test]
    fn custom_entity_reference_query_filter_parses_stable_identity() {
        let field = uuid::Uuid::new_v4();
        let entity = uuid::Uuid::new_v4();
        assert_eq!(
            parse_scene_query_filter(&format!("meta-contains={field}:character:{entity}")).unwrap(),
            story_core::SceneQueryFilter::CustomMetadataContainsEntity {
                field_id: story_core::FieldId(field),
                target_kind: "character".into(),
                target_id: entity.to_string(),
            }
        );
    }
    #[test]
    fn uncertainty_aware_query_filters_parse_signed_ticks() {
        assert_eq!(
            parse_scene_query_filter("before-definitely=-86400").unwrap(),
            story_core::SceneQueryFilter::TimeBeforeDefinitely(-86400)
        );
        assert_eq!(
            parse_scene_query_filter("after-possibly=12").unwrap(),
            story_core::SceneQueryFilter::TimeAfterPossibly(12)
        );
    }
    #[test]
    fn watch_fingerprint_tracks_manuscript_and_ignores_generated_exports() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("manuscript")).unwrap();
        fs::create_dir_all(root.path().join("exports")).unwrap();
        fs::write(root.path().join("project.toml"), "title='A'").unwrap();
        fs::write(root.path().join("story.db"), "db").unwrap();
        let scene = root.path().join("manuscript/scene.md");
        fs::write(&scene, "first").unwrap();
        let first = watch_fingerprint(root.path()).unwrap();
        fs::write(root.path().join("exports/generated.md"), "ignored").unwrap();
        assert_eq!(first, watch_fingerprint(root.path()).unwrap());
        fs::write(root.path().join("story.db-wal"), "pending transaction").unwrap();
        assert_ne!(first, watch_fingerprint(root.path()).unwrap());
        let with_wal = watch_fingerprint(root.path()).unwrap();
        fs::write(scene, "second").unwrap();
        assert_ne!(with_wal, watch_fingerprint(root.path()).unwrap());
    }

    #[test]
    fn edition_recipe_compiles_project_relative_output_and_source_map() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Novel");
        let mut project = WriterApp::create_project(&project_path, "Novel").unwrap();
        let chapter = project.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = project.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "The story begins.")
            .unwrap();
        drop(project);
        let recipe_path = root.path().join("edition.toml");
        fs::write(&recipe_path, "version=1\nname='Reader'\nformat='markdown'\noutput='exports/reader.md'\ndesign='trade-paperback'\nexpand_transclusions=false\n").unwrap();
        compile_edition(&project_path, &recipe_path).unwrap();
        assert!(
            fs::read_to_string(project_path.join("exports/reader.md"))
                .unwrap()
                .contains("The story begins.")
        );
        assert!(
            project_path
                .join("exports/reader.md.source-map.json")
                .exists()
        );
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(project_path.join("dictionaries")).unwrap();
        fs::write(project_path.join("dictionaries/en.txt"), "known\n").unwrap();
        let mock = root.path().join("aspell");
        fs::write(
            &mock,
            "#!/bin/sh\ncat >/dev/null\nprintf 'baad\\nknown\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&mock, fs::Permissions::from_mode(0o755)).unwrap();
        let checked_recipe = root.path().join("checked.toml");
        fs::write(&checked_recipe, "version=1\nname='Checked reader'\nformat='markdown'\noutput='exports/checked.md'\nspellcheck=true\nlanguage='en_GB'\ndictionary='dictionaries/en.txt'\n").unwrap();
        compile_edition_with_aspell(&project_path, &checked_recipe, mock.to_str().unwrap())
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(
            &fs::read(project_path.join("exports/checked.md.spellcheck.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report[0]["candidates"]["baad"], 1);
        assert!(report[0]["candidates"].get("known").is_none());
    }

    #[test]
    fn processing_recipe_runs_selected_dependency_aware_stages() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Novel");
        drop(WriterApp::create_project(&project, "Novel").unwrap());
        let recipe_path = root.path().join("pipeline.toml");
        fs::write(
            &recipe_path,
            "version = 1\nstages = [\"word-count\"]\nexpand_transclusions = false\n[[exports]]\nformat = \"markdown\"\noutput = \"exports/reader.md\"\ndesign = \"manuscript\"\n",
        )
        .unwrap();
        run_processing_recipe(&project, &recipe_path).unwrap();
        assert!(project.join(".writers-ide/pipeline-cache.json").is_file());
        assert!(project.join("exports/reader.md").is_file());
        assert!(project.join("exports/reader.md.source-map.json").is_file());
    }

    #[test]
    fn processing_watch_fingerprint_tracks_recipe_changes() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Novel");
        drop(WriterApp::create_project(&project, "Novel").unwrap());
        let recipe = root.path().join("pipeline.toml");
        fs::write(
            &recipe,
            "version=1\nstages=[\"compile\"]\ndebounce_ms=100\n",
        )
        .unwrap();
        let before = super::processing_watch_fingerprint(&project, &recipe).unwrap();
        fs::write(
            &recipe,
            "version=1\nstages=[\"compile\"]\ndebounce_ms=500\n",
        )
        .unwrap();
        let after = super::processing_watch_fingerprint(&project, &recipe).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn query_filter_parser_supports_or_groups_and_exclusions() {
        let one = uuid::Uuid::new_v4();
        let two = uuid::Uuid::new_v4();
        assert_eq!(
            parse_scene_query_filter(&format!("character-any={one},{two}")).unwrap(),
            story_core::SceneQueryFilter::AnyCharacter(vec![
                story_core::CharacterId(one),
                story_core::CharacterId(two),
            ])
        );
        assert_eq!(
            parse_scene_query_filter("not-status=complete").unwrap(),
            story_core::SceneQueryFilter::ExcludeStatus(story_core::SceneStatus::Complete)
        );
    }

    #[cfg(unix)]
    #[test]
    fn edition_output_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Novel");
        drop(WriterApp::create_project(&project_path, "Novel").unwrap());
        let outside = root.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(project_path.join("exports")).unwrap();
        symlink(&outside, project_path.join("exports/link")).unwrap();
        let recipe = root.path().join("edition.toml");
        fs::write(
            &recipe,
            "version=1\nname='Escape'\nformat='markdown'\noutput='exports/link/out.md'\n",
        )
        .unwrap();
        assert!(compile_edition(&project_path, &recipe).is_err());
        assert!(!outside.join("out.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn spelling_candidates_use_local_dictionary_without_modifying_prose() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Novel");
        let mut project = WriterApp::create_project(&project_path, "Novel").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        let scene = project.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "The manuscript stays unchanged.")
            .unwrap();
        drop(project);
        fs::create_dir_all(project_path.join("dictionaries")).unwrap();
        fs::write(project_path.join("dictionaries/en.txt"), "known\n").unwrap();
        let fake_aspell = root.path().join("aspell-mock");
        fs::write(
            &fake_aspell,
            "#!/bin/sh\ncat >/dev/null\nprintf 'Baad\\nKnown\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&fake_aspell, fs::Permissions::from_mode(0o755)).unwrap();
        let before = WriterApp::open_project(&project_path)
            .unwrap()
            .load_scene_text(scene)
            .unwrap()
            .0;
        let findings = spellcheck_project(
            &project_path,
            "en",
            Some("dictionaries/en.txt"),
            fake_aspell.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(findings[0].candidates.get("baad"), Some(&1));
        assert!(!findings[0].candidates.contains_key("known"));
        let after = WriterApp::open_project(&project_path)
            .unwrap()
            .load_scene_text(scene)
            .unwrap()
            .0;
        assert_eq!(before, after);
    }
}
