use std::path::Path;
use story_core::*;
use story_storage::{
    ConflictResolution, ProjectStore, Result as StorageResult, SceneRecord, StorageError,
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
}
pub type Result<T> = std::result::Result<T, AppError>;

/// Application façade: frontends use use cases here and never issue domain SQL.
pub struct WriterApp {
    store: ProjectStore,
}
impl WriterApp {
    pub fn create_project(path: impl AsRef<Path>, title: &str) -> Result<Self> {
        Ok(Self {
            store: ProjectStore::create(path, title)?,
        })
    }
    pub fn create_project_with_metadata(path: impl AsRef<Path>, project: &Project) -> Result<Self> {
        Ok(Self {
            store: ProjectStore::create_for(path, project)?,
        })
    }
    pub fn get_project(&self) -> Result<Project> {
        Ok(self.store.get_project()?)
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
    pub fn get_scene(&self, id: SceneId) -> Result<SceneRecord> {
        Ok(self.store.get_scene(id)?)
    }
    pub fn list_scenes(&self, include_archived: bool) -> Result<Vec<SceneRecord>> {
        Ok(self.store.list_scenes(include_archived)?)
    }
    pub fn analytics_dataset(&self) -> Result<Vec<story_analysis::SceneAnalysisInput>> {
        let mut result = Vec::new();
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
            });
        }
        Ok(result)
    }
    pub fn processing_input(&self) -> Result<story_pipeline::PipelineInput> {
        let project = self.get_project()?;
        let analytics = self.analytics_dataset()?;
        let mut scenes = Vec::new();
        let mut snapshots = 0usize;
        for row in self.store.list_scenes(false)? {
            let (text, _) = self.store.load_text(row.scene.id)?;
            let linked = analytics
                .iter()
                .find(|data| data.scene_id == row.scene.id.to_string());
            let characters = linked
                .map(|d| d.characters.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default();
            let plotlines = linked
                .map(|d| d.plotlines.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            snapshots += self.store.list_snapshots(row.scene.id)?.len();
            scenes.push(story_pipeline::PipelineScene {
                id: row.scene.id.to_string(),
                title: row.scene.title,
                text,
                target_words: row.scene.target_words,
                characters,
                plotlines,
                start: row.scene.start_time,
                end: row.scene.end_time,
            });
        }
        Ok(story_pipeline::PipelineInput {
            title: project.title,
            target_words: project.target_words,
            scenes,
            revision_summary: format!(
                "{} scenes; {snapshots} stored snapshots",
                self.store.list_scenes(false)?.len()
            ),
        })
    }
    pub fn run_processing(&self) -> Result<Vec<story_pipeline::StageReport>> {
        let input = self.processing_input()?;
        Ok(story_pipeline::PipelineRunner::new(self.store.project_root()).run(&input)?)
    }
    pub fn save_extraction_proposal(
        &self,
        filename: &str,
        json: &[u8],
    ) -> Result<std::path::PathBuf> {
        Ok(self.store.save_proposal_file(filename, json)?)
    }
    pub fn extract_with_ollama(
        &self,
        source_kind: &str,
        source_id: &str,
        model: &str,
        endpoint: &str,
    ) -> Result<story_ollama::ProvisionalExtraction> {
        let source = match source_kind {
            "scene" => {
                self.load_scene_text(SceneId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                ))?
                .0
            }
            "reading" => {
                self.get_reading_document(ReadingId(
                    source_id
                        .parse::<uuid::Uuid>()
                        .map_err(|e| AppError::Domain(e.to_string()))?,
                ))?
                .extracted_text
            }
            _ => {
                return Err(AppError::Domain(
                    "source kind must be scene or reading".into(),
                ));
            }
        };
        let result = story_ollama::OllamaClient::new(endpoint, model)
            .map_err(|e| AppError::Domain(e.to_string()))?
            .extract(&source)
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
    pub fn add_event(&mut self, title: &str, source_time: &str) -> Result<EventId> {
        let id = EventId::new();
        self.store.add_event(id, title, source_time)?;
        Ok(id)
    }
    pub fn link_event(&mut self, scene: SceneId, event: EventId, kind: &str) -> Result<()> {
        Ok(self.store.link_event(scene, event, kind)?)
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
    pub fn create_issue(&mut self, title: &str) -> Result<IssueId> {
        Ok(self.store.create_issue(title)?)
    }
    pub fn link_issue(&mut self, scene: SceneId, issue: IssueId) -> Result<()> {
        Ok(self.store.link_issue(scene, issue)?)
    }
    pub fn list_issues(&self) -> Result<Vec<story_storage::IssueSummary>> {
        Ok(self.store.list_issues()?)
    }
    pub fn list_characters(&self) -> Result<Vec<story_storage::CharacterSummary>> {
        Ok(self.store.list_characters()?)
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
        Ok(self.store.expand_scene_transclusions(scene)?)
    }
    pub fn broken_links(&self) -> Result<Vec<story_storage::BrokenLink>> {
        Ok(self.store.broken_links()?)
    }
    pub fn save_scene_query(&mut self, query: &SavedSceneQuery) -> Result<()> {
        Ok(self.store.save_scene_query(query)?)
    }
    pub fn list_saved_queries(&self) -> Result<Vec<story_storage::SavedQuerySummary>> {
        Ok(self.store.list_saved_queries()?)
    }
    pub fn run_scene_query(&self, query: QueryId) -> Result<Vec<SceneId>> {
        Ok(self.store.run_scene_query(query)?)
    }
    pub fn search_scenes(&self, terms: &str) -> Result<Vec<SceneId>> {
        Ok(self.store.search_scenes(terms)?)
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
    pub fn backup_project(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        Ok(self.store.backup_to(destination)?)
    }
    pub fn resolve_conflict(&mut self, txid: &str, resolution: ConflictResolution) -> Result<()> {
        Ok(self.store.resolve_write_conflict(txid, resolution)?)
    }
}

#[allow(dead_code)]
fn _storage_result(_: StorageResult<()>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
    }
}
