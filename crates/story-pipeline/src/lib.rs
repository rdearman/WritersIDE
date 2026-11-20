//! Dependency-aware, local processing of a manuscript snapshot.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use story_analysis::{CharacterOnScene, PlotlineOnScene, SceneAnalysisInput};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("processing I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("processing cache: {0}")]
    Cache(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
pub struct PipelineScene {
    pub id: String,
    pub title: String,
    pub text: String,
    pub target_words: Option<u64>,
    pub characters: Vec<String>,
    pub plotlines: Vec<String>,
    pub start: story_core::StoryTime,
    pub end: story_core::StoryTime,
}

#[derive(Clone, Debug)]
pub struct PipelineInput {
    pub title: String,
    pub target_words: Option<u64>,
    /// Ordered in manuscript order; chronology remains on each scene independently.
    pub scenes: Vec<PipelineScene>,
    pub revision_summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
    fn dependencies(self) -> &'static [Stage] {
        use Stage::*;
        match self {
            Compile => &[],
            WordCount | Export => &[Compile],
            PlotlineAnalysis | CharacterAnalysis | Continuity | RevisionReport => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageStatus {
    Complete,
    Cached,
    Failed,
    Skipped,
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
        fs::create_dir_all(self.cache_path.parent().expect("cache has parent"))?;
        fs::create_dir_all(self.root.join("exports"))?;
        let mut cache: Cache = match fs::read(&self.cache_path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Cache::default(),
            Err(error) => return Err(error.into()),
        };
        let mut outputs = BTreeMap::<Stage, String>::new();
        let mut reports = Vec::new();
        for stage in [
            Stage::Compile,
            Stage::WordCount,
            Stage::PlotlineAnalysis,
            Stage::CharacterAnalysis,
            Stage::Continuity,
            Stage::Export,
            Stage::RevisionReport,
        ] {
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
                    message: "inputs unchanged; verified cached artifact".into(),
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
                        message: "completed".into(),
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
        Stage::Compile => input
            .scenes
            .iter()
            .map(|s| format!("{}\0{}\0{}", s.id, s.title, s.text))
            .collect::<String>(),
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
    let extra = if matches!(stage, Stage::WordCount | Stage::Export) {
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
        Stage::Compile => Ok(input
            .scenes
            .iter()
            .map(|scene| format!("## {}\n\n{}\n\n", scene.title, scene.text))
            .collect()),
        Stage::WordCount => {
            let text = outputs
                .get(&Stage::Compile)
                .ok_or("compiled manuscript unavailable")?;
            let words = text.split_whitespace().count();
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
                    *counts.entry(character.clone()).or_default() += 1;
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
            for (i, first) in input.scenes.iter().enumerate() {
                for second in input.scenes.iter().skip(i + 1) {
                    if first
                        .characters
                        .iter()
                        .any(|c| second.characters.contains(c))
                        && !matches!(
                            first.end.compare(second.start),
                            story_core::TimeOrdering::Before
                        )
                        && !matches!(
                            second.end.compare(first.start),
                            story_core::TimeOrdering::Before
                        )
                    {
                        output
                            .push_str(&format!("uncertain-overlap\t{}\t{}\n", first.id, second.id));
                    }
                }
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

fn analysis_input(input: &PipelineInput) -> Vec<SceneAnalysisInput> {
    input
        .scenes
        .iter()
        .map(|scene| SceneAnalysisInput {
            scene_id: scene.id.clone(),
            title: scene.title.clone(),
            chapter_id: None,
            actual_words: scene.text.split_whitespace().count().min(u32::MAX as usize) as u32,
            target_words: scene.target_words,
            start_time: scene.start,
            end_time: scene.end,
            characters: scene
                .characters
                .iter()
                .map(|name| CharacterOnScene {
                    id: name.clone(),
                    name: name.clone(),
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
            scenes: vec![PipelineScene {
                id: "scene-1".into(),
                title: "Opening".into(),
                text: text.into(),
                target_words: Some(100),
                characters: vec!["Mara".into()],
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
            "## Opening\n\nFirst draft.\n\n"
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
}
