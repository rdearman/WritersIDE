use rusqlite::{Connection, OptionalExtension, TransactionBehavior, backup::Backup, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use story_core::*;
use thiserror::Error;
use uuid::Uuid;

const MIGRATION_1: &str = include_str!("../migrations/0001_initial.sql");
const MIGRATION_2: &str = include_str!("../migrations/0002_links_queries.sql");
const MIGRATION_3: &str = include_str!("../migrations/0003_reading.sql");
const MIGRATION_4: &str = include_str!("../migrations/0004_snapshots.sql");
const MIGRATION_5: &str = include_str!("../migrations/0005_project_search.sql");
const MIGRATION_6: &str = include_str!("../migrations/0006_search_custom_fields.sql");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("project configuration: {0}")]
    Config(String),
    #[error("project identity mismatch: config {config}, database {database}")]
    IdentityMismatch { config: String, database: String },
    #[error("unsupported database schema version {0}")]
    UnsupportedSchema(i64),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("invalid operation: {0}")]
    Invalid(String),
    #[error("manuscript conflict: {0}")]
    Conflict(String),
    #[error("recovery conflict: {0}")]
    RecoveryConflict(String),
}
pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Serialize, Deserialize)]
struct ProjectConfig {
    id: Uuid,
    title: String,
    author: String,
    genre: String,
    synopsis: String,
    target_words: Option<u64>,
    story_epoch_label: String,
    story_calendar_id: Option<String>,
}
pub struct ProjectStore {
    root: PathBuf,
    conn: Connection,
}
#[derive(Clone, Debug)]
pub struct SceneRecord {
    pub scene: Scene,
    pub chapter_id: Option<NodeId>,
    pub position: Option<u32>,
    pub actual_words: u32,
    pub content_hash: String,
}
#[derive(Clone, Debug)]
pub struct StructuralNodeSummary {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub kind: String,
    pub title: String,
    pub position: u32,
}
#[derive(Clone, Debug)]
pub struct RecoveryConflict {
    pub transaction_id: String,
    pub scene_id: String,
    pub recovery_path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct BrokenLink {
    pub link_id: LinkId,
    pub reason: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassageAnchorRecord {
    pub id: AnchorId,
    pub scene_id: SceneId,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub state: String,
    pub source_hash: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSearchHit {
    pub entity_kind: String,
    pub entity_id: String,
    pub title: String,
    pub excerpt: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscludedPassage {
    pub scene_id: SceneId,
    pub text: String,
    pub depth: usize,
}
#[derive(Clone, Debug)]
pub struct CharacterSummary {
    pub id: CharacterId,
    pub name: String,
    pub scene_count: u32,
}
#[derive(Clone, Debug)]
pub struct PlotlineSummary {
    pub id: PlotlineId,
    pub name: String,
    pub scene_count: u32,
}
#[derive(Clone, Debug)]
pub struct SavedQuerySummary {
    pub id: QueryId,
    pub name: String,
}
#[derive(Clone, Debug)]
pub struct EventSummary {
    pub id: EventId,
    pub title: String,
    pub nominal_time: Option<i64>,
    pub source_time: String,
}
#[derive(Clone, Debug)]
pub struct ResearchSummary {
    pub id: ResearchId,
    pub title: String,
    pub media_type: String,
    pub source_path: Option<String>,
}
#[derive(Clone, Debug)]
pub struct IssueSummary {
    pub id: IssueId,
    pub title: String,
    pub status: String,
    pub scene_count: u32,
}
#[derive(Clone, Debug)]
pub struct ReadingDocumentRecord {
    pub id: ReadingId,
    pub title: String,
    pub media_type: String,
    pub relative_path: String,
    pub source_identifier: String,
    pub source_sha256: String,
    pub extracted_text: String,
}
#[derive(Clone, Debug)]
pub struct ReadingAnnotationRecord {
    pub id: AnnotationId,
    pub document_id: ReadingId,
    pub kind: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub body: String,
    pub state: String,
}
#[derive(Clone, Debug)]
pub struct ReadingBookmarkRecord {
    pub id: BookmarkId,
    pub document_id: ReadingId,
    pub title: String,
    pub offset_byte: u64,
    pub state: String,
}
#[derive(Clone, Debug)]
pub struct ReadingTermRecord {
    pub id: ReadingTermId,
    pub kind: String,
    pub name: String,
    pub aliases: String,
    pub description: String,
}
#[derive(Clone, Debug)]
pub struct ReadingEntityLinkRecord {
    pub id: LinkId,
    pub annotation_id: Option<AnnotationId>,
    pub target_kind: String,
    pub target_id: String,
    pub relation: String,
}
#[derive(Clone, Debug)]
pub struct SnapshotRecord {
    pub id: SnapshotId,
    pub scene_id: SceneId,
    pub label: String,
    pub sha256: String,
    pub created_at: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotLineKind {
    Equal,
    Added,
    Removed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotDiffLine {
    pub kind: SnapshotLineKind,
    pub text: String,
}
type CustomDefaultRow = (
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<bool>,
    Option<String>,
);
type WriteJournalRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    i64,
    i64,
    String,
    i64,
    bool,
);
pub struct ImportSourceItemRecord<'a> {
    pub id: SourceItemId,
    pub run: ImportRunId,
    pub source_document_id: &'a str,
    pub source_sheet: Option<&'a str>,
    pub source_row: Option<u32>,
    pub fingerprint: &'a str,
    pub inventory_type: &'a str,
    pub raw_fields: &'a str,
}
pub struct EntityProvenanceRecord<'a> {
    pub id: ProvenanceId,
    pub source_item: SourceItemId,
    pub destination_type: &'a str,
    pub destination_id: &'a str,
    pub source_field: Option<&'a str>,
    pub destination_field: Option<&'a str>,
    pub mapping: &'a str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictResolution {
    KeepExternal,
    UsePrepared,
}

impl ProjectStore {
    pub fn project_root(&self) -> &Path {
        &self.root
    }
    pub fn create(root: impl AsRef<Path>, title: &str) -> Result<Self> {
        let project = Project {
            id: ProjectId::new(),
            title: title.into(),
            author: String::new(),
            genre: String::new(),
            synopsis: String::new(),
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
        };
        Self::create_for(root, &project)
    }
    pub fn create_for(root: impl AsRef<Path>, project: &Project) -> Result<Self> {
        let root = root.as_ref();
        if root.exists() && fs::read_dir(root)?.next().is_some() {
            return Err(StorageError::Invalid(
                "refusing to create over a non-empty directory".into(),
            ));
        }
        fs::create_dir_all(root)?;
        for d in [
            "manuscript",
            "research",
            "attachments",
            "templates",
            "exports",
            ".writers-ide/recovery",
            "research/readings",
        ] {
            fs::create_dir_all(root.join(d))?;
        }
        let cfg = ProjectConfig {
            id: project.id.0,
            title: project.title.clone(),
            author: project.author.clone(),
            genre: project.genre.clone(),
            synopsis: project.synopsis.clone(),
            target_words: project.target_words,
            story_epoch_label: project.story_epoch_label.clone(),
            story_calendar_id: project.story_calendar_id.clone(),
        };
        fs::write(
            root.join("project.toml"),
            toml::to_string_pretty(&cfg).map_err(|e| StorageError::Config(e.to_string()))?,
        )?;
        let conn = Connection::open(root.join("story.db"))?;
        configure(&conn)?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_1}\n{MIGRATION_2}\nINSERT INTO schema_migrations(version,applied_at) VALUES(2,'{}');\n{MIGRATION_3}\nINSERT INTO schema_migrations(version,applied_at) VALUES(3,'{}');\n{MIGRATION_4}\nINSERT INTO schema_migrations(version,applied_at) VALUES(4,'{}');\n{MIGRATION_5}\nINSERT INTO schema_migrations(version,applied_at) VALUES(5,'{}');\n{MIGRATION_6}\nINSERT INTO schema_migrations(version,applied_at) VALUES(6,'{}');\nCOMMIT;", now(),now(),now(),now(),now()))?;
        let now = now();
        conn.execute("INSERT INTO project(singleton,id,title,author,genre,synopsis,target_words,story_epoch_label,story_calendar_id,created_at,updated_at) VALUES(1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",params![cfg.id.to_string(),cfg.title,cfg.author,cfg.genre,cfg.synopsis,cfg.target_words,cfg.story_epoch_label,cfg.story_calendar_id,now])?;
        Ok(Self {
            root: root.to_path_buf(),
            conn,
        })
    }
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        for name in ["project.toml", "story.db"] {
            let meta = fs::symlink_metadata(root.join(name))?;
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(StorageError::Config(format!(
                    "{name} must be a regular project file"
                )));
            }
        }
        let config_text = fs::read_to_string(root.join("project.toml"))?;
        let cfg: ProjectConfig =
            toml::from_str(&config_text).map_err(|e| StorageError::Config(e.to_string()))?;
        let conn = Connection::open(root.join("story.db"))?;
        configure(&conn)?;
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        let migrated = version == 1;
        if version > 6 {
            return Err(StorageError::UnsupportedSchema(version));
        }
        if version < 1 {
            return Err(StorageError::UnsupportedSchema(version));
        }
        let mut current_version = version;
        if current_version == 1 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_2}\nINSERT INTO schema_migrations(version,applied_at) VALUES(2,'{}');\nCOMMIT;", now()))?;
            current_version = 2;
        }
        if current_version == 2 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_3}\nINSERT INTO schema_migrations(version,applied_at) VALUES(3,'{}');\nCOMMIT;", now()))?;
            current_version = 3;
        }
        if current_version == 3 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_4}\nINSERT INTO schema_migrations(version,applied_at) VALUES(4,'{}');\nCOMMIT;", now()))?;
            current_version = 4;
        }
        if current_version == 4 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_5}\nINSERT INTO schema_migrations(version,applied_at) VALUES(5,'{}');\nCOMMIT;", now()))?;
            current_version = 5;
        }
        if current_version == 5 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_6}\nINSERT INTO schema_migrations(version,applied_at) VALUES(6,'{}');\nCOMMIT;", now()))?;
        }
        let dbid: String = conn
            .query_row("SELECT id FROM project LIMIT 1", [], |r| r.get(0))
            .map_err(|_| StorageError::Config("database has no project identity".into()))?;
        if cfg.id.to_string() != dbid {
            return Err(StorageError::IdentityMismatch {
                config: cfg.id.to_string(),
                database: dbid,
            });
        }
        let mut this = Self {
            root: root.to_path_buf(),
            conn,
        };
        fs::create_dir_all(this.root.join("research/readings"))?;
        let _ = this.recover_pending_writes()?;
        if migrated {
            this.rebuild_scene_index()?;
        }
        Ok(this)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn save_proposal_file(&self, filename: &str, content: &[u8]) -> Result<PathBuf> {
        let name = Path::new(filename);
        if name.components().count() != 1
            || !matches!(
                name.components().next(),
                Some(std::path::Component::Normal(_))
            )
            || name.extension().and_then(|x| x.to_str()) != Some("json")
        {
            return Err(StorageError::Invalid(
                "proposal filename must be a simple .json filename".into(),
            ));
        }
        let directory = self.safe_path(Path::new(".writers-ide/proposals"))?;
        fs::create_dir_all(&directory)?;
        let relative = Path::new(".writers-ide/proposals").join(name);
        let target = self.safe_path(&relative)?;
        if target.exists() {
            return Err(StorageError::Conflict(
                "proposal file already exists".into(),
            ));
        }
        atomic_write(&target, content)?;
        Ok(target)
    }
    pub fn get_project(&self) -> Result<Project> {
        let row: (String,String,String,String,String,Option<u64>,String,Option<String>) = self.conn.query_row("SELECT id,title,author,genre,synopsis,target_words,story_epoch_label,story_calendar_id FROM project WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?;
        Ok(Project {
            id: ProjectId(
                Uuid::parse_str(&row.0).map_err(|e| StorageError::Config(e.to_string()))?,
            ),
            title: row.1,
            author: row.2,
            genre: row.3,
            synopsis: row.4,
            target_words: row.5,
            story_epoch_label: row.6,
            story_calendar_id: row.7,
        })
    }
    pub fn backup_to(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(StorageError::Invalid(
                "backup destination already exists".into(),
            ));
        }
        if destination.starts_with(&self.root) || self.root.starts_with(destination) {
            return Err(StorageError::Invalid(
                "backup must be outside the project directory".into(),
            ));
        }
        self.validate()?;
        let parent = destination.parent().ok_or_else(|| {
            StorageError::Invalid("backup destination needs a parent directory".into())
        })?;
        fs::create_dir_all(parent)?;
        let name = destination
            .file_name()
            .ok_or_else(|| StorageError::Invalid("invalid backup destination".into()))?
            .to_string_lossy();
        let stage = parent.join(format!(".{name}.backup-{}", Uuid::new_v4()));
        fs::create_dir(&stage)?;
        let result = (|| -> Result<()> {
            copy_synced(&self.root.join("project.toml"), &stage.join("project.toml"))?;
            let mut target_db = Connection::open(stage.join("story.db"))?;
            {
                let backup = Backup::new(&self.conn, &mut target_db)?;
                backup.run_to_completion(128, std::time::Duration::from_millis(5), None)?;
            }
            target_db.close().map_err(|(_, e)| StorageError::Sql(e))?;
            File::open(stage.join("story.db"))?.sync_all()?;
            for name in [
                "manuscript",
                "research",
                "attachments",
                "templates",
                "exports",
                ".writers-ide",
            ] {
                let from = self.root.join(name);
                if from.exists() {
                    copy_tree_sync(&from, &stage.join(name))?;
                }
            }
            sync_dir(&stage)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_dir_all(&stage);
            return Err(e);
        }
        let validate_result = (|| -> Result<()> {
            let backup = ProjectStore::open(&stage)?;
            backup.validate()?;
            Ok(())
        })();
        if let Err(error) = validate_result {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
        fs::rename(&stage, destination)?;
        sync_dir(parent)?;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA foreign_key_check")?;
        let bad: i64 =
            self.conn
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
        if bad > 0 {
            return Err(StorageError::Invalid(format!(
                "{bad} foreign key violations"
            )));
        }
        let missing:i64=self.conn.query_row("SELECT count(*) FROM scene s JOIN custom_field_definition d ON d.required=1 LEFT JOIN custom_field_value v ON v.scene_id=s.id AND v.field_id=d.id WHERE v.scene_id IS NULL",[],|r|r.get(0))?;
        if missing > 0 {
            return Err(StorageError::Invalid(format!(
                "{missing} required custom field values are missing"
            )));
        }
        let files: Vec<(String, String, u64)> = {
            let mut q = self
                .conn
                .prepare("SELECT scene_id,relative_path,byte_length FROM scene_content")?;
            q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        let expected_paths: std::collections::HashSet<String> =
            files.iter().map(|(_, path, _)| path.clone()).collect();
        for (id, rel, length) in files {
            let path = self.safe_path(Path::new(&rel))?;
            let bytes = fs::read(&path).map_err(|e| {
                StorageError::Conflict(format!("missing/unreadable manuscript for scene {id}: {e}"))
            })?;
            let expected: String = self.conn.query_row(
                "SELECT sha256 FROM scene_content WHERE scene_id=?1",
                [&id],
                |r| r.get(0),
            )?;
            if bytes.len() as u64 != length || hash_bytes(&bytes) != expected {
                return Err(StorageError::Conflict(format!(
                    "manuscript integrity mismatch for scene {id}"
                )));
            }
        }
        for entry in fs::read_dir(self.root.join("manuscript"))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".md") && !expected_paths.contains(&format!("manuscript/{name}")) {
                return Err(StorageError::Conflict(format!(
                    "untracked manuscript file: {name}"
                )));
            }
        }
        Ok(())
    }
    pub fn create_node(
        &mut self,
        parent: Option<NodeId>,
        kind: NodeKind,
        title: &str,
        position: u32,
    ) -> Result<NodeId> {
        if let Some(pid) = parent {
            let pk: String = self
                .conn
                .query_row(
                    "SELECT kind FROM structural_node WHERE id=?1",
                    [pid.to_string()],
                    |r| r.get(0),
                )
                .map_err(|_| StorageError::NotFound(pid.to_string()))?;
            if pk == "chapter" {
                return Err(StorageError::Invalid(
                    "chapter cannot contain structural nodes".into(),
                ));
            }
        }
        let id = NodeId::new();
        let now = now();
        self.conn.execute("INSERT INTO structural_node(id,parent_id,kind,title,position,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",params![id.to_string(),parent.map(|x|x.to_string()),kind.as_str(),title,i64::from(position),now])?;
        Ok(id)
    }
    pub fn list_structural_nodes(&self) -> Result<Vec<StructuralNodeSummary>> {
        let mut stmt = self.conn.prepare("SELECT id,parent_id,kind,title,position FROM structural_node ORDER BY parent_id,position,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, parent, kind, title, position) = row?;
            let uuid = |value: String| {
                Uuid::parse_str(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            };
            Ok(StructuralNodeSummary {
                id: NodeId(uuid(id)?),
                parent_id: parent.map(uuid).transpose()?.map(NodeId),
                kind,
                title,
                position,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }
    pub fn move_node(
        &mut self,
        id: NodeId,
        new_parent: Option<NodeId>,
        position: u32,
    ) -> Result<()> {
        let (kind, old_parent): (String, Option<String>) = self
            .conn
            .query_row(
                "SELECT kind,parent_id FROM structural_node WHERE id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| StorageError::NotFound(id.to_string()))?;
        if let Some(parent) = new_parent {
            let pk: Option<String> = self
                .conn
                .query_row(
                    "SELECT kind FROM structural_node WHERE id=?1",
                    [parent.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            let pk = pk.ok_or_else(|| StorageError::NotFound(parent.to_string()))?;
            if pk == "chapter" {
                return Err(StorageError::Invalid(
                    "chapter cannot contain structural nodes".into(),
                ));
            }
            let cycle:i64=self.conn.query_row("WITH RECURSIVE ancestors(id,parent_id) AS (SELECT id,parent_id FROM structural_node WHERE id=?1 UNION ALL SELECT n.id,n.parent_id FROM structural_node n JOIN ancestors a ON n.id=a.parent_id) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?2)",params![parent.to_string(),id.to_string()],|r|r.get(0))?;
            if cycle != 0 {
                return Err(StorageError::Invalid(
                    "structural move would create a cycle".into(),
                ));
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut src = query_node_ids(&tx, old_parent.as_deref())?;
        src.retain(|x| x != &id.to_string());
        let mut dst = if old_parent == new_parent.map(|x| x.to_string()) {
            src.clone()
        } else {
            query_node_ids(&tx, new_parent.map(|x| x.to_string()).as_deref())?
        };
        dst.retain(|x| x != &id.to_string());
        if position as usize > dst.len() {
            return Err(StorageError::Invalid(
                "position beyond sibling count".into(),
            ));
        }
        dst.insert(position as usize, id.to_string());
        let mut affected = src.clone();
        if old_parent != new_parent.map(|x| x.to_string()) {
            affected.extend(dst.iter().cloned());
        } else {
            affected = dst.clone();
        }
        let max: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position),0) FROM structural_node",
            [],
            |r| r.get(0),
        )?;
        let base = max
            .checked_mul(3)
            .and_then(|x| x.checked_add(100))
            .ok_or_else(|| StorageError::Invalid("position space exhausted".into()))?;
        for sid in &affected {
            tx.execute(
                "UPDATE structural_node SET position=position+?2 WHERE id=?1",
                params![sid, base],
            )?;
        }
        for (i, sid) in affected.iter().enumerate() {
            tx.execute(
                "UPDATE structural_node SET position=?2 WHERE id=?1",
                params![sid, base * 2 + i as i64],
            )?;
        }
        if old_parent != new_parent.map(|x| x.to_string()) {
            tx.execute(
                "UPDATE structural_node SET parent_id=?2 WHERE id=?1",
                params![id.to_string(), new_parent.map(|x| x.to_string())],
            )?;
            for (i, sid) in src.iter().enumerate() {
                tx.execute(
                    "UPDATE structural_node SET position=?2 WHERE id=?1",
                    params![sid, i as i64],
                )?;
            }
        }
        for (i, sid) in dst.iter().enumerate() {
            tx.execute(
                "UPDATE structural_node SET parent_id=?2,position=?3 WHERE id=?1",
                params![sid, new_parent.map(|x| x.to_string()), i as i64],
            )?;
        }
        let _ = kind;
        tx.commit()?;
        Ok(())
    }
    pub fn create_scene(&mut self, chapter: NodeId, position: u32, title: &str) -> Result<SceneId> {
        let kind: Option<String> = self
            .conn
            .query_row(
                "SELECT kind FROM structural_node WHERE id=?1",
                [chapter.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if kind.as_deref() != Some("chapter") {
            return Err(StorageError::Invalid(
                "scene destination must be an existing chapter".into(),
            ));
        }
        let id = SceneId::new();
        let now = now();
        let rel = format!("manuscript/{}.md", id);
        let scene_path = self.safe_path(Path::new(&rel))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = query_scene_ids(&tx, &chapter.to_string())?;
        if position as usize > existing.len() {
            return Err(StorageError::Invalid(
                "scene position beyond sibling count".into(),
            ));
        }
        let max: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position),0) FROM scene_placement",
            [],
            |r| r.get(0),
        )?;
        let base = max
            .checked_mul(3)
            .and_then(|x| x.checked_add(existing.len() as i64 + 100))
            .ok_or_else(|| StorageError::Invalid("position space exhausted".into()))?;
        for sid in &existing {
            tx.execute(
                "UPDATE scene_placement SET position=position+?2 WHERE scene_id=?1",
                params![sid, base],
            )?;
        }
        for (i, sid) in existing.iter().enumerate() {
            let p = i as i64 + if i as u32 >= position { 1 } else { 0 };
            tx.execute(
                "UPDATE scene_placement SET position=?2 WHERE scene_id=?1",
                params![sid, p],
            )?;
        }
        atomic_write(&scene_path, b"")?;
        let hash = hash_bytes(b"");
        tx.execute(
            "INSERT INTO scene(id,title,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![id.to_string(), title, now],
        )?;
        tx.execute(
            "INSERT INTO scene_placement(scene_id,chapter_id,position) VALUES(?1,?2,?3)",
            params![id.to_string(), chapter.to_string(), i64::from(position)],
        )?;
        tx.execute("INSERT INTO scene_content(scene_id,relative_path,sha256,byte_length) VALUES(?1,?2,?3,0)",params![id.to_string(),rel,hash])?;
        let defaults: Vec<CustomDefaultRow> = {
            let mut q=tx.prepare("SELECT id,field_type,default_text,default_integer,default_decimal,default_boolean,reference_kind FROM custom_field_definition WHERE default_text IS NOT NULL OR default_integer IS NOT NULL OR default_decimal IS NOT NULL OR default_boolean IS NOT NULL")?;
            q.query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            })?
            .collect::<std::result::Result<_, _>>()?
        };
        for (fid, ty, text, integer, decimal, boolean, ref_kind) in defaults {
            match ty.as_str() {
                "text" | "long_text" | "enum" | "datetime" => {
                    tx.execute("INSERT INTO custom_field_value(scene_id,field_id,text_value) VALUES(?1,?2,?3)",params![id.to_string(),fid,text])?;
                }
                "integer" => {
                    tx.execute("INSERT INTO custom_field_value(scene_id,field_id,integer_value) VALUES(?1,?2,?3)",params![id.to_string(),fid,integer])?;
                }
                "decimal" => {
                    tx.execute("INSERT INTO custom_field_value(scene_id,field_id,decimal_value) VALUES(?1,?2,?3)",params![id.to_string(),fid,decimal])?;
                }
                "boolean" => {
                    tx.execute("INSERT INTO custom_field_value(scene_id,field_id,boolean_value) VALUES(?1,?2,?3)",params![id.to_string(),fid,boolean])?;
                }
                "entity_ref" => {
                    let target = text.ok_or_else(|| {
                        StorageError::Invalid("entity reference default must be text ID".into())
                    })?;
                    let rk = ref_kind.as_deref().ok_or_else(|| {
                        StorageError::Invalid("entity reference field lacks target kind".into())
                    })?;
                    validate_entity_ref(&tx, Some(rk), &target)?;
                    tx.execute("INSERT INTO custom_field_value(scene_id,field_id,entity_ref) VALUES(?1,?2,?3)",params![id.to_string(),fid,target])?;
                    insert_ref(
                        &tx,
                        id,
                        FieldId(
                            Uuid::parse_str(&fid)
                                .map_err(|e| StorageError::Invalid(e.to_string()))?,
                        ),
                        0,
                        rk,
                        &target,
                    )?;
                }
                _ => return Err(StorageError::Invalid("unsupported default type".into())),
            }
        }
        tx.commit()?;
        Ok(id)
    }
    pub fn get_scene(&self, id: SceneId) -> Result<SceneRecord> {
        let row=self.conn.query_row("SELECT s.title,s.synopsis,s.status,s.target_words,s.purpose,s.goal,s.outcome,s.start_nominal,s.start_lower,s.start_upper,s.start_precision,s.end_nominal,s.end_lower,s.end_upper,s.end_precision,s.pov_character_id,s.start_location_id,s.end_location_id,s.archived,p.chapter_id,p.position,c.sha256,c.byte_length FROM scene s LEFT JOIN scene_placement p ON p.scene_id=s.id JOIN scene_content c ON c.scene_id=s.id WHERE s.id=?1",[id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<u64>>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,Option<i64>>(7)?,r.get::<_,Option<i64>>(8)?,r.get::<_,Option<i64>>(9)?,r.get::<_,String>(10)?,r.get::<_,Option<i64>>(11)?,r.get::<_,Option<i64>>(12)?,r.get::<_,Option<i64>>(13)?,r.get::<_,String>(14)?,r.get::<_,Option<String>>(15)?,r.get::<_,Option<String>>(16)?,r.get::<_,Option<String>>(17)?,r.get::<_,bool>(18)?,r.get::<_,Option<String>>(19)?,r.get::<_,Option<u32>>(20)?,r.get::<_,String>(21)?,r.get::<_,u64>(22)?))).map_err(|_|StorageError::NotFound(id.to_string()))?;
        let st = read_time(row.7, row.8, row.9, &row.10)?;
        let et = read_time(row.11, row.12, row.13, &row.14)?;
        let content = fs::read(self.safe_path(&self.content_path(id)?)?)?;
        if hash_bytes(&content) != row.21 {
            return Err(StorageError::Conflict(format!(
                "external manuscript modification for scene {id}"
            )));
        }
        let actual = word_count(&String::from_utf8_lossy(&content));
        Ok(SceneRecord {
            scene: Scene {
                id,
                title: row.0,
                synopsis: row.1,
                status: parse_status(&row.2),
                target_words: row.3,
                purpose: row.4,
                goal: row.5,
                outcome: row.6,
                start_time: st,
                end_time: et,
                pov_character: row
                    .15
                    .and_then(|x| Uuid::parse_str(&x).ok())
                    .map(CharacterId),
                start_location: row
                    .16
                    .and_then(|x| Uuid::parse_str(&x).ok())
                    .map(LocationId),
                end_location: row
                    .17
                    .and_then(|x| Uuid::parse_str(&x).ok())
                    .map(LocationId),
                archived: row.18,
            },
            chapter_id: row.19.and_then(|x| Uuid::parse_str(&x).ok()).map(NodeId),
            position: row.20,
            actual_words: actual,
            content_hash: row.21,
        })
    }
    pub fn list_scenes(&self, include_archived: bool) -> Result<Vec<SceneRecord>> {
        let sql = "WITH RECURSIVE node_order(id,path) AS (SELECT id,printf('%010d/',position) FROM structural_node WHERE parent_id IS NULL UNION ALL SELECT n.id,o.path||printf('%010d/',n.position) FROM structural_node n JOIN node_order o ON n.parent_id=o.id) SELECT s.id FROM scene s LEFT JOIN scene_placement p ON p.scene_id=s.id LEFT JOIN node_order n ON n.id=p.chapter_id WHERE (?1=1 OR s.archived=0) ORDER BY n.path,p.position,s.id";
        let ids: Vec<String> = {
            let mut q = self.conn.prepare(sql)?;
            q.query_map([include_archived], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        ids.into_iter()
            .map(|id| {
                let id = SceneId(
                    Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                );
                self.get_scene(id)
            })
            .collect()
    }
    pub fn create_snapshot(&mut self, scene: SceneId, label: &str) -> Result<SnapshotId> {
        let (text, hash) = self.load_text(scene)?;
        let id = SnapshotId::new();
        let relative = format!(".writers-ide/snapshots/{scene}/{id}.md");
        let target = self.safe_path(Path::new(&relative))?;
        fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| StorageError::Invalid("snapshot has no parent directory".into()))?,
        )?;
        atomic_write(&target, text.as_bytes())?;
        let result=self.conn.execute("INSERT INTO scene_snapshot(id,scene_id,label,relative_path,sha256,byte_length,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id.to_string(),scene.to_string(),label,relative,hash,text.len() as i64,now()]);
        if let Err(error) = result {
            let _ = fs::remove_file(target);
            return Err(error.into());
        }
        Ok(id)
    }
    pub fn list_snapshots(&self, scene: SceneId) -> Result<Vec<SnapshotRecord>> {
        let mut q=self.conn.prepare("SELECT id,label,sha256,created_at FROM scene_snapshot WHERE scene_id=?1 ORDER BY created_at DESC,id DESC")?;
        let rows = q.query_map([scene.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, label, sha256, created_at) = row?;
            Ok::<SnapshotRecord, rusqlite::Error>(SnapshotRecord {
                id: SnapshotId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                scene_id: scene,
                label,
                sha256,
                created_at,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn load_snapshot(&self, id: SnapshotId) -> Result<(SnapshotRecord, String)> {
        let row:(String,String,String,String,String)=self.conn.query_row("SELECT scene_id,label,relative_path,sha256,created_at FROM scene_snapshot WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?.ok_or_else(||StorageError::NotFound(id.to_string()))?;
        let scene =
            SceneId(Uuid::parse_str(&row.0).map_err(|e| StorageError::Invalid(e.to_string()))?);
        let text = fs::read_to_string(self.safe_path(Path::new(&row.2))?)?;
        if hash_bytes(text.as_bytes()) != row.3 {
            return Err(StorageError::Conflict(format!(
                "snapshot {id} was modified externally"
            )));
        }
        Ok((
            SnapshotRecord {
                id,
                scene_id: scene,
                label: row.1,
                sha256: row.3,
                created_at: row.4,
            },
            text,
        ))
    }
    pub fn diff_snapshot(
        &self,
        scene: SceneId,
        snapshot: SnapshotId,
    ) -> Result<Vec<SnapshotDiffLine>> {
        let (record, old) = self.load_snapshot(snapshot)?;
        if record.scene_id != scene {
            return Err(StorageError::Invalid(
                "snapshot belongs to a different scene".into(),
            ));
        }
        let (current, _) = self.load_text(scene)?;
        Ok(line_diff(&old, &current))
    }
    pub fn restore_snapshot(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
    ) -> Result<String> {
        let (record, text) = self.load_snapshot(snapshot)?;
        if record.scene_id != scene {
            return Err(StorageError::Invalid(
                "snapshot belongs to a different scene".into(),
            ));
        }
        self.create_snapshot(scene, &format!("Before restore: {}", record.label))?;
        self.save_text(scene, Some(expected_hash), &text)
    }
    pub fn restore_snapshot_lines(
        &mut self,
        scene: SceneId,
        snapshot: SnapshotId,
        expected_hash: &str,
        selection: SnapshotRestoreSelection,
    ) -> Result<String> {
        let SnapshotRestoreSelection {
            current_start,
            current_end,
            snapshot_start,
            snapshot_end,
        } = selection;
        let (record, old) = self.load_snapshot(snapshot)?;
        if record.scene_id != scene {
            return Err(StorageError::Invalid(
                "snapshot belongs to a different scene".into(),
            ));
        }
        let (current, hash) = self.load_text(scene)?;
        if hash != expected_hash {
            return Err(StorageError::Conflict(
                "scene changed before selective restore".into(),
            ));
        }
        let current_lines: Vec<&str> = current.split_inclusive('\n').collect();
        let old_lines: Vec<&str> = old.split_inclusive('\n').collect();
        if current_start > current_end
            || current_end > current_lines.len()
            || snapshot_start > snapshot_end
            || snapshot_end > old_lines.len()
        {
            return Err(StorageError::Invalid(
                "snapshot restore line range is out of bounds".into(),
            ));
        }
        let mut merged = String::new();
        for line in &current_lines[..current_start] {
            merged.push_str(line);
        }
        for line in &old_lines[snapshot_start..snapshot_end] {
            merged.push_str(line);
        }
        for line in &current_lines[current_end..] {
            merged.push_str(line);
        }
        self.create_snapshot(
            scene,
            &format!("Before selective restore: {}", record.label),
        )?;
        self.save_text(scene, Some(expected_hash), &merged)
    }
    pub fn update_scene(&mut self, scene: &Scene) -> Result<()> {
        StoryTime::new(
            scene.start_time.nominal,
            scene.start_time.lower,
            scene.start_time.upper,
            scene.start_time.precision,
        )
        .map_err(|e| StorageError::Invalid(e.to_string()))?;
        StoryTime::new(
            scene.end_time.nominal,
            scene.end_time.lower,
            scene.end_time.upper,
            scene.end_time.precision,
        )
        .map_err(|e| StorageError::Invalid(e.to_string()))?;
        let now = now();
        self.conn.execute("UPDATE scene SET title=?2,synopsis=?3,status=?4,target_words=?5,purpose=?6,goal=?7,outcome=?8,start_nominal=?9,start_lower=?10,start_upper=?11,start_precision=?12,end_nominal=?13,end_lower=?14,end_upper=?15,end_precision=?16,pov_character_id=?17,start_location_id=?18,end_location_id=?19,archived=?20,updated_at=?21 WHERE id=?1",params![scene.id.to_string(),scene.title,scene.synopsis,scene.status.as_str(),scene.target_words,scene.purpose,scene.goal,scene.outcome,scene.start_time.nominal,scene.start_time.lower,scene.start_time.upper,precision(scene.start_time.precision),scene.end_time.nominal,scene.end_time.lower,scene.end_time.upper,precision(scene.end_time.precision),scene.pov_character.map(|x|x.to_string()),scene.start_location.map(|x|x.to_string()),scene.end_location.map(|x|x.to_string()),scene.archived,now])?;
        Ok(())
    }
    pub fn move_scene(&mut self, id: SceneId, dest: NodeId, position: u32) -> Result<()> {
        self.move_scene_inner(id, dest, position, false)
    }
    fn move_scene_inner(
        &mut self,
        id: SceneId,
        dest: NodeId,
        position: u32,
        fail_after_staging: bool,
    ) -> Result<()> {
        let kind: Option<String> = self
            .conn
            .query_row(
                "SELECT kind FROM structural_node WHERE id=?1",
                [dest.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if kind.as_deref() != Some("chapter") {
            return Err(StorageError::Invalid("destination must be chapter".into()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source: Option<(String, u32)> = tx
            .query_row(
                "SELECT chapter_id,position FROM scene_placement WHERE scene_id=?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (src, old) = source.ok_or_else(|| StorageError::NotFound(id.to_string()))?;
        let mut src_ids: Vec<String> = query_scene_ids(&tx, &src)?;
        let mut dest_ids = if src == dest.to_string() {
            src_ids.clone()
        } else {
            query_scene_ids(&tx, &dest.to_string())?
        };
        src_ids.retain(|x| x != &id.to_string());
        dest_ids.retain(|x| x != &id.to_string());
        if position as usize > dest_ids.len() {
            return Err(StorageError::Invalid(
                "position beyond sibling count".into(),
            ));
        }
        dest_ids.insert(position as usize, id.to_string());
        let mut affected: Vec<String> = src_ids.clone();
        if src != dest.to_string() {
            affected.extend(dest_ids.iter().cloned());
        } else {
            affected = dest_ids.clone();
        }
        let max: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position),0) FROM scene_placement",
            [],
            |r| r.get(0),
        )?;
        let base = max
            .checked_mul(3)
            .and_then(|x| x.checked_add(100))
            .ok_or_else(|| StorageError::Invalid("position space exhausted".into()))?;
        // Two staging ranges keep all values non-negative and outside the live range.
        for sid in &affected {
            tx.execute(
                "UPDATE scene_placement SET position=position+?2 WHERE scene_id=?1",
                params![sid, base],
            )?;
        }
        for (i, sid) in affected.iter().enumerate() {
            tx.execute(
                "UPDATE scene_placement SET position=?2 WHERE scene_id=?1",
                params![sid, base * 2 + i as i64],
            )?;
        }
        if fail_after_staging {
            return Err(StorageError::Invalid("injected move rollback".into()));
        }
        if src != dest.to_string() {
            for (i, sid) in src_ids.iter().enumerate() {
                tx.execute(
                    "UPDATE scene_placement SET position=?2 WHERE scene_id=?1",
                    params![sid, i as i64],
                )?;
            }
        }
        for (i, sid) in dest_ids.iter().enumerate() {
            tx.execute(
                "UPDATE scene_placement SET chapter_id=?2,position=?3 WHERE scene_id=?1",
                params![sid, dest.to_string(), i as i64],
            )?;
        }
        tx.commit()?;
        let _ = old;
        Ok(())
    }
    pub fn archive_scene(&mut self, id: SceneId) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE scene SET archived=1,status='archived',updated_at=?2 WHERE id=?1",
            params![id.to_string(), now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }
    fn content_path(&self, id: SceneId) -> Result<PathBuf> {
        let p: String = self.conn.query_row(
            "SELECT relative_path FROM scene_content WHERE scene_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        Ok(PathBuf::from(p))
    }
    fn safe_path(&self, rel: &Path) -> Result<PathBuf> {
        let root = fs::canonicalize(&self.root)?;
        let mut path = root.clone();
        if rel.is_absolute() {
            return Err(StorageError::Invalid("unsafe project-relative path".into()));
        }
        for component in rel.components() {
            match component {
                std::path::Component::Normal(part) => path.push(part),
                std::path::Component::CurDir => {}
                _ => return Err(StorageError::Invalid("unsafe project-relative path".into())),
            }
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if meta.file_type().is_symlink() {
                    return Err(StorageError::Invalid(
                        "project-managed paths cannot traverse symlinks".into(),
                    ));
                }
                let canonical = fs::canonicalize(&path)?;
                if !canonical.starts_with(&root) {
                    return Err(StorageError::Invalid(
                        "project-managed path escaped project root".into(),
                    ));
                }
            }
        }
        Ok(path)
    }
    pub fn load_text(&self, id: SceneId) -> Result<(String, String)> {
        let rel = self.content_path(id)?;
        let p = self.safe_path(&rel)?;
        let bytes = fs::read(p)?;
        let hash = hash_bytes(&bytes);
        let expected: String = self.conn.query_row(
            "SELECT sha256 FROM scene_content WHERE scene_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        if hash != expected {
            return Err(StorageError::Conflict(format!(
                "external edit detected for scene {id}; expected {expected}, found {hash}"
            )));
        }
        Ok((
            String::from_utf8(bytes).map_err(|e| StorageError::Invalid(e.to_string()))?,
            hash,
        ))
    }
    pub fn save_text(&mut self, id: SceneId, expected: Option<&str>, text: &str) -> Result<String> {
        self.save_text_inner(id, expected, text, None)
    }
    fn save_text_inner(
        &mut self,
        id: SceneId,
        expected: Option<&str>,
        text: &str,
        fail: Option<SaveFailpoint>,
    ) -> Result<String> {
        let rel = self.content_path(id)?;
        let target = self.safe_path(&rel)?;
        let current: Option<String> = self
            .conn
            .query_row(
                "SELECT sha256 FROM scene_content WHERE scene_id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let current = current.ok_or_else(|| StorageError::NotFound(id.to_string()))?;
        if expected != Some(current.as_str()) {
            return Err(StorageError::Conflict("stale expected content hash".into()));
        }
        let actual = if target.exists() {
            hash_file(&target)?
        } else {
            String::new()
        };
        if actual != current {
            return Err(StorageError::Conflict(
                "manuscript changed outside Writer's IDE".into(),
            ));
        }
        let bytes = text.as_bytes();
        let new_hash = hash_bytes(bytes);
        let txid = Uuid::new_v4().to_string();
        let temp = target.with_file_name(format!(".{}.{}.tmp", id, txid));
        if fail == Some(SaveFailpoint::AfterPartialTemp) {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(&bytes[..bytes.len() / 2])?;
            f.sync_all()?;
            sync_dir(target.parent().unwrap())?;
            return Err(StorageError::Invalid(
                "injected crash during temporary write".into(),
            ));
        }
        write_synced(&temp, bytes)?;
        // Persist the candidate independently so a later external write cannot erase the
        // only recoverable copy while a journal transaction is pending.
        let candidate_rel = PathBuf::from(".writers-ide/recovery")
            .join(&txid)
            .join("candidate.md");
        let candidate = self.safe_path(&candidate_rel)?;
        fs::create_dir_all(candidate.parent().unwrap())?;
        write_synced(&candidate, bytes)?;
        sync_dir(target.parent().unwrap())?;
        sync_dir(candidate.parent().unwrap())?;
        sync_dir(candidate.parent().unwrap().parent().unwrap())?;
        if fail == Some(SaveFailpoint::AfterTempSync) {
            return Err(StorageError::Invalid(
                "injected crash after temp sync".into(),
            ));
        }
        let next_generation: i64 = self.conn.query_row(
            "SELECT generation+1 FROM scene_content WHERE scene_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute("INSERT INTO write_journal(tx_id,scene_id,target_path,temp_path,candidate_path,old_hash,new_hash,new_length,next_generation,state,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'prepared',?10)",params![txid,id.to_string(),rel.to_string_lossy(),temp.strip_prefix(&self.root).unwrap_or(&temp).to_string_lossy(),candidate_rel.to_string_lossy(),current,new_hash,bytes.len() as i64,next_generation,now()])?;
            tx.commit()?;
        }
        let pre_replace = if target.exists() {
            hash_file(&target)?
        } else {
            String::new()
        };
        if pre_replace != current {
            let _ = self.recover_pending_writes()?;
            return Err(StorageError::Conflict(
                "manuscript changed while save was being prepared; both versions were preserved"
                    .into(),
            ));
        }
        if fail == Some(SaveFailpoint::AfterIntent) || fail == Some(SaveFailpoint::BeforeRename) {
            return Err(StorageError::Invalid("injected crash after intent".into()));
        }
        fs::rename(&temp, &target)?;
        sync_dir(target.parent().unwrap())?;
        if fail == Some(SaveFailpoint::AfterRename) {
            return Err(StorageError::Invalid(
                "injected crash after replacement".into(),
            ));
        }
        {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "UPDATE scene_content SET sha256=?2,byte_length=?3,generation=?4 WHERE scene_id=?1",
                params![
                    id.to_string(),
                    new_hash,
                    bytes.len() as i64,
                    next_generation
                ],
            )?;
            tx.execute(
                "UPDATE write_journal SET state='db_committed' WHERE tx_id=?1",
                [&txid],
            )?;
            update_scene_index(&tx, id, text)?;
            if new_hash != current {
                revalidate_scene_anchors(&tx, id, text, &new_hash)?;
            }
            tx.commit()?;
        }
        if fail == Some(SaveFailpoint::AfterMetadataCommit)
            || fail == Some(SaveFailpoint::BeforeCleanup)
        {
            return Err(StorageError::Invalid(
                "injected crash after metadata commit".into(),
            ));
        }
        self.conn
            .execute("DELETE FROM write_journal WHERE tx_id=?1", [&txid])?;
        if fail == Some(SaveFailpoint::AfterJournalCleanup) {
            return Err(StorageError::Invalid(
                "injected crash during cleanup".into(),
            ));
        }
        let _ = fs::remove_file(&candidate);
        let _ = sync_dir(candidate.parent().unwrap());
        let _ = fs::remove_dir(candidate.parent().unwrap());
        let _ = sync_dir(candidate.parent().unwrap().parent().unwrap());
        Ok(new_hash)
    }
    pub fn recover_pending_writes(&mut self) -> Result<Vec<RecoveryConflict>> {
        let entries: Vec<WriteJournalRow> = {
            let mut q=self.conn.prepare("SELECT j.tx_id,j.scene_id,j.target_path,j.temp_path,j.candidate_path,j.old_hash,j.new_hash,j.new_length,j.next_generation,j.state,COALESCE(c.decision,0),c.tx_id IS NOT NULL FROM write_journal j LEFT JOIN write_conflict c ON c.tx_id=j.tx_id AND c.resolved=0 ORDER BY j.created_at")?;
            q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, i64>(10)?,
                    r.get::<_, bool>(11)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut conflicts = Vec::new();
        for (
            txid,
            sid,
            target_rel,
            temp_rel,
            candidate_rel,
            old,
            new,
            len,
            generation,
            state,
            decision,
            has_conflict,
        ) in entries
        {
            let target = self.safe_path(Path::new(&target_rel))?;
            let temp = self.safe_path(Path::new(&temp_rel))?;
            let candidate = self.safe_path(Path::new(&candidate_rel))?;
            let target_hash = if target.exists() {
                Some(hash_file(&target)?)
            } else {
                None
            };
            let temp_hash = if temp.exists() {
                Some(hash_file(&temp)?)
            } else {
                None
            };
            let candidate_hash = if candidate.exists() {
                Some(hash_file(&candidate)?)
            } else {
                None
            };
            if decision == 1 {
                let recovery = self.root.join(".writers-ide/recovery").join(&txid);
                let prepared = recovery.join("prepared-version.md");
                let chosen = if prepared.exists() {
                    prepared
                } else {
                    candidate.clone()
                };
                let bytes = fs::read(chosen)?;
                let resolve_temp =
                    target.with_file_name(format!(".resolve-{}.tmp", Uuid::new_v4()));
                write_synced(&resolve_temp, &bytes)?;
                fs::rename(&resolve_temp, &target)?;
                sync_dir(target.parent().unwrap())?;
                let actual = hash_bytes(&bytes);
                let actual_len = bytes.len() as i64;
                let generation: i64 = self.conn.query_row(
                    "SELECT generation+1 FROM scene_content WHERE scene_id=?1",
                    [&sid],
                    |r| r.get(0),
                )?;
                self.finish_conflict_resolution(&txid, &sid, &actual, actual_len, generation)?;
            } else if decision == 2 {
                let actual = hash_file(&target)?;
                let actual_len = fs::metadata(&target)?.len() as i64;
                let generation: i64 = self.conn.query_row(
                    "SELECT generation+1 FROM scene_content WHERE scene_id=?1",
                    [&sid],
                    |r| r.get(0),
                )?;
                self.finish_conflict_resolution(&txid, &sid, &actual, actual_len, generation)?;
            } else if target_hash.as_deref() == Some(&new) && !has_conflict {
                self.commit_recovery_metadata(&txid, &sid, &new, len, generation)?;
                if temp.exists() {
                    fs::remove_file(&temp)?;
                }
                self.conn
                    .execute("DELETE FROM write_journal WHERE tx_id=?1", [&txid])?;
                let _ = fs::remove_file(&candidate);
                let _ = sync_dir(candidate.parent().unwrap());
                let _ = fs::remove_dir(candidate.parent().unwrap());
                let _ = sync_dir(candidate.parent().unwrap().parent().unwrap());
            } else if target_hash.as_deref() == old.as_deref()
                && temp_hash.as_deref() == Some(&new)
                && !has_conflict
            {
                fs::rename(&temp, &target)?;
                sync_dir(target.parent().unwrap())?;
                self.commit_recovery_metadata(&txid, &sid, &new, len, generation)?;
                self.conn
                    .execute("DELETE FROM write_journal WHERE tx_id=?1", [&txid])?;
                let _ = fs::remove_file(&candidate);
                let _ = sync_dir(candidate.parent().unwrap());
                let _ = fs::remove_dir(candidate.parent().unwrap());
                let _ = sync_dir(candidate.parent().unwrap().parent().unwrap());
            } else if state == "prepared"
                && target_hash.as_deref() == old.as_deref()
                && temp_hash.is_none()
                && !has_conflict
            {
                self.conn
                    .execute("DELETE FROM write_journal WHERE tx_id=?1", [&txid])?;
                let _ = fs::remove_file(&candidate);
                let _ = sync_dir(candidate.parent().unwrap());
                let _ = fs::remove_dir(candidate.parent().unwrap());
                let _ = sync_dir(candidate.parent().unwrap().parent().unwrap());
            } else {
                let recovery = self.root.join(".writers-ide/recovery").join(&txid);
                fs::create_dir_all(&recovery)?;
                if target.exists() {
                    fs::copy(&target, recovery.join("external-version.md"))?;
                }
                if candidate.exists() {
                    fs::copy(&candidate, recovery.join("prepared-version.md"))?;
                } else if temp.exists() {
                    fs::copy(&temp, recovery.join("prepared-version.md"))?;
                }
                sync_dir(&recovery)?;
                let actual = target_hash.clone().unwrap_or_default();
                self.conn.execute("INSERT OR IGNORE INTO write_conflict(tx_id,scene_id,recovery_path,expected_hash,actual_hash,candidate_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![txid,sid,recovery.strip_prefix(&self.root).unwrap().to_string_lossy(),old,actual,candidate_hash.unwrap_or(new),now()])?;
                conflicts.push(RecoveryConflict {
                    transaction_id: txid,
                    scene_id: sid,
                    recovery_path: recovery,
                });
            }
        }
        self.cleanup_orphan_write_files()?;
        Ok(conflicts)
    }
    pub fn resolve_write_conflict(
        &mut self,
        txid: &str,
        resolution: ConflictResolution,
    ) -> Result<()> {
        let decision = if resolution == ConflictResolution::UsePrepared {
            1
        } else {
            2
        };
        let changed = self.conn.execute(
            "UPDATE write_conflict SET decision=?2 WHERE tx_id=?1 AND resolved=0",
            params![txid, decision],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(txid.into()));
        }
        self.recover_pending_writes()?;
        let resolved: i64 = self.conn.query_row(
            "SELECT resolved FROM write_conflict WHERE tx_id=?1",
            [txid],
            |r| r.get(0),
        )?;
        if resolved != 1 {
            return Err(StorageError::RecoveryConflict(txid.into()));
        }
        Ok(())
    }
    pub fn pending_conflicts(&self) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM write_conflict WHERE resolved=0",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn list_recovery_conflicts(&self) -> Result<Vec<RecoveryConflict>> {
        let mut q=self.conn.prepare("SELECT tx_id,scene_id,recovery_path FROM write_conflict WHERE resolved=0 ORDER BY created_at")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (tx, scene, path) = row?;
            out.push(RecoveryConflict {
                transaction_id: tx,
                scene_id: scene,
                recovery_path: self.safe_path(Path::new(&path))?,
            });
        }
        Ok(out)
    }
    fn cleanup_orphan_write_files(&self) -> Result<()> {
        let mut active = std::collections::HashSet::new();
        {
            let mut q=self.conn.prepare("SELECT tx_id FROM write_journal UNION SELECT tx_id FROM write_conflict WHERE resolved=0")?;
            for id in q.query_map([], |r| r.get::<_, String>(0))? {
                active.insert(id?);
            }
        }
        let recovery = self.root.join(".writers-ide/recovery");
        let mut recovery_changed = false;
        for entry in fs::read_dir(&recovery)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if Uuid::parse_str(&name).is_ok() && !active.contains(&name) {
                let dir = entry.path();
                let candidate = dir.join("candidate.md");
                if candidate.exists() {
                    fs::remove_file(candidate)?;
                    recovery_changed = true;
                }
                if fs::remove_dir(dir).is_ok() {
                    recovery_changed = true;
                }
            }
        }
        if recovery_changed {
            sync_dir(&recovery)?;
        }
        let manuscript = self.root.join("manuscript");
        let mut changed = false;
        for entry in fs::read_dir(&manuscript)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(body) = name.strip_prefix('.').and_then(|s| s.strip_suffix(".tmp"))
                && let Some((scene, tx)) = body.split_once('.')
                && Uuid::parse_str(scene).is_ok()
                && Uuid::parse_str(tx).is_ok()
                && !active.contains(tx)
            {
                fs::remove_file(entry.path())?;
                changed = true;
            }
        }
        if changed {
            sync_dir(&manuscript)?;
        }
        Ok(())
    }
    fn commit_recovery_metadata(
        &mut self,
        txid: &str,
        sid: &str,
        new: &str,
        len: i64,
        generation: i64,
    ) -> Result<()> {
        let scene =
            SceneId(Uuid::parse_str(sid).map_err(|e| StorageError::Invalid(e.to_string()))?);
        let text = fs::read_to_string(self.safe_path(&self.content_path(scene)?)?)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE scene_content SET sha256=?2,byte_length=?3,generation=?4 WHERE scene_id=?1",
            params![sid, new, len, generation],
        )?;
        update_scene_index(&tx, scene, &text)?;
        revalidate_scene_anchors(&tx, scene, &text, new)?;
        tx.execute(
            "UPDATE write_journal SET state='db_committed' WHERE tx_id=?1",
            [txid],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn finish_conflict_resolution(
        &mut self,
        txid: &str,
        sid: &str,
        hash: &str,
        len: i64,
        generation: i64,
    ) -> Result<()> {
        let scene =
            SceneId(Uuid::parse_str(sid).map_err(|e| StorageError::Invalid(e.to_string()))?);
        let text = fs::read_to_string(self.safe_path(&self.content_path(scene)?)?)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE scene_content SET sha256=?2,byte_length=?3,generation=?4 WHERE scene_id=?1",
            params![sid, hash, len, generation],
        )?;
        update_scene_index(&tx, scene, &text)?;
        revalidate_scene_anchors(&tx, scene, &text, hash)?;
        tx.execute("DELETE FROM write_journal WHERE tx_id=?1", [txid])?;
        tx.execute(
            "UPDATE write_conflict SET resolved=1,decision=0 WHERE tx_id=?1",
            [txid],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn add_character(&mut self, id: CharacterId, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO character VALUES(?1,?2)",
            params![id.to_string(), name],
        )?;
        Ok(())
    }
    pub fn list_characters(&self) -> Result<Vec<CharacterSummary>> {
        let mut q=self.conn.prepare("SELECT c.id,c.name,COUNT(DISTINCT sc.scene_id) FROM character c LEFT JOIN scene_character sc ON sc.character_id=c.id GROUP BY c.id,c.name ORDER BY c.name COLLATE NOCASE,c.id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, scene_count) = row?;
            Ok::<CharacterSummary, rusqlite::Error>(CharacterSummary {
                id: CharacterId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                name,
                scene_count,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn add_plotline(&mut self, id: PlotlineId, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO plotline VALUES(?1,?2)",
            params![id.to_string(), name],
        )?;
        Ok(())
    }
    pub fn list_plotlines(&self) -> Result<Vec<PlotlineSummary>> {
        let mut q=self.conn.prepare("SELECT p.id,p.name,COUNT(DISTINCT sp.scene_id) FROM plotline p LEFT JOIN scene_plotline sp ON sp.plotline_id=p.id GROUP BY p.id,p.name ORDER BY p.name COLLATE NOCASE,p.id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, scene_count) = row?;
            Ok::<PlotlineSummary, rusqlite::Error>(PlotlineSummary {
                id: PlotlineId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                name,
                scene_count,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn scene_character_analytics(
        &self,
        scene: SceneId,
    ) -> Result<Vec<(String, String, String)>> {
        let mut q = self.conn.prepare("SELECT c.id,c.name,sc.participation_role FROM scene_character sc JOIN character c ON c.id=sc.character_id WHERE sc.scene_id=?1 ORDER BY c.name COLLATE NOCASE,c.id")?;
        Ok(q.query_map([scene.to_string()], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<std::result::Result<_, _>>()?)
    }
    pub fn scene_plotline_analytics(&self, scene: SceneId) -> Result<Vec<(String, String)>> {
        let mut q = self.conn.prepare("SELECT p.id,p.name FROM scene_plotline sp JOIN plotline p ON p.id=sp.plotline_id WHERE sp.scene_id=?1 ORDER BY p.name COLLATE NOCASE,p.id")?;
        Ok(
            q.query_map([scene.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?,
        )
    }
    pub fn link_character(
        &mut self,
        scene: SceneId,
        char: CharacterId,
        role: ParticipationRole,
    ) -> Result<()> {
        self.conn.execute("INSERT INTO scene_character(scene_id,character_id,participation_role) VALUES(?1,?2,?3)",params![scene.to_string(),char.to_string(),role.as_str()])?;
        Ok(())
    }
    pub fn link_plotline(
        &mut self,
        scene: SceneId,
        plot: PlotlineId,
        role: &str,
        share: Option<u16>,
    ) -> Result<()> {
        self.conn.execute("INSERT INTO scene_plotline(scene_id,plotline_id,role,share_basis_points) VALUES(?1,?2,?3,?4)",params![scene.to_string(),plot.to_string(),role,share])?;
        Ok(())
    }
    pub fn set_scene_plotlines(
        &mut self,
        scene: SceneId,
        links: &[ScenePlotline],
        policy: WordAttributionPolicy,
    ) -> Result<()> {
        if policy == WordAttributionPolicy::AuthorShares && !links.is_empty() {
            validate_shares(links).map_err(|e| StorageError::Invalid(e.to_string()))?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM scene_plotline WHERE scene_id=?1",
            [scene.to_string()],
        )?;
        for link in links {
            tx.execute("INSERT INTO scene_plotline(scene_id,plotline_id,role,share_basis_points) VALUES(?1,?2,?3,?4)",params![scene.to_string(),link.plotline_id.to_string(),link.role,link.share_basis_points])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn scene_plotline_links(&self, scene: SceneId) -> Result<Vec<ScenePlotline>> {
        let mut query = self.conn.prepare("SELECT plotline_id,role,share_basis_points FROM scene_plotline WHERE scene_id=?1 ORDER BY plotline_id")?;
        query
            .query_map([scene.to_string()], |row| {
                let id: String = row.get(0)?;
                Ok(ScenePlotline {
                    plotline_id: PlotlineId(Uuid::parse_str(&id).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?),
                    role: row.get(1)?,
                    share_basis_points: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn record_import_run(
        &mut self,
        id: ImportRunId,
        source_type: &str,
        source_identifier: &str,
        fingerprint: &str,
    ) -> Result<()> {
        let existing: i64 = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_run WHERE source_fingerprint=?1)",
            [fingerprint],
            |r| r.get(0),
        )?;
        if existing != 0 {
            return Err(StorageError::Invalid(
                "source fingerprint already has an import run; require explicit repeat policy"
                    .into(),
            ));
        }
        self.conn.execute("INSERT INTO import_run(id,source_type,source_identifier,source_fingerprint,started_at,status) VALUES(?1,?2,?3,?4,?5,'inventory')",params![id.to_string(),source_type,source_identifier,fingerprint,now()])?;
        Ok(())
    }
    pub fn record_source_item(&mut self, item: ImportSourceItemRecord<'_>) -> Result<()> {
        self.conn.execute("INSERT INTO import_source_item(id,run_id,source_document_id,source_sheet,source_row,item_fingerprint,inventory_type,raw_fields) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![item.id.to_string(),item.run.to_string(),item.source_document_id,item.source_sheet,item.source_row,item.fingerprint,item.inventory_type,item.raw_fields])?;
        Ok(())
    }
    pub fn record_provenance(&mut self, record: EntityProvenanceRecord<'_>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO entity_provenance VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                record.id.to_string(),
                record.source_item.to_string(),
                record.destination_type,
                record.destination_id,
                record.source_field,
                record.destination_field,
                record.mapping
            ],
        )?;
        Ok(())
    }
    pub fn record_import_run_explicit_repeat(
        &mut self,
        id: ImportRunId,
        source_type: &str,
        source_identifier: &str,
        fingerprint: &str,
        allow_repeat: bool,
    ) -> Result<()> {
        let exists: i64 = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_run WHERE source_fingerprint=?1)",
            [fingerprint],
            |r| r.get(0),
        )?;
        if exists != 0 && !allow_repeat {
            return Err(StorageError::Invalid(
                "repeat import requires explicit policy".into(),
            ));
        }
        self.conn.execute("INSERT INTO import_run(id,source_type,source_identifier,source_fingerprint,started_at,status) VALUES(?1,?2,?3,?4,?5,'inventory')",params![id.to_string(),source_type,source_identifier,fingerprint,now()])?;
        Ok(())
    }
    pub fn finish_import_run(
        &mut self,
        id: ImportRunId,
        status: &str,
        warnings: &str,
        unsupported: &str,
    ) -> Result<()> {
        if !matches!(status, "completed" | "failed" | "incomplete") {
            return Err(StorageError::Invalid(
                "invalid import terminal status".into(),
            ));
        }
        let changed=self.conn.execute("UPDATE import_run SET status=?2,warnings=?3,unsupported=?4,completed_at=?5 WHERE id=?1",params![id.to_string(),status,warnings,unsupported,now()])?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }
    pub fn add_event(&mut self, id: EventId, title: &str, source_time: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO story_event(id,title,source_time_text) VALUES(?1,?2,?3)",
            params![id.to_string(), title, source_time],
        )?;
        Ok(())
    }
    pub fn add_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        validate_time(event.start)?;
        validate_time(event.end)?;
        self.conn.execute("INSERT INTO story_event(id,title,description,time_nominal,time_lower,time_upper,time_precision,end_nominal,end_lower,end_upper,end_precision,source_time_text) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![event.id.to_string(),event.title,event.description,event.start.nominal,event.start.lower,event.start.upper,precision(event.start.precision),event.end.nominal,event.end.lower,event.end.upper,precision(event.end.precision),event.source_time_text])?;
        Ok(())
    }
    pub fn list_events(&self) -> Result<Vec<EventSummary>> {
        let mut q=self.conn.prepare("SELECT id,title,time_nominal,source_time_text FROM story_event ORDER BY time_nominal IS NULL,time_nominal,id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, title, nominal_time, source_time) = row?;
            Ok::<EventSummary, rusqlite::Error>(EventSummary {
                id: EventId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                title,
                nominal_time,
                source_time,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn link_event(&mut self, scene: SceneId, event: EventId, kind: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO scene_event(scene_id,event_id,relationship_kind) VALUES(?1,?2,?3)",
            params![scene.to_string(), event.to_string(), kind],
        )?;
        Ok(())
    }
    pub fn scene_event_links(&self, scene: SceneId) -> Result<Vec<(EventId, String)>> {
        let mut query = self.conn.prepare("SELECT event_id,relationship_kind FROM scene_event WHERE scene_id=?1 ORDER BY event_id,relationship_kind")?;
        query
            .query_map([scene.to_string()], |row| {
                let id: String = row.get(0)?;
                Ok((
                    EventId(Uuid::parse_str(&id).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?),
                    row.get(1)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn create_anchor(
        &mut self,
        scene: SceneId,
        start_byte: u64,
        end_byte: u64,
        quote: &str,
        prefix: &str,
        suffix: &str,
    ) -> Result<AnchorId> {
        let (text, hash) = self.load_text(scene)?;
        let start = usize::try_from(start_byte)
            .map_err(|_| StorageError::Invalid("anchor offset overflow".into()))?;
        let end = usize::try_from(end_byte)
            .map_err(|_| StorageError::Invalid("anchor offset overflow".into()))?;
        if quote.is_empty()
            || start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
            || text.get(start..end) != Some(quote)
        {
            return Err(StorageError::Invalid(
                "anchor offsets must select the supplied UTF-8 passage".into(),
            ));
        }
        let id = AnchorId::new();
        self.conn.execute("INSERT INTO passage_anchor(id,scene_id,start_byte,end_byte,quote,prefix_context,suffix_context,source_sha256,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)", params![id.to_string(),scene.to_string(),start_byte,end_byte,quote,prefix,suffix,hash,now()])?;
        Ok(id)
    }
    pub fn list_scene_anchors(&self, scene: SceneId) -> Result<Vec<PassageAnchorRecord>> {
        let mut query = self.conn.prepare("SELECT id,start_byte,end_byte,quote,state,source_sha256 FROM passage_anchor WHERE scene_id=?1 ORDER BY start_byte,id")?;
        Ok(query
            .query_map([scene.to_string()], |row| {
                let id: String = row.get(0)?;
                Ok(PassageAnchorRecord {
                    id: AnchorId(Uuid::parse_str(&id).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?),
                    scene_id: scene,
                    start_byte: row.get(1)?,
                    end_byte: row.get(2)?,
                    quote: row.get(3)?,
                    state: row.get(4)?,
                    source_hash: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn reanchor_passage(
        &mut self,
        anchor: AnchorId,
        start_byte: u64,
        end_byte: u64,
    ) -> Result<()> {
        let scene_text: (String, String) = self.conn.query_row(
            "SELECT c.relative_path,c.sha256 FROM passage_anchor a JOIN scene_content c ON c.scene_id=a.scene_id WHERE a.id=?1",
            [anchor.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?.ok_or_else(|| StorageError::NotFound(anchor.to_string()))?;
        let text = fs::read_to_string(self.safe_path(Path::new(&scene_text.0))?)?;
        let start = usize::try_from(start_byte)
            .map_err(|_| StorageError::Invalid("anchor offset overflow".into()))?;
        let end = usize::try_from(end_byte)
            .map_err(|_| StorageError::Invalid("anchor offset overflow".into()))?;
        let quote: String = self.conn.query_row(
            "SELECT quote FROM passage_anchor WHERE id=?1",
            [anchor.to_string()],
            |row| row.get(0),
        )?;
        if start >= end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
            || text.get(start..end) != Some(quote.as_str())
        {
            return Err(StorageError::Invalid(
                "reanchor range must select the anchor's exact UTF-8 passage".into(),
            ));
        }
        self.conn.execute("UPDATE passage_anchor SET start_byte=?2,end_byte=?3,source_sha256=?4,state='current',updated_at=?5 WHERE id=?1", params![anchor.to_string(),start_byte,end_byte,scene_text.1,now()])?;
        Ok(())
    }
    pub fn create_story_link(&mut self, link: &StoryLink) -> Result<()> {
        if link.source_anchor.is_some() && !matches!(link.source, EntityRef::Scene(_)) {
            return Err(StorageError::Invalid(
                "passage anchors can only belong to scenes".into(),
            ));
        }
        if link.target_anchor.is_some() && !matches!(link.target, EntityRef::Scene(_)) {
            return Err(StorageError::Invalid(
                "passage anchors can only belong to scenes".into(),
            ));
        }
        self.conn.execute("INSERT INTO story_link(id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![link.id.to_string(),link.source.kind(),link.source.id(),link.source_anchor.map(|x|x.to_string()),link.target.kind(),link.target.id(),link.target_anchor.map(|x|x.to_string()),link.kind.as_str(),link.label,now()])?;
        Ok(())
    }
    pub fn create_research_item(
        &mut self,
        title: &str,
        body: &str,
        media_type: &str,
        source_path: Option<&str>,
        source_sha256: Option<&str>,
    ) -> Result<ResearchId> {
        let id = ResearchId::new();
        self.conn.execute("INSERT INTO research_item(id,title,body,media_type,source_path,source_sha256,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",params![id.to_string(),title,body,media_type,source_path,source_sha256,now()])?;
        Ok(id)
    }
    pub fn list_research_items(&self) -> Result<Vec<ResearchSummary>> {
        let mut q=self.conn.prepare("SELECT id,title,media_type,source_path FROM research_item ORDER BY title COLLATE NOCASE,id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, title, media_type, source_path) = row?;
            Ok::<ResearchSummary, rusqlite::Error>(ResearchSummary {
                id: ResearchId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                title,
                media_type,
                source_path,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn create_issue(&mut self, title: &str) -> Result<IssueId> {
        let id = IssueId::new();
        self.conn.execute(
            "INSERT INTO story_issue(id,title,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![id.to_string(), title, now()],
        )?;
        Ok(id)
    }
    pub fn set_issue_status(&mut self, issue: IssueId, status: &str) -> Result<()> {
        if !matches!(
            status,
            "open" | "in_progress" | "resolved" | "deferred" | "closed"
        ) {
            return Err(StorageError::Invalid(format!(
                "invalid issue status: {status}"
            )));
        }
        let changed = self.conn.execute(
            "UPDATE story_issue SET status=?2,updated_at=?3 WHERE id=?1",
            params![issue.to_string(), status, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(issue.to_string()));
        }
        Ok(())
    }
    pub fn link_issue(&mut self, scene: SceneId, issue: IssueId) -> Result<()> {
        self.conn.execute(
            "INSERT INTO scene_issue(scene_id,issue_id) VALUES(?1,?2)",
            params![scene.to_string(), issue.to_string()],
        )?;
        Ok(())
    }
    pub fn list_issues(&self) -> Result<Vec<IssueSummary>> {
        let mut q=self.conn.prepare("SELECT i.id,i.title,i.status,COUNT(DISTINCT si.scene_id) FROM story_issue i LEFT JOIN scene_issue si ON si.issue_id=i.id GROUP BY i.id,i.title,i.status ORDER BY CASE i.status WHEN 'open' THEN 0 WHEN 'in_progress' THEN 1 WHEN 'deferred' THEN 2 WHEN 'resolved' THEN 3 ELSE 4 END,i.created_at")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u32>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, title, status, scene_count) = row?;
            Ok::<IssueSummary, rusqlite::Error>(IssueSummary {
                id: IssueId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                title,
                status,
                scene_count,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn expand_scene_transclusions(&self, root: SceneId) -> Result<Vec<TranscludedPassage>> {
        fn visit(
            store: &ProjectStore,
            scene: SceneId,
            depth: usize,
            seen: &mut std::collections::BTreeSet<SceneId>,
            output: &mut Vec<TranscludedPassage>,
        ) -> Result<()> {
            if !seen.insert(scene) {
                return Err(StorageError::Invalid(
                    "transclusion cycle encountered during rendering".into(),
                ));
            }
            output.push(TranscludedPassage {
                scene_id: scene,
                text: store.load_text(scene)?.0,
                depth,
            });
            let mut q=store.conn.prepare("SELECT target_id FROM story_link WHERE source_kind='scene' AND source_id=?1 AND target_kind='scene' AND link_kind IN('transcludes','includes') ORDER BY created_at,id")?;
            let targets = q
                .query_map([scene.to_string()], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for target in targets {
                let id = SceneId(
                    Uuid::parse_str(&target).map_err(|e| StorageError::Invalid(e.to_string()))?,
                );
                visit(store, id, depth + 1, seen, output)?;
            }
            seen.remove(&scene);
            Ok(())
        }
        let mut result = Vec::new();
        visit(
            self,
            root,
            0,
            &mut std::collections::BTreeSet::new(),
            &mut result,
        )?;
        Ok(result)
    }
    pub fn backlinks(&self, target: EntityRef) -> Result<Vec<StoryLink>> {
        let mut q = self.conn.prepare("SELECT id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label FROM story_link WHERE target_kind=?1 AND target_id=?2 ORDER BY created_at,id")?;
        let rows = q.query_map(params![target.kind(), target.id()], read_story_link)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn import_reading_document(
        &mut self,
        document: &story_reading::ReferenceDocument,
        source_identifier: &str,
    ) -> Result<ReadingId> {
        let expected = format!("{:x}", Sha256::digest(&document.source_bytes));
        if expected != document.sha256 {
            return Err(StorageError::Invalid(
                "reading source fingerprint does not match bytes".into(),
            ));
        }
        let id = ReadingId::new();
        let ext = match document.media_type.as_str() {
            "text/plain" => "txt",
            "text/markdown" => "md",
            "application/epub+zip" => "epub",
            _ => {
                return Err(StorageError::Invalid(
                    "unsupported reading media type".into(),
                ));
            }
        };
        let relative = format!("research/readings/{}-{expected}.{ext}", id);
        let target = self.safe_path(Path::new(&relative))?;
        atomic_write(&target, &document.source_bytes)?;
        let stamp = now();
        let result=self.conn.execute("INSERT INTO reading_document(id,title,media_type,relative_path,source_identifier,source_sha256,extracted_text,imported_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![id.to_string(),document.title,document.media_type,relative,source_identifier,expected,document.extracted_text,stamp]);
        if let Err(error) = result {
            let _ = fs::remove_file(&target);
            return Err(error.into());
        }
        Ok(id)
    }
    pub fn get_reading_document(&self, id: ReadingId) -> Result<ReadingDocumentRecord> {
        let row:(String,String,String,String,String,String)=self.conn.query_row("SELECT title,media_type,relative_path,source_identifier,source_sha256,extracted_text FROM reading_document WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?.ok_or_else(||StorageError::NotFound(id.to_string()))?;
        let source = fs::read(self.safe_path(Path::new(&row.2))?)?;
        if format!("{:x}", Sha256::digest(&source)) != row.4 {
            return Err(StorageError::Conflict(format!(
                "reading source {id} was modified externally"
            )));
        }
        Ok(ReadingDocumentRecord {
            id,
            title: row.0,
            media_type: row.1,
            relative_path: row.2,
            source_identifier: row.3,
            source_sha256: row.4,
            extracted_text: row.5,
        })
    }
    pub fn list_reading_documents(&self) -> Result<Vec<ReadingDocumentRecord>> {
        let ids: Vec<String> = {
            let mut q = self
                .conn
                .prepare("SELECT id FROM reading_document ORDER BY title COLLATE NOCASE,id")?;
            q.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        ids.into_iter()
            .map(|id| {
                let id = ReadingId(
                    Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                );
                self.get_reading_document(id)
            })
            .collect()
    }
    pub fn reload_reading_document(
        &mut self,
        id: ReadingId,
        document: &story_reading::ReferenceDocument,
    ) -> Result<()> {
        let expected = format!("{:x}", Sha256::digest(&document.source_bytes));
        if expected != document.sha256 {
            return Err(StorageError::Invalid(
                "reading source fingerprint does not match bytes".into(),
            ));
        }
        let existing = self.get_reading_document(id)?;
        let ext = match document.media_type.as_str() {
            "text/plain" => "txt",
            "text/markdown" => "md",
            "application/epub+zip" => "epub",
            _ => {
                return Err(StorageError::Invalid(
                    "unsupported reading media type".into(),
                ));
            }
        };
        let relative = format!("research/readings/{}-{expected}.{ext}", id);
        let target = self.safe_path(Path::new(&relative))?;
        if existing.source_sha256 != expected {
            atomic_write(&target, &document.source_bytes)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE reading_document SET title=?2,media_type=?3,relative_path=?4,source_sha256=?5,extracted_text=?6,updated_at=?7 WHERE id=?1",params![id.to_string(),document.title,document.media_type,relative,expected,document.extracted_text,now()])?;
        if existing.source_sha256 != expected {
            let annotations: Vec<(String, String)> = {
                let mut q =
                    tx.prepare("SELECT id,quote FROM reading_annotation WHERE document_id=?1")?;
                q.query_map([id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for (annotation, quote) in annotations {
                let found = unique_occurrence(&document.extracted_text, &quote);
                if let Some((start, end)) = found {
                    tx.execute("UPDATE reading_annotation SET start_byte=?2,end_byte=?3,source_sha256=?4,state='current',updated_at=?5 WHERE id=?1",params![annotation,start,end,expected,now()])?;
                } else {
                    tx.execute("UPDATE reading_annotation SET source_sha256=?2,state='needs_review',updated_at=?3 WHERE id=?1",params![annotation,expected,now()])?;
                }
            }
            let bookmarks: Vec<(String, String)> = {
                let mut q =
                    tx.prepare("SELECT id,quote FROM reading_bookmark WHERE document_id=?1")?;
                q.query_map([id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for (bookmark, quote) in bookmarks {
                if let Some((start, _)) = unique_occurrence(&document.extracted_text, &quote) {
                    tx.execute("UPDATE reading_bookmark SET offset_byte=?2,source_sha256=?3,state='current' WHERE id=?1",params![bookmark,start,expected])?;
                } else {
                    tx.execute("UPDATE reading_bookmark SET source_sha256=?2,state='needs_review' WHERE id=?1",params![bookmark,expected])?;
                }
            }
            tx.execute("UPDATE reading_checkpoint SET state='needs_review',source_sha256=?2,updated_at=?3 WHERE document_id=?1",params![id.to_string(),expected,now()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn search_reading(&self, terms: &str) -> Result<Vec<(ReadingId, String, String)>> {
        if terms.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut q=self.conn.prepare("SELECT document_id,title,snippet(reading_fts,2,'[',']','…',12) FROM reading_fts WHERE reading_fts MATCH ?1 ORDER BY bm25(reading_fts),title")?;
        let rows = q.query_map([terms], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, title, snippet) = row?;
            Ok::<(ReadingId, String, String), rusqlite::Error>((
                ReadingId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                title,
                snippet,
            ))
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn create_reading_annotation(
        &mut self,
        id: ReadingId,
        draft: &ReadingAnnotationDraft,
    ) -> Result<AnnotationId> {
        let kind = draft.kind.as_str();
        let start = draft.start_byte;
        let end = draft.end_byte;
        let quote = draft.quote.as_str();
        let body = draft.body.as_str();
        let prefix = draft.prefix_context.as_str();
        let suffix = draft.suffix_context.as_str();
        if !matches!(kind, "highlight" | "note") {
            return Err(StorageError::Invalid(
                "annotation kind must be highlight or note".into(),
            ));
        }
        let doc = self.get_reading_document(id)?;
        let a = usize::try_from(start)
            .map_err(|_| StorageError::Invalid("annotation offset overflow".into()))?;
        let b = usize::try_from(end)
            .map_err(|_| StorageError::Invalid("annotation offset overflow".into()))?;
        if a > b
            || b > doc.extracted_text.len()
            || !doc.extracted_text.is_char_boundary(a)
            || !doc.extracted_text.is_char_boundary(b)
            || doc.extracted_text.get(a..b) != Some(quote)
        {
            return Err(StorageError::Invalid(
                "annotation range must select its quoted UTF-8 passage".into(),
            ));
        }
        let aid = AnnotationId::new();
        self.conn.execute("INSERT INTO reading_annotation(id,document_id,annotation_kind,start_byte,end_byte,quote,prefix_context,suffix_context,source_sha256,body,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",params![aid.to_string(),id.to_string(),kind,start,end,quote,prefix,suffix,doc.source_sha256,body,now()])?;
        Ok(aid)
    }
    pub fn list_reading_annotations(&self, id: ReadingId) -> Result<Vec<ReadingAnnotationRecord>> {
        let mut q=self.conn.prepare("SELECT id,annotation_kind,start_byte,end_byte,quote,body,state FROM reading_annotation WHERE document_id=?1 ORDER BY start_byte,id")?;
        let rows = q.query_map([id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u64>(2)?,
                r.get::<_, u64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (aid, kind, start_byte, end_byte, quote, body, state) = row?;
            Ok::<ReadingAnnotationRecord, rusqlite::Error>(ReadingAnnotationRecord {
                id: AnnotationId(Uuid::parse_str(&aid).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                document_id: id,
                kind,
                start_byte,
                end_byte,
                quote,
                body,
                state,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn create_reading_bookmark(
        &mut self,
        id: ReadingId,
        title: &str,
        offset: u64,
    ) -> Result<BookmarkId> {
        let doc = self.get_reading_document(id)?;
        let pos = usize::try_from(offset)
            .map_err(|_| StorageError::Invalid("bookmark offset overflow".into()))?;
        if pos > doc.extracted_text.len() || !doc.extracted_text.is_char_boundary(pos) {
            return Err(StorageError::Invalid(
                "bookmark must use a valid UTF-8 text offset".into(),
            ));
        }
        let quote = doc
            .extracted_text
            .get(pos..)
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        let bid = BookmarkId::new();
        self.conn.execute("INSERT INTO reading_bookmark(id,document_id,title,offset_byte,quote,source_sha256,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![bid.to_string(),id.to_string(),title,offset,quote,doc.source_sha256,now()])?;
        Ok(bid)
    }
    pub fn list_reading_bookmarks(&self, id: ReadingId) -> Result<Vec<ReadingBookmarkRecord>> {
        let mut q=self.conn.prepare("SELECT id,title,offset_byte,state FROM reading_bookmark WHERE document_id=?1 ORDER BY created_at,id")?;
        let rows = q.query_map([id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (bookmark, title, offset_byte, state) = row?;
            Ok::<ReadingBookmarkRecord, rusqlite::Error>(ReadingBookmarkRecord {
                id: BookmarkId(Uuid::parse_str(&bookmark).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                document_id: id,
                title,
                offset_byte,
                state,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn set_reading_checkpoint(&mut self, id: ReadingId, progress: u64) -> Result<()> {
        let doc = self.get_reading_document(id)?;
        let offset = usize::try_from(progress)
            .map_err(|_| StorageError::Invalid("reading checkpoint overflow".into()))?;
        if offset > doc.extracted_text.len() || !doc.extracted_text.is_char_boundary(offset) {
            return Err(StorageError::Invalid(
                "checkpoint must be a valid UTF-8 byte position".into(),
            ));
        }
        self.conn.execute("INSERT INTO reading_checkpoint(document_id,progress_byte,source_sha256,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(document_id) DO UPDATE SET progress_byte=excluded.progress_byte,source_sha256=excluded.source_sha256,state='current',updated_at=excluded.updated_at",params![id.to_string(),progress,doc.source_sha256,now()])?;
        Ok(())
    }
    pub fn reading_checkpoint(&self, id: ReadingId) -> Result<Option<u64>> {
        Ok(self.conn.query_row("SELECT progress_byte FROM reading_checkpoint WHERE document_id=?1 AND state='current'",[id.to_string()],|r|r.get(0)).optional()?)
    }
    pub fn create_reading_term(
        &mut self,
        id: ReadingId,
        kind: &str,
        name: &str,
        aliases: &str,
        description: &str,
    ) -> Result<ReadingTermId> {
        if !matches!(kind, "character" | "species" | "term") {
            return Err(StorageError::Invalid(
                "reading term kind must be character, species or term".into(),
            ));
        }
        let tid = ReadingTermId::new();
        self.conn.execute("INSERT INTO reading_term(id,document_id,term_kind,name,aliases,description) VALUES(?1,?2,?3,?4,?5,?6)",params![tid.to_string(),id.to_string(),kind,name,aliases,description])?;
        Ok(tid)
    }
    pub fn list_reading_terms(&self, id: ReadingId) -> Result<Vec<ReadingTermRecord>> {
        let mut q=self.conn.prepare("SELECT id,term_kind,name,aliases,description FROM reading_term WHERE document_id=?1 ORDER BY term_kind,name COLLATE NOCASE")?;
        let rows = q.query_map([id.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (term, kind, name, aliases, description) = row?;
            Ok::<ReadingTermRecord, rusqlite::Error>(ReadingTermRecord {
                id: ReadingTermId(Uuid::parse_str(&term).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                kind,
                name,
                aliases,
                description,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn link_reading_entity(
        &mut self,
        document: ReadingId,
        annotation: Option<AnnotationId>,
        target: EntityRef,
        relation: &str,
    ) -> Result<()> {
        let id = LinkId::new();
        self.conn.execute("INSERT INTO reading_entity_link(id,document_id,annotation_id,target_kind,target_id,relation) VALUES(?1,?2,?3,?4,?5,?6)",params![id.to_string(),document.to_string(),annotation.map(|a|a.to_string()),target.kind(),target.id(),relation])?;
        Ok(())
    }
    pub fn reading_entity_links(
        &self,
        document: ReadingId,
    ) -> Result<Vec<ReadingEntityLinkRecord>> {
        let mut stmt = self.conn.prepare("SELECT id,annotation_id,target_kind,target_id,relation FROM reading_entity_link WHERE document_id=?1 ORDER BY id")?;
        let rows = stmt.query_map([document.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, annotation, target_kind, target_id, relation) = row?;
            let parse = |value: String| {
                Uuid::parse_str(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            };
            Ok(ReadingEntityLinkRecord {
                id: LinkId(parse(id)?),
                annotation_id: annotation.map(parse).transpose()?.map(AnnotationId),
                target_kind,
                target_id,
                relation,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }
    pub fn outgoing_links(&self, source: EntityRef) -> Result<Vec<StoryLink>> {
        let mut q = self.conn.prepare("SELECT id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label FROM story_link WHERE source_kind=?1 AND source_id=?2 ORDER BY created_at,id")?;
        let rows = q.query_map(params![source.kind(), source.id()], read_story_link)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn broken_links(&self) -> Result<Vec<BrokenLink>> {
        let mut q = self.conn.prepare("SELECT l.id,CASE WHEN (l.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.source_id)) OR (l.source_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.source_id)) OR (l.source_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.source_id)) OR (l.source_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.source_id)) THEN 'missing source' WHEN (l.target_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.target_id)) OR (l.target_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.target_id)) OR (l.target_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.target_id)) OR (l.target_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.target_id)) THEN 'missing target' ELSE 'missing passage anchor' END FROM story_link l WHERE (l.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.source_id)) OR (l.source_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.source_id)) OR (l.source_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.source_id)) OR (l.source_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.source_id)) OR (l.target_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.target_id)) OR (l.target_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.target_id)) OR (l.target_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.target_id)) OR (l.target_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.target_id)) OR (l.source_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=l.source_anchor_id)) OR (l.target_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=l.target_anchor_id)) ORDER BY l.id")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (id, reason) = row?;
            Ok::<BrokenLink, rusqlite::Error>(BrokenLink {
                link_id: LinkId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                reason,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn save_scene_query(&mut self, query: &SavedSceneQuery) -> Result<()> {
        if query.name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "saved query name cannot be empty".into(),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO saved_query(id,name,description,created_at,updated_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,updated_at=excluded.updated_at",params![query.id.to_string(),query.name,query.description,now()])?;
        tx.execute(
            "DELETE FROM saved_query_filter WHERE query_id=?1",
            [query.id.to_string()],
        )?;
        for (index, filter) in query.filters.iter().enumerate() {
            let (kind, value) = match filter {
                SceneQueryFilter::Status(status) => ("status", status.as_str().to_owned()),
                SceneQueryFilter::Character(id) => ("character", id.to_string()),
                SceneQueryFilter::Plotline(id) => ("plotline", id.to_string()),
                SceneQueryFilter::TimeBefore(tick) => ("time_before", tick.to_string()),
                SceneQueryFilter::TimeAfter(tick) => ("time_after", tick.to_string()),
                SceneQueryFilter::UnresolvedIssue => ("unresolved_issue", "1".into()),
            };
            tx.execute("INSERT INTO saved_query_filter(query_id,ordinal,filter_kind,value) VALUES(?1,?2,?3,?4)",params![query.id.to_string(),index as i64,kind,value])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn list_saved_queries(&self) -> Result<Vec<SavedQuerySummary>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id,name FROM saved_query ORDER BY name COLLATE NOCASE,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, name) = row?;
            Ok(SavedQuerySummary {
                id: QueryId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                name,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }
    pub fn run_scene_query(&self, query: QueryId) -> Result<Vec<SceneId>> {
        let mut stmt = self.conn.prepare(
            "SELECT filter_kind,value FROM saved_query_filter WHERE query_id=?1 ORDER BY ordinal",
        )?;
        let filters = stmt
            .query_map([query.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut sql = String::from(
            "WITH RECURSIVE node_order(id,path) AS (SELECT id,printf('%010d/',position) FROM structural_node WHERE parent_id IS NULL UNION ALL SELECT n.id,o.path||printf('%010d/',n.position) FROM structural_node n JOIN node_order o ON n.parent_id=o.id) SELECT s.id FROM scene s LEFT JOIN scene_placement p ON p.scene_id=s.id LEFT JOIN node_order n ON n.id=p.chapter_id WHERE s.archived=0",
        );
        let mut values: Vec<String> = Vec::new();
        for (kind, value) in filters {
            match kind.as_str() {
                "status" => { sql.push_str(" AND s.status=?"); values.push(value); }
                "character" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=?)"); values.push(value); }
                "plotline" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_plotline sp WHERE sp.scene_id=s.id AND sp.plotline_id=?)"); values.push(value); }
                "time_before" => { sql.push_str(" AND s.start_nominal IS NOT NULL AND s.start_nominal<?"); values.push(value); }
                "time_after" => { sql.push_str(" AND s.start_nominal IS NOT NULL AND s.start_nominal>?"); values.push(value); }
                "unresolved_issue" => sql.push_str(" AND EXISTS(SELECT 1 FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=s.id AND i.status IN('open','in_progress'))"),
                _ => return Err(StorageError::Invalid(format!("unknown saved query filter: {kind}"))),
            }
        }
        sql.push_str(" ORDER BY n.path,p.position,s.id");
        let mut q = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> =
            values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let ids = q
            .query_map(refs.as_slice(), |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map(SceneId)
                    .map_err(|e| StorageError::Invalid(e.to_string()))
            })
            .collect()
    }
    pub fn search_scenes(&self, terms: &str) -> Result<Vec<SceneId>> {
        if terms.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut q=self.conn.prepare("SELECT scene_id FROM scene_fts WHERE scene_fts MATCH ?1 ORDER BY bm25(scene_fts),scene_id")?;
        let ids = q
            .query_map([terms], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map(SceneId)
                    .map_err(|e| StorageError::Invalid(e.to_string()))
            })
            .collect()
    }
    pub fn search_project(&self, terms: &str, limit: u32) -> Result<Vec<ProjectSearchHit>> {
        if terms.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut query = self.conn.prepare("SELECT entity_kind,entity_id,title,snippet(project_search_fts,3,'[',']','…',12) FROM project_search_fts WHERE project_search_fts MATCH ?1 ORDER BY bm25(project_search_fts),entity_kind,entity_id LIMIT ?2")?;
        Ok(query
            .query_map(params![terms, limit], |row| {
                Ok(ProjectSearchHit {
                    entity_kind: row.get(0)?,
                    entity_id: row.get(1)?,
                    title: row.get(2)?,
                    excerpt: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn rebuild_project_search_index(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM project_search_fts", [])?;
        self.conn.execute(
            "INSERT INTO project_search_fts(rowid,entity_kind,entity_id,title,content) SELECT row_id,entity_kind,entity_id,title,content FROM project_search_content",
            [],
        )?;
        Ok(())
    }
    fn rebuild_scene_index(&mut self) -> Result<()> {
        let ids: Vec<String> = {
            let mut q = self
                .conn
                .prepare("SELECT scene_id FROM scene_search_content ORDER BY scene_id")?;
            q.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        for id in ids {
            let scene =
                SceneId(Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?);
            let text = fs::read_to_string(self.safe_path(&self.content_path(scene)?)?)?;
            update_scene_index(&self.conn, scene, &text)?;
        }
        Ok(())
    }
    pub fn define_custom_field(&mut self, def: &CustomFieldDefinition) -> Result<()> {
        if def.field_type == FieldType::Enum
            && (def.enum_values.is_empty() || def.enum_values.iter().any(|v| v.contains('\n')))
        {
            return Err(StorageError::Invalid(
                "enum fields need non-empty single-line options".into(),
            ));
        }
        if matches!(def.field_type, FieldType::EntityRef | FieldType::EntityRefs)
            && !matches!(
                def.reference_kind.as_deref(),
                Some("character" | "location" | "plotline" | "event" | "scene")
            )
        {
            return Err(StorageError::Invalid(
                "entity reference field requires a supported target kind".into(),
            ));
        }
        if def.field_type == FieldType::EntityRefs && def.default_value.is_some() {
            return Err(StorageError::Invalid(
                "multiple entity references do not support scalar defaults".into(),
            ));
        }
        if let Some(default) = def.default_value.as_ref() {
            def.validate(default)
                .map_err(|e| StorageError::Invalid(e.to_string()))?;
        }
        let enum_values = def.enum_values.join("\n");
        let (dt, di, dd, db) = match def.default_value.as_ref() {
            Some(
                FieldValue::Text(v)
                | FieldValue::DateTime(v)
                | FieldValue::Enum(v)
                | FieldValue::EntityRef(v),
            ) => (Some(v.as_str()), None, None, None),
            Some(FieldValue::Integer(v)) => (None, Some(*v), None, None),
            Some(FieldValue::Decimal(v)) => (None, None, Some(v.as_str()), None),
            Some(FieldValue::Boolean(v)) => (None, None, None, Some(*v)),
            _ => (None, None, None, None),
        };
        if let (Some(kind), Some(FieldValue::EntityRef(target))) =
            (def.reference_kind.as_deref(), def.default_value.as_ref())
        {
            validate_entity_ref(&self.conn, Some(kind), target)?;
        }
        self.conn.execute("INSERT INTO custom_field_definition(id,name,field_type,required,default_text,default_integer,default_decimal,default_boolean,enum_values,reference_kind) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![def.id.to_string(),def.name,field_type(def.field_type),def.required,dt,di,dd,db,enum_values,def.reference_kind])?;
        if let Some(default) = def.default_value.as_ref() {
            let scenes: Vec<String> = {
                let mut q = self.conn.prepare("SELECT id FROM scene")?;
                q.query_map([], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for id in scenes {
                let sid = SceneId(
                    Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                );
                self.set_custom_field(sid, def.id, default)?;
            }
        }
        Ok(())
    }
    pub fn set_custom_field(
        &mut self,
        scene: SceneId,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<()> {
        let (ty,enum_blob,ref_kind):(String,String,Option<String>)=self.conn.query_row("SELECT field_type,enum_values,reference_kind FROM custom_field_definition WHERE id=?1",[field.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|StorageError::NotFound(field.to_string()))?;
        let ft = parse_field_type(&ty)?;
        let options: Vec<String> = enum_blob
            .split('\n')
            .filter(|x| !x.is_empty())
            .map(str::to_owned)
            .collect();
        let def = CustomFieldDefinition {
            id: field,
            name: String::new(),
            field_type: ft,
            required: false,
            default_value: None,
            enum_values: options,
            reference_kind: ref_kind.clone(),
        };
        def.validate(value)
            .map_err(|e| StorageError::Invalid(e.to_string()))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM custom_field_value WHERE scene_id=?1 AND field_id=?2",
            params![scene.to_string(), field.to_string()],
        )?;
        match value {
            FieldValue::Text(v) => {
                tx.execute(
                    "INSERT INTO custom_field_value(scene_id,field_id,text_value) VALUES(?1,?2,?3)",
                    params![scene.to_string(), field.to_string(), v],
                )?;
            }
            FieldValue::Integer(v) => {
                tx.execute("INSERT INTO custom_field_value(scene_id,field_id,integer_value) VALUES(?1,?2,?3)",params![scene.to_string(),field.to_string(),v])?;
            }
            FieldValue::Decimal(v) => {
                tx.execute("INSERT INTO custom_field_value(scene_id,field_id,decimal_value) VALUES(?1,?2,?3)",params![scene.to_string(),field.to_string(),v])?;
            }
            FieldValue::Boolean(v) => {
                tx.execute("INSERT INTO custom_field_value(scene_id,field_id,boolean_value) VALUES(?1,?2,?3)",params![scene.to_string(),field.to_string(),v])?;
            }
            FieldValue::DateTime(v) | FieldValue::Enum(v) => {
                tx.execute(
                    "INSERT INTO custom_field_value(scene_id,field_id,text_value) VALUES(?1,?2,?3)",
                    params![scene.to_string(), field.to_string(), v],
                )?;
            }
            FieldValue::EntityRef(v) => {
                validate_entity_ref(&tx, ref_kind.as_deref(), v)?;
                tx.execute(
                    "INSERT INTO custom_field_value(scene_id,field_id,entity_ref) VALUES(?1,?2,?3)",
                    params![scene.to_string(), field.to_string(), v],
                )?;
                insert_ref(&tx, scene, field, 0, ref_kind.as_deref().unwrap(), v)?;
            }
            FieldValue::EntityRefs(vs) => {
                tx.execute(
                    "INSERT INTO custom_field_value(scene_id,field_id) VALUES(?1,?2)",
                    params![scene.to_string(), field.to_string()],
                )?;
                for (i, v) in vs.iter().enumerate() {
                    validate_entity_ref(&tx, ref_kind.as_deref(), v)?;
                    insert_ref(&tx, scene, field, i as i64, ref_kind.as_deref().unwrap(), v)?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn filter_scenes_by_custom_field(
        &self,
        field: FieldId,
        value: &FieldValue,
    ) -> Result<Vec<SceneId>> {
        let (ty,enum_blob,ref_kind):(String,String,Option<String>)=self.conn.query_row("SELECT field_type,enum_values,reference_kind FROM custom_field_definition WHERE id=?1",[field.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|StorageError::NotFound(field.to_string()))?;
        let def = CustomFieldDefinition {
            id: field,
            name: String::new(),
            field_type: parse_field_type(&ty)?,
            required: false,
            default_value: None,
            enum_values: enum_blob
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            reference_kind: ref_kind,
        };
        def.validate(value)
            .map_err(|e| StorageError::Invalid(e.to_string()))?;
        let ids: Vec<String> = match value {
            FieldValue::Text(v) => {
                query_field_ids(&self.conn, "text_value", &field.to_string(), v)?
            }
            FieldValue::Integer(v) => query_field_ids(
                &self.conn,
                "integer_value",
                &field.to_string(),
                &v.to_string(),
            )?,
            FieldValue::Decimal(v) => {
                query_field_ids(&self.conn, "decimal_value", &field.to_string(), v)?
            }
            FieldValue::Boolean(v) => query_field_ids(
                &self.conn,
                "boolean_value",
                &field.to_string(),
                if *v { "1" } else { "0" },
            )?,
            FieldValue::DateTime(v) | FieldValue::Enum(v) => {
                query_field_ids(&self.conn, "text_value", &field.to_string(), v)?
            }
            FieldValue::EntityRef(v) => query_ref_ids(&self.conn, field, v)?,
            FieldValue::EntityRefs(vs) => {
                let mut result = std::collections::BTreeSet::new();
                for v in vs {
                    result.extend(query_ref_ids(&self.conn, field, v)?);
                }
                result.into_iter().collect()
            }
        };
        ids.into_iter()
            .map(|s| {
                Uuid::parse_str(&s)
                    .map(SceneId)
                    .map_err(|e| StorageError::Invalid(e.to_string()))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SaveFailpoint {
    AfterPartialTemp,
    AfterTempSync,
    AfterIntent,
    BeforeRename,
    AfterRename,
    AfterMetadataCommit,
    BeforeCleanup,
    AfterJournalCleanup,
}

fn configure(c: &Connection) -> Result<()> {
    c.pragma_update(None, "foreign_keys", "ON")?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.pragma_update(None, "synchronous", "FULL")?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn read_story_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoryLink> {
    let id: String = row.get(0)?;
    let source_kind: String = row.get(1)?;
    let source_id: String = row.get(2)?;
    let source_anchor: Option<String> = row.get(3)?;
    let target_kind: String = row.get(4)?;
    let target_id: String = row.get(5)?;
    let target_anchor: Option<String> = row.get(6)?;
    let kind: String = row.get(7)?;
    let uuid = |s: String| {
        Uuid::parse_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    let entity = |kind: &str, value: String| -> rusqlite::Result<EntityRef> {
        let id = uuid(value)?;
        match kind {
            "scene" => Ok(EntityRef::Scene(SceneId(id))),
            "character" => Ok(EntityRef::Character(CharacterId(id))),
            "plotline" => Ok(EntityRef::Plotline(PlotlineId(id))),
            "research" => Ok(EntityRef::Research(ResearchId(id))),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    };
    let anchor = |value: Option<String>| -> rusqlite::Result<Option<AnchorId>> {
        value.map(|s| uuid(s).map(AnchorId)).transpose()
    };
    let link_kind = match kind.as_str() {
        "related" => LinkKind::Related,
        "cites" => LinkKind::Cites,
        "transcludes" => LinkKind::Transcludes,
        "includes" => LinkKind::Includes,
        "navigates" => LinkKind::Navigates,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(StoryLink {
        id: LinkId(uuid(id)?),
        source: entity(&source_kind, source_id)?,
        source_anchor: anchor(source_anchor)?,
        target: entity(&target_kind, target_id)?,
        target_anchor: anchor(target_anchor)?,
        kind: link_kind,
        label: row.get(8)?,
    })
}

fn revalidate_scene_anchors(
    conn: &Connection,
    scene: SceneId,
    text: &str,
    new_hash: &str,
) -> Result<()> {
    let anchors = {
        let mut query = conn.prepare("SELECT id,quote,prefix_context,suffix_context FROM passage_anchor WHERE scene_id=?1 AND source_sha256<>?2 ORDER BY id")?;
        query
            .query_map(params![scene.to_string(), new_hash], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (id, quote, prefix, suffix) in anchors {
        let occurrences = text
            .match_indices(&quote)
            .map(|(start, matched)| (start, start + matched.len()))
            .collect::<Vec<_>>();
        let selected = if occurrences.len() == 1 {
            Some(occurrences[0])
        } else if occurrences.len() > 1 {
            let scores = occurrences
                .iter()
                .map(|(start, end)| {
                    let before = &text[..*start];
                    let after = &text[*end..];
                    let prefix_score = common_suffix_chars(before, &prefix);
                    let suffix_score = common_prefix_chars(after, &suffix);
                    prefix_score + suffix_score
                })
                .collect::<Vec<_>>();
            let best = scores.iter().copied().max().unwrap_or(0);
            (best > 0 && scores.iter().filter(|score| **score == best).count() == 1)
                .then(|| occurrences[scores.iter().position(|score| *score == best).unwrap()])
        } else {
            None
        };
        if let Some((start, end)) = selected {
            conn.execute("UPDATE passage_anchor SET start_byte=?2,end_byte=?3,source_sha256=?4,state='current',updated_at=?5 WHERE id=?1", params![id,start as u64,end as u64,new_hash,now()])?;
        } else {
            let state = if occurrences.is_empty() {
                "broken"
            } else {
                "needs_review"
            };
            conn.execute(
                "UPDATE passage_anchor SET source_sha256=?2,state=?3,updated_at=?4 WHERE id=?1",
                params![id, new_hash, state, now()],
            )?;
        }
    }
    Ok(())
}

fn common_suffix_chars(left: &str, right: &str) -> usize {
    left.chars()
        .rev()
        .zip(right.chars().rev())
        .take_while(|(a, b)| a == b)
        .count()
}

fn common_prefix_chars(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .count()
}

fn update_scene_index(conn: &Connection, scene: SceneId, text: &str) -> Result<()> {
    let (title, synopsis): (String, String) = conn.query_row(
        "SELECT title,synopsis FROM scene WHERE id=?1",
        [scene.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute("INSERT INTO scene_search_content(scene_id,content) VALUES(?1,?2) ON CONFLICT(scene_id) DO UPDATE SET content=excluded.content", params![scene.to_string(), text])?;
    conn.execute(
        "DELETE FROM scene_fts WHERE scene_id=?1",
        [scene.to_string()],
    )?;
    conn.execute(
        "INSERT INTO scene_fts(scene_id,title,synopsis,content) VALUES(?1,?2,?3,?4)",
        params![scene.to_string(), title, synopsis, text],
    )?;
    Ok(())
}
fn now() -> String {
    format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    )
}
fn hash_bytes(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn unique_occurrence(text: &str, quote: &str) -> Option<(usize, usize)> {
    if quote.is_empty() {
        return None;
    }
    let mut found = text.match_indices(quote).take(2);
    let (start, matched) = found.next()?;
    if found.next().is_some() {
        None
    } else {
        Some((start, start + matched.len()))
    }
}
fn line_diff(old: &str, new: &str) -> Vec<SnapshotDiffLine> {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let cells = (a.len() + 1).saturating_mul(b.len() + 1);
    if cells > 4_000_000 {
        let mut out = Vec::new();
        let mut prefix = 0;
        while prefix < a.len().min(b.len()) && a[prefix] == b[prefix] {
            out.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Equal,
                text: a[prefix].into(),
            });
            prefix += 1;
        }
        for line in &a[prefix..] {
            out.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Removed,
                text: (*line).into(),
            });
        }
        for line in &b[prefix..] {
            out.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Added,
                text: (*line).into(),
            });
        }
        return out;
    }
    let width = b.len() + 1;
    let mut lcs = vec![0u32; cells];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            let at = i * width + j;
            lcs[at] = if a[i] == b[j] {
                lcs[(i + 1) * width + j + 1] + 1
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut result = Vec::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            result.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Equal,
                text: a[i].into(),
            });
            i += 1;
            j += 1;
        } else if i < a.len()
            && (j == b.len() || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1])
        {
            result.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Removed,
                text: a[i].into(),
            });
            i += 1;
        } else {
            result.push(SnapshotDiffLine {
                kind: SnapshotLineKind::Added,
                text: b[j].into(),
            });
            j += 1;
        }
    }
    result
}
fn hash_file(p: &Path) -> Result<String> {
    let mut f = File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = [0; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn write_synced(p: &Path, b: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(p)?;
    f.write_all(b)?;
    f.sync_all()?;
    Ok(())
}
fn atomic_write(p: &Path, b: &[u8]) -> Result<()> {
    let temp = p.with_extension("new");
    if temp.exists() {
        fs::remove_file(&temp)?;
    }
    write_synced(&temp, b)?;
    fs::rename(&temp, p)?;
    sync_dir(p.parent().unwrap())?;
    Ok(())
}
fn copy_synced(source: &Path, destination: &Path) -> Result<()> {
    let bytes = fs::read(source)?;
    write_synced(destination, &bytes)?;
    Ok(())
}
fn copy_tree_sync(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree_sync(&entry.path(), &target)?;
        } else if kind.is_file() {
            copy_synced(&entry.path(), &target)?;
        } else {
            return Err(StorageError::Invalid(format!(
                "backup refuses special file {}",
                entry.path().display()
            )));
        }
    }
    sync_dir(destination)?;
    Ok(())
}
fn sync_dir(p: &Path) -> Result<()> {
    File::open(p)?.sync_all()?;
    Ok(())
}
fn word_count(s: &str) -> u32 {
    s.split_whitespace().count().min(u32::MAX as usize) as u32
}
fn parse_status(s: &str) -> SceneStatus {
    match s {
        "outlined" => SceneStatus::Outlined,
        "ready_to_draft" => SceneStatus::ReadyToDraft,
        "drafting" => SceneStatus::Drafting,
        "draft_complete" => SceneStatus::DraftComplete,
        "revising" => SceneStatus::Revising,
        "complete" => SceneStatus::Complete,
        "archived" => SceneStatus::Archived,
        _ => SceneStatus::Idea,
    }
}
fn precision(s: TimePrecision) -> &'static str {
    match s {
        TimePrecision::Exact => "exact",
        TimePrecision::Approximate => "approximate",
        TimePrecision::Bounded => "bounded",
        TimePrecision::Unknown => "unknown",
    }
}
fn read_time(n: Option<i64>, l: Option<i64>, u: Option<i64>, p: &str) -> Result<StoryTime> {
    let pr = match p {
        "exact" => TimePrecision::Exact,
        "approximate" => TimePrecision::Approximate,
        "bounded" => TimePrecision::Bounded,
        _ => TimePrecision::Unknown,
    };
    StoryTime::new(n, l, u, pr).map_err(|e| StorageError::Invalid(e.to_string()))
}
fn validate_time(t: StoryTime) -> Result<()> {
    StoryTime::new(t.nominal, t.lower, t.upper, t.precision)
        .map(|_| ())
        .map_err(|e| StorageError::Invalid(e.to_string()))
}
fn query_scene_ids(conn: &Connection, chapter: &str) -> Result<Vec<String>> {
    let mut q =
        conn.prepare("SELECT scene_id FROM scene_placement WHERE chapter_id=?1 ORDER BY position")?;
    Ok(q.query_map([chapter], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
fn query_node_ids(conn: &Connection, parent: Option<&str>) -> Result<Vec<String>> {
    match parent {
        Some(p) => {
            let mut q = conn
                .prepare("SELECT id FROM structural_node WHERE parent_id=?1 ORDER BY position")?;
            Ok(q.query_map([p], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?)
        }
        None => {
            let mut q = conn.prepare(
                "SELECT id FROM structural_node WHERE parent_id IS NULL ORDER BY position",
            )?;
            Ok(q.query_map([], |r| r.get(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?)
        }
    }
}
fn field_type(t: FieldType) -> &'static str {
    match t {
        FieldType::Text => "text",
        FieldType::LongText => "long_text",
        FieldType::Integer => "integer",
        FieldType::Decimal => "decimal",
        FieldType::Boolean => "boolean",
        FieldType::DateTime => "datetime",
        FieldType::Enum => "enum",
        FieldType::EntityRef => "entity_ref",
        FieldType::EntityRefs => "entity_refs",
    }
}
fn parse_field_type(t: &str) -> Result<FieldType> {
    match t {
        "text" => Ok(FieldType::Text),
        "long_text" => Ok(FieldType::LongText),
        "integer" => Ok(FieldType::Integer),
        "decimal" => Ok(FieldType::Decimal),
        "boolean" => Ok(FieldType::Boolean),
        "datetime" => Ok(FieldType::DateTime),
        "enum" => Ok(FieldType::Enum),
        "entity_ref" => Ok(FieldType::EntityRef),
        "entity_refs" => Ok(FieldType::EntityRefs),
        _ => Err(StorageError::Invalid(format!(
            "unknown custom field type {t}"
        ))),
    }
}
fn validate_entity_ref(c: &Connection, kind: Option<&str>, id: &str) -> Result<()> {
    let table = match kind {
        Some("character") => "character",
        Some("location") => "location",
        Some("plotline") => "plotline",
        Some("event") => "story_event",
        Some("scene") => "scene",
        _ => {
            return Err(StorageError::Invalid(
                "unsupported custom entity reference type".into(),
            ));
        }
    };
    let exists: i64 = c.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
        [id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(StorageError::Invalid(format!(
            "custom field reference target {id} does not exist"
        )));
    }
    Ok(())
}
fn insert_ref(
    c: &Connection,
    scene: SceneId,
    field: FieldId,
    ordinal: i64,
    kind: &str,
    id: &str,
) -> Result<()> {
    let (col, table) = match kind {
        "character" => ("character_id", "character"),
        "location" => ("location_id", "location"),
        "plotline" => ("plotline_id", "plotline"),
        "event" => ("event_id", "story_event"),
        "scene" => ("scene_id_ref", "scene"),
        _ => {
            return Err(StorageError::Invalid(
                "unsupported entity reference type".into(),
            ));
        }
    };
    let _ = table;
    c.execute(&format!("INSERT INTO custom_field_entity_ref(scene_id,field_id,ordinal,target_kind,target_id,{col}) VALUES(?1,?2,?3,?4,?5,?5)"),params![scene.to_string(),field.to_string(),ordinal,kind,id])?;
    Ok(())
}
fn query_field_ids(
    conn: &Connection,
    column: &str,
    field: &str,
    value: &str,
) -> Result<Vec<String>> {
    let col = match column {
        "text_value" => "text_value",
        "integer_value" => "integer_value",
        "decimal_value" => "decimal_value",
        "boolean_value" => "boolean_value",
        _ => {
            return Err(StorageError::Invalid(
                "unsupported custom field filter".into(),
            ));
        }
    };
    let sql = format!(
        "SELECT v.scene_id FROM custom_field_value v LEFT JOIN scene_placement p ON p.scene_id=v.scene_id WHERE v.field_id=?1 AND v.{col}=?2 ORDER BY p.chapter_id,p.position"
    );
    let mut q = conn.prepare(&sql)?;
    Ok(q.query_map(params![field, value], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
fn query_ref_ids(conn: &Connection, field: FieldId, value: &str) -> Result<Vec<String>> {
    let mut q=conn.prepare("SELECT DISTINCT r.scene_id FROM custom_field_entity_ref r LEFT JOIN scene_placement p ON p.scene_id=r.scene_id WHERE r.field_id=?1 AND r.target_id=?2 ORDER BY p.chapter_id,p.position")?;
    Ok(
        q.query_map(params![field.to_string(), value], |r| r.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn links_anchors_backlinks_and_transclusion_cycles_are_integrity_checked() {
        let d = tempdir().unwrap();
        let mut st = ProjectStore::create(d.path().join("N"), "Novel").unwrap();
        let chapter = st
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let a = st.create_scene(chapter, 0, "A").unwrap();
        let b = st.create_scene(chapter, 1, "B").unwrap();
        let other_chapter = st.create_node(None, NodeKind::Chapter, "Later", 1).unwrap();
        let (_, hash) = st.load_text(a).unwrap();
        st.save_text(a, Some(&hash), "Mara opened the red door.")
            .unwrap();
        let anchor = st.create_anchor(a, 0, 5, "Mara ", "", "opened").unwrap();
        let link = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(a),
            source_anchor: Some(anchor),
            target: EntityRef::Scene(b),
            target_anchor: None,
            kind: LinkKind::Cites,
            label: "consequence".into(),
        };
        st.create_story_link(&link).unwrap();
        assert_eq!(st.backlinks(EntityRef::Scene(b)).unwrap()[0].id, link.id);
        let cycle = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(b),
            source_anchor: None,
            target: EntityRef::Scene(a),
            target_anchor: None,
            kind: LinkKind::Transcludes,
            label: String::new(),
        };
        st.create_story_link(&cycle).unwrap();
        assert_eq!(
            st.expand_scene_transclusions(b)
                .unwrap()
                .iter()
                .map(|p| p.scene_id)
                .collect::<Vec<_>>(),
            vec![b, a]
        );
        let reverse = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(a),
            source_anchor: None,
            target: EntityRef::Scene(b),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: String::new(),
        };
        assert!(st.create_story_link(&reverse).is_err());
        assert!(st.broken_links().unwrap().is_empty());
        st.move_scene(b, other_chapter, 0).unwrap();
        let renamed = Scene {
            title: "Renamed B".into(),
            ..st.get_scene(b).unwrap().scene
        };
        st.update_scene(&renamed).unwrap();
        assert_eq!(st.backlinks(EntityRef::Scene(b)).unwrap()[0].id, link.id);
        let character = CharacterId::new();
        st.add_character(character, "Soon deleted").unwrap();
        let dangling = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(a),
            source_anchor: None,
            target: EntityRef::Character(character),
            target_anchor: None,
            kind: LinkKind::Related,
            label: String::new(),
        };
        st.create_story_link(&dangling).unwrap();
        st.conn
            .execute("DELETE FROM character WHERE id=?1", [character.to_string()])
            .unwrap();
        assert!(
            st.broken_links()
                .unwrap()
                .iter()
                .any(|broken| broken.link_id == dangling.id)
        );
        let bad: rusqlite::Result<usize> = st.conn.execute("INSERT INTO story_link(id,source_kind,source_id,target_kind,target_id,link_kind,created_at) VALUES('x','scene',?1,'scene','missing','related','now')", [a.to_string()]);
        assert!(bad.is_err());
    }

    #[test]
    fn passage_anchors_revalidate_unique_matches_and_flag_ambiguous_or_missing_text() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("AnchorNovel"), "Anchors").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let target = store.create_scene(chapter, 1, "Target").unwrap();
        let (text, hash) = store.load_text(scene).unwrap();
        let new_hash = store
            .save_text(scene, Some(&hash), "Before QUOTE after")
            .unwrap();
        let anchor = store
            .create_anchor(scene, 7, 12, "QUOTE", "Before ", " after")
            .unwrap();
        let linked = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(scene),
            source_anchor: Some(anchor),
            target: EntityRef::Scene(target),
            target_anchor: None,
            kind: LinkKind::Cites,
            label: "anchored reference".into(),
        };
        store.create_story_link(&linked).unwrap();

        let moved = "A preface. Before QUOTE after";
        let moved_hash = store.save_text(scene, Some(&new_hash), moved).unwrap();
        let record = &store.list_scene_anchors(scene).unwrap()[0];
        assert_eq!(record.id, anchor);
        assert_eq!(record.state, "current");
        assert_eq!(record.start_byte, moved.find("QUOTE").unwrap() as u64);
        assert_eq!(record.source_hash, moved_hash);
        assert_eq!(
            store.outgoing_links(EntityRef::Scene(scene)).unwrap()[0].source_anchor,
            Some(anchor)
        );

        let repeated = "Before QUOTE after; Before QUOTE after";
        let repeated_hash = store.save_text(scene, Some(&moved_hash), repeated).unwrap();
        let record = store.list_scene_anchors(scene).unwrap().remove(0);
        assert_eq!(record.state, "needs_review");
        assert_eq!(record.source_hash, repeated_hash);
        let second = repeated.rfind("QUOTE").unwrap() as u64;
        store.reanchor_passage(anchor, second, second + 5).unwrap();
        let record = store.list_scene_anchors(scene).unwrap().remove(0);
        assert_eq!(record.state, "current");
        assert_eq!(record.start_byte, second);

        let final_hash = store
            .save_text(scene, Some(&repeated_hash), "The passage was removed.")
            .unwrap();
        let record = store.list_scene_anchors(scene).unwrap().remove(0);
        assert_eq!(record.state, "broken");
        assert_eq!(record.source_hash, final_hash);
        assert!(store.reanchor_passage(anchor, 0, 5).is_err());
        let _ = text;
    }

    #[test]
    fn anchor_revalidation_is_consistent_when_save_recovers_after_file_replacement() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("RecoveryNovel"), "Recovery").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        let first = "One anchor.";
        let hash = store.save_text(scene, Some(&hash), first).unwrap();
        let anchor = store
            .create_anchor(scene, 4, 10, "anchor", "One ", ".")
            .unwrap();
        assert!(
            store
                .save_text_inner(
                    scene,
                    Some(&hash),
                    "Prefix. One anchor.",
                    Some(SaveFailpoint::AfterRename)
                )
                .is_err()
        );
        store.recover_pending_writes().unwrap();
        let record = store.list_scene_anchors(scene).unwrap().remove(0);
        assert_eq!(record.id, anchor);
        assert_eq!(record.state, "current");
        assert_eq!(record.start_byte, 12);
        assert_eq!(store.load_text(scene).unwrap().0, "Prefix. One anchor.");
    }

    #[test]
    fn saved_queries_refresh_and_preserve_manuscript_order() {
        let d = tempdir().unwrap();
        let mut st = ProjectStore::create(d.path().join("N"), "Novel").unwrap();
        let chapter = st
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let first = st.create_scene(chapter, 0, "First").unwrap();
        let second = st.create_scene(chapter, 1, "Second").unwrap();
        let character = CharacterId::new();
        st.add_character(character, "Mara").unwrap();
        st.link_character(second, character, ParticipationRole::Pov)
            .unwrap();
        let query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Mara scenes".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::Character(character)],
        };
        st.save_scene_query(&query).unwrap();
        assert_eq!(st.list_saved_queries().unwrap()[0].name, "Mara scenes");
        assert_eq!(st.run_scene_query(query.id).unwrap(), vec![second]);
        st.link_character(first, character, ParticipationRole::Minor)
            .unwrap();
        assert_eq!(st.run_scene_query(query.id).unwrap(), vec![first, second]);
        let order = query_scene_ids(&st.conn, &chapter.to_string()).unwrap();
        assert_eq!(order, vec![first.to_string(), second.to_string()]);
    }

    #[test]
    fn full_text_search_tracks_scenes_and_metadata() {
        let d = tempdir().unwrap();
        let mut st = ProjectStore::create(d.path().join("N"), "Novel").unwrap();
        let chapter = st
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = st.create_scene(chapter, 0, "The Brass Key").unwrap();
        let (_, hash) = st.load_text(scene).unwrap();
        st.save_text(scene, Some(&hash), "Mara discovers the brass key.")
            .unwrap();
        assert_eq!(st.search_scenes("brass").unwrap(), vec![scene]);
        st.update_scene(&Scene {
            title: "Moonlit Door".into(),
            ..st.get_scene(scene).unwrap().scene
        })
        .unwrap();
        assert_eq!(st.search_scenes("moonlit").unwrap(), vec![scene]);
        assert!(st.search_scenes("brass").unwrap().contains(&scene));
    }

    #[test]
    fn project_search_indexes_entities_and_updates_incrementally() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("SearchNovel"), "Search").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        store
            .save_text(
                scene,
                Some(&hash),
                "A silver key waits beneath the floorboards.",
            )
            .unwrap();
        let character = CharacterId::new();
        store.add_character(character, "Mara Venn").unwrap();
        let research = store
            .create_research_item(
                "Archive notes",
                "silver key provenance",
                "text/plain",
                None,
                None,
            )
            .unwrap();

        let hits = store.search_project("silver", 20).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(
            hits.iter()
                .any(|hit| hit.entity_kind == "scene" && hit.entity_id == scene.to_string())
        );
        assert!(
            hits.iter()
                .any(|hit| hit.entity_kind == "research" && hit.entity_id == research.to_string())
        );
        store
            .conn
            .execute(
                "UPDATE character SET name='Mara North' WHERE id=?1",
                [character.to_string()],
            )
            .unwrap();
        let hits = store.search_project("North", 20).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_kind, "character");
        let field = FieldId::new();
        store
            .define_custom_field(&CustomFieldDefinition {
                id: field,
                name: "World secret".into(),
                field_type: FieldType::Text,
                required: false,
                default_value: None,
                enum_values: Vec::new(),
                reference_kind: None,
            })
            .unwrap();
        store
            .set_custom_field(scene, field, &FieldValue::Text("glassmoon".into()))
            .unwrap();
        assert_eq!(
            store.search_project("glassmoon", 10).unwrap()[0].entity_id,
            scene.to_string()
        );
        let (_, text_hash) = store.load_text(scene).unwrap();
        store
            .save_text(
                scene,
                Some(&text_hash),
                "A silver key waits. The moon is bright.",
            )
            .unwrap();
        assert_eq!(
            store.search_project("glassmoon", 10).unwrap()[0].entity_id,
            scene.to_string()
        );
        store
            .set_custom_field(scene, field, &FieldValue::Text("redmoon".into()))
            .unwrap();
        assert!(store.search_project("glassmoon", 10).unwrap().is_empty());
        assert_eq!(
            store.search_project("redmoon", 10).unwrap()[0].entity_id,
            scene.to_string()
        );
        store
            .conn
            .execute(
                "DELETE FROM custom_field_value WHERE scene_id=?1 AND field_id=?2",
                params![scene.to_string(), field.to_string()],
            )
            .unwrap();
        assert!(store.search_project("redmoon", 10).unwrap().is_empty());
        store.rebuild_project_search_index().unwrap();
        assert!(
            store
                .search_project("silver", 20)
                .unwrap()
                .iter()
                .any(|hit| hit.entity_id == scene.to_string())
        );
        assert!(store.search_project("   ", 20).unwrap().is_empty());
        assert!(store.search_project("silver", 0).unwrap().is_empty());
    }

    #[test]
    fn reading_copy_annotations_search_and_reanchoring_are_persistent() {
        let d = tempdir().unwrap();
        let project = d.path().join("Novel");
        let original = d.path().join("reference.txt");
        fs::write(&original, "Mara finds a silver key.").unwrap();
        let parsed = story_reading::import_reference(&original).unwrap();
        let mut store = ProjectStore::create(&project, "Novel").unwrap();
        let reading = store
            .import_reading_document(&parsed, original.to_str().unwrap())
            .unwrap();
        assert_eq!(fs::read(&original).unwrap(), parsed.source_bytes);
        let at = parsed.extracted_text.find("silver key").unwrap() as u64;
        let annotation = store
            .create_reading_annotation(
                reading,
                &ReadingAnnotationDraft {
                    kind: "highlight".into(),
                    start_byte: at,
                    end_byte: at + 10,
                    quote: "silver key".into(),
                    body: "important clue".into(),
                    prefix_context: "a ".into(),
                    suffix_context: ".".into(),
                },
            )
            .unwrap();
        let character = CharacterId::new();
        store.add_character(character, "Mara").unwrap();
        store
            .link_reading_entity(
                reading,
                Some(annotation),
                EntityRef::Character(character),
                "identified-as",
            )
            .unwrap();
        store.create_reading_bookmark(reading, "Key", 0).unwrap();
        store.set_reading_checkpoint(reading, 6).unwrap();
        store
            .create_reading_term(reading, "character", "Mara", "M. Aran", "Protagonist")
            .unwrap();
        assert_eq!(store.search_reading("silver").unwrap().len(), 1);
        let changed = d.path().join("updated.txt");
        fs::write(&changed, "Earlier text. Mara finds a silver key.").unwrap();
        let update = story_reading::import_reference(&changed).unwrap();
        store.reload_reading_document(reading, &update).unwrap();
        let annotations = store.list_reading_annotations(reading).unwrap();
        assert_eq!(annotations[0].id, annotation);
        assert_eq!(annotations[0].state, "current");
        assert_eq!(
            annotations[0].start_byte,
            update.extracted_text.find("silver key").unwrap() as u64
        );
        assert_eq!(
            store.list_reading_bookmarks(reading).unwrap()[0].state,
            "current"
        );
        assert_eq!(store.list_reading_terms(reading).unwrap()[0].name, "Mara");
        let links = store.reading_entity_links(reading).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].annotation_id, Some(annotation));
        assert_eq!(links[0].target_id, character.to_string());
        assert_eq!(store.reading_checkpoint(reading).unwrap(), None);
        drop(store);
        let store = ProjectStore::open(&project).unwrap();
        assert_eq!(
            store.get_reading_document(reading).unwrap().extracted_text,
            update.extracted_text
        );
    }

    #[test]
    fn version_four_projects_migrate_and_build_project_search_index() {
        let d = tempdir().unwrap();
        let project = d.path().join("N");
        let mut st = ProjectStore::create(&project, "Novel").unwrap();
        let chapter = st.create_node(None, NodeKind::Chapter, "Ch", 0).unwrap();
        let scene = st.create_scene(chapter, 0, "Old title").unwrap();
        let (_, hash) = st.load_text(scene).unwrap();
        st.save_text(scene, Some(&hash), "An obsidian compass points north.")
            .unwrap();
        drop(st);
        let conn = Connection::open(project.join("story.db")).unwrap();
        conn.execute_batch("DROP TRIGGER project_search_custom_field_insert; DROP TRIGGER project_search_custom_field_update; DROP TRIGGER project_search_custom_field_delete; DROP TRIGGER project_search_content_insert; DROP TRIGGER project_search_content_update; DROP TRIGGER project_search_content_delete; DROP TRIGGER project_search_scene_insert; DROP TRIGGER project_search_scene_update; DROP TRIGGER project_search_scene_delete; DROP TRIGGER project_search_scene_text_insert; DROP TRIGGER project_search_scene_text_update; DROP TRIGGER project_search_scene_text_delete; DROP TRIGGER project_search_character_insert; DROP TRIGGER project_search_character_update; DROP TRIGGER project_search_character_delete; DROP TRIGGER project_search_location_insert; DROP TRIGGER project_search_location_update; DROP TRIGGER project_search_location_delete; DROP TRIGGER project_search_plotline_insert; DROP TRIGGER project_search_plotline_update; DROP TRIGGER project_search_plotline_delete; DROP TRIGGER project_search_event_insert; DROP TRIGGER project_search_event_update; DROP TRIGGER project_search_event_delete; DROP TRIGGER project_search_research_insert; DROP TRIGGER project_search_research_update; DROP TRIGGER project_search_research_delete; DROP TRIGGER project_search_issue_insert; DROP TRIGGER project_search_issue_update; DROP TRIGGER project_search_issue_delete; DROP TRIGGER project_search_reading_insert; DROP TRIGGER project_search_reading_update; DROP TRIGGER project_search_reading_delete; DROP TABLE project_search_fts; DROP TABLE project_search_content; DELETE FROM schema_migrations WHERE version>4;").unwrap();
        drop(conn);
        let st = ProjectStore::open(&project).unwrap();
        assert_eq!(st.search_scenes("obsidian").unwrap(), vec![scene]);
        let version: i64 = st
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 6);
        let hits = st.search_project("obsidian", 10).unwrap();
        assert_eq!(hits[0].entity_id, scene.to_string());
    }

    #[test]
    fn snapshots_diff_selective_restore_and_full_restore_round_trip() {
        let d = tempdir().unwrap();
        let mut st = ProjectStore::create(d.path().join("N"), "Novel").unwrap();
        let chapter = st.create_node(None, NodeKind::Chapter, "Ch", 0).unwrap();
        let scene = st.create_scene(chapter, 0, "Scene").unwrap();
        let (_, hash) = st.load_text(scene).unwrap();
        st.save_text(scene, Some(&hash), "Opening\nOld middle\nEnding\n")
            .unwrap();
        let snapshot = st.create_snapshot(scene, "First draft").unwrap();
        let (_, hash) = st.load_text(scene).unwrap();
        st.save_text(scene, Some(&hash), "Opening\nNew middle\nEnding\n")
            .unwrap();
        let diff = st.diff_snapshot(scene, snapshot).unwrap();
        assert!(
            diff.iter()
                .any(|line| line.kind == SnapshotLineKind::Removed && line.text == "Old middle\n")
        );
        assert!(
            diff.iter()
                .any(|line| line.kind == SnapshotLineKind::Added && line.text == "New middle\n")
        );
        let (_, hash) = st.load_text(scene).unwrap();
        st.restore_snapshot_lines(
            scene,
            snapshot,
            &hash,
            SnapshotRestoreSelection {
                current_start: 1,
                current_end: 2,
                snapshot_start: 1,
                snapshot_end: 2,
            },
        )
        .unwrap();
        assert_eq!(
            st.load_text(scene).unwrap().0,
            "Opening\nOld middle\nEnding\n"
        );
        let (_, hash) = st.load_text(scene).unwrap();
        st.save_text(scene, Some(&hash), "A later edit\n").unwrap();
        let (_, hash) = st.load_text(scene).unwrap();
        st.restore_snapshot(scene, snapshot, &hash).unwrap();
        assert_eq!(
            st.load_text(scene).unwrap().0,
            "Opening\nOld middle\nEnding\n"
        );
        assert!(st.list_snapshots(scene).unwrap().len() >= 3);
    }
    #[test]
    fn create_reopen_relocate_and_save_scene() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut st = ProjectStore::create(&p, "Novel").unwrap();
        let ch = st.create_node(None, NodeKind::Chapter, "Ch 1", 0).unwrap();
        let moved_to = st.create_node(None, NodeKind::Chapter, "Ch 2", 1).unwrap();
        let sc = st.create_scene(ch, 0, "Opening").unwrap();
        let character = CharacterId::new();
        st.add_character(character, "Mara").unwrap();
        st.link_character(sc, character, ParticipationRole::Pov)
            .unwrap();
        let plot = PlotlineId::new();
        st.add_plotline(plot, "Main").unwrap();
        st.set_scene_plotlines(
            sc,
            &[ScenePlotline {
                plotline_id: plot,
                role: "advances".into(),
                share_basis_points: Some(10000),
            }],
            WordAttributionPolicy::AuthorShares,
        )
        .unwrap();
        let (txt, h) = st.load_text(sc).unwrap();
        assert_eq!(txt, "");
        let new = st.save_text(sc, Some(&h), "Once upon a time.\n").unwrap();
        st.update_scene(&Scene {
            id: sc,
            title: "Opening revised".into(),
            synopsis: "Start".into(),
            status: SceneStatus::Drafting,
            target_words: Some(100),
            purpose: "Introduce".into(),
            goal: "Escape".into(),
            outcome: "Fails".into(),
            start_time: StoryTime::exact(-3),
            end_time: StoryTime::unknown(),
            pov_character: None,
            start_location: None,
            end_location: None,
            archived: false,
        })
        .unwrap();
        st.move_scene(sc, moved_to, 0).unwrap();
        drop(st);
        let relocated = d.path().join("Moved");
        fs::rename(&p, &relocated).unwrap();
        let st = ProjectStore::open(&relocated).unwrap();
        let rec = st.get_scene(sc).unwrap();
        assert_eq!(rec.content_hash, new);
        assert_eq!(rec.scene.title, "Opening revised");
        assert_eq!(rec.actual_words, 4);
        assert_eq!(rec.chapter_id, Some(moved_to));
        assert_eq!(
            st.conn
                .query_row(
                    "SELECT count(*) FROM scene_character WHERE scene_id=?1",
                    [sc.to_string()],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            st.conn
                .query_row(
                    "SELECT count(*) FROM scene_plotline WHERE scene_id=?1",
                    [sc.to_string()],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }
    #[test]
    fn mismatched_identity_rejected() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        ProjectStore::create(&p, "Novel").unwrap();
        let cfg = fs::read_to_string(p.join("project.toml")).unwrap().replace(
            "id = \"",
            "id = \"00000000-0000-0000-0000-000000000000\" # ",
        );
        fs::write(p.join("project.toml"), cfg).unwrap();
        assert!(ProjectStore::open(&p).is_err());
    }
    #[test]
    fn refuses_to_overwrite_nonempty_directory_and_newer_schema() {
        let d = tempdir().unwrap();
        let occupied = d.path().join("occupied");
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("notes.txt"), "keep").unwrap();
        assert!(ProjectStore::create(&occupied, "N").is_err());
        assert_eq!(
            fs::read_to_string(occupied.join("notes.txt")).unwrap(),
            "keep"
        );
        let p = d.path().join("N");
        let s = ProjectStore::create(&p, "N").unwrap();
        s.conn
            .execute(
                "UPDATE schema_migrations SET version=99 WHERE version=2",
                [],
            )
            .unwrap();
        drop(s);
        assert!(matches!(
            ProjectStore::open(&p),
            Err(StorageError::UnsupportedSchema(99))
        ));
    }
    #[test]
    fn project_metadata_and_fictional_epoch_round_trip() {
        let d = tempdir().unwrap();
        let p = d.path().join("Novel");
        let project = Project {
            id: ProjectId::new(),
            title: "Working title".into(),
            author: "Author".into(),
            genre: "Fantasy".into(),
            synopsis: "Synopsis".into(),
            target_words: Some(90000),
            story_epoch_label: "Before the Sundering".into(),
            story_calendar_id: Some("twelve-moons".into()),
        };
        let s = ProjectStore::create_for(&p, &project).unwrap();
        let got = s.get_project().unwrap();
        assert_eq!(got.id, project.id);
        assert_eq!(got.target_words, Some(90000));
        assert_eq!(got.story_epoch_label, "Before the Sundering");
        assert_eq!(got.story_calendar_id.as_deref(), Some("twelve-moons"));
        assert!(s.conn.execute("UPDATE project SET id='00000000-0000-0000-0000-000000000000' WHERE singleton=1",[]).is_err());
        assert!(s.conn.execute("INSERT INTO project(singleton,id,title,created_at,updated_at) VALUES(1,?1,'Other','0','0')",[Uuid::new_v4().to_string()]).is_err());
    }
    #[test]
    fn backup_contains_consistent_database_and_manuscript() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let (_, h) = s.load_text(sc).unwrap();
        s.save_text(sc, Some(&h), "backup text").unwrap();
        let backup = d.path().join("N-backup");
        s.backup_to(&backup).unwrap();
        let b = ProjectStore::open(&backup).unwrap();
        assert_eq!(b.load_text(sc).unwrap().0, "backup text");
        assert!(b.validate().is_ok());
    }
    #[test]
    fn placement_trigger_rejects_non_chapter_sql() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let section = s.create_node(None, NodeKind::Section, "Act", 0).unwrap();
        let chapter = s
            .create_node(Some(section), NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = s.create_scene(chapter, 0, "S").unwrap();
        let result = s.conn.execute(
            "UPDATE scene_placement SET chapter_id=?2 WHERE scene_id=?1",
            params![scene.to_string(), section.to_string()],
        );
        assert!(result.is_err());
    }
    #[test]
    fn root_positions_unique_and_move_scene() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let _ = s.create_node(None, NodeKind::Section, "A", 0).unwrap();
        assert!(
            s.create_node(None, NodeKind::Chapter, "Duplicate", 0)
                .is_err()
        );
        let a = s.create_node(None, NodeKind::Chapter, "A", 1).unwrap();
        let b = s.create_node(None, NodeKind::Chapter, "B", 2).unwrap();
        let x = s.create_scene(a, 0, "x").unwrap();
        let y = s.create_scene(a, 1, "y").unwrap();
        assert!(s.move_scene_inner(y, b, 0, true).is_err());
        assert_eq!(s.get_scene(x).unwrap().position, Some(0));
        assert_eq!(s.get_scene(y).unwrap().position, Some(1));
        s.move_scene(y, b, 0).unwrap();
        s.move_scene(x, a, 0).unwrap();
        assert_eq!(s.get_scene(x).unwrap().position, Some(0));
        assert_eq!(s.get_scene(y).unwrap().chapter_id, Some(b));
    }
    #[test]
    fn structural_moves_reorder_and_reject_cycles_without_partial_changes() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let a = s.create_node(None, NodeKind::Section, "A", 0).unwrap();
        let b = s.create_node(None, NodeKind::Section, "B", 1).unwrap();
        let ch = s
            .create_node(Some(a), NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        s.move_node(ch, Some(b), 0).unwrap();
        let parent: String = s
            .conn
            .query_row(
                "SELECT parent_id FROM structural_node WHERE id=?1",
                [ch.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(parent, b.to_string());
        assert!(s.move_node(b, Some(ch), 0).is_err());
        let c = s.create_node(None, NodeKind::Section, "C", 2).unwrap();
        let dnode = s.create_node(None, NodeKind::Section, "D", 3).unwrap();
        let result = s.conn.execute(
            "UPDATE structural_node SET parent_id=?2 WHERE id=?1",
            params![c.to_string(), dnode.to_string()],
        );
        assert!(result.is_ok());
        let result = s.conn.execute(
            "UPDATE structural_node SET parent_id=?2 WHERE id=?1",
            params![dnode.to_string(), c.to_string()],
        );
        assert!(result.is_err());
        let root_order = query_node_ids(&s.conn, None).unwrap();
        assert_eq!(
            root_order,
            vec![a.to_string(), b.to_string(), dnode.to_string()]
        );
    }
    #[test]
    fn direct_sql_cannot_duplicate_roots_or_retype_populated_chapter() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let sec = s.create_node(None, NodeKind::Section, "Sec", 1).unwrap();
        assert!(
            s.conn
                .execute(
                    "UPDATE structural_node SET position=0 WHERE id=?1",
                    [sec.to_string()]
                )
                .is_err()
        );
        assert!(
            s.conn
                .execute(
                    "UPDATE structural_node SET kind='group' WHERE id=?1",
                    [ch.to_string()]
                )
                .is_err()
        );
        assert!(
            s.conn
                .execute("DELETE FROM structural_node WHERE id=?1", [ch.to_string()])
                .is_err()
        );
        assert!(
            s.conn
                .execute(
                    "DELETE FROM scene_placement WHERE scene_id=?1",
                    [sc.to_string()]
                )
                .is_ok()
        );
    }
    #[test]
    fn custom_metadata_and_relationships_are_typed_and_transactional() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let char = CharacterId::new();
        s.add_character(char, "Mara").unwrap();
        s.link_character(sc, char, ParticipationRole::Pov).unwrap();
        let char2 = CharacterId::new();
        s.add_character(char2, "Tomas").unwrap();
        s.link_character(sc, char2, ParticipationRole::Minor)
            .unwrap();
        let plot1 = PlotlineId::new();
        let plot2 = PlotlineId::new();
        s.add_plotline(plot1, "Main").unwrap();
        s.add_plotline(plot2, "Sub").unwrap();
        let links = vec![
            ScenePlotline {
                plotline_id: plot1,
                role: "advances".into(),
                share_basis_points: Some(6000),
            },
            ScenePlotline {
                plotline_id: plot2,
                role: "advances".into(),
                share_basis_points: Some(4000),
            },
        ];
        s.set_scene_plotlines(sc, &links, WordAttributionPolicy::AuthorShares)
            .unwrap();
        let bad = vec![links[0].clone()];
        assert!(
            s.set_scene_plotlines(sc, &bad, WordAttributionPolicy::AuthorShares)
                .is_err()
        );
        let sum: i64 = s
            .conn
            .query_row(
                "SELECT sum(share_basis_points) FROM scene_plotline WHERE scene_id=?1",
                [sc.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sum, 10000);
        let f = FieldId::new();
        let def = CustomFieldDefinition {
            id: f,
            name: "Threat Level".into(),
            field_type: FieldType::Enum,
            required: true,
            default_value: None,
            enum_values: vec!["low".into(), "high".into()],
            reference_kind: None,
        };
        s.define_custom_field(&def).unwrap();
        assert!(s.validate().is_err());
        assert!(
            s.set_custom_field(sc, f, &FieldValue::Enum("medium".into()))
                .is_err()
        );
        s.set_custom_field(sc, f, &FieldValue::Enum("high".into()))
            .unwrap();
        let got: String = s
            .conn
            .query_row(
                "SELECT text_value FROM custom_field_value WHERE scene_id=?1 AND field_id=?2",
                params![sc.to_string(), f.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got, "high");
        assert_eq!(
            s.filter_scenes_by_custom_field(f, &FieldValue::Enum("high".into()))
                .unwrap(),
            vec![sc]
        );
        s.validate().unwrap();
        let ev1 = EventId::new();
        let ev2 = EventId::new();
        s.add_event(ev1, "Arrival", "").unwrap();
        s.add_event(ev2, "Discovery", "").unwrap();
        s.link_event(sc, ev1, "depicts").unwrap();
        s.link_event(sc, ev2, "depicts").unwrap();
        assert_eq!(
            s.conn
                .query_row(
                    "SELECT count(*) FROM scene_event WHERE scene_id=?1",
                    [sc.to_string()],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
        let second_scene = s.create_scene(ch, 1, "Second").unwrap();
        s.link_event(second_scene, ev1, "depicts").unwrap();
        assert_eq!(
            s.conn
                .query_row(
                    "SELECT count(*) FROM scene_event WHERE event_id=?1",
                    [ev1.to_string()],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }
    #[test]
    fn provenance_records_source_and_mapping_and_reject_duplicate_document() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let source = b"synthetic source";
        let fingerprint = hash_bytes(source);
        let run = ImportRunId::new();
        s.record_import_run(run, "spreadsheet", "fixture.ods", &fingerprint)
            .unwrap();
        assert!(
            s.record_import_run(
                ImportRunId::new(),
                "spreadsheet",
                "fixture.ods",
                &fingerprint
            )
            .is_err()
        );
        s.record_import_run_explicit_repeat(
            ImportRunId::new(),
            "spreadsheet",
            "fixture.ods",
            &fingerprint,
            true,
        )
        .unwrap();
        let item = SourceItemId::new();
        s.record_source_item(ImportSourceItemRecord {
            id: item,
            run,
            source_document_id: "Arc:row:2",
            source_sheet: Some("Arc"),
            source_row: Some(2),
            fingerprint: "rowhash",
            inventory_type: "scene",
            raw_fields: "Title=Opening",
        })
        .unwrap();
        let dest = SceneId::new();
        s.record_provenance(EntityProvenanceRecord {
            id: ProvenanceId::new(),
            source_item: item,
            destination_type: "scene",
            destination_id: &dest.to_string(),
            source_field: Some("Title"),
            destination_field: Some("title"),
            mapping: "direct",
        })
        .unwrap();
        assert!(
            s.record_source_item(ImportSourceItemRecord {
                id: SourceItemId::new(),
                run,
                source_document_id: "Arc:row:2",
                source_sheet: Some("Arc"),
                source_row: Some(2),
                fingerprint: "rowhash",
                inventory_type: "scene",
                raw_fields: ""
            })
            .is_err()
        );
        assert_eq!(hash_bytes(source), fingerprint);
    }
    #[test]
    fn custom_defaults_are_applied_and_entity_references_are_checked() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let character = CharacterId::new();
        s.add_character(character, "Mara").unwrap();
        let field = FieldId::new();
        s.define_custom_field(&CustomFieldDefinition {
            id: field,
            name: "Tone".into(),
            field_type: FieldType::Text,
            required: true,
            default_value: Some(FieldValue::Text("grim".into())),
            enum_values: vec![],
            reference_kind: None,
        })
        .unwrap();
        let scene = s.create_scene(ch, 0, "S").unwrap();
        let value: String = s
            .conn
            .query_row(
                "SELECT text_value FROM custom_field_value WHERE scene_id=?1 AND field_id=?2",
                params![scene.to_string(), field.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, "grim");
        let rf = FieldId::new();
        s.define_custom_field(&CustomFieldDefinition {
            id: rf,
            name: "Related character".into(),
            field_type: FieldType::EntityRef,
            required: false,
            default_value: None,
            enum_values: vec![],
            reference_kind: Some("character".into()),
        })
        .unwrap();
        assert!(
            s.set_custom_field(
                scene,
                rf,
                &FieldValue::EntityRef(Uuid::new_v4().to_string())
            )
            .is_err()
        );
        s.set_custom_field(scene, rf, &FieldValue::EntityRef(character.to_string()))
            .unwrap();
    }
    #[test]
    fn external_change_is_detected() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let (_, h) = s.load_text(sc).unwrap();
        fs::write(p.join("manuscript").join(format!("{sc}.md")), "external").unwrap();
        assert!(s.save_text(sc, Some(&h), "ours").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn project_managed_paths_reject_symlinks() {
        use std::os::unix::fs::symlink;
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let target = p.join("manuscript").join(format!("{sc}.md"));
        fs::remove_file(&target).unwrap();
        let outside = d.path().join("outside.md");
        fs::write(&outside, "external").unwrap();
        symlink(outside, &target).unwrap();
        assert!(s.load_text(sc).is_err());
    }

    #[test]
    fn proposal_files_are_project_local_and_never_overwritten() {
        let d = tempdir().unwrap();
        let store = ProjectStore::create(d.path().join("Novel"), "Novel").unwrap();
        let path = store
            .save_proposal_file("candidate.json", br#"{"approved":false}"#)
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), br#"{"approved":false}"#);
        assert!(
            store
                .save_proposal_file("candidate.json", b"overwrite")
                .is_err()
        );
        assert!(
            store
                .save_proposal_file("../escape.json", b"escape")
                .is_err()
        );
        assert!(path.starts_with(store.root()));
    }
    #[test]
    fn journal_recovers_all_committed_crash_windows() {
        for (idx, point, expected) in [
            (0, SaveFailpoint::AfterTempSync, ""),
            (1, SaveFailpoint::AfterIntent, "new prose"),
            (2, SaveFailpoint::BeforeRename, "new prose"),
            (3, SaveFailpoint::AfterRename, "new prose"),
            (4, SaveFailpoint::AfterMetadataCommit, "new prose"),
            (5, SaveFailpoint::BeforeCleanup, "new prose"),
            (6, SaveFailpoint::AfterJournalCleanup, "new prose"),
            (7, SaveFailpoint::AfterPartialTemp, ""),
        ] {
            let d = tempdir().unwrap();
            let p = d.path().join(format!("N{idx}"));
            let mut s = ProjectStore::create(&p, "N").unwrap();
            let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
            let sc = s.create_scene(ch, 0, "S").unwrap();
            let (_, h) = s.load_text(sc).unwrap();
            assert!(
                s.save_text_inner(sc, Some(&h), "new prose", Some(point))
                    .is_err()
            );
            drop(s);
            let s = ProjectStore::open(&p).unwrap();
            let path = p.join("manuscript").join(format!("{sc}.md"));
            assert_eq!(fs::read_to_string(path).unwrap(), expected);
            assert_eq!(
                s.get_scene(sc).unwrap().content_hash,
                hash_bytes(expected.as_bytes())
            );
        }
    }
    #[test]
    fn recovery_preserves_external_and_prepared_versions() {
        let d = tempdir().unwrap();
        let p = d.path().join("N");
        let mut s = ProjectStore::create(&p, "N").unwrap();
        let ch = s.create_node(None, NodeKind::Chapter, "C", 0).unwrap();
        let sc = s.create_scene(ch, 0, "S").unwrap();
        let (_, h) = s.load_text(sc).unwrap();
        assert!(
            s.save_text_inner(
                sc,
                Some(&h),
                "prepared content",
                Some(SaveFailpoint::AfterRename)
            )
            .is_err()
        );
        fs::write(
            p.join("manuscript").join(format!("{sc}.md")),
            "external edit",
        )
        .unwrap();
        drop(s);
        let mut s = ProjectStore::open(&p).unwrap();
        let conflicts = s
            .conn
            .query_row("SELECT count(*) FROM write_conflict", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(conflicts, 1);
        let mut found = false;
        for e in fs::read_dir(p.join(".writers-ide/recovery")).unwrap() {
            let d = e.unwrap().path();
            let candidate = d.join("prepared-version.md");
            if candidate.exists() {
                assert_eq!(fs::read_to_string(candidate).unwrap(), "prepared content");
                found = true;
            }
        }
        assert!(found);
        assert_eq!(
            fs::read_to_string(p.join("manuscript").join(format!("{sc}.md"))).unwrap(),
            "external edit"
        );
        let txid: String = s
            .conn
            .query_row(
                "SELECT tx_id FROM write_conflict WHERE resolved=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        s.resolve_write_conflict(&txid, ConflictResolution::UsePrepared)
            .unwrap();
        assert_eq!(s.load_text(sc).unwrap().0, "prepared content");
        assert_eq!(s.pending_conflicts().unwrap(), 0);
        assert_eq!(
            fs::read_to_string(
                p.join(".writers-ide/recovery")
                    .join(txid)
                    .join("external-version.md")
            )
            .unwrap(),
            "external edit"
        );
    }
}
