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
    ContentUnitId, EntityRef, LinkId, LinkKind, NodeId, QueryId, SavedSceneQuery, SceneId,
    SceneQueryFilter, SnapshotId,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        usage();
        return Ok(());
    };
    match cmd.as_str() {
        "writing-language-default" => {
            if let Some(language) = args.next() {
                story_app::WriterApp::set_default_writing_language(&language)?;
            }
            println!(
                "{}",
                story_app::WriterApp::user_preferences()?.default_writing_language
            );
        }
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
        "create-from-template" => {
            let path = args.next().ok_or("missing project path")?;
            let template_path = args.next().ok_or("missing template TOML path")?;
            let title = args.next().unwrap_or_else(|| "Untitled Book".into());
            let source = fs::read_to_string(&template_path)?;
            let template: story_core::ProjectTemplate = toml::from_str(&source)?;
            drop(WriterApp::create_project_from_definition(
                path, &title, &template,
            )?);
            println!(
                "Created {title} from template {} revision {}",
                template.id, template.template_revision
            );
        }
        "template-export" => {
            let template_id = args.next().ok_or("missing built-in template ID")?;
            let output = args.next().ok_or("missing output TOML path")?;
            let template = story_core::BuiltinTemplate::parse(&template_id)
                .ok_or("unknown built-in template")?
                .definition();
            fs::write(output, toml::to_string_pretty(&template)?)?;
            println!("Exported editable template {template_id}");
        }
        "template-save" => {
            let project = args.next().ok_or("missing project path")?;
            let template_id = args.next().ok_or("missing template ID")?;
            let template_name = args.next().ok_or("missing template name")?;
            let output = args.next().ok_or("missing output TOML path")?;
            let app = WriterApp::open_project(project)?;
            let template = app.project_template_from_current(&template_id, &template_name)?;
            let encoded = toml::to_string_pretty(&template)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            file.write_all(encoded.as_bytes())?;
            file.sync_all()?;
            println!(
                "Saved document structure, word targets, and custom-field definitions as template {} at {}; prose, project identity, graph, research, attachments, custom-field values, and workspace preferences are excluded",
                template.id, output
            );
        }
        "template-check" => {
            let source = fs::read_to_string(args.next().ok_or("missing template TOML path")?)?;
            let template: story_core::ProjectTemplate = toml::from_str(&source)?;
            template.validate()?;
            println!(
                "Template {} revision {} is valid",
                template.id, template.template_revision
            );
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
        "tags-export" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            println!("{}", app.export_emacs_tags()?.display());
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
        "scene-language" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene id")?.parse()?);
            let requested = args.next();
            let mut app = WriterApp::open_project(path)?;
            if let Some(tag) = requested.as_deref() {
                app.set_scene_language(scene, (tag != "inherit").then_some(tag))?;
            }
            let project = app.get_project()?;
            let record = app.get_scene(scene)?;
            println!("{}", record.scene.effective_language_code(&project));
        }
        "content-language" => {
            let path = args.next().ok_or("missing project path")?;
            let content = ContentUnitId(args.next().ok_or("missing content unit id")?.parse()?);
            let requested = args.next();
            let mut app = WriterApp::open_project(path)?;
            if let Some(tag) = requested.as_deref() {
                app.set_content_unit_language(content, (tag != "inherit").then_some(tag))?;
            }
            let project = app.get_project()?;
            println!(
                "{}",
                app.get_content_unit(content)?
                    .effective_language_code(&project)
            );
        }
        "scene-word-count-policy" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene id")?.parse()?);
            let requested = args.next();
            let mut app = WriterApp::open_project(path)?;
            if let Some(value) = requested.as_deref() {
                let mut record = app.get_scene(scene)?;
                record.scene.word_count_policy = if value == "inherit" {
                    None
                } else {
                    Some(
                        story_core::WordCountPolicy::parse(value)
                            .ok_or("policy must be unicode-words, script-graphemes, or inherit")?,
                    )
                };
                app.update_scene_metadata(&record.scene)?;
            }
            let project = app.get_project()?;
            println!(
                "{}",
                app.get_scene(scene)?
                    .scene
                    .effective_word_count_policy(&project)
                    .as_str()
            );
        }
        "passage-language" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene id")?.parse()?);
            let start = args.next().ok_or("missing start byte")?.parse()?;
            let end = args.next().ok_or("missing end byte")?.parse()?;
            let language = args.next().ok_or("missing language tag")?;
            let mut app = WriterApp::open_project(path)?;
            let anchor = app.tag_passage_language(scene, start, end, &language)?;
            println!("{anchor}");
        }
        "passage-languages" => {
            let path = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene id")?.parse()?);
            let app = WriterApp::open_project(path)?;
            for tag in app.list_passage_language_tags(scene)? {
                println!(
                    "{} {}..{} {} {}",
                    tag.anchor.id,
                    tag.anchor.start_byte,
                    tag.anchor.end_byte,
                    tag.language_code,
                    tag.anchor.state
                );
            }
        }
        "word-count-policy" => {
            let path = args.next().ok_or("missing project path")?;
            let requested = args.next();
            let mut app = WriterApp::open_project(path)?;
            if let Some(value) = requested {
                let policy = story_core::WordCountPolicy::parse(&value)
                    .ok_or("policy must be unicode-words or script-graphemes")?;
                app.set_project_word_count_policy(policy)?;
            }
            println!("{}", app.get_project()?.word_count_policy.as_str());
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
            let scenes = scenes_for_scope(&app, &scope)?;
            for scene in scenes {
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
                .ok_or("use: replace PROJECT MODE SCOPE PATTERN => REPLACEMENT")?;
            let mut app = WriterApp::open_project(path)?;
            let scenes = scenes_for_scope(&app, &scope)?;
            if scenes.len() > 1 {
                return Err(
                    "multi-scene replacement requires replace-preview; no files were changed"
                        .into(),
                );
            }
            let mut changed = 0;
            for scene in scenes {
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
        "replace-preview" => {
            let path = args.next().ok_or("missing project path")?;
            let mode = parse_find_mode(&args.next().ok_or("missing mode")?)?;
            let scope = args.next().ok_or("missing scope: all or scene UUID")?;
            validate_scene_scope(&scope)?;
            let rest = args.collect::<Vec<_>>().join(" ");
            let (pattern, replacement) = rest
                .split_once(" => ")
                .ok_or("use: replace-preview PROJECT MODE all|SCENE_UUID PATTERN => REPLACEMENT")?;
            let app = WriterApp::open_project(path)?;
            let mut preview = Vec::new();
            for scene in scenes_for_scope(&app, &scope)? {
                let (original, hash) = app.load_scene_text(scene.scene.id)?;
                let (proposed, replacements) =
                    story_core::replace_matches(&original, pattern, replacement, mode)?;
                if replacements > 0 {
                    preview.push(serde_json::json!({
                        "scene_id": scene.scene.id,
                        "title": scene.scene.title,
                        "content_hash": hash,
                        "replacements": replacements,
                        "original": original,
                        "proposed": proposed,
                    }));
                }
            }
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        "replace-apply" => {
            let path = args.next().ok_or("missing project path")?;
            let preview_path = args.next().ok_or("missing replace-preview JSON file")?;
            let preview: Vec<CliReplaceTarget> = serde_json::from_slice(&fs::read(preview_path)?)?;
            let mut app = WriterApp::open_project(path)?;
            let recovery = recover_cli_replace_groups(&mut app)?;
            for notice in &recovery {
                eprintln!("{notice}");
            }
            if recovery
                .iter()
                .any(|notice| notice.contains("conflicts with scene"))
            {
                return Err("resolve the pending CLI replacement recovery conflict before applying another replacement".into());
            }
            let (scenes, count) = apply_replace_preview(&mut app, &preview)?;
            println!("Applied {count} replacement(s) to {scenes} scene(s)");
        }
        "replace-recover" => {
            let path = args.next().ok_or("missing project path")?;
            let mut app = WriterApp::open_project(path)?;
            let notices = recover_cli_replace_groups(&mut app)?;
            if notices.is_empty() {
                println!("No interrupted CLI replacement groups");
            } else {
                for notice in notices {
                    println!("{notice}");
                }
            }
        }
        "replace-undo" => {
            let path = args.next().ok_or("missing project path")?;
            let mut app = WriterApp::open_project(path)?;
            let recovery = recover_cli_replace_groups(&mut app)?;
            for notice in &recovery {
                eprintln!("{notice}");
            }
            if recovery
                .iter()
                .any(|notice| notice.contains("conflicts with scene"))
            {
                return Err(
                    "resolve the pending CLI replacement recovery conflict before undoing".into(),
                );
            }
            let (scenes, count) = undo_cli_replace(&mut app)?;
            println!("Undid {count} replacement(s) in {scenes} scene(s)");
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
        "entity-mentions" => {
            let path = args.next().ok_or("missing project path")?;
            let kind = match args.next().as_deref().ok_or("missing kind")? {
                "character" => story_core::MentionEntityKind::Character,
                "location" => story_core::MentionEntityKind::Location,
                "object" => story_core::MentionEntityKind::Object,
                _ => return Err("kind must be character, location, or object".into()),
            };
            let id = args.next().ok_or("missing entity UUID")?;
            let app = WriterApp::open_project(path)?;
            for appearance in app.confirmed_entity_mention_appearances(kind, &id)? {
                println!(
                    "{}\t{}\t{}..{}\t{}",
                    appearance.scene_id,
                    appearance.scene_title,
                    appearance.mention.start_byte,
                    appearance.mention.end_byte,
                    appearance.mention.quote
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
        "assembly-add-content" => {
            let path = args.next().ok_or("missing project path")?;
            let assembly =
                story_core::AssemblyId(args.next().ok_or("missing assembly UUID")?.parse()?);
            let content =
                story_core::ContentUnitId(args.next().ok_or("missing content-unit UUID")?.parse()?);
            let position = args.next().ok_or("missing position")?.parse()?;
            let mut app = WriterApp::open_project(path)?;
            println!(
                "{}",
                app.add_assembly_content_unit(assembly, content, position)?
            );
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
        "board-resize" => {
            let path = args.next().ok_or("missing project path")?;
            let item = story_core::PlanningBoardItemId(
                args.next().ok_or("missing board item UUID")?.parse()?,
            );
            let width = args.next().ok_or("missing width")?.parse()?;
            let height = args.next().ok_or("missing height")?.parse()?;
            WriterApp::open_project(path)?.resize_planning_board_item(item, width, height)?;
        }
        "board-export" => {
            let path = args.next().ok_or("missing project path")?;
            let board =
                story_core::PlanningBoardId(args.next().ok_or("missing board UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            let title = app
                .list_planning_boards()?
                .into_iter()
                .find(|candidate| candidate.id == board)
                .ok_or("planning board not found")?
                .title;
            print!(
                "{}",
                format_board_markdown(&title, &app.list_planning_board_items(board)?)
            );
        }
        "board-export-svg" => {
            let path = args.next().ok_or("missing project path")?;
            let board =
                story_core::PlanningBoardId(args.next().ok_or("missing board UUID")?.parse()?);
            let app = WriterApp::open_project(path)?;
            print!("{}", app.planning_board_svg(board)?);
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
        "git-auto-snapshot" => {
            let path = args.next().ok_or("missing project path")?;
            let interval = args.next().ok_or("missing interval minutes or off")?;
            let interval = if interval.eq_ignore_ascii_case("off") {
                None
            } else {
                Some(interval.parse::<u32>()?)
            };
            let app = WriterApp::open_project(path)?;
            app.set_git_auto_snapshot_interval(interval)?;
            match interval {
                Some(minutes) => {
                    println!("Automatic Git snapshots enabled every {minutes} minutes.")
                }
                None => println!("Automatic Git snapshots disabled."),
            }
        }
        "git-auto-snapshot-exit" => {
            let path = args.next().ok_or("missing project path")?;
            let setting = args.next().ok_or("missing on or off setting")?;
            let enabled = match setting.to_ascii_lowercase().as_str() {
                "on" | "true" => true,
                "off" | "false" => false,
                _ => return Err("exit snapshot setting must be on or off".into()),
            };
            let app = WriterApp::open_project(path)?;
            app.set_git_snapshot_on_exit(enabled)?;
            println!(
                "Automatic project snapshot on exit {}.",
                if enabled { "enabled" } else { "disabled" }
            );
        }
        "git-auto-snapshot-check" => {
            let path = PathBuf::from(args.next().ok_or("missing project path")?);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            match run_auto_snapshot_check(&path, now)? {
                Some(commit) => println!("Automatic project snapshot created: {commit}"),
                None => println!("Automatic snapshot not due or project unchanged."),
            }
        }
        "git-history" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for commit in app.git_history(100)? {
                println!("{}\t{}\t{}", commit.id, commit.date, commit.subject);
            }
        }
        "git-remote-add" => {
            let path = args.next().ok_or("missing project path")?;
            let name = args.next().ok_or("missing remote name")?;
            let url = args.next().ok_or("missing remote URL")?;
            let app = WriterApp::open_project(path)?;
            app.add_git_remote(&name, &url)?;
            println!("Added remote {name}.");
        }
        "git-remotes" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            for remote in app.git_remotes()? {
                println!("{}", remote.name);
            }
        }
        "git-back-up" => {
            let path = args.next().ok_or("missing project path")?;
            let remote = args.next().ok_or("missing remote name")?;
            let branch = args.next().ok_or("missing branch name")?;
            let mut app = WriterApp::open_project(path)?;
            let commit = app.back_up_git_branch(&remote, &branch)?;
            println!("Backed up {branch} to {remote} at {commit}.");
        }
        "git-retrieve" => {
            let path = args.next().ok_or("missing project path")?;
            let remote = args.next().ok_or("missing remote name")?;
            let branch = args.next().ok_or("missing branch name")?;
            let mut app = WriterApp::open_project(path)?;
            let commit = app.retrieve_git_branch(&remote, &branch)?;
            println!("Retrieved {remote}/{branch} at {commit} (fast-forward only).");
        }
        "git-restore-preview" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.preview_git_restore(&commit)?)?
            );
        }
        "git-restore-project" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let confirmation = args
                .next()
                .ok_or("pass --confirm after reviewing git-restore-preview")?;
            if confirmation != "--confirm" || args.next().is_some() {
                return Err(
                    "full project restore requires exactly one --confirm flag after the commit"
                        .into(),
                );
            }
            let mut app = WriterApp::open_project(path)?;
            let preview = app.restore_git_snapshot_in_place(&commit)?;
            println!(
                "Restored project from {} ({} committed file changes, {} pending working-tree changes previewed); a safety snapshot was taken first.",
                preview.commit_id,
                preview.changes.len(),
                preview.working_tree_changes.len()
            );
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
        "git-restore-scene" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let scene = story_core::SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.restore_scene_from_git(&commit, scene)?;
            println!("Scene prose restored from {commit}; previous project state snapshotted.");
        }
        "git-restore-scene-archived" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let scene = story_core::SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.restore_archived_scene_from_git(&commit, scene)?;
            println!(
                "Archived scene {scene} reactivated from {commit}; current state snapshotted."
            );
        }
        "git-restore-scene-new" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let scene = story_core::SceneId(
                args.next()
                    .ok_or("missing historical scene UUID")?
                    .parse()?,
            );
            let chapter = story_core::NodeId(
                args.next()
                    .ok_or("missing destination chapter UUID")?
                    .parse()?,
            );
            let position = args
                .next()
                .ok_or("missing destination position")?
                .parse::<u32>()?;
            let mut app = WriterApp::open_project(path)?;
            let restored = app.restore_deleted_scene_as_new(&commit, scene, chapter, position)?;
            println!(
                "Recovered as new scene {restored}; manuscript and core scene metadata restored. Character/plot/event relationships were not copied and should be reviewed."
            );
        }
        "git-restore-content" => {
            let path = args.next().ok_or("missing project path")?;
            let commit = args.next().ok_or("missing commit identifier")?;
            let content_unit =
                story_core::ContentUnitId(args.next().ok_or("missing content-unit UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.restore_content_unit_from_git(&commit, content_unit)?;
            println!(
                "Content-unit prose restored from {commit}; previous project state snapshotted."
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
        "query-explain" => {
            let path = args.next().ok_or("missing project path")?;
            let query = QueryId(args.next().ok_or("missing query id")?.parse()?);
            let scene = SceneId(args.next().ok_or("missing scene id")?.parse()?);
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.explain_scene_query_match(query, scene)?)?
            );
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
        "event-dependency-add" => {
            let path = args.next().ok_or("missing project path")?;
            let predecessor = story_core::EventId(
                args.next()
                    .ok_or("missing predecessor event UUID")?
                    .parse()?,
            );
            let successor =
                story_core::EventId(args.next().ok_or("missing successor event UUID")?.parse()?);
            let kind = args.next().ok_or("missing precedes|requires")?;
            let notes = args.collect::<Vec<_>>().join(" ");
            WriterApp::open_project(path)?.add_event_dependency(
                &story_core::StoryEventDependency {
                    predecessor,
                    successor,
                    kind,
                    notes,
                },
            )?;
        }
        "event-time-set" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::EventId(args.next().ok_or("missing event UUID")?.parse()?);
            let start = parse_cli_story_time(&args.next().ok_or("missing start time")?)?;
            let end = parse_cli_story_time(&args.next().ok_or("missing end time")?)?;
            let source_time = args.collect::<Vec<_>>().join(" ");
            WriterApp::open_project(path)?.update_story_event_time(id, start, end, &source_time)?;
            println!("Updated story-time bounds for event {id}");
        }
        "event-dependency-list" => {
            let path = args.next().ok_or("missing project path")?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &WriterApp::open_project(path)?.list_event_dependencies()?
                )?
            );
        }
        "event-dependency-update" => {
            let path = args.next().ok_or("missing project path")?;
            let predecessor = story_core::EventId(
                args.next()
                    .ok_or("missing predecessor event UUID")?
                    .parse()?,
            );
            let successor =
                story_core::EventId(args.next().ok_or("missing successor event UUID")?.parse()?);
            let kind = args.next().ok_or("missing precedes|requires")?;
            let notes = args.collect::<Vec<_>>().join(" ");
            WriterApp::open_project(path)?.update_event_dependency(
                &story_core::StoryEventDependency {
                    predecessor,
                    successor,
                    kind,
                    notes,
                },
            )?;
        }
        "event-dependency-remove" => {
            let path = args.next().ok_or("missing project path")?;
            let predecessor = story_core::EventId(
                args.next()
                    .ok_or("missing predecessor event UUID")?
                    .parse()?,
            );
            let successor =
                story_core::EventId(args.next().ok_or("missing successor event UUID")?.parse()?);
            let kind = args.next().ok_or("missing precedes|requires")?;
            WriterApp::open_project(path)?.remove_event_dependency(
                predecessor,
                successor,
                &kind,
            )?;
        }
        "event-dependency-check" => {
            let path = args.next().ok_or("missing project path")?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &WriterApp::open_project(path)?.inspect_event_dependencies()?
                )?
            );
        }
        "location-rule-add" => {
            let path = args.next().ok_or("missing project path")?;
            let first_location =
                story_core::LocationId(args.next().ok_or("missing first location UUID")?.parse()?);
            let second_location =
                story_core::LocationId(args.next().ok_or("missing second location UUID")?.parse()?);
            let description = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            let rule = story_core::LocationIncompatibilityRule {
                id: story_core::ContinuityRuleId::new(),
                first_location,
                second_location,
                description,
                enabled: true,
            };
            app.add_location_incompatibility_rule(&rule)?;
            println!("{}", rule.id);
        }
        "location-rule-list" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.list_location_incompatibility_rules()?)?
            );
        }
        "location-rule-update" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::ContinuityRuleId(args.next().ok_or("missing rule UUID")?.parse()?);
            let enabled = match args.next().as_deref().ok_or("missing enabled|disabled")? {
                "enabled" => true,
                "disabled" => false,
                _ => return Err("rule state must be enabled or disabled".into()),
            };
            let description = args.collect::<Vec<_>>().join(" ");
            let mut app = WriterApp::open_project(path)?;
            let mut rule = app
                .list_location_incompatibility_rules()?
                .into_iter()
                .find(|rule| rule.id == id)
                .ok_or("unknown location rule")?;
            rule.enabled = enabled;
            rule.description = description;
            app.update_location_incompatibility_rule(&rule)?;
        }
        "location-rule-remove" => {
            let path = args.next().ok_or("missing project path")?;
            let id = story_core::ContinuityRuleId(args.next().ok_or("missing rule UUID")?.parse()?);
            let mut app = WriterApp::open_project(path)?;
            app.remove_location_incompatibility_rule(id)?;
        }
        "location-rule-check" => {
            let path = args.next().ok_or("missing project path")?;
            let app = WriterApp::open_project(path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&app.inspect_location_incompatibilities()?)?
            );
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
        "typography-preview" => {
            let path = args.next().ok_or("missing project path")?;
            let scope = args.next().unwrap_or_else(|| "all".into());
            if args.next().is_some() {
                return Err("too many arguments".into());
            }
            let app = WriterApp::open_project(path)?;
            let project = app.get_project()?;
            let scene_id = if scope == "all" {
                None
            } else {
                let value = scope.strip_prefix("scene:").unwrap_or(&scope);
                Some(story_core::SceneId(uuid::Uuid::parse_str(value)?))
            };
            for scene in app
                .list_scenes(false)?
                .into_iter()
                .filter(|record| scene_id.is_none_or(|id| id == record.scene.id))
            {
                let language = scene.scene.effective_language_code(&project);
                let (text, _) = app.load_scene_text(scene.scene.id)?;
                for suggestion in story_analysis::typography_suggestions(&text, language) {
                    println!(
                        "{}\t{}\t{}..{}\t'{}' -> '{}'\t{}",
                        scene.scene.id,
                        scene.scene.title,
                        suggestion.byte_range.start,
                        suggestion.byte_range.end,
                        suggestion.original,
                        suggestion.replacement,
                        suggestion.message
                    );
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
            let scenes = app.list_scenes(false)?;
            let chapter_id = scope
                .strip_prefix("chapter:")
                .map(|value| uuid::Uuid::parse_str(value).map(story_core::NodeId))
                .transpose()?;
            let scene_id = if scope == "all" || chapter_id.is_some() {
                None
            } else {
                let value = scope.strip_prefix("scene:").unwrap_or(&scope);
                Some(story_core::SceneId(uuid::Uuid::parse_str(value)?))
            };
            if let Some(chapter) = chapter_id
                && !app
                    .list_structural_nodes()?
                    .iter()
                    .any(|node| node.id == chapter && node.kind == "chapter")
            {
                return Err("chapter scope does not identify a chapter".into());
            }
            for scene in scenes.into_iter().filter(|scene| {
                scope == "all" || scene_id == Some(scene.scene.id) || chapter_id == scene.chapter_id
            }) {
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
            let mut successful = WatchSuccess::default();
            println!("Watching processing recipe; Ctrl-C stops the watcher.");
            loop {
                let recipe_config =
                    story_pipeline::ProcessingRecipe::from_toml(&fs::read_to_string(&recipe)?)?;
                let current = processing_watch_fingerprint(&project, &recipe)?;
                if successful.needs_run(&current) {
                    if recipe_config.debounce_ms > 0 {
                        thread::sleep(Duration::from_millis(recipe_config.debounce_ms));
                    }
                    let settled = processing_watch_fingerprint(&project, &recipe)?;
                    if successful.needs_run(&settled) {
                        match run_processing_recipe(&project, &recipe) {
                            Ok(()) => successful.mark_successful(settled),
                            Err(error) => {
                                eprintln!(
                                    "Processing run incomplete; it will retry while inputs remain unchanged: {error}"
                                );
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        }
        "compile-recovery" => {
            let project = args.next().ok_or("missing project path")?;
            let action = args.next().as_deref().unwrap_or("status").to_owned();
            let app = WriterApp::open_project(project)?;
            let notice = match action.as_str() {
                "status" => app.compile_recovery_notice(),
                "retry" => app.retry_compile_publication_recovery()?,
                _ => return Err("compile-recovery action must be status or retry".into()),
            };
            if let Some(notice) = notice {
                println!("Recovery requires review: {notice}");
            } else {
                println!("No pending Compile output recovery");
            }
        }
        "compile" => {
            let project = args.next().ok_or("missing project path")?;
            let format_name = args
                .next()
                .ok_or("missing format (text, markdown, latex, html, epub, docx, pdf, odt, rtf, azw3, mobi)")?;
            let format = story_document::CompileFormat::parse(&format_name)
                .ok_or("format must be text, markdown, latex, html, epub, docx, pdf, odt, rtf, azw3, or mobi")?;
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
            let artifact = if let Some(profile_path) = remaining
                .iter()
                .find_map(|argument| argument.strip_prefix("--design-file="))
            {
                let root = app.project_root().canonicalize()?;
                let path = root.join(profile_path).canonicalize()?;
                if !path.starts_with(&root) || !path.is_file() {
                    return Err("design profile must be a project-local regular file".into());
                }
                let profile =
                    story_document::BookDesignProfile::from_toml(&std::fs::read_to_string(path)?)?;
                app.compile_artifact_with_profile(format, expand, profile)?
            } else {
                app.compile_artifact_with_design(format, expand, design)?
            };
            let destination = output.map(PathBuf::from).unwrap_or_else(|| {
                app.project_root()
                    .join("exports")
                    .join(format!("compiled.{}", format.extension()))
            });
            let destination = if destination.is_absolute() {
                destination
            } else {
                env::current_dir()?.join(destination)
            };
            let overwrite = remaining.iter().any(|argument| argument == "--overwrite");
            let output_files = app.publish_compile_artifact(&destination, &artifact, overwrite)?;
            println!(
                "Compiled {} bytes to {}",
                artifact.bytes.len(),
                destination.display()
            );
            for path in output_files.iter().skip(1) {
                println!("Generated: {}", path.display());
            }
        }
        "compile-config" => {
            let project = args.next().ok_or("missing project path")?;
            let action = args
                .next()
                .ok_or("missing action (list|create|run|all|rebuild|clean)")?;
            let app = WriterApp::open_project(project)?;
            match action.as_str() {
                "list" => {
                    for name in app.list_compile_configurations()? {
                        println!("{name}");
                    }
                }
                "results" => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&app.list_compile_results()?)?
                    );
                }
                "create" => {
                    let preset = args.next().ok_or("missing preset name")?;
                    let name = args.next().unwrap_or_else(|| preset.clone());
                    let path = app.create_builtin_compile_configuration(&name, &preset)?;
                    println!("Created Compile Configuration {name}: {}", path.display());
                }
                "run" | "rebuild" => {
                    let name = args.next().ok_or("missing configuration name")?;
                    let overwrite = action == "rebuild" || args.any(|value| value == "--overwrite");
                    let result = app.compile_configuration(&name, overwrite)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                "all" => {
                    let overwrite = args.any(|value| value == "--overwrite");
                    let mut failures = Vec::new();
                    for name in app.list_compile_configurations()? {
                        match app.compile_configuration(&name, overwrite) {
                            Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                            Err(error) => failures.push(format!("{name}: {error}")),
                        }
                    }
                    if !failures.is_empty() {
                        return Err(format!(
                            "Compile All incomplete; {} configuration(s) failed: {}",
                            failures.len(),
                            failures.join("; ")
                        )
                        .into());
                    }
                }
                "clean" => {
                    let name = args.next().ok_or("missing configuration name or all")?;
                    if !args.any(|value| value == "--yes") {
                        return Err("Clean requires explicit --yes confirmation".into());
                    }
                    let names = if name == "all" {
                        app.list_compile_configurations()?
                    } else {
                        vec![name]
                    };
                    for name in names {
                        let outputs = app.clean_compile_configuration(&name)?;
                        println!("Cleaned {name}: {} managed artefact(s)", outputs.len());
                    }
                }
                _ => {
                    return Err(
                        "action must be list, results, create, run, all, rebuild, or clean".into(),
                    );
                }
            }
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
                .ok_or("format must be text, markdown, latex, html, epub, docx, pdf, odt, rtf, azw3, or mobi")?;
            let output = PathBuf::from(args.next().ok_or("missing output path")?);
            let poll_ms = args
                .next()
                .map(|v| v.parse::<u64>())
                .transpose()?
                .unwrap_or(1000)
                .clamp(250, 60_000);
            let mut successful = WatchSuccess::default();
            println!("Watching project for changes; Ctrl-C stops the watcher.");
            loop {
                let fingerprint = watch_fingerprint(&project)?;
                if successful.needs_run(&fingerprint) {
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        let artifact =
                            WriterApp::open_project(&project)?.compile_artifact(format, false)?;
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
                        Ok(())
                    })();
                    match result {
                        Ok(()) => successful.mark_successful(fingerprint),
                        Err(error) => eprintln!(
                            "Compile failed; previous output preserved; it will retry while inputs remain unchanged: {error}"
                        ),
                    }
                }
                thread::sleep(Duration::from_millis(poll_ms));
            }
        }
        "compile-edition" => {
            let project = PathBuf::from(args.next().ok_or("missing project path")?);
            let recipe = PathBuf::from(args.next().ok_or("missing edition recipe TOML")?);
            let overwrite = match args.next().as_deref() {
                None => false,
                Some("--overwrite") => true,
                Some(_) => return Err("expected optional --overwrite".into()),
            };
            compile_edition_with_overwrite(&project, &recipe, overwrite)?;
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
            let mut successful = WatchSuccess::default();
            println!("Watching edition recipe; Ctrl-C stops the watcher.");
            loop {
                let current = edition_watch_fingerprint(&project, &recipe)?;
                if successful.needs_run(&current) {
                    match compile_edition_with_overwrite(&project, &recipe, true) {
                        Ok(()) => successful.mark_successful(current),
                        Err(error) => {
                            eprintln!(
                                "Edition compile failed; previous output preserved; it will retry while inputs remain unchanged: {error}"
                            );
                        }
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
        "inspect-scrivener" => {
            let source = args
                .next()
                .ok_or("missing .scriv project directory or .scrivx manifest")?;
            let inventory = story_import::inspect_scrivener_project(&source)?;
            let plan = story_import::ScrivenerImportPlan::proposed(&inventory);
            let preview = plan.preview(&inventory)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "inventory": inventory,
                    "proposed_plan": plan,
                    "preview": preview,
                    "compatibility": "experimental; tested against a small licensed Scrivener for Mac 3.1.4 fixture and a local Scrivener for Windows 3.1.1.0 project; broad product/version/platform coverage is unverified"
                }))?
            );
        }
        "import-scrivener" => {
            let source = args
                .next()
                .ok_or("missing .scriv project directory or .scrivx manifest")?;
            let destination = PathBuf::from(args.next().ok_or("missing new project directory")?);
            let mapping_path = args.next().map(PathBuf::from);
            let requested_title = args.next();
            let mode = match args.next().as_deref() {
                None => story_import::ScrivenerImportMode::Both,
                Some("--editable") => story_import::ScrivenerImportMode::Editable,
                Some(value) => {
                    return Err(format!("unknown Scrivener import option: {value}").into());
                }
            };
            if destination.exists() {
                return Err(
                    "Scrivener import requires a new project directory that does not already exist"
                        .into(),
                );
            }
            let inventory = story_import::inspect_scrivener_project(&source)?;
            let plan = if let Some(path) = mapping_path.as_ref() {
                serde_json::from_slice::<story_import::ScrivenerImportPlan>(&fs::read(path)?)?
            } else {
                story_import::ScrivenerImportPlan::proposed(&inventory)
            };
            let preview = plan.preview(&inventory)?;
            if mapping_path.is_none() || !preview.unresolved_source_ids.is_empty() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "inventory": inventory,
                        "plan": plan,
                        "preview": preview,
                        "action": "review the plan, set each selected binder item's destination_kind, save it as JSON, and rerun import-scrivener",
                        "compatibility": "experimental; tested against a small licensed Scrivener for Mac 3.1.4 fixture and a local Scrivener for Windows 3.1.1.0 project; broad product/version/platform coverage is unverified"
                    }))?
                );
                if mapping_path.is_some() {
                    return Err("Scrivener import plan still contains unmapped binder items".into());
                }
            } else {
                let parent = destination
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."));
                fs::create_dir_all(parent)?;
                let staging_parent = tempfile::tempdir_in(parent)?;
                let staging_project = staging_parent.path().join("book");
                let title = requested_title.as_deref().unwrap_or(&inventory.title);
                let mut app = WriterApp::create_project(&staging_project, title)?;
                let (run, scenes) = app.import_scrivener_inventory(&inventory, &plan, mode)?;
                app.validate_project()?;
                let report = serde_json::json!({
                    "inventory": inventory,
                    "plan": plan,
                    "import_run": app.get_import_run_summary(run)?,
                    "imported_scene_ids": scenes,
                    "reference_mode": format!("{mode:?}").to_ascii_lowercase()
                });
                atomic_write(
                    &staging_project.join("exports/scrivener-import-report.json"),
                    &serde_json::to_vec_pretty(&report)?,
                )?;
                drop(app);
                fs::rename(&staging_project, &destination)?;
                println!("Imported Scrivener project into {}", destination.display());
            }
        }
        "series-create" => {
            let root = args.next().ok_or("missing series directory")?;
            let title = args.next().ok_or("missing series title")?;
            let series = story_series::SeriesStore::create(&root, &title)?;
            println!(
                "Created series '{}' ({}) at {}",
                series.title(),
                series.id(),
                series.root().display()
            );
        }
        "series-list" => {
            let root = args.next().ok_or("missing series directory")?;
            let series = story_series::SeriesStore::open(&root)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "series_id": series.id(),
                    "title": series.title(),
                    "books": series.list_books()?
                }))?
            );
        }
        "series-references" => {
            let root = args.next().ok_or("missing series directory")?;
            let series = story_series::SeriesStore::open(&root)?;
            let mut references = Vec::new();
            for book in series.list_books()? {
                let book_root = series.root().join(&book.relative_path);
                if !book_root.is_dir() {
                    continue;
                }
                let app = WriterApp::open_project(&book_root)?;
                for document in app.list_reading_documents()? {
                    references.push(serde_json::json!({
                        "book_id": book.id,
                        "book_title": book.title,
                        "document_id": document.id,
                        "title": document.title,
                        "source_identifier": document.source_identifier,
                        "source_sha256": document.source_sha256,
                        "extracted_text": document.extracted_text
                    }));
                }
            }
            println!("{}", serde_json::to_string_pretty(&references)?);
        }
        "series-add-book" => {
            let series_root = args.next().ok_or("missing series directory")?;
            let book_root = PathBuf::from(args.next().ok_or("missing book project directory")?);
            let title = args.next().ok_or("missing book title")?;
            let source_fingerprint = args.next();
            let app = WriterApp::open_project(&book_root)?;
            let project_id = app.get_project()?.id;
            drop(app);
            let mut series = story_series::SeriesStore::open(series_root)?;
            let book = series.add_book(
                project_id,
                &book_root,
                &title,
                source_fingerprint.as_deref(),
            )?;
            println!(
                "Added '{}' as book {} ({})",
                book.title, book.id, book.relative_path
            );
        }
        "series-new-book" => {
            let series_root = PathBuf::from(args.next().ok_or("missing series directory")?);
            let folder_name = args.next().ok_or("missing new book folder name")?;
            let title = args.next().ok_or("missing new book title")?;
            let relative = Path::new(&folder_name);
            if relative.components().count() != 1
                || !relative
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_)))
            {
                return Err("book folder name must be a single normal path component".into());
            }
            let mut series = story_series::SeriesStore::open(&series_root)?;
            let book_root = series_root.join(relative);
            if book_root.exists() {
                return Err("new book folder already exists".into());
            }
            let app = WriterApp::create_project(&book_root, &title)?;
            let project_id = app.get_project()?.id;
            drop(app);
            match series.add_book(project_id, &book_root, &title, None) {
                Ok(book) => println!(
                    "Created '{}' in series '{}' at {} (book {}; project {})",
                    book.title,
                    series.title(),
                    book.relative_path,
                    book.id,
                    book.project_id
                ),
                Err(error) => {
                    if series.book_for_project(project_id)?.is_none() {
                        let _ = fs::remove_dir_all(&book_root);
                    } else {
                        eprintln!(
                            "Book is registered in the series; run series-repair-memberships if its membership sidecar is missing."
                        );
                    }
                    return Err(error.into());
                }
            }
        }
        "series-repair-memberships" => {
            let root = args.next().ok_or("missing series directory")?;
            let series = story_series::SeriesStore::open(root)?;
            let paths = series.repair_memberships()?;
            println!(
                "Reconciled membership metadata for {} existing books",
                paths.len()
            );
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
        "review-resolve" => {
            let project = args.next().ok_or("missing project path")?;
            let scene = SceneId(args.next().ok_or("missing scene UUID")?.parse()?);
            let base = fs::read_to_string(args.next().ok_or("missing base manuscript file")?)?;
            let reviewed =
                fs::read_to_string(args.next().ok_or("missing reviewed manuscript file")?)?;
            let choices = parse_merge_choices(
                &args
                    .next()
                    .ok_or("missing comma-separated conflict choices")?,
            )?;
            let mut app = WriterApp::open_project(project)?;
            app.resolve_review_text(scene, &base, &reviewed, &choices)?;
            println!(
                "Editorial conflicts resolved and saved through the manuscript write journal."
            );
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
    } else if let Some(assembly) = scope.strip_prefix("assembly:") {
        let _: uuid::Uuid = assembly.parse()?;
    } else if let Some(selected) = scope.strip_prefix("selected:") {
        let values = selected.split(',').collect::<Vec<_>>();
        if values.is_empty() || values.iter().any(|value| value.is_empty()) {
            return Err("selected scope needs comma-separated scene UUIDs".into());
        }
        for value in values {
            let _: uuid::Uuid = value.parse()?;
        }
    } else {
        let _: uuid::Uuid = scope.parse()?;
    }
    Ok(())
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CliReplaceTarget {
    scene_id: SceneId,
    title: String,
    content_hash: String,
    replacements: usize,
    original: String,
    proposed: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CliReplaceGroup {
    version: u16,
    operation_id: String,
    entries: Vec<CliReplaceGroupEntry>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CliReplaceGroupEntry {
    scene_id: SceneId,
    title: String,
    original: String,
    expected_hash: String,
    proposed: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CliReplaceUndo {
    version: u16,
    operation_id: String,
    entries: Vec<CliReplaceUndoEntry>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CliReplaceUndoEntry {
    scene_id: SceneId,
    title: String,
    replacements: usize,
    before: String,
    after: String,
    after_hash: String,
}

fn cli_replace_recovery_dir(app: &WriterApp) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = app.project_root().canonicalize()?;
    let metadata_dir = root.join(".writers-ide");
    fs::create_dir_all(&metadata_dir)?;
    let metadata = fs::symlink_metadata(&metadata_dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("CLI replacement metadata path must be a real project directory".into());
    }
    let directory = metadata_dir.join("recovery");
    fs::create_dir_all(&directory)?;
    let metadata = fs::symlink_metadata(&directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("CLI replacement recovery path must be a real project directory".into());
    }
    let directory = directory.canonicalize()?;
    if !directory.starts_with(&root) {
        return Err("CLI replacement recovery path escapes the project".into());
    }
    Ok(directory)
}

fn cli_replace_group_path(directory: &Path, operation_id: &str) -> PathBuf {
    directory.join(format!("cli-replace-group-{operation_id}.json"))
}

fn cli_replace_undo_path(directory: &Path) -> PathBuf {
    directory.join("cli-replace-undo.json")
}

fn persist_cli_replace_group(
    directory: &Path,
    operation: &CliReplaceGroup,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = cli_replace_group_path(directory, &operation.operation_id);
    atomic_write(&path, &serde_json::to_vec_pretty(operation)?)?;
    Ok(path)
}

fn remove_cli_recovery_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match fs::remove_file(path) {
        Ok(()) => {
            fs::File::open(path.parent().ok_or("recovery file has no parent")?)?.sync_all()?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn read_cli_replace_undo(
    directory: &Path,
) -> Result<Option<CliReplaceUndo>, Box<dyn std::error::Error>> {
    let path = cli_replace_undo_path(directory);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("CLI replacement undo record cannot be a symlink".into());
    }
    let undo: CliReplaceUndo = serde_json::from_slice(&fs::read(path)?)?;
    if undo.version != 1 || undo.entries.is_empty() {
        return Err("invalid CLI replacement undo record".into());
    }
    Ok(Some(undo))
}

fn recover_cli_replace_groups(
    app: &mut WriterApp,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let directory = cli_replace_recovery_dir(app)?;
    let mut paths = fs::read_dir(&directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("cli-replace-group-") && name.ends_with(".json")
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut notices = Vec::new();
    for path in paths {
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(format!("refusing symlinked recovery file {}", path.display()).into());
        }
        let operation: CliReplaceGroup = serde_json::from_slice(&fs::read(&path)?)?;
        if operation.version != 1 || operation.entries.is_empty() {
            return Err(
                format!("invalid CLI replacement recovery record {}", path.display()).into(),
            );
        }
        let mut states = Vec::new();
        let mut conflict = None;
        for entry in &operation.entries {
            let (current, hash) = app.load_scene_text(entry.scene_id)?;
            if current == entry.original && hash == entry.expected_hash {
                states.push((entry.scene_id, hash, false));
            } else if current == entry.proposed {
                states.push((entry.scene_id, hash, true));
            } else {
                conflict = Some(format!(
                    "CLI replacement recovery {} conflicts with scene {}; all content was preserved and the record remains at {}",
                    operation.operation_id,
                    entry.title,
                    path.display()
                ));
                break;
            }
        }
        if let Some(conflict) = conflict {
            notices.push(conflict);
            continue;
        }
        for (scene_id, hash, was_replaced) in states {
            if was_replaced {
                let entry = operation
                    .entries
                    .iter()
                    .find(|entry| entry.scene_id == scene_id)
                    .ok_or("CLI recovery entry disappeared")?;
                app.save_scene_text(scene_id, Some(&hash), &entry.original)?;
            }
        }
        remove_cli_recovery_file(&path)?;
        if read_cli_replace_undo(&directory)?
            .is_some_and(|undo| undo.operation_id == operation.operation_id)
        {
            remove_cli_recovery_file(&cli_replace_undo_path(&directory))?;
        }
        notices.push(format!(
            "Interrupted CLI replacement {} was restored to its original text",
            operation.operation_id
        ));
    }
    Ok(notices)
}

fn undo_cli_replace(app: &mut WriterApp) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let directory = cli_replace_recovery_dir(app)?;
    let undo =
        read_cli_replace_undo(&directory)?.ok_or("no CLI replacement is available to undo")?;
    for entry in &undo.entries {
        let (current, hash) = app.load_scene_text(entry.scene_id)?;
        if current != entry.after || hash != entry.after_hash {
            return Err(format!(
                "cannot undo CLI replacement because scene {} changed after the operation",
                entry.title
            )
            .into());
        }
    }
    if app.project_root().join(".git").exists() {
        app.git_snapshot(&format!(
            "Before CLI replacement undo {}",
            undo.operation_id
        ))?;
    }
    let operation_id = uuid::Uuid::new_v4().to_string();
    let group = CliReplaceGroup {
        version: 1,
        operation_id,
        entries: undo
            .entries
            .iter()
            .map(|entry| CliReplaceGroupEntry {
                scene_id: entry.scene_id,
                title: entry.title.clone(),
                original: entry.after.clone(),
                expected_hash: entry.after_hash.clone(),
                proposed: entry.before.clone(),
            })
            .collect(),
    };
    let recovery_path = persist_cli_replace_group(&directory, &group)?;
    let mut applied = Vec::new();
    for entry in &undo.entries {
        match app.save_scene_text(entry.scene_id, Some(&entry.after_hash), &entry.before) {
            Ok(hash) => applied.push((entry, hash)),
            Err(error) => {
                let mut rollback_errors = Vec::new();
                for (previous, hash) in applied.iter().rev() {
                    if let Err(rollback) =
                        app.save_scene_text(previous.scene_id, Some(hash), &previous.after)
                    {
                        rollback_errors.push(format!("{}: {rollback}", previous.title));
                    }
                }
                if rollback_errors.is_empty() {
                    remove_cli_recovery_file(&recovery_path)?;
                    return Err(format!(
                        "CLI replacement undo failed and was rolled back: {error}"
                    )
                    .into());
                }
                return Err(format!(
                    "CLI replacement undo failed: {error}; recovery is required for {}",
                    rollback_errors.join(", ")
                )
                .into());
            }
        }
    }
    remove_cli_recovery_file(&recovery_path)?;
    remove_cli_recovery_file(&cli_replace_undo_path(&directory))?;
    let scenes = undo.entries.len();
    let replacements = undo.entries.iter().map(|entry| entry.replacements).sum();
    Ok((scenes, replacements))
}

fn apply_replace_preview(
    app: &mut WriterApp,
    targets: &[CliReplaceTarget],
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut seen = std::collections::HashSet::new();
    for target in targets {
        if target.replacements == 0 || !seen.insert(target.scene_id) {
            return Err("preview contains an empty or duplicate scene target".into());
        }
        let (text, hash) = app.load_scene_text(target.scene_id)?;
        if text != target.original || hash != target.content_hash {
            return Err(format!(
                "replacement preview is stale for {}; no files were changed",
                target.title
            )
            .into());
        }
    }
    let total = targets.iter().map(|target| target.replacements).sum();
    if targets.is_empty() {
        return Ok((0, 0));
    }
    if app.project_root().join(".git").exists() {
        app.git_snapshot("Before CLI scoped replacement")?;
    }
    let directory = cli_replace_recovery_dir(app)?;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let group = CliReplaceGroup {
        version: 1,
        operation_id: operation_id.clone(),
        entries: targets
            .iter()
            .map(|target| CliReplaceGroupEntry {
                scene_id: target.scene_id,
                title: target.title.clone(),
                original: target.original.clone(),
                expected_hash: target.content_hash.clone(),
                proposed: target.proposed.clone(),
            })
            .collect(),
    };
    let recovery_path = persist_cli_replace_group(&directory, &group)?;
    let mut applied: Vec<(&CliReplaceTarget, String)> = Vec::new();
    for target in targets {
        match app.save_scene_text(
            target.scene_id,
            Some(&target.content_hash),
            &target.proposed,
        ) {
            Ok(hash) => applied.push((target, hash)),
            Err(error) => {
                let mut rollback_errors = Vec::new();
                for (previous, hash) in applied.iter().rev() {
                    if let Err(rollback) =
                        app.save_scene_text(previous.scene_id, Some(hash), &previous.original)
                    {
                        rollback_errors.push(format!("{}: {rollback}", previous.title));
                    }
                }
                if rollback_errors.is_empty() {
                    remove_cli_recovery_file(&recovery_path)?;
                    return Err(format!(
                        "replacement failed and earlier files were restored: {error}"
                    )
                    .into());
                }
                return Err(format!(
                    "replacement failed: {error}; rollback needs attention for {}",
                    rollback_errors.join(", ")
                )
                .into());
            }
        }
    }
    let undo = CliReplaceUndo {
        version: 1,
        operation_id,
        entries: targets
            .iter()
            .map(|target| {
                let after_hash = applied
                    .iter()
                    .find(|(applied_target, _)| applied_target.scene_id == target.scene_id)
                    .map(|(_, hash)| hash.clone())
                    .ok_or("applied CLI replacement is missing its saved hash")?;
                Ok(CliReplaceUndoEntry {
                    scene_id: target.scene_id,
                    title: target.title.clone(),
                    replacements: target.replacements,
                    before: target.original.clone(),
                    after: target.proposed.clone(),
                    after_hash,
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
    };
    if let Err(error) = atomic_write(
        &cli_replace_undo_path(&directory),
        &serde_json::to_vec_pretty(&undo)?,
    ) {
        // Keep the durable group intent. The next replace-recover invocation
        // can safely restore every changed scene from its original snapshot.
        return Err(format!(
            "replacement completed, but its undo record could not be saved; run replace-recover: {error}"
        )
        .into());
    }
    remove_cli_recovery_file(&recovery_path)?;
    Ok((targets.len(), total))
}

fn scenes_for_scope(
    app: &WriterApp,
    scope: &str,
) -> Result<Vec<story_storage::SceneRecord>, Box<dyn std::error::Error>> {
    let scenes = app.list_scenes(false)?;
    if scope == "all" {
        return Ok(scenes);
    }
    if let Some(chapter) = scope.strip_prefix("chapter:") {
        let chapter = story_core::NodeId(chapter.parse()?);
        return Ok(scenes
            .into_iter()
            .filter(|scene| scene.chapter_id == Some(chapter))
            .collect());
    }
    if let Some(assembly) = scope.strip_prefix("assembly:") {
        let assembly = story_core::AssemblyId(assembly.parse()?);
        if !app
            .list_assemblies()?
            .iter()
            .any(|candidate| candidate.id == assembly)
        {
            return Err("manuscript assembly does not exist".into());
        }
        let mut by_id = scenes
            .into_iter()
            .map(|record| (record.scene.id, record))
            .collect::<std::collections::HashMap<_, _>>();
        let mut ordered = Vec::new();
        for item in app.list_assembly_items(assembly)? {
            if item.included
                && let Some(scene_id) = item.scene_id
                && let Some(record) = by_id.remove(&scene_id)
            {
                ordered.push(record);
            }
        }
        return Ok(ordered);
    }
    if let Some(selected) = scope.strip_prefix("selected:") {
        let ids = selected
            .split(',')
            .map(|value| value.parse().map(SceneId))
            .collect::<Result<std::collections::BTreeSet<_>, uuid::Error>>()?;
        let selected_records = scenes
            .into_iter()
            .filter(|record| ids.contains(&record.scene.id))
            .collect::<Vec<_>>();
        if selected_records.len() != ids.len() {
            return Err("selected scope contains a scene that does not exist".into());
        }
        return Ok(selected_records);
    }
    let scene = SceneId(scope.parse()?);
    Ok(scenes
        .into_iter()
        .filter(|record| record.scene.id == scene)
        .collect())
}
fn parse_cli_story_time(value: &str) -> Result<story_core::StoryTime, Box<dyn std::error::Error>> {
    use story_core::{StoryTime, TimePrecision};
    if value == "?" {
        return Ok(StoryTime::unknown());
    }
    if let Some(value) = value.strip_prefix('~') {
        let parts = value.split(',').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err("approximate time syntax must be ~NOMINAL,LOWER,UPPER".into());
        }
        return Ok(StoryTime::new(
            Some(parts[0].parse()?),
            Some(parts[1].parse()?),
            Some(parts[2].parse()?),
            TimePrecision::Approximate,
        )?);
    }
    if let Some((lower, upper)) = value.split_once("..") {
        return Ok(StoryTime::new(
            None,
            Some(lower.parse()?),
            Some(upper.parse()?),
            TimePrecision::Bounded,
        )?);
    }
    Ok(StoryTime::exact(value.parse()?))
}

fn parse_merge_choices(
    value: &str,
) -> Result<Vec<story_import::MergeConflictChoice>, Box<dyn std::error::Error>> {
    value
        .split(',')
        .map(|choice| match choice {
            "base" => Ok(story_import::MergeConflictChoice::Base),
            "local" => Ok(story_import::MergeConflictChoice::Local),
            "external" => Ok(story_import::MergeConflictChoice::External),
            _ => Err(format!("invalid merge choice {choice}; use base, local, or external").into()),
        })
        .collect()
}

fn usage() {
    eprintln!("writers-ide git-restore-preview <project> <commit>");
    eprintln!("writers-ide git-restore-project <project> <commit> --confirm");
    eprintln!(
        "writers-ide git-remote-add <project> <name> <url>\nwriters-ide git-remotes <project>\nwriters-ide git-back-up <project> <remote> <branch>\nwriters-ide git-retrieve <project> <remote> <branch>"
    );
    eprintln!("writers-ide writing-language-default [BCP-47-TAG]");
    eprintln!(
        "writers-ide compile <project> <text|markdown|latex|html|epub|docx|pdf|odt|rtf|azw3|mobi (legacy)> [output] [--overwrite] [--expand-transclusions] [--design=preset|--design-file=project-relative.toml]"
    );
    eprintln!("writers-ide compile-check <project>");
    eprintln!("writers-ide compile-recovery <project> [status|retry]");
    eprintln!(
        "writers-ide compile-config <project> <list|results|create <preset> [name>|run <name> [--overwrite]|all [--overwrite]|rebuild <name>|clean <name|all> --yes>"
    );
    eprintln!("writers-ide word-count-policy <project> [unicode-words|script-graphemes]");
    eprintln!("writers-ide scene-language <project> <scene-uuid> [language-tag|inherit]");
    eprintln!(
        "writers-ide scene-word-count-policy <project> <scene-uuid> [unicode-words|script-graphemes|inherit]"
    );
    eprintln!(
        "writers-ide passage-language <project> <scene-uuid> <start-byte> <end-byte> <language-tag>"
    );
    eprintln!("writers-ide passage-languages <project> <scene-uuid>");
    eprintln!("writers-ide git-auto-snapshot <project> <interval-minutes|off> (1–10080 minutes)");
    eprintln!("writers-ide git-auto-snapshot-exit <project> <on|off>");
    eprintln!(
        "writers-ide git-auto-snapshot-check <project> (one-shot; suitable for cron/systemd timers)"
    );
    eprintln!(
        "writers-ide git-restore-scene-new <project> <commit> <historical-scene-uuid> <destination-chapter-uuid> <position>"
    );
    eprintln!("writers-ide git-restore-scene-archived <project> <commit> <scene-uuid>");
    eprintln!("writers-ide inspect-manuscript <DOCX|RTF|ODT|EPUB-source>");
    eprintln!(
        "writers-ide inspect-scrivener <.scriv-project-directory|.scrivx-manifest> (experimental inventory only)"
    );
    eprintln!(
        "writers-ide import-scrivener <source> <new-project-directory> [mapping.json] [title] [--editable]"
    );
    eprintln!("writers-ide series-create <series-directory> <title>");
    eprintln!("writers-ide series-list <series-directory>");
    eprintln!(
        "writers-ide series-references <series-directory> (read-only references from member books)"
    );
    eprintln!(
        "writers-ide series-add-book <series-directory> <book-project-directory> <title> [source-sha256]"
    );
    eprintln!("writers-ide series-new-book <series-directory> <folder-name> <title>");
    eprintln!("writers-ide series-repair-memberships <series-directory>");
    eprintln!(
        "writers-ide event-time-set <project> <event-uuid> <start-tick|?|~nominal,lower,upper|lower..upper> <end-tick|?|~nominal,lower,upper|lower..upper> [source-time-text]"
    );
    eprintln!(
        "writers-ide review-resolve <project> <scene-uuid> <base-file> <reviewed-file> <base|local|external,...>"
    );
    eprintln!("writers-ide import-manuscript <source-file> <new-project-directory> [title]");
    eprintln!("writers-ide browse <project> [loopback-port]");
    eprintln!(
        "writers-ide compile-edition <project> <recipe.toml> [--overwrite]\nwriters-ide watch-edition <project> <recipe.toml> [poll-ms]\nwriters-ide spellcheck <project> <language-tag> [project-relative-wordlist]"
    );
    eprintln!(
        "writers-ide query-assert-save <project> <query-uuid> <non-empty|empty|at-least:N|at-most:N|exactly:N>...\nwriters-ide query-assert-check <project> <query-uuid>"
    );
    eprintln!(
        "writers-ide proofread-config <project> (creates editable .writers-ide/diagnostics.toml)"
    );
    eprintln!(
        "writers-ide typography-preview <project> [all|scene:SCENE-UUID] (read-only suggestions)"
    );
    eprintln!(
        "writers-ide find <path> <normal|extended|regex> <all|scene-uuid|chapter:uuid|assembly:uuid|selected:uuid,uuid> <pattern>\nwriters-ide replace-preview <path> <normal|extended|regex> <scope> <pattern> => <replacement>\nwriters-ide replace-apply <path> <preview.json>\nwriters-ide replace-undo <path>\nwriters-ide replace-recover <path>\nwriters-ide tags-export <path>\nwriters-ide replace <path> <normal|extended|regex> <scope> <pattern> => <replacement>\nwriters-ide language <path> [language-tag]"
    );
    eprintln!(
        "writers-ide create <path> [title]\nwriters-ide create-kind <path> <fiction|nonfiction|custom> [title]\nwriters-ide create-template <path> <builtin-template-id> [title]\nwriters-ide template-export <builtin-template-id> <output.toml>\nwriters-ide template-save <project> <template-id> <template-name> <output.toml>\nwriters-ide template-check <template.toml>\nwriters-ide create-from-template <path> <template.toml> [title]\nwriters-ide content <path> <chapter|section|front|back> <parent-uuid|-> <title>\nwriters-ide read-content <path> <content-uuid>\nwriters-ide write-content <path> <content-uuid> <text>\nwriters-ide inspect <path>\nwriters-ide chapter <path> <title>\nwriters-ide scene <path> <chapter-uuid> <title>\nwriters-ide show <path> <scene-uuid>\nwriters-ide write <path> <scene-uuid> <text>\nwriters-ide search <path> <terms>\nwriters-ide analytics <path>\nwriters-ide process <path> [--expand-transclusions] [--only stage,stage]\nwriters-ide compile <path> <markdown|html|epub|docx|pdf> [output-path] [--expand-transclusions]\nwriters-ide extract <path> <scene|reading> <source-uuid> <model> [endpoint]\nwriters-ide study <path> <scene|reading> <source-uuid> <model> [endpoint] [--questions]\nwriters-ide proposals <project>\nwriters-ide approve-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide dismiss-extraction <project> <proposal-json-file> <claim-index>\nwriters-ide knowledge <project>\nwriters-ide query-save <path> <name> [status=...|character=UUID|present=UUID|mentioned=UUID|absent=UUID|location=UUID|event=UUID|chapter=UUID|text=TERM|plotline=UUID|assembly=UUID|issue-status=STATUS|meta=FIELD:VALUE|meta-contains=FIELD:KIND:ENTITY|meta-exists=FIELD|meta-text=FIELD:TEXT|meta-number=FIELD:OP:int|decimal:VALUE|before-definitely=TICK|before-possibly=TICK|after-definitely=TICK|after-possibly=TICK|before=TICK|after=TICK]...\nwriters-ide character-absence <path> <scene-uuid> <character-uuid> [notes]\nwriters-ide query-list <path>\nwriters-ide query-run <path> <query-uuid>\nwriters-ide query-explain <path> <query-uuid> <scene-uuid>\nwriters-ide link <path> <source-kind> <source-id> <target-kind> <target-id> <related|cites|transcludes|includes|navigates> [label]\nwriters-ide outgoing <path> <entity-kind> <entity-id>\nwriters-ide backlinks <path> <entity-kind> <entity-id>\nwriters-ide broken-links <path>\nwriters-ide snapshot <path> <scene-uuid> <label>\nwriters-ide history <path> <scene-uuid>\nwriters-ide diff <path> <scene-uuid> <snapshot-uuid>\nwriters-ide restore <path> <scene-uuid> <snapshot-uuid>\nwriters-ide resolve <path> <transaction-uuid> external|prepared\nwriters-ide backup <path> <destination>\nwriters-ide character-profile <path> <character-uuid>\nwriters-ide character-set <path> <character-uuid> <field> <value>\nwriters-ide character-alias <path> <character-uuid> <alias>\nwriters-ide character-un-alias <path> <character-uuid> <alias>"
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
        "writers-ide create-template <path> <blank-fiction|novel|short-story|blank-nonfiction|nonfiction-book> [title]\nwriters-ide content <path> <chapter|section|front|back> <parent-uuid|-> <title>\nwriters-ide read-content <path> <content-uuid>\nwriters-ide write-content <path> <content-uuid> <text>\nwriters-ide resolve-content <path> <transaction-id> external|prepared\nwriters-ide git-snapshot <path> <label>\nwriters-ide git-history <path>\nwriters-ide git-diff <path> <commit-a> <commit-b> [project-relative-path]\nwriters-ide git-restore-new <source-project> <commit> <new-project-directory>\nwriters-ide git-restore-scene <project> <commit> <scene-uuid>\nwriters-ide git-restore-content <project> <commit> <content-unit-uuid>"
    );
    eprintln!(
        "writers-ide scratchpad-create <path> <title> [body]\nwriters-ide scratchpad-list <path>\nwriters-ide scratchpad-promote <path> <note-uuid> <scene|character|location|object|research> <entity-uuid>\nwriters-ide mentions <path> <scene-uuid>\nwriters-ide entity-mentions <path> <character|location|object> <entity-uuid>\nwriters-ide mention-confirm <path> <mention-uuid> <character|location|object> <entity-uuid>\nwriters-ide mention-dismiss <path> <mention-uuid>\nwriters-ide object-create <path> <name> [description]\nwriters-ide assembly-draft-create <path> <assembly-uuid>\nwriters-ide assembly-draft-list <path>\nwriters-ide assembly-draft-set <path> <draft-uuid> <scene-uuid> <text.md>\nwriters-ide assembly-draft-commit <path> <draft-uuid>\nwriters-ide board-create <path> <title>\nwriters-ide board-add-scene <path> <board-uuid> <scene-uuid> <x> <y> [note]\nwriters-ide board-list <path> <board-uuid>\nwriters-ide board-resize <path> <item-uuid> <width> <height>\nwriters-ide board-export <path> <board-uuid> (Markdown to stdout)\nwriters-ide board-export-svg <path> <board-uuid> (SVG to stdout)\nwriters-ide backup-archive <project-path> <archive.zip>\nwriters-ide restore-archive <archive.zip> <new-project-directory>"
    );
    eprintln!(
        "writers-ide compile-watch <path> <format> <output> [poll-ms]\nwriters-ide process-recipe <project> <recipe.toml>\nwriters-ide watch-process <project> <recipe.toml> [poll-ms]\nwriters-ide query-assert <path> <query-uuid> <empty|non-empty|at-least:N|at-most:N|exactly:N>\nwriters-ide coverage <path> <dimension> <entity-uuid> <minimum-scenes>\nwriters-ide event-dependency-add <project> <before-event> <after-event> <precedes|requires> [notes]\nwriters-ide event-dependency-list|event-dependency-check <project>\nwriters-ide event-dependency-update <project> <before-event> <after-event> <kind> [notes]\nwriters-ide event-dependency-remove <project> <before-event> <after-event> <kind>\nwriters-ide location-rule-add <project> <location-a> <location-b> <description>\nwriters-ide location-rule-list <project>\nwriters-ide location-rule-update <project> <rule-uuid> <enabled|disabled> <description>\nwriters-ide location-rule-remove <project> <rule-uuid>\nwriters-ide location-rule-check <project>\nwriters-ide challenge-start <path> <target-words> <duration-seconds> [steady|kamikaze]\nwriters-ide challenge-finish <path> <challenge-id> <words-written> [abandoned]\nwriters-ide challenge-list <path>\nwriters-ide goal-create <project> <start> <end> <target> <baseline> <UTC|±HH:MM> <raw|real> <project|scene-uuid> <title>\nwriters-ide goal-list <project> [--all]\nwriters-ide goal-record <project> <goal> <unix-time> <origin> <source-id> <fingerprint> <scene-uuid|none> <raw> <real> [detail]\nwriters-ide goal-events <project> <goal>\nwriters-ide goal-progress <project> <goal>\nwriters-ide goal-weekly <project> <goal>\nwriters-ide goal-archive <project> <goal>\nwriters-ide goal-reconcile <project> <event> <fingerprint> <raw> <real> <reason>"
    );
}

fn format_board_markdown(title: &str, items: &[story_storage::PlanningBoardItemRecord]) -> String {
    let escape = |value: &str| value.replace('|', "\\|").replace('\n', " ");
    let mut output = format!(
        "# {}\n\n| Entity | ID | X | Y | Width | Height | Note |\n|---|---|---:|---:|---:|---:|---|\n",
        escape(title)
    );
    for item in items {
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            escape(&item.entity_kind),
            item.entity_id,
            item.x,
            item.y,
            item.width,
            item.height,
            escape(&item.note)
        ));
    }
    output
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

fn run_auto_snapshot_check(
    project: &Path,
    now_unix: i64,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut app = WriterApp::open_project(project)?;
    Ok(app.git_auto_snapshot_if_due(now_unix)?)
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

fn edition_watch_fingerprint(
    project: &Path,
    recipe_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let recipe_bytes = fs::read(recipe_path)?;
    let recipe = story_document::EditionRecipe::from_toml(std::str::from_utf8(&recipe_bytes)?)?;
    let mut fingerprint = Sha256::new();
    fingerprint.update(watch_fingerprint(project)?);
    fingerprint.update(&recipe_bytes);
    if let Some(dictionary) = recipe.dictionary {
        let dictionary = Path::new(&dictionary);
        ensure_project_path_has_no_symlink(project, dictionary)?;
        let dictionary_path = project.join(dictionary);
        if !fs::symlink_metadata(&dictionary_path)?.is_file() {
            return Err("personal dictionary must be a regular project-local file".into());
        }
        fingerprint.update(fs::read(dictionary_path)?);
    }
    Ok(format!("{:x}", fingerprint.finalize()))
}

#[derive(Default)]
struct WatchSuccess {
    fingerprint: Option<String>,
}

impl WatchSuccess {
    fn needs_run(&self, fingerprint: &str) -> bool {
        self.fingerprint.as_deref() != Some(fingerprint)
    }

    fn mark_successful(&mut self, fingerprint: String) {
        self.fingerprint = Some(fingerprint);
    }
}

#[cfg(test)]
fn compile_edition(project: &Path, recipe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    compile_edition_with_overwrite(project, recipe_path, false)
}

fn compile_edition_with_overwrite(
    project: &Path,
    recipe_path: &Path,
    overwrite: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_edition_with_aspell_and_overwrite(project, recipe_path, "aspell", overwrite)
}

#[cfg(test)]
fn compile_edition_with_aspell(
    project: &Path,
    recipe_path: &Path,
    aspell: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_edition_with_aspell_and_overwrite(project, recipe_path, aspell, false)
}

fn compile_edition_with_aspell_and_overwrite(
    project: &Path,
    recipe_path: &Path,
    aspell: &str,
    overwrite: bool,
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
    let artifact = app.compile_artifact_for_edition(&recipe)?;
    let output = project.join(output_relative);
    let mut sidecars = Vec::new();
    if let Some(findings) = spelling {
        let report_name = format!(
            "{}.spellcheck.json",
            output.file_name().unwrap().to_string_lossy()
        );
        let report = output.with_file_name(&report_name);
        sidecars.push((report_name, serde_json::to_vec_pretty(&findings)?));
        if !findings.is_empty() {
            eprintln!(
                "Edition spelling check found candidate words in {} scene(s): {}",
                findings.len(),
                report.display()
            );
        }
    }
    app.publish_compile_artifact_with_sidecars(&output, &artifact, overwrite, &sidecars)?;
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
        "meta-exists" => SceneQueryFilter::CustomMetadataExists {
            field_id: story_core::FieldId(value.parse()?),
        },
        "meta-text" => {
            let (field_id, text) = value
                .split_once(':')
                .ok_or("meta-text uses FIELD-UUID:TEXT")?;
            SceneQueryFilter::CustomMetadataTextContains {
                field_id: story_core::FieldId(field_id.parse()?),
                text: text.to_owned(),
            }
        }
        "meta-number" => {
            let mut parts = value.splitn(4, ':');
            let field_id = story_core::FieldId(
                parts
                    .next()
                    .ok_or("meta-number uses FIELD-UUID:OP:int|decimal:VALUE")?
                    .parse()?,
            );
            let comparison = match parts.next().ok_or("missing numeric comparison")? {
                "lt" => story_core::MetadataComparison::LessThan,
                "lte" => story_core::MetadataComparison::LessThanOrEqual,
                "eq" => story_core::MetadataComparison::Equal,
                "ne" => story_core::MetadataComparison::NotEqual,
                "gte" => story_core::MetadataComparison::GreaterThanOrEqual,
                "gt" => story_core::MetadataComparison::GreaterThan,
                _ => return Err("numeric comparison must be lt, lte, eq, ne, gte, or gt".into()),
            };
            let raw = parts.next().ok_or("missing numeric metadata type")?;
            let value = parts.next().ok_or("missing numeric metadata value")?;
            let value = match raw {
                "int" => story_core::FieldValue::Integer(value.parse()?),
                "decimal" => story_core::FieldValue::Decimal(value.to_owned()),
                _ => return Err("numeric metadata type must be int or decimal".into()),
            };
            SceneQueryFilter::CustomMetadataNumberCompare {
                field_id,
                comparison,
                value,
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
        CliReplaceGroup, CliReplaceGroupEntry, CliReplaceTarget, WatchSuccess,
        apply_replace_preview, compile_edition, compile_edition_with_aspell,
        compile_edition_with_aspell_and_overwrite, compile_edition_with_overwrite,
        edition_watch_fingerprint, parse_scene_query_filter, recover_cli_replace_groups,
        run_auto_snapshot_check, run_processing_recipe, scenes_for_scope, spellcheck_project,
        undo_cli_replace, watch_fingerprint,
    };
    use std::fs;
    use story_app::WriterApp;
    #[test]
    fn watch_fingerprint_is_acknowledged_only_after_success() {
        let mut watcher = WatchSuccess::default();
        assert!(watcher.needs_run("inputs-a"));
        // A failed invocation makes no state transition, so the same input retries.
        assert!(watcher.needs_run("inputs-a"));
        watcher.mark_successful("inputs-a".into());
        assert!(!watcher.needs_run("inputs-a"));
        assert!(watcher.needs_run("inputs-b"));
    }
    #[test]
    fn one_shot_git_snapshot_check_supports_external_schedulers() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Novel");
        let mut project = WriterApp::create_project(&project_path, "Novel").unwrap();
        project.set_git_auto_snapshot_interval(Some(1)).unwrap();
        let chapter = project.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = project.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "First version.")
            .unwrap();
        drop(project);

        assert!(
            run_auto_snapshot_check(&project_path, 1_000)
                .unwrap()
                .is_some()
        );
        assert!(
            run_auto_snapshot_check(&project_path, 1_059)
                .unwrap()
                .is_none()
        );
        assert!(
            run_auto_snapshot_check(&project_path, 1_060)
                .unwrap()
                .is_none()
        );

        let mut project = WriterApp::open_project(&project_path).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "Revised version.")
            .unwrap();
        drop(project);
        assert!(
            run_auto_snapshot_check(&project_path, 1_120)
                .unwrap()
                .is_some()
        );
    }
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
    fn custom_metadata_query_operands_parse_typed_filters() {
        let field = uuid::Uuid::new_v4();
        assert_eq!(
            parse_scene_query_filter(&format!("meta-exists={field}")).unwrap(),
            story_core::SceneQueryFilter::CustomMetadataExists {
                field_id: story_core::FieldId(field),
            }
        );
        assert_eq!(
            parse_scene_query_filter(&format!("meta-text={field}:red key")).unwrap(),
            story_core::SceneQueryFilter::CustomMetadataTextContains {
                field_id: story_core::FieldId(field),
                text: "red key".into(),
            }
        );
        assert_eq!(
            parse_scene_query_filter(&format!("meta-number={field}:gte:int:42")).unwrap(),
            story_core::SceneQueryFilter::CustomMetadataNumberCompare {
                field_id: story_core::FieldId(field),
                comparison: story_core::MetadataComparison::GreaterThanOrEqual,
                value: story_core::FieldValue::Integer(42),
            }
        );
        assert!(parse_scene_query_filter(&format!("meta-number={field}:bad:int:1")).is_err());
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
        let initial_output = fs::read(project_path.join("exports/reader.md")).unwrap();
        assert!(compile_edition(&project_path, &recipe_path).is_err());
        assert_eq!(
            fs::read(project_path.join("exports/reader.md")).unwrap(),
            initial_output
        );
        compile_edition_with_overwrite(&project_path, &recipe_path, true).unwrap();
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
        let dictionary_fingerprint =
            edition_watch_fingerprint(&project_path, &checked_recipe).unwrap();
        compile_edition_with_aspell(&project_path, &checked_recipe, mock.to_str().unwrap())
            .unwrap();
        assert!(
            compile_edition_with_aspell(&project_path, &checked_recipe, mock.to_str().unwrap())
                .is_err()
        );
        compile_edition_with_aspell_and_overwrite(
            &project_path,
            &checked_recipe,
            mock.to_str().unwrap(),
            true,
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_slice(
            &fs::read(project_path.join("exports/checked.md.spellcheck.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report[0]["candidates"]["baad"], 1);
        assert!(report[0]["candidates"].get("known").is_none());
        fs::write(
            project_path.join("dictionaries/en.txt"),
            "known\nadditional\n",
        )
        .unwrap();
        assert_ne!(
            dictionary_fingerprint,
            edition_watch_fingerprint(&project_path, &checked_recipe).unwrap()
        );
        let outside_dictionary = root.path().join("outside.txt");
        fs::write(&outside_dictionary, "outside\n").unwrap();
        std::os::unix::fs::symlink(
            &outside_dictionary,
            project_path.join("dictionaries/linked.txt"),
        )
        .unwrap();
        fs::write(&checked_recipe, "version=1\nname='Checked reader'\nformat='markdown'\noutput='exports/checked.md'\nspellcheck=true\nlanguage='en_GB'\ndictionary='dictionaries/linked.txt'\n").unwrap();
        assert!(edition_watch_fingerprint(&project_path, &checked_recipe).is_err());
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

    #[test]
    fn assembly_search_scope_follows_included_assembly_scene_order() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let omitted = app.create_scene(chapter, "Omitted", 1).unwrap();
        let last = app.create_scene(chapter, "Last", 2).unwrap();
        let assembly = app.create_assembly("Selected edition").unwrap();
        app.add_assembly_scene(assembly, last, 0).unwrap();
        let omitted_item = app.add_assembly_scene(assembly, omitted, 1).unwrap();
        app.set_assembly_item_included(omitted_item, false).unwrap();
        app.add_assembly_scene(assembly, first, 2).unwrap();

        let selected = scenes_for_scope(&app, &format!("assembly:{assembly}")).unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|record| record.scene.id)
                .collect::<Vec<_>>(),
            [last, first]
        );
    }

    #[test]
    fn selected_cli_scope_contains_only_the_named_scene_files() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let selected = scenes_for_scope(&app, &format!("selected:{second},{first}")).unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|record| record.scene.id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        assert!(
            scenes_for_scope(&app, &format!("selected:{first},{}", uuid::Uuid::new_v4())).is_err()
        );
    }

    #[test]
    fn replace_preview_apply_rechecks_all_targets_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let mut targets = Vec::new();
        for scene_id in [first, second] {
            let (_, content_hash) = app.load_scene_text(scene_id).unwrap();
            app.save_scene_text(scene_id, Some(&content_hash), "A cat waits.")
                .unwrap();
            let (original, content_hash) = app.load_scene_text(scene_id).unwrap();
            targets.push(CliReplaceTarget {
                scene_id,
                title: scene_id.to_string(),
                content_hash,
                replacements: 1,
                original,
                proposed: "A fox waits.".into(),
            });
        }
        let (_, stale_hash) = app.load_scene_text(second).unwrap();
        app.save_scene_text(second, Some(&stale_hash), "Externally edited.")
            .unwrap();
        assert!(apply_replace_preview(&mut app, &targets).is_err());
        assert_eq!(app.load_scene_text(first).unwrap().0, "A cat waits.");
        assert_eq!(app.load_scene_text(second).unwrap().0, "Externally edited.");
    }

    #[test]
    fn cli_scoped_replace_is_recoverable_and_has_hash_checked_undo() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let mut targets = Vec::new();
        for scene_id in [first, second] {
            let (_, hash) = app.load_scene_text(scene_id).unwrap();
            app.save_scene_text(scene_id, Some(&hash), "cat cat")
                .unwrap();
            let (original, content_hash) = app.load_scene_text(scene_id).unwrap();
            targets.push(CliReplaceTarget {
                scene_id,
                title: scene_id.to_string(),
                content_hash,
                replacements: 2,
                original,
                proposed: "fox fox".into(),
            });
        }
        assert_eq!(apply_replace_preview(&mut app, &targets).unwrap(), (2, 4));
        assert_eq!(app.load_scene_text(first).unwrap().0, "fox fox");
        assert_eq!(app.load_scene_text(second).unwrap().0, "fox fox");
        assert_eq!(undo_cli_replace(&mut app).unwrap(), (2, 4));
        assert_eq!(app.load_scene_text(first).unwrap().0, "cat cat");
        assert_eq!(app.load_scene_text(second).unwrap().0, "cat cat");
        assert!(undo_cli_replace(&mut app).is_err());
    }

    #[test]
    fn cli_replace_recovery_restores_unambiguous_files_and_preserves_divergence() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let mut entries = Vec::new();
        for scene_id in [first, second] {
            let (_, hash) = app.load_scene_text(scene_id).unwrap();
            app.save_scene_text(scene_id, Some(&hash), "original")
                .unwrap();
            let (original, expected_hash) = app.load_scene_text(scene_id).unwrap();
            entries.push(CliReplaceGroupEntry {
                scene_id,
                title: scene_id.to_string(),
                original,
                expected_hash,
                proposed: "proposed".into(),
            });
        }
        let operation = CliReplaceGroup {
            version: 1,
            operation_id: uuid::Uuid::new_v4().to_string(),
            entries,
        };
        let directory = super::cli_replace_recovery_dir(&app).unwrap();
        super::persist_cli_replace_group(&directory, &operation).unwrap();
        let (_, hash) = app.load_scene_text(first).unwrap();
        app.save_scene_text(first, Some(&hash), "proposed").unwrap();
        let notices = recover_cli_replace_groups(&mut app).unwrap();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].contains("restored"));
        assert_eq!(app.load_scene_text(first).unwrap().0, "original");
        assert_eq!(app.load_scene_text(second).unwrap().0, "original");

        let operation = CliReplaceGroup {
            version: 1,
            operation_id: uuid::Uuid::new_v4().to_string(),
            entries: [first, second]
                .into_iter()
                .map(|scene_id| {
                    let (original, expected_hash) = app.load_scene_text(scene_id).unwrap();
                    CliReplaceGroupEntry {
                        scene_id,
                        title: scene_id.to_string(),
                        original,
                        expected_hash,
                        proposed: "proposed".into(),
                    }
                })
                .collect(),
        };
        super::persist_cli_replace_group(&directory, &operation).unwrap();
        let (_, hash) = app.load_scene_text(first).unwrap();
        app.save_scene_text(first, Some(&hash), "proposed").unwrap();
        let (_, hash) = app.load_scene_text(second).unwrap();
        app.save_scene_text(second, Some(&hash), "external")
            .unwrap();
        let notices = recover_cli_replace_groups(&mut app).unwrap();
        assert!(notices[0].contains("conflicts with scene"));
        assert_eq!(app.load_scene_text(first).unwrap().0, "proposed");
        assert_eq!(app.load_scene_text(second).unwrap().0, "external");
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
