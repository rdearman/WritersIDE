//! Dependency-aware, local processing of a manuscript snapshot.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use story_analysis::{
    CharacterChronologyStatus, CharacterOnScene, PlotlineOnScene, SceneAnalysisInput,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("processing I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("processing cache: {0}")]
    Cache(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct PipelineCharacter {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PipelineScene {
    pub id: String,
    pub chapter_id: Option<String>,
    pub title: String,
    pub text: String,
    /// Resolved inclusion text. This is a derived compilation view; `text` remains canonical.
    pub expanded_text: Option<String>,
    /// Ordered source documents used to build an inclusion-aware semantic IR.
    pub expanded_passages: Option<Vec<PipelinePassage>>,
    pub target_words: Option<u64>,
    pub characters: Vec<PipelineCharacter>,
    pub plotlines: Vec<String>,
    pub start: story_core::StoryTime,
    pub end: story_core::StoryTime,
}

#[derive(Clone, Debug)]
pub struct PipelineGroup {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: story_document::StructuralKind,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct PipelineContentUnit {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: story_document::ContentUnitKind,
    pub title: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct PipelinePassage {
    pub source_entity_id: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct PipelineInput {
    pub title: String,
    pub target_words: Option<u64>,
    pub expand_transclusions: bool,
    pub groups: Vec<PipelineGroup>,
    pub content_units: Vec<PipelineContentUnit>,
    /// Ordered in manuscript order; chronology remains on each scene independently.
    pub scenes: Vec<PipelineScene>,
    pub revision_summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    Compile,
    WordCount,
    PlotlineAnalysis,
    CharacterAnalysis,
    Continuity,
    Export,
    RevisionReport,
}

impl Stage {
    pub const ALL: [Self; 7] = [
        Self::Compile,
        Self::WordCount,
        Self::PlotlineAnalysis,
        Self::CharacterAnalysis,
        Self::Continuity,
        Self::Export,
        Self::RevisionReport,
    ];

    pub fn parse_name(name: &str) -> Option<Self> {
        match name {
            "compile" => Some(Self::Compile),
            "word-count" => Some(Self::WordCount),
            "plotline-analysis" => Some(Self::PlotlineAnalysis),
            "character-analysis" => Some(Self::CharacterAnalysis),
            "continuity" => Some(Self::Continuity),
            "export" => Some(Self::Export),
            "revision-report" => Some(Self::RevisionReport),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Compile => "compile",
            Self::WordCount => "word-count",
            Self::PlotlineAnalysis => "plotline-analysis",
            Self::CharacterAnalysis => "character-analysis",
            Self::Continuity => "continuity",
            Self::Export => "export",
            Self::RevisionReport => "revision-report",
        }
    }

    fn dependencies(self) -> &'static [Stage] {
        use Stage::*;
        match self {
            Compile => &[],
            WordCount | Export => &[Compile],
            PlotlineAnalysis | CharacterAnalysis | Continuity | RevisionReport => &[],
        }
    }
}

/// Portable project-local selection of processing stages for repeatable automation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProcessingRecipe {
    pub version: u16,
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub expand_transclusions: bool,
}

impl ProcessingRecipe {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "unsupported processing recipe version {}",
                self.version
            ));
        }
        if self.stages.is_empty() {
            return Err("processing recipe must select at least one stage".into());
        }
        let mut stages = self.stages.clone();
        stages.sort_unstable();
        stages.dedup();
        if stages.len() != self.stages.len() {
            return Err("processing recipe contains duplicate stages".into());
        }
        Ok(())
    }
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let recipe: Self = toml::from_str(source).map_err(|error| error.to_string())?;
        recipe.validate()?;
        Ok(recipe)
    }
    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageStatus {
    Complete,
    Cached,
    Failed,
    Skipped,
    NotSelected,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageReport {
    pub stage: Stage,
    pub status: StageStatus,
    pub fingerprint: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Cache {
    stages: BTreeMap<Stage, CacheEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CacheEntry {
    fingerprint: String,
    artifact_hash: String,
    relative_path: String,
}

pub struct PipelineRunner {
    root: PathBuf,
    cache_path: PathBuf,
}

impl PipelineRunner {
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        let root = project_root.as_ref().to_path_buf();
        let cache_path = root.join(".writers-ide").join("pipeline-cache.json");
        Self { root, cache_path }
    }

    pub fn run(&self, input: &PipelineInput) -> Result<Vec<StageReport>, PipelineError> {
        self.run_selected(input, &Stage::ALL)
    }

    /// Runs the requested stages plus their transitive dependencies. Other stages are reported
    /// as NotSelected, making partial processing explicit to CLI and UI callers.
    pub fn run_selected(
        &self,
        input: &PipelineInput,
        requested: &[Stage],
    ) -> Result<Vec<StageReport>, PipelineError> {
        let selected = dependency_closure(requested);
        fs::create_dir_all(self.cache_path.parent().expect("cache has parent"))?;
        let mut cache: Cache = match fs::read(&self.cache_path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Cache::default(),
            Err(error) => return Err(error.into()),
        };
        let mut outputs = BTreeMap::<Stage, String>::new();
        let mut reports = Vec::new();
        for stage in Stage::ALL {
            if !selected.contains(&stage) {
                reports.push(StageReport {
                    stage,
                    status: StageStatus::NotSelected,
                    fingerprint: None,
                    message: "not requested or required by a requested stage".into(),
                });
                continue;
            }
            if stage
                .dependencies()
                .iter()
                .any(|dependency| !outputs.contains_key(dependency))
            {
                reports.push(StageReport {
                    stage,
                    status: StageStatus::Skipped,
                    fingerprint: None,
                    message: "dependency stage did not complete".into(),
                });
                continue;
            }
            let fingerprint = stage_fingerprint(stage, input, &outputs);
            let relative_path = artifact_path(stage).to_owned();
            let output_path = self.root.join(&relative_path);
            if let Some(entry) = cache
                .stages
                .get(&stage)
                .filter(|e| e.fingerprint == fingerprint && e.relative_path == relative_path)
                && let Ok(bytes) = fs::read(&output_path)
                && hash(&bytes) == entry.artifact_hash
            {
                outputs.insert(stage, String::from_utf8_lossy(&bytes).into_owned());
                reports.push(StageReport {
                    stage,
                    status: StageStatus::Cached,
                    fingerprint: Some(fingerprint),
                    message: if stage == Stage::Compile && input.expand_transclusions {
                        "inputs unchanged; verified cached artifact with inclusions expanded".into()
                    } else {
                        "inputs unchanged; verified cached artifact".into()
                    },
                });
                continue;
            }
            match render_stage(stage, input, &outputs) {
                Ok(text) => {
                    let bytes = text.as_bytes();
                    if let Err(error) = atomic_write(&output_path, bytes) {
                        cache.stages.remove(&stage);
                        reports.push(StageReport {
                            stage,
                            status: StageStatus::Failed,
                            fingerprint: Some(fingerprint),
                            message: format!("could not persist artifact: {error}"),
                        });
                        continue;
                    }
                    let artifact_hash = hash(bytes);
                    cache.stages.insert(
                        stage,
                        CacheEntry {
                            fingerprint: fingerprint.clone(),
                            artifact_hash,
                            relative_path,
                        },
                    );
                    outputs.insert(stage, text);
                    reports.push(StageReport {
                        stage,
                        status: StageStatus::Complete,
                        fingerprint: Some(fingerprint),
                        message: if stage == Stage::Compile && input.expand_transclusions {
                            "completed with scene inclusions expanded".into()
                        } else {
                            "completed".into()
                        },
                    });
                    atomic_write(&self.cache_path, &serde_json::to_vec_pretty(&cache)?)?;
                }
                Err(message) => reports.push(StageReport {
                    stage,
                    status: StageStatus::Failed,
                    fingerprint: Some(fingerprint),
                    message,
                }),
            }
        }
        Ok(reports)
    }
}

fn dependency_closure(requested: &[Stage]) -> std::collections::BTreeSet<Stage> {
    fn include(stage: Stage, selected: &mut std::collections::BTreeSet<Stage>) {
        if selected.insert(stage) {
            for dependency in stage.dependencies() {
                include(*dependency, selected);
            }
        }
    }
    let mut selected = std::collections::BTreeSet::new();
    for stage in requested {
        include(*stage, &mut selected);
    }
    selected
}

fn artifact_path(stage: Stage) -> &'static str {
    match stage {
        Stage::Export => "exports/compiled.md",
        _ => match stage {
            Stage::Compile => ".writers-ide/processing/compiled.md",
            Stage::WordCount => ".writers-ide/processing/word-count.txt",
            Stage::PlotlineAnalysis => ".writers-ide/processing/plotlines.txt",
            Stage::CharacterAnalysis => ".writers-ide/processing/characters.txt",
            Stage::Continuity => ".writers-ide/processing/continuity.txt",
            Stage::RevisionReport => ".writers-ide/processing/revisions.txt",
            Stage::Export => unreachable!(),
        },
    }
}

fn stage_fingerprint(
    stage: Stage,
    input: &PipelineInput,
    outputs: &BTreeMap<Stage, String>,
) -> String {
    let source = match stage {
        Stage::Compile => format!(
            "semantic-schema={}\ncompile-ir={:?}",
            story_document::SEMANTIC_SCHEMA_VERSION,
            compile_ir(input)
        ),
        Stage::WordCount | Stage::Export => {
            outputs.get(&Stage::Compile).cloned().unwrap_or_default()
        }
        Stage::PlotlineAnalysis => input
            .scenes
            .iter()
            .map(|s| format!("{}:{:?}", s.id, s.plotlines))
            .collect(),
        Stage::CharacterAnalysis => input
            .scenes
            .iter()
            .map(|s| format!("{}:{:?}", s.id, s.characters))
            .collect(),
        Stage::Continuity => input
            .scenes
            .iter()
            .map(|s| format!("{}:{:?}:{:?}", s.id, s.start, s.end))
            .collect(),
        Stage::RevisionReport => input.revision_summary.clone(),
    };
    let extra = if stage == Stage::Compile {
        format!("expand_transclusions={}", input.expand_transclusions)
    } else if matches!(stage, Stage::WordCount | Stage::Export) {
        format!("{:?}", input.target_words)
    } else {
        String::new()
    };
    hash(format!("{:?}\0{source}\0{extra}", stage).as_bytes())
}

fn render_stage(
    stage: Stage,
    input: &PipelineInput,
    outputs: &BTreeMap<Stage, String>,
) -> Result<String, String> {
    match stage {
        Stage::Compile => {
            let ir = compile_ir(input);
            if !ir.validate() {
                return Err(
                    "compile IR contains duplicate IDs, missing parents, or a cycle".into(),
                );
            }
            Ok(ir.render_markdown().0)
        }
        Stage::WordCount => {
            let text = outputs
                .get(&Stage::Compile)
                .ok_or("compiled manuscript unavailable")?;
            let words = story_core::count_words(text) as usize;
            let mut output = format!(
                "{} words\nTarget: {}\n",
                words,
                input
                    .target_words
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "unset".into())
            );
            if let Some(target) = input.target_words {
                let preview =
                    story_analysis::preview_budget_redistribution(target, &analysis_input(input));
                output.push_str(&format!(
                    "Budget preview: {} fixed, {} unallocated\n",
                    preview.fixed_words, preview.unallocated_words
                ));
            }
            Ok(output)
        }
        Stage::PlotlineAnalysis => {
            let mut counts = BTreeMap::<String, usize>::new();
            for scene in &input.scenes {
                for plot in &scene.plotlines {
                    *counts.entry(plot.clone()).or_default() += 1;
                }
            }
            let mut output = counts
                .into_iter()
                .map(|(plot, count)| format!("{plot}\t{count} scenes\n"))
                .collect::<String>();
            for gap in story_analysis::subplot_gaps(&analysis_input(input), 3) {
                output.push_str(&format!(
                    "gap\t{}\t{} scenes\t{} → {}\n",
                    gap.plotline_name, gap.intervening_scenes, gap.from_scene_id, gap.to_scene_id
                ));
            }
            Ok(output)
        }
        Stage::CharacterAnalysis => {
            let mut counts = BTreeMap::<String, usize>::new();
            for scene in &input.scenes {
                for character in &scene.characters {
                    *counts.entry(character.name.clone()).or_default() += 1;
                }
            }
            let mut output = counts
                .into_iter()
                .map(|(character, count)| format!("{character}\t{count} appearances\n"))
                .collect::<String>();
            for pair in story_analysis::character_interactions(&analysis_input(input)) {
                output.push_str(&format!(
                    "interaction\t{}\t{}\t{} scenes\n",
                    pair.first_character, pair.second_character, pair.shared_scenes
                ));
            }
            Ok(output)
        }
        Stage::Continuity => {
            let mut output = String::new();
            for finding in story_analysis::character_chronology_findings(&analysis_input(input)) {
                let label = match finding.status {
                    CharacterChronologyStatus::DefiniteOverlap => "definite-overlap",
                    CharacterChronologyStatus::PossibleOverlap => "possible-overlap",
                    CharacterChronologyStatus::Unverified => "unverified-overlap",
                };
                output.push_str(&format!(
                    "{label}\t{}\t{}\t{}\n",
                    finding.first_scene_id,
                    finding.second_scene_id,
                    finding.shared_character_ids.join(",")
                ));
            }
            Ok(output)
        }
        Stage::Export => outputs
            .get(&Stage::Compile)
            .cloned()
            .ok_or_else(|| "compiled manuscript unavailable".into()),
        Stage::RevisionReport => Ok(input.revision_summary.clone()),
    }
}

pub fn compile_ir(input: &PipelineInput) -> story_document::CompileIr {
    let mut ir = story_document::CompileIr::new(input.title.clone());
    for group in &input.groups {
        ir.push_group(story_document::CompileGroup {
            id: group.id.clone(),
            parent_id: group.parent_id.clone(),
            kind: group.kind,
            title: group.title.clone(),
        });
    }
    let mut scenes = Vec::new();
    for scene in &input.scenes {
        let segments = if input.expand_transclusions {
            scene
                .expanded_passages
                .as_ref()
                .map(|passages| {
                    passages
                        .iter()
                        .map(|passage| {
                            story_document::CompileSegment::new(
                                passage.source_entity_id.clone(),
                                passage.text.clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    vec![story_document::CompileSegment::new(
                        scene.id.clone(),
                        scene
                            .expanded_text
                            .as_deref()
                            .unwrap_or(scene.text.as_str()),
                    )]
                })
        } else {
            vec![story_document::CompileSegment::new(
                scene.id.clone(),
                scene.text.clone(),
            )]
        };
        scenes.push(story_document::CompileUnit {
            id: scene.id.clone(),
            parent_id: scene.chapter_id.clone(),
            kind: story_document::ContentUnitKind::Scene,
            title: scene.title.clone(),
            segments,
        });
    }
    let mut used_scenes = std::collections::BTreeSet::new();
    let mut used_content = std::collections::BTreeSet::new();
    for content in input
        .content_units
        .iter()
        .filter(|unit| unit.kind == story_document::ContentUnitKind::FrontMatter)
    {
        push_content(&mut ir, content);
        used_content.insert(content.id.clone());
    }
    fn emit_group(
        id: &str,
        input: &PipelineInput,
        scenes: &[story_document::CompileUnit],
        ir: &mut story_document::CompileIr,
        used_scenes: &mut std::collections::BTreeSet<String>,
        used_content: &mut std::collections::BTreeSet<String>,
    ) {
        for content in input
            .content_units
            .iter()
            .filter(|unit| unit.parent_id.as_deref() == Some(id))
        {
            push_content(ir, content);
            used_content.insert(content.id.clone());
        }
        for scene in scenes
            .iter()
            .filter(|scene| scene.parent_id.as_deref() == Some(id))
        {
            ir.push(scene.clone());
            used_scenes.insert(scene.id.clone());
        }
        for group in input
            .groups
            .iter()
            .filter(|group| group.parent_id.as_deref() == Some(id))
        {
            emit_group(&group.id, input, scenes, ir, used_scenes, used_content);
        }
    }
    for group in input
        .groups
        .iter()
        .filter(|group| group.parent_id.is_none())
    {
        emit_group(
            &group.id,
            input,
            &scenes,
            &mut ir,
            &mut used_scenes,
            &mut used_content,
        );
    }
    for scene in &scenes {
        if !used_scenes.contains(&scene.id) {
            ir.push(scene.clone());
        }
    }
    for content in input
        .content_units
        .iter()
        .filter(|unit| unit.kind == story_document::ContentUnitKind::BackMatter)
    {
        push_content(&mut ir, content);
        used_content.insert(content.id.clone());
    }
    for content in &input.content_units {
        if !used_content.contains(&content.id) {
            push_content(&mut ir, content);
        }
    }
    ir
}

fn push_content(ir: &mut story_document::CompileIr, content: &PipelineContentUnit) {
    ir.push(story_document::CompileUnit {
        id: content.id.clone(),
        parent_id: content.parent_id.clone(),
        kind: content.kind,
        title: content.title.clone(),
        segments: vec![story_document::CompileSegment::new(
            content.id.clone(),
            content.text.clone(),
        )],
    });
}

fn analysis_input(input: &PipelineInput) -> Vec<SceneAnalysisInput> {
    input
        .scenes
        .iter()
        .map(|scene| SceneAnalysisInput {
            scene_id: scene.id.clone(),
            title: scene.title.clone(),
            chapter_id: None,
            actual_words: story_core::count_words(&scene.text),
            target_words: scene.target_words,
            start_time: scene.start,
            end_time: scene.end,
            characters: scene
                .characters
                .iter()
                .map(|character| CharacterOnScene {
                    id: character.id.clone(),
                    name: character.name.clone(),
                    role: story_core::ParticipationRole::Major,
                })
                .collect(),
            plotlines: scene
                .plotlines
                .iter()
                .map(|name| PlotlineOnScene {
                    id: name.clone(),
                    name: name.clone(),
                })
                .collect(),
        })
        .collect()
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("artifact has no parent"))?;
    fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use story_core::StoryTime;

    fn input(text: &str) -> PipelineInput {
        PipelineInput {
            title: "Novel".into(),
            target_words: Some(100),
            expand_transclusions: false,
            groups: vec![PipelineGroup {
                id: "chapter-1".into(),
                parent_id: None,
                kind: story_document::StructuralKind::Chapter,
                title: "Opening".into(),
            }],
            content_units: vec![],
            scenes: vec![PipelineScene {
                id: "scene-1".into(),
                chapter_id: Some("chapter-1".into()),
                title: "Opening".into(),
                text: text.into(),
                expanded_text: None,
                expanded_passages: None,
                target_words: Some(100),
                characters: vec![PipelineCharacter {
                    id: "character-1".into(),
                    name: "Mara".into(),
                }],
                plotlines: vec!["Main".into()],
                start: StoryTime::unknown(),
                end: StoryTime::unknown(),
            }],
            revision_summary: "one snapshot".into(),
        }
    }

    #[test]
    fn pipeline_compiles_exports_and_caches_only_matching_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let runner = PipelineRunner::new(dir.path());
        let first = runner.run(&input("First draft.")).unwrap();
        assert!(first.iter().all(|r| r.status == StageStatus::Complete));
        assert_eq!(
            fs::read_to_string(dir.path().join("exports/compiled.md")).unwrap(),
            "## Opening\n\n### Opening\n\nFirst draft.\n\n"
        );
        let cached = runner.run(&input("First draft.")).unwrap();
        assert!(cached.iter().all(|r| r.status == StageStatus::Cached));
        let changed = runner.run(&input("Revised draft.")).unwrap();
        assert_eq!(changed[0].status, StageStatus::Complete);
        assert_eq!(changed[1].status, StageStatus::Complete);
        assert_eq!(changed[2].status, StageStatus::Cached);
        assert_eq!(changed[3].status, StageStatus::Cached);
        assert_eq!(changed[5].status, StageStatus::Complete);
    }

    #[test]
    fn chapter_level_prose_compiles_without_a_fake_scene_and_before_scene_units() {
        let mut novel = input("Scene prose.");
        novel.content_units.push(PipelineContentUnit {
            id: "chapter-body".into(),
            parent_id: Some("chapter-1".into()),
            kind: story_document::ContentUnitKind::ChapterBody,
            title: "Chapter".into(),
            text: "Chapter-level introduction.".into(),
        });
        let (compiled, _) = compile_ir(&novel).render_markdown();
        assert!(compiled.starts_with(
            "## Opening\n\nChapter-level introduction.\n\n### Opening\n\nScene prose."
        ));
        let mut nonfiction = novel;
        nonfiction.scenes.clear();
        let (compiled, _) = compile_ir(&nonfiction).render_markdown();
        assert_eq!(compiled, "## Opening\n\nChapter-level introduction.\n\n");
    }

    #[test]
    fn continuity_report_distinguishes_definite_and_unverified_character_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let mut novel = input("Draft.");
        let mut second = novel.scenes[0].clone();
        novel.scenes[0].start = StoryTime::exact(0);
        novel.scenes[0].end = StoryTime::exact(10);
        novel.scenes[0].characters[0].id = "character-1".into();
        second.id = "scene-2".into();
        second.title = "Second".into();
        second.start = StoryTime::exact(5);
        second.end = StoryTime::exact(20);
        second.characters[0].id = "character-1".into();
        let mut unknown = second.clone();
        unknown.id = "scene-3".into();
        unknown.title = "Unknown time".into();
        unknown.start = StoryTime::unknown();
        unknown.end = StoryTime::unknown();
        novel.scenes.extend([second, unknown]);

        PipelineRunner::new(dir.path())
            .run_selected(&novel, &[Stage::Continuity])
            .unwrap();
        let report =
            fs::read_to_string(dir.path().join(".writers-ide/processing/continuity.txt")).unwrap();
        assert!(report.contains("definite-overlap\tscene-1\tscene-2\tcharacter-1"));
        assert!(report.contains("unverified-overlap\tscene-1\tscene-3\tcharacter-1"));
        assert!(report.contains("unverified-overlap\tscene-2\tscene-3\tcharacter-1"));
    }

    #[test]
    fn transclusion_compilation_uses_derived_text_without_replacing_canonical_text() {
        let dir = tempfile::tempdir().unwrap();
        let runner = PipelineRunner::new(dir.path());
        let mut source = input("Canonical source.");
        source.expand_transclusions = true;
        source.scenes[0].expanded_text =
            Some("Canonical source.\n\n---\n\nIncluded canonical target.".into());
        source.scenes[0].expanded_passages = Some(vec![
            PipelinePassage {
                source_entity_id: "scene:scene-1".into(),
                text: "Canonical source.".into(),
            },
            PipelinePassage {
                source_entity_id: "research:reference-2".into(),
                text: "Included canonical target.".into(),
            },
        ]);
        let ir = compile_ir(&source);
        let (_, map) = ir.render_markdown();
        assert!(ir.validate());
        assert_eq!(ir.units[0].id, "scene-1");
        assert_eq!(map.len(), 2);
        assert_eq!(map[1].source_entity_id, "research:reference-2");
        runner.run(&source).unwrap();
        let compiled = fs::read_to_string(dir.path().join("exports/compiled.md")).unwrap();
        assert!(compiled.contains("Canonical source.\n\n---\n\nIncluded canonical target."));
        assert!(source.scenes[0].text == "Canonical source.");
        assert!(
            runner
                .run(&source)
                .unwrap()
                .iter()
                .all(|report| report.status == StageStatus::Cached)
        );
        source.expand_transclusions = false;
        runner.run(&source).unwrap();
        let plain = fs::read_to_string(dir.path().join("exports/compiled.md")).unwrap();
        assert!(!plain.contains("Included canonical target."));
    }

    #[test]
    fn missing_or_corrupt_artifacts_are_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let runner = PipelineRunner::new(dir.path());
        runner.run(&input("A.")).unwrap();
        fs::write(dir.path().join("exports/compiled.md"), "corrupt").unwrap();
        let result = runner.run(&input("A.")).unwrap();
        assert_eq!(result[5].status, StageStatus::Complete);
    }

    #[test]
    fn failed_stage_is_reported_and_dependents_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".writers-ide/processing/compiled.md")).unwrap();
        let result = PipelineRunner::new(dir.path()).run(&input("A.")).unwrap();
        assert_eq!(result[0].status, StageStatus::Failed);
        assert_eq!(result[1].status, StageStatus::Skipped);
        assert!(result[0].message.contains("persist artifact"));
    }

    #[test]
    fn selected_stages_run_dependency_closure_and_report_exclusions() {
        let dir = tempfile::tempdir().unwrap();
        let runner = PipelineRunner::new(dir.path());
        let selected = runner.run_selected(&input("A."), &[Stage::Export]).unwrap();
        assert_eq!(selected.len(), Stage::ALL.len());
        assert_eq!(selected[0].stage, Stage::Compile);
        assert_eq!(selected[0].status, StageStatus::Complete);
        assert_eq!(selected[1].status, StageStatus::NotSelected);
        assert_eq!(selected[2].status, StageStatus::NotSelected);
        assert_eq!(selected[3].status, StageStatus::NotSelected);
        assert_eq!(selected[4].status, StageStatus::NotSelected);
        assert_eq!(selected[5].status, StageStatus::Complete);
        assert_eq!(selected[6].status, StageStatus::NotSelected);

        let cached = runner.run_selected(&input("A."), &[Stage::Export]).unwrap();
        assert_eq!(cached[0].status, StageStatus::Cached);
        assert_eq!(cached[5].status, StageStatus::Cached);
        assert_eq!(cached[1].status, StageStatus::NotSelected);

        let failed_dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(
            failed_dir
                .path()
                .join(".writers-ide/processing/compiled.md"),
        )
        .unwrap();
        let failed = PipelineRunner::new(failed_dir.path())
            .run_selected(&input("A."), &[Stage::Export])
            .unwrap();
        assert_eq!(failed[0].status, StageStatus::Failed);
        assert_eq!(failed[5].status, StageStatus::Skipped);
        assert!(failed[5].message.contains("dependency"));
    }

    #[test]
    fn stage_names_are_stable_and_parseable() {
        for stage in Stage::ALL {
            assert_eq!(Stage::parse_name(stage.name()), Some(stage));
        }
        assert_eq!(Stage::parse_name("unknown"), None);
    }

    #[test]
    fn processing_recipes_validate_unique_stages_and_round_trip() {
        let recipe = ProcessingRecipe {
            version: 1,
            stages: vec![Stage::WordCount, Stage::Continuity],
            expand_transclusions: true,
        };
        let source = recipe.to_toml().unwrap();
        assert!(source.contains("\"word-count\""));
        assert_eq!(ProcessingRecipe::from_toml(&source).unwrap(), recipe);
        assert!(
            ProcessingRecipe {
                stages: vec![Stage::Compile, Stage::Compile],
                ..recipe.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProcessingRecipe {
                stages: vec![],
                ..recipe
            }
            .validate()
            .is_err()
        );
    }
}
