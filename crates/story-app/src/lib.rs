use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    fs,
    path::{Path, PathBuf},
};
use story_core::*;
use story_storage::{
    ApprovedKnowledgeClaim, ConflictResolution, ProjectStore, Result as StorageResult, SceneRecord,
    StorageError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("invalid domain input: {0}")]
    Domain(String),
    #[error(transparent)]
    Pipeline(#[from] story_pipeline::PipelineError),
    #[error(transparent)]
    Git(#[from] story_git::GitError),
    #[error("user preferences error: {0}")]
    Preferences(String),
}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolDefinition {
    pub entity_kind: String,
    pub entity_id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub source_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmacsTagsExport {
    pub symbols_markdown: String,
    pub tags_file: String,
}

fn write_project_file_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Domain("project export path has no parent".into()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| AppError::Domain(format!("create symbol export temporary: {error}")))?;
    use std::io::Write;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| AppError::Domain(format!("write symbol export: {error}")))?;
    temporary
        .persist(path)
        .map_err(|error| AppError::Domain(format!("publish symbol export: {}", error.error)))?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| AppError::Domain(format!("sync symbol export directory: {error}")))?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct UserPreferences {
    /// Interface language is independent of manuscript language.
    pub ui_language: String,
    /// Applied to projects created through the default project creation flow.
    pub default_writing_language: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            ui_language: "en".into(),
            default_writing_language: default_language_code(),
        }
    }
}

fn user_preferences_path() -> Option<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("writers-ide/preferences.toml"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config/writers-ide/preferences.toml"))
}

fn read_user_preferences_at(path: &Path) -> Result<UserPreferences> {
    let preferences = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .map_err(|error| AppError::Preferences(format!("{}: {error}", path.display())))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => UserPreferences::default(),
        Err(error) => {
            return Err(AppError::Preferences(format!(
                "{}: {error}",
                path.display()
            )));
        }
    };
    if !is_valid_language_tag(&preferences.ui_language)
        || !is_valid_language_tag(&preferences.default_writing_language)
    {
        return Err(AppError::Preferences(format!(
            "{} contains an invalid BCP 47 language tag",
            path.display()
        )));
    }
    Ok(preferences)
}

fn write_user_preferences_at(path: &Path, preferences: &UserPreferences) -> Result<()> {
    if !is_valid_language_tag(&preferences.ui_language)
        || !is_valid_language_tag(&preferences.default_writing_language)
    {
        return Err(AppError::Domain(
            "preferences require valid BCP 47 language tags".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Preferences("preferences path has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|error| AppError::Preferences(error.to_string()))?;
    let temporary = path.with_extension("toml.tmp");
    let bytes = toml::to_string_pretty(preferences)
        .map_err(|error| AppError::Preferences(error.to_string()))?;
    use std::io::Write;
    let mut file =
        fs::File::create(&temporary).map_err(|error| AppError::Preferences(error.to_string()))?;
    file.write_all(bytes.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| AppError::Preferences(error.to_string()))?;
    fs::rename(&temporary, path).map_err(|error| AppError::Preferences(error.to_string()))?;
    Ok(())
}

fn escape_svg_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if matches!(ch, '\t' | '\n' | '\r')
                || (ch >= '\u{20}' && ch != '\u{fffe}' && ch != '\u{ffff}')
            {
                ch
            } else {
                '\u{fffd}'
            }
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

/// Application façade: frontends use use cases here and never issue domain SQL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityMentionAppearance {
    pub scene_id: SceneId,
    pub scene_title: String,
    pub mention: story_storage::DetectedMentionRecord,
}

pub struct WriterApp {
    store: ProjectStore,
}
impl WriterApp {
    pub fn user_preferences() -> Result<UserPreferences> {
        match user_preferences_path() {
            Some(path) => read_user_preferences_at(&path),
            None => Ok(UserPreferences::default()),
        }
    }
    pub fn save_user_preferences(preferences: &UserPreferences) -> Result<()> {
        let path = user_preferences_path().ok_or_else(|| {
            AppError::Preferences("HOME and XDG_CONFIG_HOME are both unset".into())
        })?;
        write_user_preferences_at(&path, preferences)
    }
    pub fn set_default_writing_language(language_code: &str) -> Result<()> {
        let mut preferences = Self::user_preferences()?;
        preferences.default_writing_language = language_code.into();
        Self::save_user_preferences(&preferences)
    }
    pub fn project_root(&self) -> &Path {
        self.store.project_root()
    }
    pub fn create_project(path: impl AsRef<Path>, title: &str) -> Result<Self> {
        let preferences = Self::user_preferences()?;
        Self::create_project_with_preferences(path, title, &preferences)
    }
    pub fn create_project_with_preferences(
        path: impl AsRef<Path>,
        title: &str,
        preferences: &UserPreferences,
    ) -> Result<Self> {
        if !is_valid_language_tag(&preferences.default_writing_language) {
            return Err(AppError::Domain(
                "default writing language must be a valid BCP 47 language tag".into(),
            ));
        }
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type: BookType::Fiction,
            template_id: None,
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: preferences.default_writing_language.clone(),
            word_count_policy: WordCountPolicy::default(),
        };
        Self::create_project_with_metadata(path, &project)
    }
    pub fn create_project_with_type(
        path: impl AsRef<Path>,
        title: &str,
        book_type: BookType,
    ) -> Result<Self> {
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type,
            template_id: None,
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: Self::user_preferences()?.default_writing_language,
            word_count_policy: WordCountPolicy::default(),
        };
        Self::create_project_with_metadata(path, &project)
    }
    pub fn create_project_from_template(
        path: impl AsRef<Path>,
        title: &str,
        template: BuiltinTemplate,
    ) -> Result<Self> {
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            book_type: template.book_type(),
            template_id: Some(template.id().into()),
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: Self::user_preferences()?.default_writing_language,
            word_count_policy: WordCountPolicy::default(),
        };
        Self::create_project_with_metadata(path, &project)
    }
    pub fn create_project_with_metadata(path: impl AsRef<Path>, project: &Project) -> Result<Self> {
        Ok(Self {
            store: ProjectStore::create_for(path, project)?,
        })
    }
    pub fn get_project(&self) -> Result<Project> {
        Ok(self.store.get_project()?)
    }
    pub fn set_project_language(&mut self, language_code: &str) -> Result<()> {
        Ok(self.store.set_project_language(language_code)?)
    }
    pub fn set_scene_language(
        &mut self,
        scene_id: SceneId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self.store.set_scene_language(scene_id, language_code)?)
    }
    pub fn tag_passage_language(
        &mut self,
        scene_id: SceneId,
        start_byte: u64,
        end_byte: u64,
        language_code: &str,
    ) -> Result<AnchorId> {
        Ok(self
            .store
            .tag_passage_language(scene_id, start_byte, end_byte, language_code)?)
    }
    pub fn list_passage_language_tags(
        &self,
        scene_id: SceneId,
    ) -> Result<Vec<story_storage::PassageLanguageRecord>> {
        Ok(self.store.list_passage_language_tags(scene_id)?)
    }
    pub fn remove_passage_language_tag(&mut self, anchor_id: AnchorId) -> Result<()> {
        Ok(self.store.remove_passage_language_tag(anchor_id)?)
    }
    pub fn set_project_word_count_policy(&mut self, policy: WordCountPolicy) -> Result<()> {
        Ok(self.store.set_project_word_count_policy(policy)?)
    }
    pub fn open_project(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: ProjectStore::open(path)?,
        })
    }
    pub fn create_section(&mut self, title: &str, position: u32) -> Result<NodeId> {
        Ok(self
            .store
            .create_node(None, NodeKind::Section, title, position)?)
    }
    pub fn create_chapter(
        &mut self,
        parent: Option<NodeId>,
        title: &str,
        position: u32,
    ) -> Result<NodeId> {
        Ok(self
            .store
            .create_node(parent, NodeKind::Chapter, title, position)?)
    }
    pub fn move_structural_node(
        &mut self,
        node: NodeId,
        parent: Option<NodeId>,
        position: u32,
    ) -> Result<()> {
        Ok(self.store.move_node(node, parent, position)?)
    }
    pub fn create_scene(&mut self, chapter: NodeId, title: &str, position: u32) -> Result<SceneId> {
        Ok(self.store.create_scene(chapter, position, title)?)
    }
    pub fn create_content_unit(
        &mut self,
        kind: ContentUnitKind,
        title: &str,
        parent: Option<NodeId>,
        position: u32,
        target_words: Option<u64>,
    ) -> Result<ContentUnitId> {
        Ok(self
            .store
            .create_content_unit(kind, title, parent, position, target_words)?)
    }
    pub fn get_content_unit(&self, id: ContentUnitId) -> Result<story_storage::ContentUnitRecord> {
        Ok(self.store.get_content_unit(id)?)
    }
    pub fn set_content_unit_language(
        &mut self,
        id: ContentUnitId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self.store.set_content_unit_language(id, language_code)?)
    }
    pub fn list_content_units(
        &self,
        include_archived: bool,
    ) -> Result<Vec<story_storage::ContentUnitRecord>> {
        Ok(self.store.list_content_units(include_archived)?)
    }
    pub fn load_content_unit_text(&self, id: ContentUnitId) -> Result<(String, String)> {
        Ok(self.store.load_content_unit_text(id)?)
    }
    pub fn save_content_unit_text(
        &mut self,
        id: ContentUnitId,
        expected: Option<&str>,
        text: &str,
    ) -> Result<String> {
        Ok(self.store.save_content_unit_text(id, expected, text)?)
    }
    pub fn rename_content_unit(&mut self, id: ContentUnitId, title: &str) -> Result<()> {
        Ok(self.store.rename_content_unit(id, title)?)
    }
    pub fn list_structural_nodes(&self) -> Result<Vec<story_storage::StructuralNodeSummary>> {
        Ok(self.store.list_structural_nodes()?)
    }
    pub fn get_scene(&self, id: SceneId) -> Result<SceneRecord> {
        Ok(self.store.get_scene(id)?)
    }
    pub fn list_scenes(&self, include_archived: bool) -> Result<Vec<SceneRecord>> {
        Ok(self.store.list_scenes(include_archived)?)
    }
    pub fn list_project_symbols(&self) -> Result<Vec<ProjectSymbolDefinition>> {
        let mut symbols = Vec::new();
        for scene in self.list_scenes(false)? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "scene".into(),
                entity_id: scene.scene.id.to_string(),
                name: scene.scene.title,
                aliases: Vec::new(),
                source_path: Some(self.store.scene_content_relative_path(scene.scene.id)?),
            });
        }
        for node in self
            .list_structural_nodes()?
            .into_iter()
            .filter(|node| node.kind == "chapter")
        {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "chapter".into(),
                entity_id: node.id.to_string(),
                name: node.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for character in self.list_characters()? {
            let record = self.get_character(character.id)?;
            let aliases = std::iter::once(record.full_name)
                .chain(std::iter::once(record.preferred_name))
                .chain(record.aliases)
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>();
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "character".into(),
                entity_id: character.id.to_string(),
                name: character.name,
                aliases,
                source_path: None,
            });
        }
        for location in self.list_locations()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "location".into(),
                entity_id: location.id.to_string(),
                name: location.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for object in self.list_story_objects()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "object".into(),
                entity_id: object.id.to_string(),
                name: object.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for plotline in self.list_plotlines()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "plotline".into(),
                entity_id: plotline.id.to_string(),
                name: plotline.name,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for event in self.list_events()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "event".into(),
                entity_id: event.id.to_string(),
                name: event.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        for research in self.list_research_items()? {
            symbols.push(ProjectSymbolDefinition {
                entity_kind: "research".into(),
                entity_id: research.id.to_string(),
                name: research.title,
                aliases: Vec::new(),
                source_path: None,
            });
        }
        symbols.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.entity_kind.cmp(&right.entity_kind))
                .then_with(|| left.entity_id.cmp(&right.entity_id))
        });
        Ok(symbols)
    }

    pub fn build_emacs_tags_export(&self) -> Result<EmacsTagsExport> {
        let symbols = self.list_project_symbols()?;
        let mut markdown = String::from(
            "# Generated Writer's IDE symbol index\n\nGenerated from stable project entity IDs. Re-export after project changes.\n",
        );
        let mut entries = Vec::new();
        for symbol in symbols {
            let clean = |value: &str| {
                value
                    .chars()
                    .map(|character| {
                        if character.is_control() {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let name = clean(&symbol.name);
            if name.is_empty() {
                continue;
            }
            let heading = format!(
                "## {}: {} [{}]",
                clean(&symbol.entity_kind),
                name,
                clean(&symbol.entity_id)
            );
            let offset = markdown.len();
            let line = markdown.bytes().filter(|byte| *byte == b'\n').count() + 1;
            markdown.push_str(&heading);
            markdown.push('\n');
            markdown.push_str("Entity ID: `");
            markdown.push_str(&clean(&symbol.entity_id));
            markdown.push_str("`\n");
            let aliases = symbol
                .aliases
                .iter()
                .map(|alias| clean(alias))
                .filter(|alias| !alias.is_empty())
                .collect::<Vec<_>>();
            if !aliases.is_empty() {
                markdown.push_str("Aliases: ");
                markdown.push_str(
                    &aliases
                        .iter()
                        .map(|alias| format!("`{alias}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                markdown.push('\n');
            }
            if let Some(path) = symbol.source_path {
                markdown.push_str("Project source: `");
                markdown.push_str(&clean(&path.to_string_lossy()));
                markdown.push_str("`\n");
            } else {
                markdown.push_str("Definition: structured project record\n");
            }
            let mut tag_names = vec![name];
            tag_names.extend(aliases);
            tag_names.sort();
            tag_names.dedup();
            for tag_name in tag_names
                .into_iter()
                .filter(|tag_name| !tag_name.is_empty())
            {
                entries.push((heading.clone(), tag_name, line, offset));
            }
        }
        let mut tags_file = format!("\u{c}\nsymbols.md,{}\n", markdown.len());
        for (text, name, line, offset) in entries {
            tags_file.push_str(&text);
            tags_file.push('\u{7f}');
            tags_file.push_str(&name);
            tags_file.push('\u{1}');
            tags_file.push_str(&format!("{line},{offset}\n"));
        }
        Ok(EmacsTagsExport {
            symbols_markdown: markdown,
            tags_file,
        })
    }

    pub fn export_emacs_tags(&self) -> Result<PathBuf> {
        let root = self
            .project_root()
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("open project for TAGS export: {error}")))?;
        let control = root.join(".writers-ide");
        fs::create_dir_all(&control).map_err(|error| {
            AppError::Domain(format!("create symbol export directory: {error}"))
        })?;
        let metadata = fs::symlink_metadata(&control).map_err(|error| {
            AppError::Domain(format!("inspect symbol export directory: {error}"))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AppError::Domain(
                "symbol export directory must be a real project directory".into(),
            ));
        }
        let directory = control.join("tags");
        fs::create_dir_all(&directory)
            .map_err(|error| AppError::Domain(format!("create TAGS directory: {error}")))?;
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|error| AppError::Domain(format!("inspect TAGS directory: {error}")))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AppError::Domain(
                "symbol TAGS directory must be a real project directory".into(),
            ));
        }
        let directory = directory
            .canonicalize()
            .map_err(|error| AppError::Domain(format!("resolve TAGS directory: {error}")))?;
        if !directory.starts_with(&root) {
            return Err(AppError::Domain(
                "symbol export path escapes the project".into(),
            ));
        }
        let export = self.build_emacs_tags_export()?;
        write_project_file_atomically(
            &directory.join("symbols.md"),
            export.symbols_markdown.as_bytes(),
        )?;
        write_project_file_atomically(&directory.join("TAGS"), export.tags_file.as_bytes())?;
        Ok(directory.join("TAGS"))
    }
    pub fn analytics_dataset(&self) -> Result<Vec<story_analysis::SceneAnalysisInput>> {
        let mut result = Vec::new();
        let locations = self
            .store
            .list_locations()?
            .into_iter()
            .map(|item| (item.id.to_string(), item.name))
            .collect::<std::collections::HashMap<_, _>>();
        let events = self
            .store
            .list_events()?
            .into_iter()
            .map(|item| (item.id.to_string(), item.title))
            .collect::<std::collections::HashMap<_, _>>();
        for row in self.store.list_scenes(false)? {
            let characters = self
                .store
                .scene_character_analytics(row.scene.id)?
                .into_iter()
                .map(|(id, name, role)| {
                    let role = match role.as_str() {
                        "pov" => ParticipationRole::Pov,
                        "major" => ParticipationRole::Major,
                        "minor" => ParticipationRole::Minor,
                        "off_camera" => ParticipationRole::OffCamera,
                        _ => ParticipationRole::Mentioned,
                    };
                    story_analysis::CharacterOnScene { id, name, role }
                })
                .collect();
            let plotlines = self
                .store
                .scene_plotline_analytics(row.scene.id)?
                .into_iter()
                .map(|(id, name)| story_analysis::PlotlineOnScene { id, name })
                .collect();
            let mut location_ids = std::collections::BTreeSet::new();
            location_ids.extend(row.scene.start_location.map(|id| id.to_string()));
            location_ids.extend(row.scene.end_location.map(|id| id.to_string()));
            let locations = location_ids
                .into_iter()
                .map(|id| story_analysis::StoryEntityOnScene {
                    name: locations.get(&id).cloned().unwrap_or_else(|| id.clone()),
                    id,
                })
                .collect();
            let events = self
                .store
                .scene_event_links(row.scene.id)?
                .into_iter()
                .map(|(id, _kind)| id.to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|id| story_analysis::StoryEntityOnScene {
                    name: events.get(&id).cloned().unwrap_or_else(|| id.clone()),
                    id,
                })
                .collect();
            result.push(story_analysis::SceneAnalysisInput {
                scene_id: row.scene.id.to_string(),
                title: row.scene.title,
                chapter_id: row.chapter_id.map(|id| id.to_string()),
                actual_words: row.actual_words,
                target_words: row.scene.target_words,
                start_time: row.scene.start_time,
                end_time: row.scene.end_time,
                characters,
                plotlines,
                locations,
                events,
            });
        }
        Ok(result)
    }
    pub fn processing_input(&self) -> Result<story_pipeline::PipelineInput> {
        self.processing_input_mode(false)
    }
    pub fn processing_input_with_transclusions(&self) -> Result<story_pipeline::PipelineInput> {
        self.processing_input_mode(true)
    }
    /// Render the current manuscript snapshot through the shared Compile IR.
    /// Non-Markdown formats require a local Pandoc installation; PDF also
    /// requires LuaLaTeX. No network access or source-file mutation occurs.
    pub fn compile_artifact(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_with_design(
            format,
            expand_transclusions,
            story_document::BookDesign::default(),
        )
    }
    pub fn compile_artifact_with_design(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesign,
    ) -> Result<story_document::CompileArtifact> {
        self.compile_artifact_with_profile(format, expand_transclusions, design.profile())
    }
    pub fn compile_artifact_with_profile(
        &self,
        format: story_document::CompileFormat,
        expand_transclusions: bool,
        design: story_document::BookDesignProfile,
    ) -> Result<story_document::CompileArtifact> {
        let input = self.processing_input_mode(expand_transclusions)?;
        let ir = story_pipeline::compile_ir(&input);
        let renderer = story_document::PandocRenderer::default();
        renderer
            .render_with_profile(&ir, format, Some(self.store.project_root()), &design)
            .map_err(AppError::Domain)
    }
    pub fn compile_diagnostics(&self) -> Result<Vec<story_document::CompileDiagnostic>> {
        let input = self.processing_input()?;
        Ok(story_document::publication_diagnostics(
            &story_pipeline::compile_ir(&input),
        ))
    }
    pub fn proofreading_config(&self) -> Result<story_analysis::ProofreadingConfig> {
        let path = self
            .store
            .project_root()
            .join(".writers-ide/diagnostics.toml");
        match std::fs::read_to_string(path) {
            Ok(source) => {
                story_analysis::ProofreadingConfig::from_toml(&source).map_err(AppError::Domain)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(story_analysis::ProofreadingConfig::default())
            }
            Err(error) => Err(AppError::Domain(format!(
                "read diagnostics config: {error}"
            ))),
        }
    }
    pub fn proofread_text(&self, text: &str) -> Result<Vec<story_analysis::ProofreadingFinding>> {
        story_analysis::proofread_with_config(text, &self.proofreading_config()?)
            .map_err(AppError::Domain)
    }
    pub fn write_default_proofreading_config(&self) -> Result<std::path::PathBuf> {
        let path = self
            .store
            .project_root()
            .join(".writers-ide/diagnostics.toml");
        let source = story_analysis::ProofreadingConfig::default()
            .to_toml()
            .map_err(AppError::Domain)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| AppError::Domain(format!("create diagnostics config: {error}")))?;
        file.write_all(source.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| AppError::Domain(format!("write diagnostics config: {error}")))?;
        Ok(path)
    }
    fn processing_input_mode(
        &self,
        expand_transclusions: bool,
    ) -> Result<story_pipeline::PipelineInput> {
        let project = self.get_project()?;
        let analytics = self.analytics_dataset()?;
        let groups = self
            .store
            .list_structural_nodes()?
            .into_iter()
            .map(|node| story_pipeline::PipelineGroup {
                id: node.id.to_string(),
                parent_id: node.parent_id.map(|id| id.to_string()),
                kind: match node.kind.as_str() {
                    "section" => story_document::StructuralKind::Section,
                    "chapter" => story_document::StructuralKind::Chapter,
                    _ => story_document::StructuralKind::Group,
                },
                title: node.title,
            })
            .collect();
        let mut scenes = Vec::new();
        let mut content_units = Vec::new();
        for unit in self.store.list_content_units(false)? {
            let (text, _) = self.store.load_content_unit_text(unit.id)?;
            let language_code = Some(unit.effective_language_code(&project).to_owned());
            content_units.push(story_pipeline::PipelineContentUnit {
                id: unit.id.to_string(),
                parent_id: unit.parent_id.map(|id| id.to_string()),
                kind: match unit.kind {
                    ContentUnitKind::FrontMatter => story_document::ContentUnitKind::FrontMatter,
                    ContentUnitKind::SectionBody => story_document::ContentUnitKind::SectionBody,
                    ContentUnitKind::ChapterBody => story_document::ContentUnitKind::ChapterBody,
                    ContentUnitKind::BackMatter => story_document::ContentUnitKind::BackMatter,
                },
                title: unit.title,
                language_code,
                text,
            });
        }
        let mut snapshots = 0usize;
        for row in self.store.list_scenes(false)? {
            let (text, _) = self.store.load_text(row.scene.id)?;
            let language_code = Some(row.scene.effective_language_code(&project).to_owned());
            let language_spans = self
                .store
                .list_passage_language_tags(row.scene.id)?
                .into_iter()
                .filter(|tag| tag.anchor.state == "current")
                .map(|tag| story_document::CompileLanguageSpan {
                    start_byte: tag.anchor.start_byte,
                    end_byte: tag.anchor.end_byte,
                    language_code: tag.language_code,
                })
                .collect();
            let expanded_passages = if expand_transclusions {
                Some(
                    self.store
                        .expand_document_inclusions(EntityRef::Scene(row.scene.id))?
                        .into_iter()
                        .map(|passage| story_pipeline::PipelinePassage {
                            source_entity_id: format!(
                                "{}:{}",
                                passage.source.kind(),
                                passage.source.id()
                            ),
                            text: passage.text,
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            let expanded_text = expanded_passages.as_ref().map(|passages| {
                passages
                    .iter()
                    .map(|passage| passage.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            });
            let linked = analytics
                .iter()
                .find(|data| data.scene_id == row.scene.id.to_string());
            let characters = linked
                .map(|d| {
                    d.characters
                        .iter()
                        .map(|c| story_pipeline::PipelineCharacter {
                            id: c.id.clone(),
                            name: c.name.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let plotlines = linked
                .map(|d| d.plotlines.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            snapshots += self.store.list_snapshots(row.scene.id)?.len();
            scenes.push(story_pipeline::PipelineScene {
                id: row.scene.id.to_string(),
                chapter_id: row.chapter_id.map(|id| id.to_string()),
                title: row.scene.title,
                language_code,
                language_spans,
                text,
                expanded_text,
                expanded_passages,
                target_words: row.scene.target_words,
                word_count_policy: row.scene.word_count_policy,
                characters,
                plotlines,
                start: row.scene.start_time,
                end: row.scene.end_time,
            });
        }
        let events = self
            .store
            .list_events()?
            .into_iter()
            .map(|event| story_pipeline::PipelineEvent {
                id: event.id.to_string(),
                title: event.title,
                start: event.start,
                end: event.end,
            })
            .collect();
        let event_dependencies = self
            .store
            .list_event_dependencies()?
            .into_iter()
            .map(|dependency| story_pipeline::PipelineEventDependency {
                predecessor: dependency.predecessor.to_string(),
                successor: dependency.successor.to_string(),
                kind: dependency.kind,
            })
            .collect();
        Ok(story_pipeline::PipelineInput {
            title: project.title,
            language_code: Some(project.language_code.clone()),
            target_words: project.target_words,
            word_count_policy: project.word_count_policy,
            expand_transclusions,
            groups,
            content_units,
            scenes,
            events,
            event_dependencies,
            revision_summary: format!(
                "{} scenes; {snapshots} stored snapshots",
                self.store.list_scenes(false)?.len()
            ),
        })
    }
    pub fn run_processing(&self) -> Result<Vec<story_pipeline::StageReport>> {
        self.run_processing_stages(&story_pipeline::Stage::ALL, false)
    }
    pub fn run_processing_with_transclusions(&self) -> Result<Vec<story_pipeline::StageReport>> {
        self.run_processing_stages(&story_pipeline::Stage::ALL, true)
    }
    pub fn run_processing_stages(
        &self,
        stages: &[story_pipeline::Stage],
        expand_transclusions: bool,
    ) -> Result<Vec<story_pipeline::StageReport>> {
        let input = self.processing_input_mode(expand_transclusions)?;
        Ok(
            story_pipeline::PipelineRunner::new(self.store.project_root())
                .run_selected(&input, stages)?,
        )
    }
    pub fn run_processing_recipe(
        &self,
        recipe: &story_pipeline::ProcessingRecipe,
    ) -> Result<Vec<story_pipeline::StageReport>> {
        let input = self.processing_input_mode(recipe.expand_transclusions)?;
        Ok(
            story_pipeline::PipelineRunner::new(self.store.project_root())
                .run_recipe(&input, recipe)?,
        )
    }
    pub fn save_extraction_proposal(
        &self,
        filename: &str,
        json: &[u8],
    ) -> Result<std::path::PathBuf> {
        Ok(self.store.save_proposal_file(filename, json)?)
    }
    pub fn list_extraction_proposal_files(&self) -> Result<Vec<std::path::PathBuf>> {
        Ok(self.store.list_proposal_files()?)
    }
    pub fn load_extraction_proposal_file(&self, path: &Path) -> Result<(Vec<u8>, String)> {
        let bytes = self.store.load_proposal_file(path)?;
        let fingerprint = format!("{:x}", Sha256::digest(&bytes));
        Ok((bytes, fingerprint))
    }
    pub fn update_extraction_proposal_file(
        &self,
        path: &Path,
        expected_fingerprint: &str,
        json: &[u8],
    ) -> Result<()> {
        Ok(self
            .store
            .update_proposal_file(path, expected_fingerprint, json)?)
    }
    pub fn dismiss_extraction_claim(
        &self,
        proposal: &mut story_ollama::ProvisionalExtraction,
        claim_index: usize,
    ) -> Result<()> {
        let claim = proposal.claims.get_mut(claim_index).ok_or_else(|| {
            AppError::Domain(format!("claim index {claim_index} is out of range"))
        })?;
        if claim.approved {
            return Err(AppError::Domain(
                "an approved claim cannot be dismissed; retract it through knowledge management"
                    .into(),
            ));
        }
        claim.dismissed = true;
        Ok(())
    }
    /// Promotes exactly one evidence-anchored provisional claim after revalidating its source.
    pub fn approve_extraction_claim(
        &mut self,
        proposal: &story_ollama::ProvisionalExtraction,
        claim_index: usize,
    ) -> Result<KnowledgeClaimId> {
        let claim = proposal.claims.get(claim_index).ok_or_else(|| {
            AppError::Domain(format!("claim index {claim_index} is out of range"))
        })?;
        if claim.dismissed {
            return Err(AppError::Domain(
                "dismissed extraction claims cannot be approved".into(),
            ));
        }
        let (source_id, source_text) = match proposal.source_kind.as_str() {
            "scene" => {
                let id = SceneId(
                    proposal
                        .source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|error| AppError::Domain(error.to_string()))?,
                );
                let (text, fingerprint) = self.load_scene_text(id)?;
                if fingerprint != proposal.source_fingerprint {
                    return Err(AppError::Domain(
                        "proposal source changed since extraction; regenerate or review the evidence".into(),
                    ));
                }
                (id.to_string(), text)
            }
            "reading" => {
                let id = ReadingId(
                    proposal
                        .source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|error| AppError::Domain(error.to_string()))?,
                );
                let reading = self.get_reading_document(id)?;
                let text = reading.extracted_text;
                let fingerprint = format!("{:x}", Sha256::digest(text.as_bytes()));
                if fingerprint != proposal.source_fingerprint {
                    return Err(AppError::Domain(
                        "proposal reading source changed since extraction; regenerate or review the evidence".into(),
                    ));
                }
                (id.to_string(), text)
            }
            other => {
                return Err(AppError::Domain(format!(
                    "unsupported extraction source kind: {other}"
                )));
            }
        };
        if proposal.source_length_bytes != source_text.len() {
            return Err(AppError::Domain(
                "proposal source length no longer matches its source".into(),
            ));
        }
        let start = claim
            .evidence_start_byte
            .ok_or_else(|| AppError::Domain("claim has no source evidence offset".into()))?;
        let end = claim
            .evidence_end_byte
            .ok_or_else(|| AppError::Domain("claim has no source evidence end offset".into()))?;
        let evidence = source_text
            .get(start..end)
            .ok_or_else(|| AppError::Domain("claim evidence offsets are invalid".into()))?;
        if evidence != claim.claim.evidence_quote || evidence.is_empty() {
            return Err(AppError::Domain(
                "claim evidence no longer matches its exact source passage".into(),
            ));
        }
        let matches_existing = |existing: &ApprovedKnowledgeClaim| {
            existing.source_kind == proposal.source_kind
                && existing.source_id == source_id
                && existing.source_fingerprint == proposal.source_fingerprint
                && existing.evidence_start_byte == start
                && existing.evidence_end_byte == end
                && existing.kind == claim.claim.kind
                && existing.name == claim.claim.name
                && existing.claim == claim.claim.claim
        };
        if let Some(existing) = self
            .store
            .list_approved_knowledge_claims()?
            .into_iter()
            .find(matches_existing)
        {
            return Ok(existing.id);
        }
        if claim.claim.kind.trim().is_empty()
            || claim.claim.name.trim().is_empty()
            || claim.claim.claim.trim().is_empty()
        {
            return Err(AppError::Domain(
                "approved claim kind, name, and statement must be non-empty".into(),
            ));
        }
        let approved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        let record = ApprovedKnowledgeClaim {
            id: KnowledgeClaimId::new(),
            kind: claim.claim.kind.clone(),
            name: claim.claim.name.clone(),
            claim: claim.claim.claim.clone(),
            approximate_story_time: claim.claim.approximate_story_time.clone(),
            source_kind: proposal.source_kind.clone(),
            source_id,
            source_fingerprint: proposal.source_fingerprint.clone(),
            source_length_bytes: proposal.source_length_bytes,
            evidence_quote: claim.claim.evidence_quote.clone(),
            evidence_start_byte: start,
            evidence_end_byte: end,
            model: proposal.model.clone(),
            approved_at: format!(
                "{:010}.{:03}",
                approved_at.as_secs(),
                approved_at.subsec_millis()
            ),
        };
        self.store.add_approved_knowledge_claim(&record)?;
        Ok(record.id)
    }
    pub fn list_approved_knowledge_claims(&self) -> Result<Vec<ApprovedKnowledgeClaim>> {
        Ok(self.store.list_approved_knowledge_claims()?)
    }
    pub fn extract_with_ollama(
        &self,
        source_kind: &str,
        source_id: &str,
        model: &str,
        endpoint: &str,
    ) -> Result<story_ollama::ProvisionalExtraction> {
        let project = self.get_project()?;
        let (source, language_code, passage_languages) = match source_kind {
            "scene" => {
                let scene_id = SceneId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                );
                let scene = self.get_scene(scene_id)?;
                let passage_languages = self
                    .store
                    .list_passage_language_tags(scene_id)?
                    .into_iter()
                    .filter(|tag| tag.anchor.state == "current")
                    .map(|tag| {
                        Ok(story_ollama::LanguageContextSpan {
                            start_byte: usize::try_from(tag.anchor.start_byte)
                                .map_err(|error| AppError::Domain(error.to_string()))?,
                            end_byte: usize::try_from(tag.anchor.end_byte)
                                .map_err(|error| AppError::Domain(error.to_string()))?,
                            language_code: tag.language_code,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                (
                    self.load_scene_text(scene_id)?.0,
                    scene.scene.effective_language_code(&project).to_owned(),
                    passage_languages,
                )
            }
            "reading" => {
                let document = self.get_reading_document(ReadingId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                ))?;
                (
                    document.extracted_text,
                    document.language_code.unwrap_or(project.language_code),
                    Vec::new(),
                )
            }
            _ => {
                return Err(AppError::Domain(
                    "source kind must be scene or reading".into(),
                ));
            }
        };
        let result = story_ollama::OllamaClient::new(endpoint, model)
            .map_err(|e| AppError::Domain(e.to_string()))?
            .extract_with_language_context(&source, Some(&language_code), &passage_languages)
            .map_err(|e| AppError::Domain(e.to_string()))?;
        Ok(story_ollama::attach_provenance(
            source_kind,
            source_id,
            &source,
            model,
            result,
        ))
    }
    pub fn create_snapshot(&mut self, scene: SceneId, label: &str) -> Result<SnapshotId> {
        Ok(self.store.create_snapshot(scene, label)?)
    }
    pub fn list_snapshots(&self, scene: SceneId) -> Result<Vec<story_storage::SnapshotRecord>> {
        Ok(self.store.list_snapshots(scene)?)
    }
    pub fn load_snapshot(
        &self,
        snapshot: SnapshotId,
    ) -> Result<(story_storage::SnapshotRecord, String)> {
        Ok(self.store.load_snapshot(snapshot)?)
    }
    pub fn diff_snapshot(
        &self,
        scene: SceneId,
        snapshot: SnapshotId,
    ) -> Result<Vec<story_storage::SnapshotDiffLine>> {
        Ok(self.store.diff_snapshot(scene, snapshot)?)
    }
    pub fn restore_snapshot(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
    ) -> Result<String> {
        Ok(self
            .store
            .restore_snapshot(scene, snapshot, expected_hash)?)
    }
    pub fn restore_snapshot_lines(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
        selection: SnapshotRestoreSelection,
    ) -> Result<String> {
        Ok(self
            .store
            .restore_snapshot_lines(scene, snapshot, expected_hash, selection)?)
    }
    pub fn update_scene_metadata(&mut self, scene: &Scene) -> Result<()> {
        Ok(self.store.update_scene(scene)?)
    }
    pub fn move_scene(&mut self, scene: SceneId, destination: NodeId, position: u32) -> Result<()> {
        Ok(self.store.move_scene(scene, destination, position)?)
    }
    pub fn reorder_scene(&mut self, scene: SceneId, position: u32) -> Result<()> {
        let record = self.store.get_scene(scene)?;
        let chapter = record
            .chapter_id
            .ok_or_else(|| AppError::Domain("scene has no chapter".into()))?;
        Ok(self.store.move_scene(scene, chapter, position)?)
    }
    pub fn archive_scene(&mut self, id: SceneId) -> Result<()> {
        Ok(self.store.archive_scene(id)?)
    }
    pub fn create_character(&mut self, name: &str) -> Result<CharacterId> {
        let id = CharacterId::new();
        self.store.add_character(id, name)?;
        Ok(id)
    }
    pub fn get_character(&self, id: CharacterId) -> Result<story_core::Character> {
        Ok(self.store.get_character(id)?)
    }
    pub fn update_character(&mut self, character: &story_core::Character) -> Result<()> {
        Ok(self.store.update_character(character)?)
    }
    pub fn set_character_field(&mut self, id: CharacterId, field: &str, value: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        match field {
            "full_name" => character.full_name = value.into(),
            "preferred_name" => character.preferred_name = value.into(),
            "description" => character.description = value.into(),
            "background" => character.background = value.into(),
            "goals" => character.goals = value.into(),
            "motivations" => character.motivations = value.into(),
            "secrets" => character.secrets = value.into(),
            "internal_conflict" => character.internal_conflict = value.into(),
            "external_conflict" => character.external_conflict = value.into(),
            "status" => character.status = value.into(),
            _ => {
                return Err(AppError::Domain(format!(
                    "unknown character field '{field}'"
                )));
            }
        }
        self.update_character(&character)
    }
    pub fn add_character_alias(&mut self, id: CharacterId, alias: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        if character
            .aliases
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(alias.trim()))
        {
            return Err(AppError::Domain("character alias already exists".into()));
        }
        character.aliases.push(alias.into());
        self.update_character(&character)
    }
    pub fn remove_character_alias(&mut self, id: CharacterId, alias: &str) -> Result<()> {
        let mut character = self.get_character(id)?;
        let Some(index) = character
            .aliases
            .iter()
            .position(|existing| existing.eq_ignore_ascii_case(alias.trim()))
        else {
            return Err(AppError::Domain("character alias does not exist".into()));
        };
        character.aliases.remove(index);
        self.update_character(&character)
    }
    pub fn create_location(&mut self, name: &str) -> Result<LocationId> {
        let id = LocationId::new();
        self.store.add_location(id, name)?;
        Ok(id)
    }
    pub fn get_location(&self, id: story_core::LocationId) -> Result<story_core::Location> {
        Ok(self.store.get_location(id)?)
    }
    pub fn update_location(&mut self, location: &story_core::Location) -> Result<()> {
        Ok(self.store.update_location(location)?)
    }
    pub fn create_plotline(&mut self, name: &str) -> Result<PlotlineId> {
        let id = PlotlineId::new();
        self.store.add_plotline(id, name)?;
        Ok(id)
    }
    pub fn link_character(
        &mut self,
        scene: SceneId,
        character: CharacterId,
        role: ParticipationRole,
    ) -> Result<()> {
        Ok(self.store.link_character(scene, character, role)?)
    }
    pub fn set_scene_plotlines(
        &mut self,
        scene: SceneId,
        links: &[ScenePlotline],
        policy: WordAttributionPolicy,
    ) -> Result<()> {
        Ok(self.store.set_scene_plotlines(scene, links, policy)?)
    }
    pub fn scene_plotline_links(&self, scene: SceneId) -> Result<Vec<ScenePlotline>> {
        Ok(self.store.scene_plotline_links(scene)?)
    }
    pub fn add_event(&mut self, title: &str, source_time: &str) -> Result<EventId> {
        let id = EventId::new();
        self.store.add_event(id, title, source_time)?;
        Ok(id)
    }
    pub fn add_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        Ok(self.store.add_story_event(event)?)
    }
    pub fn update_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        Ok(self.store.update_story_event(event)?)
    }
    pub fn update_story_event_time(
        &mut self,
        id: EventId,
        start: story_core::StoryTime,
        end: story_core::StoryTime,
        source_time_text: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .update_story_event_time(id, start, end, source_time_text)?)
    }
    pub fn link_event(&mut self, scene: SceneId, event: EventId, kind: &str) -> Result<()> {
        Ok(self.store.link_event(scene, event, kind)?)
    }
    pub fn scene_event_links(&self, scene: SceneId) -> Result<Vec<(EventId, String)>> {
        Ok(self.store.scene_event_links(scene)?)
    }
    pub fn add_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        Ok(self.store.add_event_dependency(dependency)?)
    }
    pub fn list_event_dependencies(&self) -> Result<Vec<StoryEventDependency>> {
        Ok(self.store.list_event_dependencies()?)
    }
    pub fn update_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        Ok(self.store.update_event_dependency(dependency)?)
    }
    pub fn remove_event_dependency(
        &mut self,
        predecessor: EventId,
        successor: EventId,
        kind: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .remove_event_dependency(predecessor, successor, kind)?)
    }
    pub fn inspect_event_dependencies(
        &self,
    ) -> Result<Vec<(StoryEventDependency, story_analysis::InspectionResult)>> {
        let events = self.store.list_events()?;
        self.store
            .list_event_dependencies()
            .map(|dependencies| {
                dependencies
                    .into_iter()
                    .map(|dependency| {
                        let earlier = events
                            .iter()
                            .find(|event| event.id == dependency.predecessor);
                        let later = events.iter().find(|event| event.id == dependency.successor);
                        let comparison = earlier.zip(later).map(|(a, b)| a.end.compare(b.start));
                        let result = story_analysis::evaluate_event_precedence(
                            &dependency.predecessor.to_string(),
                            &dependency.successor.to_string(),
                            comparison,
                        );
                        (dependency, result)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
    pub fn add_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        Ok(self.store.add_location_incompatibility_rule(rule)?)
    }
    pub fn list_location_incompatibility_rules(&self) -> Result<Vec<LocationIncompatibilityRule>> {
        Ok(self.store.list_location_incompatibility_rules()?)
    }
    pub fn update_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        Ok(self.store.update_location_incompatibility_rule(rule)?)
    }
    pub fn remove_location_incompatibility_rule(&mut self, id: ContinuityRuleId) -> Result<()> {
        Ok(self.store.remove_location_incompatibility_rule(id)?)
    }
    pub fn inspect_location_incompatibilities(
        &self,
    ) -> Result<Vec<story_analysis::LocationRuleFinding>> {
        Ok(story_analysis::location_incompatibility_findings(
            &self.analytics_dataset()?,
            &self.store.list_location_incompatibility_rules()?,
        ))
    }
    pub fn create_passage_anchor(
        &mut self,
        scene: SceneId,
        start: u64,
        end: u64,
        quote: &str,
        prefix: &str,
        suffix: &str,
    ) -> Result<AnchorId> {
        Ok(self
            .store
            .create_anchor(scene, start, end, quote, prefix, suffix)?)
    }
    pub fn list_passage_anchors(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::PassageAnchorRecord>> {
        Ok(self.store.list_scene_anchors(scene)?)
    }
    pub fn reanchor_passage(
        &mut self,
        anchor: story_core::AnchorId,
        start: u64,
        end: u64,
    ) -> Result<()> {
        Ok(self.store.reanchor_passage(anchor, start, end)?)
    }
    pub fn create_story_link(&mut self, link: &StoryLink) -> Result<()> {
        Ok(self.store.create_story_link(link)?)
    }
    pub fn create_research_item(
        &mut self,
        title: &str,
        body: &str,
        media_type: &str,
        source_path: Option<&str>,
        source_hash: Option<&str>,
    ) -> Result<ResearchId> {
        Ok(self
            .store
            .create_research_item(title, body, media_type, source_path, source_hash)?)
    }
    pub fn list_research_items(&self) -> Result<Vec<story_storage::ResearchSummary>> {
        Ok(self.store.list_research_items()?)
    }
    pub fn load_research_text(&self, id: ResearchId) -> Result<String> {
        Ok(self.store.load_research_text(id)?)
    }
    pub fn create_issue(&mut self, title: &str) -> Result<IssueId> {
        Ok(self.store.create_issue(title)?)
    }
    pub fn set_issue_status(&mut self, issue: IssueId, status: &str) -> Result<()> {
        Ok(self.store.set_issue_status(issue, status)?)
    }
    pub fn link_issue(&mut self, scene: SceneId, issue: IssueId) -> Result<()> {
        Ok(self.store.link_issue(scene, issue)?)
    }
    pub fn list_issues(&self) -> Result<Vec<story_storage::IssueSummary>> {
        Ok(self.store.list_issues()?)
    }
    pub fn start_writing_challenge(
        &mut self,
        target_words: u32,
        duration_seconds: u32,
        mode: &str,
    ) -> Result<String> {
        Ok(self
            .store
            .start_writing_challenge(target_words, duration_seconds, mode)?)
    }
    pub fn finish_writing_challenge(
        &mut self,
        id: &str,
        words_written: u32,
        abandoned: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .finish_writing_challenge(id, words_written, abandoned)?)
    }
    pub fn finish_writing_challenge_detailed(
        &mut self,
        id: &str,
        words_written: u32,
        words_deleted: u32,
        pauses_consumed: u32,
        outcome: &str,
    ) -> Result<()> {
        Ok(self.store.finish_writing_challenge_detailed(
            id,
            words_written,
            words_deleted,
            pauses_consumed,
            outcome,
        )?)
    }
    pub fn finish_writing_challenge_with_activity(
        &mut self,
        id: &str,
        words_written: u32,
        words_deleted: u32,
        pauses_consumed: u32,
        outcome: &str,
        scene_id: Option<SceneId>,
    ) -> Result<()> {
        Ok(self.store.finish_writing_challenge_with_activity(
            id,
            words_written,
            words_deleted,
            pauses_consumed,
            outcome,
            scene_id,
        )?)
    }
    pub fn list_writing_challenges(&self) -> Result<Vec<story_storage::WritingChallengeRecord>> {
        Ok(self.store.list_writing_challenges()?)
    }
    pub fn create_writing_goal(&mut self, goal: &story_storage::NewWritingGoal) -> Result<String> {
        Ok(self.store.create_writing_goal(goal)?)
    }
    pub fn update_writing_goal(
        &mut self,
        id: &str,
        title: &str,
        target_words: i64,
        baseline_words: i64,
        success_metric: &str,
    ) -> Result<()> {
        Ok(self.store.update_writing_goal(
            id,
            title,
            target_words,
            baseline_words,
            success_metric,
        )?)
    }
    pub fn archive_writing_goal(&mut self, id: &str) -> Result<()> {
        Ok(self.store.archive_writing_goal(id)?)
    }
    pub fn list_writing_goals(
        &self,
        include_archived: bool,
    ) -> Result<Vec<story_storage::WritingGoalRecord>> {
        Ok(self.store.list_writing_goals(include_archived)?)
    }
    pub fn record_writing_goal_event(
        &mut self,
        event: &story_storage::NewWritingGoalEvent,
    ) -> Result<String> {
        Ok(self.store.record_writing_goal_event(event)?)
    }
    pub fn record_writing_activity_for_goals(
        &mut self,
        activity: &story_storage::NewWritingActivity,
    ) -> Result<usize> {
        Ok(self.store.record_writing_activity_for_goals(activity)?)
    }
    pub fn list_writing_goal_events(
        &self,
        goal_id: &str,
    ) -> Result<Vec<story_storage::WritingGoalEventRecord>> {
        Ok(self.store.list_writing_goal_events(goal_id)?)
    }
    pub fn reconcile_writing_goal_event(
        &mut self,
        event_id: &str,
        expected_fingerprint: &str,
        corrected_raw_delta: i64,
        corrected_real_delta: i64,
        reason: &str,
    ) -> Result<String> {
        Ok(self.store.reconcile_writing_goal_event(
            event_id,
            expected_fingerprint,
            corrected_raw_delta,
            corrected_real_delta,
            reason,
        )?)
    }
    pub fn writing_goal_progress(
        &self,
        goal_id: &str,
        as_of_timestamp: i64,
    ) -> Result<Vec<story_storage::WritingGoalProgressPoint>> {
        Ok(self.store.writing_goal_progress(goal_id, as_of_timestamp)?)
    }
    pub fn recover_interrupted_writing_challenges(&mut self) -> Result<u64> {
        Ok(self.store.recover_interrupted_writing_challenges()?)
    }
    pub fn start_writing_session(
        &mut self,
        scene_id: Option<SceneId>,
        start_words: u32,
    ) -> Result<String> {
        Ok(self.store.start_writing_session(scene_id, start_words)?)
    }
    pub fn finish_writing_session(
        &mut self,
        id: &str,
        end_words: u32,
        abandoned: bool,
    ) -> Result<()> {
        Ok(self
            .store
            .finish_writing_session(id, end_words, abandoned)?)
    }
    pub fn list_writing_sessions(&self) -> Result<Vec<story_storage::WritingSessionRecord>> {
        Ok(self.store.list_writing_sessions()?)
    }
    pub fn list_characters(&self) -> Result<Vec<story_storage::CharacterSummary>> {
        Ok(self.store.list_characters()?)
    }
    pub fn list_locations(&self) -> Result<Vec<story_storage::LocationSummary>> {
        Ok(self.store.list_locations()?)
    }
    pub fn list_plotlines(&self) -> Result<Vec<story_storage::PlotlineSummary>> {
        Ok(self.store.list_plotlines()?)
    }
    pub fn list_events(&self) -> Result<Vec<story_storage::EventSummary>> {
        Ok(self.store.list_events()?)
    }
    pub fn import_reading_document(
        &mut self,
        document: &story_reading::ReferenceDocument,
        source: &str,
    ) -> Result<ReadingId> {
        Ok(self.store.import_reading_document(document, source)?)
    }
    pub fn set_reading_document_language(
        &mut self,
        id: ReadingId,
        language_code: Option<&str>,
    ) -> Result<()> {
        Ok(self
            .store
            .set_reading_document_language(id, language_code)?)
    }
    /// Applies a previously inspected manuscript staging object to a new project.
    /// Callers should import into a fresh destination so any failed write can
    /// be discarded without touching the source document.
    pub fn import_staged_manuscript(
        &mut self,
        staging: &story_import::ManuscriptStaging,
    ) -> Result<(ImportRunId, Vec<SceneId>)> {
        let source_type = match &staging.source_type {
            story_import::SourceType::Docx => "docx",
            story_import::SourceType::Rtf => "rtf",
            story_import::SourceType::Odt => "odt",
            story_import::SourceType::Epub => "epub",
            _ => {
                return Err(AppError::Domain(
                    "unsupported manuscript import source type".into(),
                ));
            }
        };
        let run = ImportRunId::new();
        self.store.record_import_run(
            run,
            source_type,
            &staging.source_path,
            &staging.source_fingerprint,
        )?;
        let mut imported = Vec::new();
        let mut unsupported = staging.unsupported.clone();
        for (position, unit) in staging.units.iter().enumerate() {
            let chapter_title = format!("Chapter {}", position + 1);
            let chapter = self.create_chapter(None, &chapter_title, position as u32)?;
            let scene = self.create_scene(chapter, &unit.title, 0)?;
            let (_, current_hash) = self.load_scene_text(scene)?;
            self.save_scene_text(scene, Some(&current_hash), &unit.body_markdown)?;
            let source_item = SourceItemId::new();
            let source_fingerprint = format!("{:x}", Sha256::digest(unit.body_markdown.as_bytes()));
            let raw_fields =
                serde_json::to_string(unit).map_err(|error| AppError::Domain(error.to_string()))?;
            self.store
                .record_source_item(story_storage::ImportSourceItemRecord {
                    id: source_item,
                    run,
                    source_document_id: &unit.source_document_id,
                    source_sheet: None,
                    source_row: Some(position as u32 + 1),
                    fingerprint: &source_fingerprint,
                    inventory_type: "manuscript-unit",
                    raw_fields: &raw_fields,
                })?;
            for (source_field, destination_field) in
                [("title", "title"), ("body_markdown", "manuscript-text")]
            {
                self.store
                    .record_provenance(story_storage::EntityProvenanceRecord {
                        id: ProvenanceId::new(),
                        source_item,
                        destination_type: "scene",
                        destination_id: &scene.to_string(),
                        source_field: Some(source_field),
                        destination_field: Some(destination_field),
                        mapping: "manuscript-import-v1",
                    })?;
            }
            unsupported.extend(unit.unsupported.iter().cloned());
            imported.push(scene);
        }
        unsupported.sort();
        unsupported.dedup();
        let status = if unsupported.is_empty() && staging.warnings.is_empty() {
            "completed"
        } else {
            "incomplete"
        };
        self.store.finish_import_run(
            run,
            status,
            &serde_json::to_string(&staging.warnings)
                .map_err(|error| AppError::Domain(error.to_string()))?,
            &serde_json::to_string(&unsupported)
                .map_err(|error| AppError::Domain(error.to_string()))?,
        )?;
        Ok((run, imported))
    }
    pub fn get_import_run_summary(
        &self,
        run: ImportRunId,
    ) -> Result<story_storage::ImportRunSummary> {
        Ok(self.store.get_import_run_summary(run)?)
    }
    pub fn merge_review_text(
        &mut self,
        scene: SceneId,
        base: &str,
        reviewed_text: &str,
    ) -> Result<story_import::ThreeWayMerge> {
        let (current, expected_hash) = self.load_scene_text(scene)?;
        let result = story_import::three_way_merge(base, &current, reviewed_text);
        if let Some(merged) = &result.merged_text {
            self.save_scene_text(scene, Some(&expected_hash), merged)?;
        }
        Ok(result)
    }
    pub fn resolve_review_text(
        &mut self,
        scene: SceneId,
        base: &str,
        reviewed_text: &str,
        choices: &[story_import::MergeConflictChoice],
    ) -> Result<String> {
        let (current, expected_hash) = self.load_scene_text(scene)?;
        let merged = story_import::resolve_three_way_merge(base, &current, reviewed_text, choices)
            .map_err(AppError::Domain)?;
        self.save_scene_text(scene, Some(&expected_hash), &merged)?;
        Ok(merged)
    }
    pub fn merge_docx_review(
        &mut self,
        scene: SceneId,
        base: &str,
        review_file: impl AsRef<Path>,
        accept_changes: bool,
    ) -> Result<story_import::ThreeWayMerge> {
        let reviewed = story_import::docx_review_text(review_file, accept_changes)
            .map_err(|error| AppError::Domain(error.to_string()))?;
        self.merge_review_text(scene, base, &reviewed)
    }
    pub fn inspect_docx_review(
        &self,
        review_file: impl AsRef<Path>,
    ) -> Result<Vec<story_import::DocxReviewItem>> {
        story_import::inspect_docx_review(review_file)
            .map_err(|error| AppError::Domain(error.to_string()))
    }
    pub fn get_reading_document(
        &self,
        id: ReadingId,
    ) -> Result<story_storage::ReadingDocumentRecord> {
        Ok(self.store.get_reading_document(id)?)
    }
    pub fn list_reading_documents(&self) -> Result<Vec<story_storage::ReadingDocumentRecord>> {
        Ok(self.store.list_reading_documents()?)
    }
    pub fn reload_reading_document(
        &mut self,
        id: ReadingId,
        document: &story_reading::ReferenceDocument,
    ) -> Result<()> {
        Ok(self.store.reload_reading_document(id, document)?)
    }
    pub fn search_reading(&self, terms: &str) -> Result<Vec<(ReadingId, String, String)>> {
        Ok(self.store.search_reading(terms)?)
    }
    pub fn create_reading_annotation(
        &mut self,
        id: ReadingId,
        draft: &ReadingAnnotationDraft,
    ) -> Result<AnnotationId> {
        Ok(self.store.create_reading_annotation(id, draft)?)
    }
    pub fn list_reading_annotations(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingAnnotationRecord>> {
        Ok(self.store.list_reading_annotations(id)?)
    }
    pub fn create_reading_bookmark(
        &mut self,
        id: ReadingId,
        title: &str,
        offset: u64,
    ) -> Result<BookmarkId> {
        Ok(self.store.create_reading_bookmark(id, title, offset)?)
    }
    pub fn list_reading_bookmarks(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingBookmarkRecord>> {
        Ok(self.store.list_reading_bookmarks(id)?)
    }
    pub fn set_reading_checkpoint(&mut self, id: ReadingId, progress: u64) -> Result<()> {
        Ok(self.store.set_reading_checkpoint(id, progress)?)
    }
    pub fn reading_checkpoint(&self, id: ReadingId) -> Result<Option<u64>> {
        Ok(self.store.reading_checkpoint(id)?)
    }
    pub fn create_reading_term(
        &mut self,
        id: ReadingId,
        kind: &str,
        name: &str,
        aliases: &str,
        description: &str,
    ) -> Result<ReadingTermId> {
        Ok(self
            .store
            .create_reading_term(id, kind, name, aliases, description)?)
    }
    pub fn list_reading_terms(
        &self,
        id: ReadingId,
    ) -> Result<Vec<story_storage::ReadingTermRecord>> {
        Ok(self.store.list_reading_terms(id)?)
    }
    pub fn link_reading_entity(
        &mut self,
        doc: ReadingId,
        annotation: Option<AnnotationId>,
        target: EntityRef,
        relation: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .link_reading_entity(doc, annotation, target, relation)?)
    }
    pub fn reading_entity_links(
        &self,
        document: ReadingId,
    ) -> Result<Vec<story_storage::ReadingEntityLinkRecord>> {
        Ok(self.store.reading_entity_links(document)?)
    }
    pub fn backlinks(&self, target: EntityRef) -> Result<Vec<StoryLink>> {
        Ok(self.store.backlinks(target)?)
    }
    pub fn outgoing_links(&self, source: EntityRef) -> Result<Vec<StoryLink>> {
        Ok(self.store.outgoing_links(source)?)
    }
    pub fn expand_scene_transclusions(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::TranscludedPassage>> {
        self.expand_document_inclusions(EntityRef::Scene(scene))
    }
    pub fn expand_document_inclusions(
        &self,
        root: EntityRef,
    ) -> Result<Vec<story_storage::TranscludedPassage>> {
        Ok(self.store.expand_document_inclusions(root)?)
    }
    pub fn broken_links(&self) -> Result<Vec<story_storage::BrokenLink>> {
        Ok(self.store.broken_links()?)
    }
    pub fn save_scene_query(&mut self, query: &SavedSceneQuery) -> Result<()> {
        Ok(self.store.save_scene_query(query)?)
    }
    pub fn set_scene_query_expression(
        &mut self,
        query: QueryId,
        expression: Option<&QueryExpression>,
    ) -> Result<()> {
        Ok(self.store.set_scene_query_expression(query, expression)?)
    }
    pub fn get_scene_query_expression(&self, query: QueryId) -> Result<Option<QueryExpression>> {
        Ok(self.store.get_scene_query_expression(query)?)
    }
    pub fn list_saved_queries(&self) -> Result<Vec<story_storage::SavedQuerySummary>> {
        Ok(self.store.list_saved_queries()?)
    }
    pub fn run_scene_query(&self, query: QueryId) -> Result<Vec<SceneId>> {
        Ok(self.store.run_scene_query(query)?)
    }
    pub fn explain_scene_query_match(
        &self,
        query: QueryId,
        scene: SceneId,
    ) -> Result<Vec<story_core::QueryEvidence>> {
        Ok(self.store.explain_scene_query_match(query, scene)?)
    }
    pub fn record_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
        notes: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .record_character_absence(scene, character, notes)?)
    }
    pub fn clear_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
    ) -> Result<()> {
        Ok(self.store.clear_character_absence(scene, character)?)
    }
    pub fn set_query_assertions(
        &mut self,
        query: QueryId,
        assertions: &[QueryAssertion],
    ) -> Result<()> {
        Ok(self.store.set_query_assertions(query, assertions)?)
    }
    pub fn get_query_assertions(&self, query: QueryId) -> Result<Vec<QueryAssertion>> {
        Ok(self.store.get_query_assertions(query)?)
    }
    pub fn search_scenes(&self, terms: &str) -> Result<Vec<SceneId>> {
        Ok(self.store.search_scenes(terms)?)
    }
    pub fn search_project(
        &self,
        terms: &str,
        limit: u32,
    ) -> Result<Vec<story_storage::ProjectSearchHit>> {
        Ok(self.store.search_project(terms, limit)?)
    }
    pub fn rebuild_project_search_index(&mut self) -> Result<()> {
        Ok(self.store.rebuild_project_search_index()?)
    }
    pub fn define_custom_field(&mut self, def: &CustomFieldDefinition) -> Result<()> {
        Ok(self.store.define_custom_field(def)?)
    }
    pub fn set_custom_field(
        &mut self,
        scene: SceneId,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<()> {
        Ok(self.store.set_custom_field(scene, field, value)?)
    }
    pub fn filter_scenes_by_custom_field(
        &self,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<Vec<SceneId>> {
        Ok(self.store.filter_scenes_by_custom_field(field, value)?)
    }
    pub fn save_scene_text(
        &mut self,
        id: SceneId,
        expected_hash: Option<&str>,
        text: &str,
    ) -> Result<String> {
        Ok(self.store.save_text(id, expected_hash, text)?)
    }
    pub fn create_story_object(&mut self, name: &str, description: &str) -> Result<StoryObjectId> {
        Ok(self.store.create_story_object(name, description)?)
    }
    pub fn list_story_objects(&self) -> Result<Vec<story_storage::StoryObjectRecord>> {
        Ok(self.store.list_story_objects()?)
    }
    pub fn list_scene_mentions(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::DetectedMentionRecord>> {
        Ok(self.store.list_scene_mentions(scene)?)
    }
    pub fn confirmed_entity_mention_appearances(
        &self,
        kind: MentionEntityKind,
        entity_id: &str,
    ) -> Result<Vec<EntityMentionAppearance>> {
        let mut appearances = Vec::new();
        for scene in self.store.list_scenes(false)? {
            for mention in self.store.list_scene_mentions(scene.scene.id)? {
                if mention.state == "confirmed"
                    && mention.entity_kind.as_deref() == Some(kind.as_str())
                    && mention.entity_id.as_deref() == Some(entity_id)
                {
                    appearances.push(EntityMentionAppearance {
                        scene_id: scene.scene.id,
                        scene_title: scene.scene.title.clone(),
                        mention,
                    });
                }
            }
        }
        Ok(appearances)
    }

    pub fn reindex_scene_mentions(&mut self, scene: SceneId, text: &str, hash: &str) -> Result<()> {
        Ok(self.store.reindex_scene_mentions(scene, text, hash)?)
    }
    pub fn confirm_scene_mention(
        &mut self,
        mention: MentionId,
        kind: MentionEntityKind,
        entity_id: &str,
    ) -> Result<()> {
        Ok(self.store.confirm_scene_mention(mention, kind, entity_id)?)
    }
    pub fn dismiss_scene_mention(&mut self, mention: MentionId) -> Result<()> {
        Ok(self.store.dismiss_scene_mention(mention)?)
    }
    pub fn create_personal_comment(
        &mut self,
        scene: SceneId,
        start: u64,
        end: u64,
        quote: &str,
        body: &str,
    ) -> Result<PersonalCommentId> {
        Ok(self
            .store
            .create_personal_comment(scene, start, end, quote, body)?)
    }
    pub fn create_assembly(&mut self, title: &str) -> Result<AssemblyId> {
        Ok(self.store.create_assembly(title)?)
    }

    pub fn create_assembly_draft(&mut self, assembly: AssemblyId) -> Result<AssemblyDraft> {
        Ok(self.store.create_assembly_draft(assembly)?)
    }

    pub fn load_assembly_draft(&self, id: AssemblyDraftId) -> Result<AssemblyDraft> {
        Ok(self.store.load_assembly_draft(id)?)
    }

    pub fn list_assembly_drafts(&self) -> Result<Vec<AssemblyDraft>> {
        Ok(self.store.list_assembly_drafts()?)
    }

    pub fn update_assembly_draft_item(
        &mut self,
        draft: AssemblyDraftId,
        scene: SceneId,
        text: &str,
    ) -> Result<()> {
        Ok(self.store.update_assembly_draft_item(draft, scene, text)?)
    }

    pub fn replace_assembly_draft_items(
        &mut self,
        draft: AssemblyDraftId,
        items: &[(SceneId, String)],
    ) -> Result<()> {
        Ok(self.store.replace_assembly_draft_items(draft, items)?)
    }

    pub fn commit_assembly_draft(&mut self, draft: AssemblyDraftId) -> Result<()> {
        Ok(self.store.commit_assembly_draft(draft)?)
    }

    pub fn create_planning_board(&mut self, title: &str) -> Result<PlanningBoardId> {
        Ok(self.store.create_planning_board(title)?)
    }

    pub fn list_planning_boards(&self) -> Result<Vec<story_storage::PlanningBoardSummary>> {
        Ok(self.store.list_planning_boards()?)
    }

    pub fn save_story_bible_template(&self, template: &StoryBibleTemplate) -> Result<()> {
        Ok(self.store.save_story_bible_template(template)?)
    }

    pub fn list_story_bible_templates(&self) -> Result<Vec<StoryBibleTemplate>> {
        Ok(self.store.list_story_bible_templates()?)
    }

    pub fn load_story_bible_template(
        &self,
        id: StoryBibleTemplateId,
    ) -> Result<StoryBibleTemplate> {
        Ok(self.store.load_story_bible_template(id)?)
    }

    pub fn save_story_bible_profile(&mut self, profile: &StoryBibleProfile) -> Result<()> {
        Ok(self.store.save_story_bible_profile(profile)?)
    }

    pub fn load_story_bible_profile(
        &self,
        kind: StoryBibleEntityKind,
        id: &str,
    ) -> Result<StoryBibleProfile> {
        Ok(self.store.load_story_bible_profile(kind, id)?)
    }

    pub fn add_planning_board_item(
        &mut self,
        board: PlanningBoardId,
        kind: &str,
        entity_id: &str,
        x: f64,
        y: f64,
        note: &str,
    ) -> Result<PlanningBoardItemId> {
        Ok(self
            .store
            .add_planning_board_item(board, kind, entity_id, x, y, note)?)
    }

    pub fn list_planning_board_items(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<story_storage::PlanningBoardItemRecord>> {
        Ok(self.store.list_planning_board_items(board)?)
    }

    pub fn list_planning_board_connectors(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<story_storage::PlanningBoardConnectorRecord>> {
        Ok(self.store.list_planning_board_connectors(board)?)
    }

    pub fn planning_board_svg(&self, board: PlanningBoardId) -> Result<String> {
        let summary = self
            .list_planning_boards()?
            .into_iter()
            .find(|item| item.id == board)
            .ok_or_else(|| StorageError::NotFound(board.to_string()))?;
        let items = self.list_planning_board_items(board)?;
        let connectors = self.list_planning_board_connectors(board)?;
        if items.iter().any(|item| {
            [item.x, item.y, item.width, item.height]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.0e9)
        }) {
            return Err(StorageError::Invalid(
                "planning board coordinates exceed the SVG export range".into(),
            )
            .into());
        }
        let min_x = items.iter().map(|item| item.x).fold(0.0_f64, f64::min) - 12.0;
        let min_y = items.iter().map(|item| item.y).fold(0.0_f64, f64::min) - 12.0;
        let max_x = items
            .iter()
            .map(|item| item.x + item.width)
            .fold(240.0_f64, f64::max)
            + 12.0;
        let max_y = items
            .iter()
            .map(|item| item.y + item.height)
            .fold(160.0_f64, f64::max)
            + 12.0;
        let width = max_x - min_x;
        let height = max_y - min_y;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(StorageError::Invalid(
                "planning board dimensions cannot be rendered".into(),
            )
            .into());
        }
        let positions = items
            .iter()
            .map(|item| (item.id, item))
            .collect::<std::collections::HashMap<_, _>>();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{min_x} {min_y} {width} {height}\" role=\"img\" aria-label=\"{}\">\n<title>{}</title>\n<rect x=\"{min_x}\" y=\"{min_y}\" width=\"{width}\" height=\"{height}\" fill=\"white\"/>\n",
            escape_svg_text(&summary.title),
            escape_svg_text(&summary.title)
        );
        for connector in connectors {
            let source = positions
                .get(&connector.source_item_id)
                .ok_or_else(|| StorageError::Invalid("board connector source is missing".into()))?;
            let target = positions
                .get(&connector.target_item_id)
                .ok_or_else(|| StorageError::Invalid("board connector target is missing".into()))?;
            let (sx, sy) = (
                source.x + source.width / 2.0,
                source.y + source.height / 2.0,
            );
            let (tx, ty) = (
                target.x + target.width / 2.0,
                target.y + target.height / 2.0,
            );
            svg.push_str(&format!("<line x1=\"{sx}\" y1=\"{sy}\" x2=\"{tx}\" y2=\"{ty}\" stroke=\"#64748b\" stroke-width=\"0.8\" marker-end=\"url(#arrow)\"/>\n"));
            if !connector.label.is_empty() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"3\" fill=\"#334155\">{}</text>\n",
                    (sx + tx) / 2.0,
                    (sy + ty) / 2.0 - 1.0,
                    escape_svg_text(&connector.label)
                ));
            }
        }
        svg.push_str("<defs><marker id=\"arrow\" markerWidth=\"6\" markerHeight=\"6\" refX=\"5\" refY=\"3\" orient=\"auto\"><path d=\"M0,0 L6,3 L0,6 z\" fill=\"#64748b\"/></marker></defs>\n");
        for item in items {
            svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1.5\" fill=\"#f8fafc\" stroke=\"#334155\" stroke-width=\"0.6\"/>\n", item.x, item.y, item.width, item.height));
            let title = format!("{} · {}", item.entity_kind, item.entity_id);
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"3.2\" font-weight=\"bold\">{}</text>\n",
                item.x + 2.0,
                item.y + 3.8,
                escape_svg_text(&title)
            ));
            for (index, line) in item.note.lines().take(3).enumerate() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-size=\"2.8\">{}</text>\n",
                    item.x + 2.0,
                    item.y + 7.5 + index as f64 * 3.4,
                    escape_svg_text(line)
                ));
            }
        }
        svg.push_str("</svg>\n");
        Ok(svg)
    }

    pub fn move_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        x: f64,
        y: f64,
    ) -> Result<()> {
        Ok(self.store.move_planning_board_item(item, x, y)?)
    }
    pub fn resize_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        width: f64,
        height: f64,
    ) -> Result<()> {
        Ok(self.store.resize_planning_board_item(item, width, height)?)
    }

    pub fn connect_planning_board_items(
        &mut self,
        board: PlanningBoardId,
        source: PlanningBoardItemId,
        target: PlanningBoardItemId,
        label: &str,
    ) -> Result<PlanningBoardConnectorId> {
        Ok(self
            .store
            .connect_planning_board_items(board, source, target, label)?)
    }
    pub fn list_assemblies(&self) -> Result<Vec<story_storage::AssemblySummary>> {
        Ok(self.store.list_assemblies()?)
    }
    pub fn list_assembly_items(
        &self,
        assembly: AssemblyId,
    ) -> Result<Vec<story_storage::AssemblyItemRecord>> {
        Ok(self.store.list_assembly_items(assembly)?)
    }
    pub fn add_assembly_scene(
        &mut self,
        assembly: AssemblyId,
        scene: SceneId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        Ok(self.store.add_assembly_scene(assembly, scene, position)?)
    }
    pub fn add_assembly_content_unit(
        &mut self,
        assembly: AssemblyId,
        content: ContentUnitId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        Ok(self
            .store
            .add_assembly_content_unit(assembly, content, position)?)
    }
    pub fn move_assembly_item(
        &mut self,
        assembly: AssemblyId,
        item: AssemblyItemId,
        position: u32,
    ) -> Result<()> {
        Ok(self.store.move_assembly_item(assembly, item, position)?)
    }
    pub fn set_assembly_item_included(
        &mut self,
        item: AssemblyItemId,
        included: bool,
    ) -> Result<()> {
        Ok(self.store.set_assembly_item_included(item, included)?)
    }
    pub fn compile_assembly_markdown(&self, assembly: AssemblyId) -> Result<String> {
        let title = self
            .store
            .list_assemblies()?
            .into_iter()
            .find(|item| item.id == assembly)
            .ok_or_else(|| StorageError::NotFound(assembly.to_string()))?
            .title;
        let mut ir = story_document::CompileIr::new(&title);
        ir.language_code = Some(self.get_project()?.language_code);
        for item in self.store.list_assembly_items(assembly)? {
            if !item.included {
                continue;
            }
            let (kind, title, language_code, source_id, text) = if let Some(scene) = item.scene_id {
                let row = self.store.get_scene(scene)?;
                let (text, _) = self.store.load_text(scene)?;
                let language_code = Some(
                    row.scene
                        .effective_language_code(&self.get_project()?)
                        .to_owned(),
                );
                (
                    story_document::ContentUnitKind::Scene,
                    row.scene.title,
                    language_code,
                    scene.to_string(),
                    text,
                )
            } else if let Some(content) = item.content_unit_id {
                let row = self.store.get_content_unit(content)?;
                let (text, _) = self.store.load_content_unit_text(content)?;
                let kind = match row.kind {
                    ContentUnitKind::FrontMatter => story_document::ContentUnitKind::FrontMatter,
                    ContentUnitKind::SectionBody => story_document::ContentUnitKind::SectionBody,
                    ContentUnitKind::ChapterBody => story_document::ContentUnitKind::ChapterBody,
                    ContentUnitKind::BackMatter => story_document::ContentUnitKind::BackMatter,
                };
                let language_code =
                    Some(row.effective_language_code(&self.get_project()?).to_owned());
                (kind, row.title, language_code, content.to_string(), text)
            } else {
                return Err(AppError::Domain(
                    "assembly item has no source entity".into(),
                ));
            };
            ir.push(story_document::CompileUnit {
                id: item.id.to_string(),
                parent_id: None,
                kind,
                title,
                language_code,
                segments: vec![story_document::CompileSegment::new(source_id, text)],
            });
        }
        if !ir.validate() {
            return Err(AppError::Domain(
                "assembly produced invalid Compile IR".into(),
            ));
        }
        let language = ir.language_code.clone();
        let (content, _) = ir.render_markdown();
        if let Some(language) = language {
            let metadata = format!("---\nlang: {language}\n---\n\n");
            let body = content.strip_prefix(&metadata).unwrap_or(&content);
            Ok(format!("{metadata}# {title}\n\n{body}"))
        } else {
            Ok(format!("# {title}\n\n{content}"))
        }
    }
    pub fn list_scene_comments(
        &self,
        scene: SceneId,
    ) -> Result<Vec<story_storage::PersonalCommentRecord>> {
        Ok(self.store.list_scene_comments(scene)?)
    }
    pub fn set_personal_comment_state(
        &mut self,
        comment: PersonalCommentId,
        state: &str,
    ) -> Result<()> {
        Ok(self.store.set_personal_comment_state(comment, state)?)
    }
    pub fn create_scratchpad_note(&mut self, title: &str, body: &str) -> Result<ScratchpadId> {
        Ok(self.store.create_scratchpad_note(title, body)?)
    }
    pub fn list_scratchpad_notes(&self, include_archived: bool) -> Result<Vec<ScratchpadNote>> {
        Ok(self.store.list_scratchpad_notes(include_archived)?)
    }
    pub fn update_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        title: &str,
        body: &str,
        pinned: bool,
    ) -> Result<()> {
        Ok(self.store.update_scratchpad_note(id, title, body, pinned)?)
    }
    pub fn promote_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        destination_kind: &str,
        destination_id: &str,
    ) -> Result<()> {
        Ok(self
            .store
            .promote_scratchpad_note(id, destination_kind, destination_id)?)
    }
    pub fn load_scene_text(&self, id: SceneId) -> Result<(String, String)> {
        Ok(self.store.load_text(id)?)
    }
    pub fn validate_project(&self) -> Result<()> {
        Ok(self.store.validate()?)
    }
    pub fn recover_pending_writes(&mut self) -> Result<usize> {
        Ok(self.store.recover_pending_writes()?.len())
    }
    pub fn pending_conflicts(&self) -> Result<u32> {
        Ok(self.store.pending_conflicts()?)
    }
    pub fn list_recovery_conflicts(&self) -> Result<Vec<story_storage::RecoveryConflict>> {
        Ok(self.store.list_recovery_conflicts()?)
    }
    pub fn list_content_unit_recovery_conflicts(
        &self,
    ) -> Result<Vec<story_storage::ContentUnitRecoveryConflict>> {
        Ok(self.store.list_content_unit_recovery_conflicts()?)
    }
    pub fn backup_project(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        Ok(self.store.backup_to(destination)?)
    }
    pub fn backup_project_archive(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        Ok(self.store.backup_archive_to(destination)?)
    }
    pub fn restore_project_archive(
        archive: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        Ok(ProjectStore::restore_archive_to(archive, destination)?)
    }
    pub fn restore_git_snapshot(
        project: impl AsRef<Path>,
        commit: &str,
        destination: impl AsRef<Path>,
    ) -> Result<Vec<std::path::PathBuf>> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(AppError::Domain(
                "Git restore destination already exists".into(),
            ));
        }
        let stage =
            destination.with_extension(format!("writers-ide-stage-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            let files = story_git::GitHistory::open(project)?
                .restore_files_to_new_directory(commit, &stage)?;
            ProjectStore::restore_metadata_snapshot(&stage)?;
            std::fs::rename(&stage, destination).map_err(story_storage::StorageError::from)?;
            if let Some(parent) = destination.parent() {
                std::fs::File::open(parent)
                    .and_then(|directory| directory.sync_all())
                    .map_err(story_storage::StorageError::from)?;
            }
            Ok(files)
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&stage);
        }
        result
    }
    pub fn git_snapshot(&mut self, label: &str) -> Result<Option<String>> {
        let author = self.get_project()?.author;
        Ok(self.store.with_git_snapshot_barrier(|root| {
            story_git::GitHistory::open(root)
                .and_then(|git| git.snapshot(label, &author))
                .map_err(|e| e.to_string())
        })?)
    }
    pub fn set_git_auto_snapshot_interval(&self, minutes: Option<u32>) -> Result<()> {
        let policy = story_git::AutoSnapshotPolicy::new(minutes)?;
        story_git::save_auto_snapshot_policy(self.store.project_root(), &policy)?;
        Ok(())
    }
    /// Create an interval snapshot when due. `now_unix` is injectable for deterministic tests.
    pub fn git_auto_snapshot_if_due(&mut self, now_unix: i64) -> Result<Option<String>> {
        let mut policy = story_git::load_auto_snapshot_policy(self.store.project_root())?;
        if !policy.due(now_unix) {
            return Ok(None);
        }
        let result = self.git_snapshot("Automatic project snapshot")?;
        policy.record_check(now_unix);
        story_git::save_auto_snapshot_policy(self.store.project_root(), &policy)?;
        Ok(result)
    }
    /// Restore one scene's prose from a local Git commit through the normal
    /// optimistic write journal. Current project state is snapshotted first.
    pub fn restore_scene_from_git(&mut self, commit: &str, scene: SceneId) -> Result<String> {
        let relative_path = self.store.scene_content_relative_path(scene)?;
        let relative_path = relative_path
            .to_str()
            .ok_or_else(|| AppError::Domain("scene manuscript path is not UTF-8".into()))?;
        let historical = story_git::GitHistory::open(self.store.project_root())?
            .read_file_at(commit, relative_path)?;
        let historical = String::from_utf8(historical)
            .map_err(|_| AppError::Domain("historical scene text is not UTF-8".into()))?;
        let (current, expected_hash) = self.load_scene_text(scene)?;
        if current == historical {
            return Ok(expected_hash);
        }
        let title = self.get_scene(scene)?.scene.title;
        self.git_snapshot(&format!("Before restoring scene {title} from {commit}"))?;
        self.save_scene_text(scene, Some(&expected_hash), &historical)
    }
    /// Restore one chapter/front/section content unit from a local Git commit
    /// through its optimistic write journal after snapshotting current state.
    pub fn restore_content_unit_from_git(
        &mut self,
        commit: &str,
        content_unit: ContentUnitId,
    ) -> Result<String> {
        let relative_path = self
            .store
            .content_unit_content_relative_path(content_unit)?;
        let relative_path = relative_path
            .to_str()
            .ok_or_else(|| AppError::Domain("content-unit path is not UTF-8".into()))?;
        let historical = story_git::GitHistory::open(self.store.project_root())?
            .read_file_at(commit, relative_path)?;
        let historical = String::from_utf8(historical)
            .map_err(|_| AppError::Domain("historical content-unit text is not UTF-8".into()))?;
        let (current, expected_hash) = self.load_content_unit_text(content_unit)?;
        if current == historical {
            return Ok(expected_hash);
        }
        let title = self.get_content_unit(content_unit)?.title;
        self.git_snapshot(&format!(
            "Before restoring content unit {title} from {commit}"
        ))?;
        self.save_content_unit_text(content_unit, Some(&expected_hash), &historical)
    }
    pub fn git_history(&self, limit: usize) -> Result<Vec<story_git::GitCommit>> {
        Ok(story_git::GitHistory::open(self.store.project_root())?.history(limit)?)
    }
    pub fn git_diff(&self, from: &str, to: &str, path: Option<&str>) -> Result<String> {
        Ok(story_git::GitHistory::open(self.store.project_root())?.diff(from, to, path)?)
    }
    pub fn git_search_history(
        &self,
        text: &str,
        path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<story_git::HistoricalTextChange>> {
        Ok(story_git::GitHistory::open(self.store.project_root())?
            .search_text_history(text, path, limit)?)
    }
    pub fn git_blame(
        &self,
        path: &str,
        first_line: usize,
        last_line: usize,
    ) -> Result<Vec<story_git::BlamedLine>> {
        Ok(story_git::GitHistory::open(self.store.project_root())?
            .blame(path, first_line, last_line)?)
    }
    pub fn resolve_conflict(&mut self, txid: &str, resolution: ConflictResolution) -> Result<()> {
        Ok(self.store.resolve_write_conflict(txid, resolution)?)
    }
    pub fn resolve_content_unit_conflict(&mut self, txid: &str, use_prepared: bool) -> Result<()> {
        Ok(self
            .store
            .resolve_content_unit_conflict(txid, use_prepared)?)
    }
}

#[allow(dead_code)]
fn _storage_result(_: StorageResult<()>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn emacs_tags_export_points_to_stable_symbol_index_entries_and_keeps_utf8_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Mára arrives", 0).unwrap();
        let character = app.create_character("Mára Venn").unwrap();
        app.add_character_alias(character, "Mára").unwrap();

        let definitions = app.list_project_symbols().unwrap();
        assert!(definitions.iter().any(|symbol| {
            symbol.entity_kind == "scene"
                && symbol.entity_id == scene.to_string()
                && symbol
                    .source_path
                    .as_ref()
                    .is_some_and(|path| path.starts_with("manuscript"))
        }));
        let export = app.build_emacs_tags_export().unwrap();
        assert!(
            export
                .symbols_markdown
                .contains("## character: Mára Venn [")
        );
        assert!(export.symbols_markdown.contains("## scene: Mára arrives ["));
        assert!(export.tags_file.starts_with("\u{c}\nsymbols.md,"));
        assert!(export.tags_file.contains("\u{7f}Mára\u{1}"));
        assert!(export.tags_file.contains("\u{7f}Mára Venn\u{1}"));

        let record = export
            .tags_file
            .split('\n')
            .find(|line| line.contains("\u{7f}Mára\u{1}"))
            .unwrap();
        let (_, coordinates) = record.split_once('\u{1}').unwrap();
        let (line, offset) = coordinates.split_once(',').unwrap();
        let line: usize = line.parse().unwrap();
        let offset: usize = offset.parse().unwrap();
        assert!(line > 1);
        assert!(export.symbols_markdown.is_char_boundary(offset));
        assert!(export.symbols_markdown[offset..].starts_with("## character: Mára Venn ["));

        let path = app.export_emacs_tags().unwrap();
        assert!(path.ends_with(".writers-ide/tags/TAGS"));
        assert_eq!(fs::read_to_string(path).unwrap(), export.tags_file);
    }

    #[test]
    fn user_writing_language_is_independent_and_seeds_new_projects() {
        let dir = tempfile::tempdir().unwrap();
        let preferences_path = dir.path().join("config/preferences.toml");
        let preferences = UserPreferences {
            ui_language: "en-GB".into(),
            default_writing_language: "it-IT".into(),
        };
        write_user_preferences_at(&preferences_path, &preferences).unwrap();
        assert_eq!(
            read_user_preferences_at(&preferences_path).unwrap(),
            preferences
        );
        let app = WriterApp::create_project_with_preferences(
            dir.path().join("Novel"),
            "Novel",
            &preferences,
        )
        .unwrap();
        assert_eq!(app.get_project().unwrap().language_code, "it-IT");
        assert_eq!(preferences.ui_language, "en-GB");
    }

    #[test]
    fn invalid_user_language_preferences_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.toml");
        fs::write(
            &path,
            "ui_language = 'en'\ndefault_writing_language = 'not a tag'\n",
        )
        .unwrap();
        assert!(read_user_preferences_at(&path).is_err());
    }

    #[test]
    fn event_dependency_review_uses_uncertain_story_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Events"), "Events").unwrap();
        let first = EventId::new();
        let second = EventId::new();
        app.add_story_event(&StoryEvent {
            id: first,
            title: "Message sent".into(),
            description: String::new(),
            start: StoryTime::exact(10),
            end: StoryTime::exact(12),
            source_time_text: "day 1".into(),
        })
        .unwrap();
        let mut second_event = StoryEvent {
            id: second,
            title: "Reply received".into(),
            description: String::new(),
            start: StoryTime::new(Some(20), Some(15), Some(25), TimePrecision::Bounded).unwrap(),
            end: StoryTime::unknown(),
            source_time_text: "sometime later".into(),
        };
        app.add_story_event(&second_event).unwrap();
        app.add_event_dependency(&StoryEventDependency {
            predecessor: first,
            successor: second,
            kind: "precedes".into(),
            notes: "Communication order".into(),
        })
        .unwrap();
        let findings = app.inspect_event_dependencies().unwrap();
        assert_eq!(findings[0].1.status, story_analysis::InspectionStatus::Pass);
        second_event.start =
            StoryTime::new(Some(12), Some(11), Some(13), TimePrecision::Bounded).unwrap();
        app.update_story_event(&second_event).unwrap();
        assert_eq!(
            app.inspect_event_dependencies().unwrap()[0].1.status,
            story_analysis::InspectionStatus::Unknown
        );
    }
    #[test]
    fn application_vertical_slice_survives_reopen_and_relocation() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Novel");
        let mut app = WriterApp::create_project(&p, "Novel").unwrap();
        let first = app.create_chapter(None, "First", 0).unwrap();
        let second = app.create_chapter(None, "Second", 1).unwrap();
        let scene = app.create_scene(first, "Arrival", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "The gate opened. Mara stepped through.")
            .unwrap();
        let mut row = app.get_scene(scene).unwrap().scene;
        row.synopsis = "Arrival and choice".into();
        row.status = SceneStatus::DraftComplete;
        row.target_words = Some(14);
        app.update_scene_metadata(&row).unwrap();
        app.move_scene(scene, second, 0).unwrap();
        drop(app);
        let moved = d.path().join("Relocated");
        fs::rename(&p, &moved).unwrap();
        let app = WriterApp::open_project(&moved).unwrap();
        app.validate_project().unwrap();
        let got = app.get_scene(scene).unwrap();
        assert_eq!(got.chapter_id, Some(second));
        assert_eq!(got.scene.synopsis, "Arrival and choice");
        assert_eq!(got.scene.status, SceneStatus::DraftComplete);
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "The gate opened. Mara stepped through."
        );
        let pipeline = app.processing_input().unwrap();
        assert_eq!(pipeline.scenes[0].id, scene.to_string());
        assert_eq!(
            pipeline.scenes[0].text,
            "The gate opened. Mara stepped through."
        );
        assert!(
            app.run_processing()
                .unwrap()
                .iter()
                .all(|stage| stage.status == story_pipeline::StageStatus::Complete)
        );
        assert!(
            app.run_processing()
                .unwrap()
                .iter()
                .all(|stage| stage.status == story_pipeline::StageStatus::Cached)
        );
    }

    #[test]
    fn nonfiction_chapter_prose_is_a_content_unit_not_a_scene() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Field Guide");
        let mut app = WriterApp::create_project_from_template(
            &path,
            "Field Guide",
            BuiltinTemplate::BlankNonfiction,
        )
        .unwrap();
        let chapter = app.create_chapter(None, "Introduction", 0).unwrap();
        let body = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Introduction",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "This is direct chapter prose.")
            .unwrap();
        assert!(app.list_scenes(false).unwrap().is_empty());
        assert_eq!(
            app.get_project().unwrap().template_id.as_deref(),
            Some("blank-nonfiction")
        );
        let input = app.processing_input().unwrap();
        assert!(input.scenes.is_empty());
        assert_eq!(input.content_units.len(), 1);
        assert!(input.content_units[0].text.contains("direct chapter prose"));
        let (compiled, _) = story_pipeline::compile_ir(&input).render_markdown();
        assert_eq!(
            compiled,
            "---\nlang: en-GB\n---\n\n## Introduction\n\nThis is direct chapter prose.\n\n"
        );
    }

    #[test]
    fn git_snapshots_pair_manuscript_and_metadata_and_skip_unchanged_projects() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "First draft.")
            .unwrap();
        let first = app.git_snapshot("First draft").unwrap().unwrap();
        assert!(app.git_snapshot("No changes").unwrap().is_none());
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Revised draft.")
            .unwrap();
        let second = app.git_snapshot("Revision").unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(app.git_history(10).unwrap().len(), 2);
        assert!(
            app.git_diff(&first, &second, Some(&format!("manuscript/{scene}.md")))
                .unwrap()
                .contains("Revised draft")
        );
        assert!(project.join("snapshots/database.sql").exists());
        assert!(app.git_diff(&first, &second, Some("story.db")).is_err());
    }

    #[test]
    fn automatic_git_snapshots_run_only_when_due_and_skip_unchanged_projects() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Novel"), "Novel").unwrap();
        app.set_git_auto_snapshot_interval(Some(1)).unwrap();
        assert!(app.git_auto_snapshot_if_due(1_000).unwrap().is_some());
        assert!(app.git_auto_snapshot_if_due(1_059).unwrap().is_none());
        assert!(app.git_auto_snapshot_if_due(1_060).unwrap().is_none());
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Changed prose.")
            .unwrap();
        assert!(app.git_auto_snapshot_if_due(1_119).unwrap().is_none());
        assert!(app.git_auto_snapshot_if_due(1_120).unwrap().is_some());
        app.set_git_auto_snapshot_interval(None).unwrap();
        assert!(app.git_auto_snapshot_if_due(10_000).unwrap().is_none());
    }

    #[test]
    fn git_restores_one_scene_through_journal_and_snapshots_current_prose() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Original version.")
            .unwrap();
        let original = app.git_snapshot("Original prose").unwrap().unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Current revision.")
            .unwrap();

        app.restore_scene_from_git(&original, scene).unwrap();
        assert_eq!(app.load_scene_text(scene).unwrap().0, "Original version.");
        let history = app.git_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(
            history[0]
                .subject
                .contains("Before restoring scene Opening")
        );
        assert!(
            app.git_diff(
                &history[1].id,
                &history[0].id,
                Some(&format!("manuscript/{scene}.md"))
            )
            .unwrap()
            .contains("Current revision.")
        );
        assert!(app.restore_scene_from_git("not-a-commit", scene).is_err());
    }

    #[test]
    fn git_restores_one_nonfiction_content_unit_through_its_journal() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Book");
        let mut app =
            WriterApp::create_project_with_type(&project, "Field Guide", BookType::Nonfiction)
                .unwrap();
        let chapter = app.create_chapter(None, "Introduction", 0).unwrap();
        let body = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Introduction prose",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "Original chapter text.")
            .unwrap();
        let original = app.git_snapshot("Original chapter body").unwrap().unwrap();
        let (_, hash) = app.load_content_unit_text(body).unwrap();
        app.save_content_unit_text(body, Some(&hash), "Current chapter revision.")
            .unwrap();

        app.restore_content_unit_from_git(&original, body).unwrap();
        assert_eq!(
            app.load_content_unit_text(body).unwrap().0,
            "Original chapter text."
        );
        let history = app.git_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].subject.contains("Before restoring content unit"));
        assert!(
            app.restore_content_unit_from_git("not-a-commit", body)
                .is_err()
        );
    }

    #[test]
    fn git_snapshot_restores_complete_project_to_new_relocatable_directory() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Original"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter One", 0).unwrap();
        let scene = app.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "A restored passage.")
            .unwrap();
        let commit = app.git_snapshot("Initial complete state").unwrap().unwrap();

        let restored_path = root.path().join("Restored");
        let files =
            WriterApp::restore_git_snapshot(app.store.project_root(), &commit, &restored_path)
                .unwrap();
        assert!(files.iter().any(|path| path == Path::new("project.toml")));
        let restored = WriterApp::open_project(&restored_path).unwrap();
        assert_eq!(restored.get_project().unwrap().title, "Novel");
        assert_eq!(
            restored.load_scene_text(scene).unwrap().0,
            "A restored passage."
        );

        let duplicate =
            WriterApp::restore_git_snapshot(app.store.project_root(), &commit, &restored_path);
        assert!(duplicate.is_err());
        assert_eq!(
            restored.load_scene_text(scene).unwrap().0,
            "A restored passage."
        );
    }

    #[test]
    fn compile_artifact_exposes_portable_markdown_and_source_map() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "A Novel").unwrap();
        app.set_project_language("it-IT").unwrap();
        let chapter = app.create_chapter(None, "Opening", 0).unwrap();
        let scene = app.create_scene(chapter, "Arrival", 0).unwrap();
        app.set_scene_language(scene, Some("fr-FR")).unwrap();
        let chapter_prose = app
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Chapter prose",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        app.set_content_unit_language(chapter_prose, Some("ko-KR"))
            .unwrap();
        let (_, prose_hash) = app.load_content_unit_text(chapter_prose).unwrap();
        app.save_content_unit_text(chapter_prose, Some(&prose_hash), "한국어")
            .unwrap();
        let (_, expected) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&expected), "Élodie arrives.")
            .unwrap();
        app.tag_passage_language(scene, 8, 15, "it-IT").unwrap();
        let artifact = app
            .compile_artifact(story_document::CompileFormat::Markdown, false)
            .unwrap();
        let markdown = String::from_utf8(artifact.bytes).unwrap();
        assert!(markdown.starts_with("---\nlang: it-IT\n---"));
        assert!(markdown.contains("::: {lang=fr-FR}"));
        assert!(markdown.contains("::: {lang=ko-KR}"));
        assert!(markdown.contains("## Opening"));
        assert!(markdown.contains("### Arrival"));
        assert!(markdown.contains("Élodie [arrives]"));
        assert!(markdown.contains("[arrives]{lang=it-IT}"));
        assert_eq!(artifact.source_map.len(), 2);
        assert!(
            artifact
                .source_map
                .iter()
                .any(|entry| entry.source_entity_id == scene.to_string())
        );
        let plain = app
            .compile_artifact(story_document::CompileFormat::PlainText, false)
            .unwrap();
        let plain = String::from_utf8(plain.bytes).unwrap();
        assert!(plain.contains("Élodie arrives."));
        assert!(!plain.contains("{lang=it-IT}"));
    }

    #[test]
    fn staged_manuscript_import_creates_scenes_and_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Imported"), "Imported").unwrap();
        let staging = story_import::ManuscriptStaging {
            source_type: story_import::SourceType::Rtf,
            source_path: "/fixtures/source.rtf".into(),
            source_fingerprint: "a".repeat(64),
            title: "Source book".into(),
            units: vec![story_import::ManuscriptUnit {
                source_document_id: "source.rtf:unit:0".into(),
                title: "Opening scene".into(),
                body_markdown: "Imported prose.".into(),
                unsupported: vec!["Formatting needs review".into()],
            }],
            warnings: Vec::new(),
            unsupported: vec!["Some formatting is unsupported".into()],
        };
        let (run, scenes) = app.import_staged_manuscript(&staging).unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(app.load_scene_text(scenes[0]).unwrap().0, "Imported prose.");
        let summary = app.get_import_run_summary(run).unwrap();
        assert_eq!(summary.source_items, 1);
        assert_eq!(summary.provenance_records, 2);
        assert_eq!(summary.status, "incomplete");
    }

    #[test]
    fn editorial_merge_saves_disjoint_changes_and_preserves_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "One.\nTwo.\nThree.\n";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "One.\nTwo local.\nThree.\n")
            .unwrap();
        let merged = app
            .merge_review_text(scene, base, "One.\nTwo.\nThree external.\n")
            .unwrap();
        assert_eq!(
            merged.merged_text.as_deref(),
            Some("One.\nTwo local.\nThree external.\n")
        );
        let conflict = app
            .merge_review_text(scene, base, "One.\nTwo reviewed.\nThree.\n")
            .unwrap();
        assert!(conflict.merged_text.is_none());
        assert_eq!(
            app.load_scene_text(scene).unwrap().0,
            "One.\nTwo local.\nThree external.\n"
        );
        let resolved = app
            .resolve_review_text(
                scene,
                base,
                "One.\nTwo reviewed.\nThree.\n",
                &[story_import::MergeConflictChoice::External],
            )
            .unwrap();
        assert_eq!(resolved, "One.\nTwo reviewed.\nThree.\n");
    }

    #[test]
    fn docx_revision_disposition_merges_without_modifying_the_review_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "The old door.\n";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), base).unwrap();
        let docx = dir.path().join("review.docx");
        let file = std::fs::File::create(&docx).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("word/document.xml", zip::write::FileOptions::default())
            .unwrap();
        archive
            .write_all(b"<w:document><w:body><w:p><w:r><w:t>The </w:t></w:r><w:del w:id=\"1\"><w:r><w:delText>old</w:delText></w:r></w:del><w:ins w:id=\"2\"><w:r><w:t>new</w:t></w:r></w:ins><w:r><w:t> door.</w:t></w:r></w:p></w:body></w:document>")
            .unwrap();
        archive.finish().unwrap();
        let source = std::fs::read(&docx).unwrap();

        let merged = app.merge_docx_review(scene, base, &docx, true).unwrap();
        assert_eq!(merged.merged_text.as_deref(), Some("The new door."));
        assert_eq!(app.load_scene_text(scene).unwrap().0, "The new door.");
        assert_eq!(std::fs::read(&docx).unwrap(), source);
    }

    #[test]
    fn diagnostics_config_is_project_local_typed_and_used_by_application() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let app = WriterApp::create_project(&project, "Novel").unwrap();
        let config_path = app.write_default_proofreading_config().unwrap();
        assert!(config_path.starts_with(&project));
        assert!(app.write_default_proofreading_config().is_err());
        let default_findings = app.proofread_text("Mara was followed.").unwrap();
        assert!(default_findings.is_empty());
        std::fs::write(
            &config_path,
            "version=1\n[[rules]]\nid='passive'\nenabled=true\nkind='regex'\npattern='\\bwas\\s+\\w+ed\\b'\nseverity='warning'\nmessage='Review passive phrasing'\n",
        ).unwrap();
        let configured = app.proofread_text("Mara was followed.").unwrap();
        assert_eq!(configured.len(), 1);
        assert_eq!(configured[0].category, "passive");
        assert!(configured[0].byte_range.end <= "Mara was followed.".len());
    }

    #[test]
    fn optional_processing_transclusion_expands_a_derived_compile_view() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let source = app.create_scene(chapter, "Source", 0).unwrap();
        let target = app.create_scene(chapter, "Target", 1).unwrap();
        let (_, source_hash) = app.load_scene_text(source).unwrap();
        app.save_scene_text(source, Some(&source_hash), "Source passage.")
            .unwrap();
        let (_, target_hash) = app.load_scene_text(target).unwrap();
        app.save_scene_text(target, Some(&target_hash), "Included passage.")
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(source),
            source_anchor: None,
            target: EntityRef::Scene(target),
            target_anchor: None,
            kind: LinkKind::Transcludes,
            label: String::new(),
        })
        .unwrap();
        let research = app
            .create_research_item(
                "Background document",
                "Document body included in the compilation.",
                "text/markdown",
                None,
                None,
            )
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(target),
            source_anchor: None,
            target: EntityRef::Research(research),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: "appendix excerpt".into(),
        })
        .unwrap();
        let source_bytes = b"Reference body is read-only.".to_vec();
        let reference = story_reading::ReferenceDocument {
            title: "Imported reference".into(),
            media_type: "text/plain".into(),
            sha256: format!("{:x}", Sha256::digest(&source_bytes)),
            extracted_text: "Reference body is read-only.".into(),
            source_bytes,
            language_code: None,
        };
        let reading = app
            .import_reading_document(&reference, "fixture:read-only")
            .unwrap();
        app.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(target),
            source_anchor: None,
            target: EntityRef::Reading(reading),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: "quoted source".into(),
        })
        .unwrap();

        app.run_processing().unwrap();
        let ordinary = fs::read_to_string(project_path.join("exports/compiled.md")).unwrap();
        assert_eq!(ordinary.matches("Included passage.").count(), 1);

        let reports = app.run_processing_with_transclusions().unwrap();
        assert!(reports[0].message.contains("inclusions expanded"));
        let expanded = fs::read_to_string(project_path.join("exports/compiled.md")).unwrap();
        assert_eq!(expanded.matches("Included passage.").count(), 2);
        assert!(expanded.contains("Source passage.\n\n---\n\nIncluded passage."));
        assert!(expanded.contains("Document body included in the compilation."));
        assert!(expanded.contains("Reference body is read-only."));
        let reading_record = app.get_reading_document(reading).unwrap();
        assert_eq!(
            fs::read(project_path.join(reading_record.relative_path)).unwrap(),
            reference.source_bytes
        );
        assert_eq!(app.load_scene_text(source).unwrap().0, "Source passage.");
        assert_eq!(app.load_scene_text(target).unwrap().0, "Included passage.");
    }

    #[test]
    fn analytics_reads_relationships_without_mutating_story_data() {
        let d = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(d.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let _second = app.create_scene(chapter, "Second", 1).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(first, character, ParticipationRole::Pov)
            .unwrap();
        let plot = app.create_plotline("A thread").unwrap();
        app.set_scene_plotlines(
            first,
            &[ScenePlotline {
                plotline_id: plot,
                role: "primary".into(),
                share_basis_points: None,
            }],
            WordAttributionPolicy::Overlap,
        )
        .unwrap();
        let location = app.create_location("Harbor").unwrap();
        let mut scene_metadata = app.get_scene(first).unwrap().scene;
        scene_metadata.start_location = Some(location);
        app.update_scene_metadata(&scene_metadata).unwrap();
        let event = app.add_event("The signal", "day 1").unwrap();
        app.link_event(first, event, "depicts").unwrap();
        let before = app
            .list_scenes(false)
            .unwrap()
            .iter()
            .map(|s| s.scene.id)
            .collect::<Vec<_>>();
        let data = app.analytics_dataset().unwrap();
        let after = app
            .list_scenes(false)
            .unwrap()
            .iter()
            .map(|s| s.scene.id)
            .collect::<Vec<_>>();
        assert_eq!(before, after);
        assert_eq!(data[0].characters[0].name, "Mara");
        assert_eq!(data[0].plotlines[0].name, "A thread");
        assert_eq!(data[0].locations[0].name, "Harbor");
        assert_eq!(data[0].events[0].name, "The signal");
    }

    #[test]
    fn location_incompatibility_rules_are_persisted_and_evaluated_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let first = app.create_scene(chapter, "At the bridge", 0).unwrap();
        let second = app.create_scene(chapter, "In the vault", 1).unwrap();
        let character = app.create_character("Mara").unwrap();
        app.link_character(first, character, ParticipationRole::Major)
            .unwrap();
        app.link_character(second, character, ParticipationRole::Major)
            .unwrap();
        let bridge = app.create_location("Bridge").unwrap();
        let vault = app.create_location("Vault").unwrap();
        for (scene_id, location, start, end) in [(first, bridge, 0, 10), (second, vault, 5, 15)] {
            let mut scene = app.get_scene(scene_id).unwrap().scene;
            scene.start_location = Some(location);
            scene.start_time = StoryTime::exact(start);
            scene.end_time = StoryTime::exact(end);
            app.update_scene_metadata(&scene).unwrap();
        }
        let rule = LocationIncompatibilityRule {
            id: ContinuityRuleId::new(),
            first_location: bridge,
            second_location: vault,
            description: "Mara cannot be at both locations at once".into(),
            enabled: true,
        };
        app.add_location_incompatibility_rule(&rule).unwrap();
        assert_eq!(
            app.inspect_location_incompatibilities().unwrap()[0].status,
            story_analysis::LocationRuleFindingStatus::DefiniteConflict
        );
        let mut disabled = rule.clone();
        disabled.enabled = false;
        app.update_location_incompatibility_rule(&disabled).unwrap();
        assert!(app.inspect_location_incompatibilities().unwrap().is_empty());
        app.update_location_incompatibility_rule(&rule).unwrap();
        drop(app);
        let reopened = WriterApp::open_project(project).unwrap();
        let reopened_rules = reopened.list_location_incompatibility_rules().unwrap();
        assert_eq!(reopened_rules.len(), 1);
        assert_eq!(reopened_rules[0].id, rule.id);
        assert!(
            (reopened_rules[0].first_location == bridge
                && reopened_rules[0].second_location == vault)
                || (reopened_rules[0].first_location == vault
                    && reopened_rules[0].second_location == bridge)
        );
        assert_eq!(
            reopened.inspect_location_incompatibilities().unwrap().len(),
            1
        );
    }

    #[test]
    fn extracted_claims_require_current_exact_evidence_before_authoritative_promotion() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let scene = app.create_scene(chapter, "Discovery", 0).unwrap();
        let source = "Mara knows the ward opens at dawn.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), source).unwrap();
        let proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "character_knowledge".into(),
                    name: "Mara".into(),
                    claim: "Mara knows the ward opens at dawn.".into(),
                    evidence_quote: "Mara knows the ward opens at dawn.".into(),
                    approximate_story_time: Some("day 1 morning".into()),
                }],
            },
        );
        let mut invalid_evidence = proposal.clone();
        invalid_evidence.claims[0].evidence_start_byte = Some(1);
        assert!(app.approve_extraction_claim(&invalid_evidence, 0).is_err());
        let id = app.approve_extraction_claim(&proposal, 0).unwrap();
        assert_eq!(app.approve_extraction_claim(&proposal, 0).unwrap(), id);
        let approved = app.list_approved_knowledge_claims().unwrap();
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].id, id);
        assert_eq!(
            approved[0].evidence_quote,
            "Mara knows the ward opens at dawn."
        );
        assert_eq!(approved[0].source_id, scene.to_string());
        assert_eq!(approved[0].model, "fixture-model");
        assert!(
            app.search_project("character_knowledge dawn", 20)
                .unwrap()
                .iter()
                .any(|hit| hit.entity_kind == "knowledge_claim" && hit.entity_id == id.to_string())
        );

        let (_, latest_hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&latest_hash), "Mara learned this yesterday.")
            .unwrap();
        assert!(app.approve_extraction_claim(&proposal, 0).is_err());
        assert_eq!(app.list_approved_knowledge_claims().unwrap().len(), 1);

        let reading_path = dir.path().join("reference.txt");
        std::fs::write(&reading_path, "The bell sounds at noon.").unwrap();
        let reference = story_reading::import_reference(&reading_path).unwrap();
        let reading_id = app
            .import_reading_document(&reference, reading_path.to_str().unwrap())
            .unwrap();
        let reading_proposal = story_ollama::attach_provenance(
            "reading",
            &reading_id.to_string(),
            &reference.extracted_text,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "event".into(),
                    name: "Noon bell".into(),
                    claim: "A bell sounds at noon.".into(),
                    evidence_quote: "The bell sounds at noon.".into(),
                    approximate_story_time: Some("noon".into()),
                }],
            },
        );
        let reading_claim = app.approve_extraction_claim(&reading_proposal, 0).unwrap();
        assert!(
            app.list_approved_knowledge_claims()
                .unwrap()
                .iter()
                .any(|record| record.id == reading_claim && record.source_kind == "reading")
        );
        assert_eq!(
            std::fs::read(&reading_path).unwrap(),
            reference.source_bytes
        );
        drop(app);
        let app = WriterApp::open_project(dir.path().join("Novel")).unwrap();
        let persisted = app.list_approved_knowledge_claims().unwrap();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[1].source_kind, "reading");
    }

    #[test]
    fn provisional_claims_can_be_reviewed_and_dismissed_without_losing_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "One", 0).unwrap();
        let scene = app.create_scene(chapter, "Discovery", 0).unwrap();
        let source = "Mara found the silver key.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), source).unwrap();
        let mut proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "object".into(),
                    name: "Silver key".into(),
                    claim: "Mara found the key.".into(),
                    evidence_quote: "Mara found the silver key.".into(),
                    approximate_story_time: None,
                }],
            },
        );
        let filename = "review.json";
        let path = app
            .save_extraction_proposal(filename, &serde_json::to_vec(&proposal).unwrap())
            .unwrap();
        let (original, fingerprint) = app.load_extraction_proposal_file(&path).unwrap();
        let review: story_ollama::ProvisionalExtraction =
            serde_json::from_slice(&original).unwrap();
        assert!(!review.claims[0].approved);
        assert!(!review.claims[0].dismissed);
        assert!(review.claims[0].evidence_start_byte.is_some());

        app.dismiss_extraction_claim(&mut proposal, 0).unwrap();
        assert!(proposal.claims[0].dismissed);
        app.update_extraction_proposal_file(
            &path,
            &fingerprint,
            &serde_json::to_vec(&proposal).unwrap(),
        )
        .unwrap();
        let (updated, _) = app.load_extraction_proposal_file(&path).unwrap();
        let persisted: story_ollama::ProvisionalExtraction =
            serde_json::from_slice(&updated).unwrap();
        assert!(persisted.claims[0].dismissed);
        assert_eq!(
            persisted.claims[0].claim.evidence_quote,
            "Mara found the silver key."
        );
        assert_eq!(app.load_scene_text(scene).unwrap().0, source);
        assert!(app.approve_extraction_claim(&persisted, 0).is_err());
        assert!(
            app.list_extraction_proposal_files()
                .unwrap()
                .contains(&path)
        );

        proposal.claims[0].approved = true;
        assert!(app.dismiss_extraction_claim(&mut proposal, 0).is_err());
    }

    #[test]
    fn character_profile_use_cases_validate_and_persist_fields_and_aliases() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let id = app.create_character("Mara Venn").unwrap();
        app.set_character_field(id, "preferred_name", "Mara")
            .unwrap();
        app.set_character_field(id, "goals", "Find the atlas")
            .unwrap();
        app.add_character_alias(id, "The Cartographer").unwrap();
        assert!(app.add_character_alias(id, "the cartographer").is_err());
        app.remove_character_alias(id, "THE CARTOGRAPHER").unwrap();
        assert!(app.remove_character_alias(id, "unknown").is_err());
        assert!(app.set_character_field(id, "unknown", "value").is_err());
        let character = app.get_character(id).unwrap();
        assert_eq!(character.preferred_name, "Mara");
        assert_eq!(character.goals, "Find the atlas");
        assert!(character.aliases.is_empty());
        drop(app);
        let app = WriterApp::open_project(dir.path().join("Novel")).unwrap();
        assert_eq!(app.get_character(id).unwrap(), character);
    }

    #[test]
    fn planning_board_svg_exports_cards_connectors_and_escaped_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Board"), "Board").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first_scene = app.create_scene(chapter, "First", 0).unwrap();
        let second_scene = app.create_scene(chapter, "Second", 1).unwrap();
        let board = app.create_planning_board("<Idea & map>\u{1}").unwrap();
        let first = app
            .add_planning_board_item(board, "scene", &first_scene.to_string(), 0.0, 0.0, "A & B")
            .unwrap();
        let second = app
            .add_planning_board_item(
                board,
                "scene",
                &second_scene.to_string(),
                42.0,
                0.0,
                "Ending",
            )
            .unwrap();
        app.connect_planning_board_items(board, first, second, "<leads>")
            .unwrap();
        let svg = app.planning_board_svg(board).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("&lt;Idea &amp; map&gt;\u{fffd}"));
        assert!(!svg.contains('\u{1}'));
        assert!(svg.contains("A &amp; B"));
        assert!(svg.contains("&lt;leads&gt;"));
        assert!(svg.contains("marker-end=\"url(#arrow)\""));
    }

    #[test]
    fn confirmed_mention_appearances_are_cross_scene_and_do_not_create_presence() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Mentions"), "Mentions").unwrap();
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
            let records = app.list_scene_mentions(scene).unwrap();
            let mention = records
                .iter()
                .find(|mention| mention.quote == "Silver Key")
                .unwrap();
            app.confirm_scene_mention(mention.id, MentionEntityKind::Object, &object.to_string())
                .unwrap();
        }
        let appearances = app
            .confirmed_entity_mention_appearances(MentionEntityKind::Object, &object.to_string())
            .unwrap();
        assert_eq!(
            appearances
                .iter()
                .map(|item| item.scene_id)
                .collect::<Vec<_>>(),
            [first, second]
        );
        let first_analysis = app
            .analytics_dataset()
            .unwrap()
            .into_iter()
            .find(|row| row.scene_id == first.to_string())
            .unwrap();
        assert!(first_analysis.characters.is_empty());
    }

    #[test]
    fn assembly_compile_uses_alternate_order_and_exclusion_without_mutating_canonical_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        for (scene, content) in [
            (first, "First canonical prose."),
            (second, "Second canonical prose."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), content).unwrap();
        }
        let assembly = app.create_assembly("Audio Edition").unwrap();
        let front_matter = app
            .create_content_unit(ContentUnitKind::FrontMatter, "Dedication", None, 0, None)
            .unwrap();
        let (_, front_hash) = app.load_content_unit_text(front_matter).unwrap();
        app.save_content_unit_text(front_matter, Some(&front_hash), "For the readers.")
            .unwrap();
        app.add_assembly_content_unit(assembly, front_matter, 0)
            .unwrap();
        let first_item = app.add_assembly_scene(assembly, first, 1).unwrap();
        let second_item = app.add_assembly_scene(assembly, second, 2).unwrap();
        app.move_assembly_item(assembly, second_item, 1).unwrap();
        app.set_assembly_item_included(first_item, false).unwrap();
        let compiled = app.compile_assembly_markdown(assembly).unwrap();
        assert!(compiled.contains("Audio Edition"));
        assert!(compiled.contains("For the readers."));
        assert!(compiled.contains("Second canonical prose."));
        assert!(!compiled.contains("First canonical prose."));
        assert!(
            compiled.find("For the readers.").unwrap()
                < compiled.find("Second canonical prose.").unwrap()
        );
        assert_eq!(
            app.list_scenes(false)
                .unwrap()
                .into_iter()
                .map(|scene| scene.scene.id)
                .collect::<Vec<_>>(),
            [first, second]
        );
    }
}
