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
    }
}
