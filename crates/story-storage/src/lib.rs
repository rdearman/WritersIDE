use rusqlite::{
    Connection, OptionalExtension, TransactionBehavior, backup::Backup, params, types::ValueRef,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use story_core::*;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

const MIGRATION_1: &str = include_str!("../migrations/0001_initial.sql");
const MIGRATION_2: &str = include_str!("../migrations/0002_links_queries.sql");
const MIGRATION_3: &str = include_str!("../migrations/0003_reading.sql");
const MIGRATION_4: &str = include_str!("../migrations/0004_snapshots.sql");
const MIGRATION_5: &str = include_str!("../migrations/0005_project_search.sql");
const MIGRATION_6: &str = include_str!("../migrations/0006_search_custom_fields.sql");
const MIGRATION_7: &str = include_str!("../migrations/0007_approved_knowledge.sql");
const MIGRATION_8: &str = include_str!("../migrations/0008_document_inclusion.sql");
const MIGRATION_9: &str = include_str!("../migrations/0009_reading_document_links.sql");
const MIGRATION_10: &str = include_str!("../migrations/0010_character_profiles.sql");
const MIGRATION_11: &str = include_str!("../migrations/0011_book_types_content_units.sql");
const MIGRATION_12: &str = include_str!("../migrations/0012_project_template.sql");
const MIGRATION_13: &str = include_str!("../migrations/0013_writing_workflow.sql");
const MIGRATION_14: &str = include_str!("../migrations/0014_editorial_comments.sql");
const MIGRATION_15: &str = include_str!("../migrations/0015_manuscript_assemblies.sql");
const MIGRATION_16: &str = include_str!("../migrations/0016_planning_boards.sql");
const MIGRATION_17: &str = include_str!("../migrations/0017_project_language.sql");
const MIGRATION_18: &str = include_str!("../migrations/0018_writing_challenges.sql");
const MIGRATION_19: &str = include_str!("../migrations/0019_query_assertions.sql");
const MIGRATION_20: &str = include_str!("../migrations/0020_writing_sessions.sql");
const MIGRATION_21: &str = include_str!("../migrations/0021_query_boolean_filters.sql");
const MIGRATION_22: &str = include_str!("../migrations/0022_challenge_outcomes.sql");
const MIGRATION_23: &str = include_str!("../migrations/0023_query_expressions.sql");
const MIGRATION_24: &str = include_str!("../migrations/0024_writing_goal_ledger.sql");
const MIGRATION_25: &str = include_str!("../migrations/0025_inspection_query_filters.sql");
const MIGRATION_26: &str = include_str!("../migrations/0026_explicit_scene_absence.sql");
const MIGRATION_27: &str = include_str!("../migrations/0027_event_dependencies.sql");
const MIGRATION_28: &str = include_str!("../migrations/0028_assembly_issue_query_filters.sql");
const MIGRATION_29: &str = include_str!("../migrations/0029_entity_ref_query_filters.sql");
const MIGRATION_30: &str =
    include_str!("../migrations/0030_uncertainty_aware_time_query_filters.sql");
const MIGRATION_31: &str = include_str!("../migrations/0031_location_incompatibility_rules.sql");
const MIGRATION_32: &str = include_str!("../migrations/0032_location_profiles.sql");
const MIGRATION_33: &str = include_str!("../migrations/0033_custom_metadata_query_operands.sql");
const MIGRATION_34: &str = include_str!("../migrations/0034_word_count_policy.sql");
const MIGRATION_35: &str = include_str!("../migrations/0035_scene_language.sql");
const MIGRATION_36: &str = include_str!("../migrations/0036_passage_language_tags.sql");
const MIGRATION_37: &str = include_str!("../migrations/0037_scene_word_count_policy.sql");
const MIGRATION_38: &str = include_str!("../migrations/0038_content_unit_language.sql");
const MIGRATION_39: &str = include_str!("../migrations/0039_reading_language.sql");
const MIGRATION_40: &str = include_str!("../migrations/0040_story_interval_boundaries.sql");
const MIGRATION_41: &str = include_str!("../migrations/0041_event_relationships.sql");
const MIGRATION_42: &str = include_str!("../migrations/0042_diagnostic_suppressions.sql");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("backup archive: {0}")]
    Archive(#[from] zip::result::ZipError),
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
type ProjectRow = (
    String,
    String,
    String,
    String,
    String,
    Option<u64>,
    String,
    Option<String>,
    String,
    Option<String>,
);
type ContentUnitJournalRow = (
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
);

fn valid_issue_status(status: &str) -> bool {
    matches!(
        status,
        "open" | "in_progress" | "resolved" | "deferred" | "closed"
    )
}

fn validate_query_expression(expression: &QueryExpression) -> Result<()> {
    fn walk(expression: &QueryExpression, depth: usize, nodes: &mut usize) -> Result<()> {
        *nodes += 1;
        if depth > 32 || *nodes > 512 {
            return Err(StorageError::Invalid(
                "saved query expression exceeds the 32-level/512-node limit".into(),
            ));
        }
        match expression {
            QueryExpression::Filter(SceneQueryFilter::AnyCharacter(ids)) if ids.is_empty() => Err(
                StorageError::Invalid("character-any query filter cannot be empty".into()),
            ),
            QueryExpression::Filter(SceneQueryFilter::AnyPlotline(ids)) if ids.is_empty() => Err(
                StorageError::Invalid("plotline-any query filter cannot be empty".into()),
            ),
            QueryExpression::Filter(SceneQueryFilter::IssueStatus(status))
                if !valid_issue_status(status) =>
            {
                Err(StorageError::Invalid(format!(
                    "invalid issue status query filter: {status}"
                )))
            }
            QueryExpression::All(children) | QueryExpression::Any(children) => {
                if children.is_empty() {
                    return Err(StorageError::Invalid(
                        "saved query AND/OR groups cannot be empty".into(),
                    ));
                }
                for child in children {
                    walk(child, depth + 1, nodes)?;
                }
                Ok(())
            }
            QueryExpression::Not(child) => walk(child, depth + 1, nodes),
            QueryExpression::Filter(_) => Ok(()),
        }
    }
    walk(expression, 1, &mut 0)
}

fn validate_custom_metadata_filter(conn: &Connection, filter: &SceneQueryFilter) -> Result<()> {
    let (field_id, expected) = match filter {
        SceneQueryFilter::CustomMetadataExists { field_id } => (*field_id, None),
        SceneQueryFilter::CustomMetadataTextContains { field_id, text } => {
            if text.trim().is_empty() {
                return Err(StorageError::Invalid(
                    "custom metadata text query cannot be empty".into(),
                ));
            }
            (*field_id, Some("text"))
        }
        SceneQueryFilter::CustomMetadataNumberCompare {
            field_id,
            value: FieldValue::Integer(_),
            ..
        } => (*field_id, Some("integer")),
        SceneQueryFilter::CustomMetadataNumberCompare {
            field_id,
            value: FieldValue::Decimal(value),
            ..
        } => {
            if !matches!(value.parse::<f64>(), Ok(number) if number.is_finite()) {
                return Err(StorageError::Invalid(
                    "custom metadata decimal comparison is not finite".into(),
                ));
            }
            (*field_id, Some("decimal"))
        }
        SceneQueryFilter::CustomMetadataNumberCompare { .. } => {
            return Err(StorageError::Invalid(
                "numeric metadata comparisons require an integer or decimal value".into(),
            ));
        }
        _ => return Ok(()),
    };
    let field_type: String = conn
        .query_row(
            "SELECT field_type FROM custom_field_definition WHERE id=?1",
            [field_id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::NotFound(field_id.to_string()))?;
    if let Some(expected) = expected {
        let compatible = match expected {
            "text" => matches!(
                field_type.as_str(),
                "text" | "long_text" | "enum" | "date_time"
            ),
            "integer" | "decimal" => field_type == expected,
            _ => false,
        };
        if !compatible {
            return Err(StorageError::Invalid(format!(
                "custom metadata query requires a {expected}-compatible field, found {field_type}"
            )));
        }
    }
    Ok(())
}

fn validate_custom_metadata_expression(
    conn: &Connection,
    expression: &QueryExpression,
) -> Result<()> {
    match expression {
        QueryExpression::Filter(filter) => validate_custom_metadata_filter(conn, filter),
        QueryExpression::All(children) | QueryExpression::Any(children) => {
            for child in children {
                validate_custom_metadata_expression(conn, child)?;
            }
            Ok(())
        }
        QueryExpression::Not(child) => validate_custom_metadata_expression(conn, child),
    }
}

fn positive_text_query_terms(expression: &QueryExpression, negated: bool, terms: &mut Vec<String>) {
    match expression {
        QueryExpression::Filter(SceneQueryFilter::TextContains(term)) if !negated => {
            terms.push(term.clone());
        }
        QueryExpression::All(children) | QueryExpression::Any(children) => {
            for child in children {
                positive_text_query_terms(child, negated, terms);
            }
        }
        QueryExpression::Not(child) => positive_text_query_terms(child, !negated, terms),
        QueryExpression::Filter(_) => {}
    }
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Vec<(u64, u64)> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let bytes = haystack.as_bytes();
    let needle = needle.as_bytes();
    bytes
        .windows(needle.len())
        .enumerate()
        .filter(|(offset, candidate)| {
            haystack.is_char_boundary(*offset)
                && haystack.is_char_boundary(offset + needle.len())
                && candidate.eq_ignore_ascii_case(needle)
        })
        .take(32)
        .map(|(start, _)| (start as u64, (start + needle.len()) as u64))
        .collect()
}

fn compile_query_expression(
    expression: &QueryExpression,
    values: &mut Vec<String>,
) -> Result<String> {
    let combine = |operator: &str,
                   children: &[QueryExpression],
                   values: &mut Vec<String>|
     -> Result<String> {
        let parts = children
            .iter()
            .map(|child| compile_query_expression(child, values))
            .collect::<Result<Vec<_>>>()?;
        Ok(format!("({})", parts.join(operator)))
    };
    Ok(match expression {
        QueryExpression::All(children) => combine(" AND ", children, values)?,
        QueryExpression::Any(children) => combine(" OR ", children, values)?,
        QueryExpression::Not(child) => {
            format!("(NOT {})", compile_query_expression(child, values)?)
        }
        QueryExpression::Filter(filter) => match filter {
            SceneQueryFilter::Status(status) => {
                values.push(status.as_str().to_owned());
                "s.status=?".into()
            }
            SceneQueryFilter::ExcludeStatus(status) => {
                values.push(status.as_str().to_owned());
                "s.status<>?".into()
            }
            SceneQueryFilter::Character(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=?)".into()
            }
            SceneQueryFilter::CharacterPresent(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=? AND sc.participation_role IN('pov','major','minor','off_camera'))".into()
            }
            SceneQueryFilter::CharacterMentioned(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=? AND sc.participation_role='mentioned')".into()
            }
            SceneQueryFilter::CharacterAbsent(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_character_absence a WHERE a.scene_id=s.id AND a.character_id=?)".into()
            }
            SceneQueryFilter::AnyCharacter(ids) => {
                if ids.is_empty() {
                    return Err(StorageError::Invalid("character-any query filter cannot be empty".into()));
                }
                values.extend(ids.iter().map(ToString::to_string));
                format!("EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id IN ({}))", vec!["?"; ids.len()].join(","))
            }
            SceneQueryFilter::Plotline(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_plotline sp WHERE sp.scene_id=s.id AND sp.plotline_id=?)".into()
            }
            SceneQueryFilter::AnyPlotline(ids) => {
                if ids.is_empty() {
                    return Err(StorageError::Invalid("plotline-any query filter cannot be empty".into()));
                }
                values.extend(ids.iter().map(ToString::to_string));
                format!("EXISTS(SELECT 1 FROM scene_plotline sp WHERE sp.scene_id=s.id AND sp.plotline_id IN ({}))", vec!["?"; ids.len()].join(","))
            }
            SceneQueryFilter::Location(id) => {
                values.push(id.to_string());
                values.push(id.to_string());
                "(s.start_location_id=? OR s.end_location_id=?)".into()
            }
            SceneQueryFilter::Event(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM scene_event se WHERE se.scene_id=s.id AND se.event_id=?)".into()
            }
            SceneQueryFilter::Assembly(id) => {
                values.push(id.to_string());
                "EXISTS(SELECT 1 FROM assembly_item ai WHERE ai.scene_id=s.id AND ai.assembly_id=? AND ai.included=1)".into()
            }
            SceneQueryFilter::IssueStatus(status) => {
                if !valid_issue_status(status) { return Err(StorageError::Invalid(format!("invalid issue status query filter: {status}"))); }
                values.push(status.clone());
                "EXISTS(SELECT 1 FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=s.id AND i.status=?)".into()
            }
            SceneQueryFilter::Chapter(id) => {
                values.push(id.clone());
                "p.chapter_id=?".into()
            }
            SceneQueryFilter::TextContains(text) => {
                if text.trim().is_empty() { return Err(StorageError::Invalid("text query cannot be empty".into())); }
                let pattern = format!("%{}%", text.replace('%', "\\%").replace('_', "\\_"));
                values.extend([pattern.clone(), pattern.clone(), pattern]);
                "(s.title LIKE ? ESCAPE '\\' OR s.synopsis LIKE ? ESCAPE '\\' OR EXISTS(SELECT 1 FROM scene_search_content sc WHERE sc.scene_id=s.id AND sc.content LIKE ? ESCAPE '\\'))".into()
            }
            SceneQueryFilter::CustomMetadataEquals { field_id, value } => {
                let (column, encoded) = match value {
                    FieldValue::Text(value) | FieldValue::DateTime(value) | FieldValue::Enum(value) => ("text_value", value.clone()),
                    FieldValue::Integer(value) => ("integer_value", value.to_string()),
                    FieldValue::Decimal(value) => ("decimal_value", value.clone()),
                    FieldValue::Boolean(value) => ("boolean_value", i64::from(*value).to_string()),
                    FieldValue::EntityRef(value) => ("entity_ref", value.clone()),
                    FieldValue::EntityRefs(_) => return Err(StorageError::Invalid("querying entity-reference lists for equality is unsupported; use a related-entity filter".into())),
                };
                values.push(field_id.to_string());
                values.push(encoded);
                format!("EXISTS(SELECT 1 FROM custom_field_value cv WHERE cv.scene_id=s.id AND cv.field_id=? AND CAST(cv.{column} AS TEXT)=?)")
            }
            SceneQueryFilter::CustomMetadataContainsEntity { field_id, target_kind, target_id } => {
                if !matches!(target_kind.as_str(), "scene" | "character" | "location" | "object" | "plotline" | "event") {
                    return Err(StorageError::Invalid(format!("unsupported custom-metadata entity kind: {target_kind}")));
                }
                values.extend([field_id.to_string(), target_kind.clone(), target_id.clone()]);
                "EXISTS(SELECT 1 FROM custom_field_entity_ref r WHERE r.scene_id=s.id AND r.field_id=? AND r.target_kind=? AND r.target_id=?)".into()
            }
            SceneQueryFilter::CustomMetadataExists { field_id } => {
                values.push(field_id.to_string());
                "EXISTS(SELECT 1 FROM custom_field_value cv WHERE cv.scene_id=s.id AND cv.field_id=?)".into()
            }
            SceneQueryFilter::CustomMetadataTextContains { field_id, text } => {
                if text.trim().is_empty() {
                    return Err(StorageError::Invalid("custom metadata text query cannot be empty".into()));
                }
                let pattern = format!("%{}%", text.replace('%', "\\%").replace('_', "\\_"));
                values.extend([field_id.to_string(), pattern]);
                "EXISTS(SELECT 1 FROM custom_field_value cv JOIN custom_field_definition d ON d.id=cv.field_id WHERE cv.scene_id=s.id AND cv.field_id=? AND d.field_type IN('text','long_text','enum','date_time') AND cv.text_value LIKE ? ESCAPE '\\')".into()
            }
            SceneQueryFilter::CustomMetadataNumberCompare { field_id, comparison, value } => {
                let operator = comparison.sql_operator();
                match value {
                    FieldValue::Integer(value) => {
                        values.extend([field_id.to_string(), value.to_string()]);
                        format!("EXISTS(SELECT 1 FROM custom_field_value cv JOIN custom_field_definition d ON d.id=cv.field_id WHERE cv.scene_id=s.id AND cv.field_id=? AND d.field_type='integer' AND cv.integer_value {operator} CAST(? AS INTEGER))")
                    }
                    FieldValue::Decimal(value) if matches!(value.parse::<f64>(), Ok(number) if number.is_finite()) => {
                        values.extend([field_id.to_string(), value.clone()]);
                        format!("EXISTS(SELECT 1 FROM custom_field_value cv JOIN custom_field_definition d ON d.id=cv.field_id WHERE cv.scene_id=s.id AND cv.field_id=? AND d.field_type='decimal' AND CAST(cv.decimal_value AS REAL) {operator} CAST(? AS REAL))")
                    }
                    _ => return Err(StorageError::Invalid("numeric metadata comparisons require a finite decimal or integer operand".into())),
                }
            }
            SceneQueryFilter::TimeBefore(tick) => {
                values.push(tick.to_string());
                "s.start_nominal IS NOT NULL AND s.start_nominal<?".into()
            }
            SceneQueryFilter::TimeAfter(tick) => {
                values.push(tick.to_string());
                "s.start_nominal IS NOT NULL AND s.start_nominal>?".into()
            }
            SceneQueryFilter::TimeBeforeDefinitely(tick) => {
                values.push(tick.to_string());
                "s.start_upper IS NOT NULL AND s.start_upper<?".into()
            }
            SceneQueryFilter::TimeBeforePossibly(tick) => {
                values.push(tick.to_string());
                "s.start_lower IS NOT NULL AND s.start_lower<?".into()
            }
            SceneQueryFilter::TimeAfterDefinitely(tick) => {
                values.push(tick.to_string());
                "s.start_lower IS NOT NULL AND s.start_lower>?".into()
            }
            SceneQueryFilter::TimeAfterPossibly(tick) => {
                values.push(tick.to_string());
                "s.start_upper IS NOT NULL AND s.start_upper>?".into()
            }
            SceneQueryFilter::UnresolvedIssue => "EXISTS(SELECT 1 FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=s.id AND i.status IN('open','in_progress'))".into(),
        },
    })
}

#[derive(Serialize, Deserialize)]
struct ProjectConfig {
    id: Uuid,
    title: String,
    author: String,
    genre: String,
    synopsis: String,
    #[serde(default)]
    book_type: BookType,
    #[serde(default)]
    template_id: Option<String>,
    target_words: Option<u64>,
    story_epoch_label: String,
    story_calendar_id: Option<String>,
}
pub struct ProjectStore {
    root: PathBuf,
    conn: Connection,
}

#[derive(Clone, Debug)]
pub struct GitRestoreFile {
    pub relative_path: String,
    pub contents: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GitRestoreJournal {
    format_version: u8,
    committed: bool,
    entries: Vec<GitRestoreJournalEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GitRestoreJournalEntry {
    relative_path: String,
    old_exists: bool,
    old_sha256: Option<String>,
    new_sha256: Option<String>,
    backup_name: String,
    prepared_name: String,
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
pub struct ContentUnitRecord {
    pub id: ContentUnitId,
    pub kind: ContentUnitKind,
    pub title: String,
    pub parent_id: Option<NodeId>,
    pub position: u32,
    pub target_words: Option<u64>,
    pub actual_words: u32,
    pub content_hash: String,
    pub archived: bool,
    pub language_code: Option<String>,
}
impl ContentUnitRecord {
    pub fn effective_language_code<'a>(&'a self, project: &'a Project) -> &'a str {
        self.language_code
            .as_deref()
            .unwrap_or(&project.language_code)
    }
}
#[derive(Clone, Debug)]
pub struct ContentUnitRecoveryConflict {
    pub transaction_id: String,
    pub content_unit_id: ContentUnitId,
    pub recovery_path: PathBuf,
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
pub struct PassageLanguageRecord {
    pub anchor: PassageAnchorRecord,
    pub language_code: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSearchHit {
    pub entity_kind: String,
    pub entity_id: String,
    pub title: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MentionCandidate {
    pub entity_kind: String,
    pub entity_id: String,
    pub matched_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedMentionRecord {
    pub id: MentionId,
    pub scene_id: SceneId,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub state: String,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub candidates: Vec<MentionCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoryObjectRecord {
    pub id: StoryObjectId,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersonalCommentRecord {
    pub id: PersonalCommentId,
    pub scene_id: SceneId,
    pub anchor_id: Option<AnchorId>,
    pub quote: Option<String>,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
    pub anchor_state: Option<String>,
    pub body: String,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblySummary {
    pub id: AssemblyId,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyItemRecord {
    pub id: AssemblyItemId,
    pub assembly_id: AssemblyId,
    pub position: u32,
    pub scene_id: Option<SceneId>,
    pub content_unit_id: Option<ContentUnitId>,
    pub included: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanningBoardItemRecord {
    pub id: PlanningBoardItemId,
    pub board_id: PlanningBoardId,
    pub entity_kind: String,
    pub entity_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningBoardConnectorRecord {
    pub id: PlanningBoardConnectorId,
    pub board_id: PlanningBoardId,
    pub source_item_id: PlanningBoardItemId,
    pub target_item_id: PlanningBoardItemId,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningBoardSummary {
    pub id: PlanningBoardId,
    pub title: String,
}

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    format_version: u32,
    project_id: String,
    files: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovedKnowledgeClaim {
    pub id: story_core::KnowledgeClaimId,
    pub kind: String,
    pub name: String,
    pub claim: String,
    pub approximate_story_time: Option<String>,
    pub source_kind: String,
    pub source_id: String,
    pub source_fingerprint: String,
    pub source_length_bytes: usize,
    pub evidence_quote: String,
    pub evidence_start_byte: usize,
    pub evidence_end_byte: usize,
    pub model: String,
    pub approved_at: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscludedPassage {
    pub source: EntityRef,
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
pub struct LocationSummary {
    pub id: LocationId,
    pub name: String,
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
    pub start: StoryTime,
    pub end: StoryTime,
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
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WritingChallengeRecord {
    pub id: String,
    pub started_at: i64,
    pub deadline_at: i64,
    pub target_words: u32,
    pub mode: String,
    pub ended_at: Option<i64>,
    pub words_written: Option<u32>,
    pub status: String,
    pub words_deleted: u32,
    pub pauses_consumed: u32,
    pub outcome: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WritingSessionRecord {
    pub id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub scene_id: Option<SceneId>,
    pub start_words: u32,
    pub end_words: Option<u32>,
    pub words_delta: Option<i64>,
    pub status: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WritingGoalRecord {
    pub id: String,
    pub title: String,
    pub scope_type: String,
    pub scope_scene_id: Option<SceneId>,
    pub start_date: String,
    pub end_date: String,
    pub target_words: i64,
    pub baseline_words: i64,
    pub time_zone: String,
    pub word_count_policy: String,
    pub success_metric: String,
    pub archived: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewWritingGoal {
    pub title: String,
    pub scope_scene_id: Option<SceneId>,
    pub start_date: String,
    pub end_date: String,
    pub target_words: i64,
    pub baseline_words: i64,
    pub time_zone: String,
    pub word_count_policy: String,
    pub success_metric: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewWritingGoalEvent {
    pub goal_id: String,
    pub occurred_at: i64,
    pub origin: String,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub scene_id: Option<SceneId>,
    pub raw_delta: i64,
    pub real_delta: i64,
    pub detail: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewWritingActivity {
    pub occurred_at: i64,
    pub origin: String,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub scene_id: Option<SceneId>,
    pub raw_delta: i64,
    pub real_delta: i64,
    pub detail: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WritingGoalEventRecord {
    pub id: String,
    pub goal_id: String,
    pub occurred_at: i64,
    pub origin: String,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub scene_id: Option<SceneId>,
    pub raw_delta: i64,
    pub real_delta: i64,
    pub reconciliation_state: String,
    pub detail: String,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WritingGoalProgressPoint {
    pub date: String,
    pub target_words: i64,
    pub raw_words: Option<i64>,
    pub real_words: Option<i64>,
    pub projected: bool,
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
    pub language_code: Option<String>,
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportRunSummary {
    pub id: ImportRunId,
    pub source_type: String,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub status: String,
    pub source_items: u64,
    pub provenance_records: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticSuppressionRecord {
    pub id: String,
    pub rule_id: String,
    pub scope: String,
    pub scene_id: Option<String>,
    pub finding_fingerprint: Option<String>,
    pub reason: String,
    pub created_unix_ms: i64,
    pub expires_unix_ms: Option<i64>,
    pub triggering_fingerprint: Option<String>,
    pub status: String,
    pub last_checked_unix_ms: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictResolution {
    KeepExternal,
    UsePrepared,
}

impl ProjectStore {
    pub fn create_diagnostic_suppression(
        &self,
        suppression: &DiagnosticSuppressionRecord,
    ) -> Result<()> {
        let valid_scope = match suppression.scope.as_str() {
            "rule" => suppression.scene_id.is_none() && suppression.finding_fingerprint.is_none(),
            "scene" => suppression.scene_id.is_some() && suppression.finding_fingerprint.is_none(),
            "finding" => {
                suppression.scene_id.is_some() && suppression.finding_fingerprint.is_some()
            }
            _ => false,
        };
        if !valid_scope
            || suppression.rule_id.trim().is_empty()
            || suppression.reason.trim().is_empty()
            || suppression.created_unix_ms < 0
            || suppression.last_checked_unix_ms < 0
            || suppression
                .expires_unix_ms
                .is_some_and(|expires| expires <= suppression.created_unix_ms)
            || suppression.status != "active"
        {
            return Err(StorageError::Invalid(
                "diagnostic suppression fields are invalid".into(),
            ));
        }
        self.conn.execute(
            "INSERT INTO diagnostic_suppression(id,rule_id,scope,scene_id,finding_fingerprint,reason,created_unix_ms,expires_unix_ms,triggering_fingerprint,status,last_checked_unix_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![suppression.id, suppression.rule_id, suppression.scope,
                suppression.scene_id, suppression.finding_fingerprint, suppression.reason,
                suppression.created_unix_ms, suppression.expires_unix_ms,
                suppression.triggering_fingerprint, suppression.status,
                suppression.last_checked_unix_ms],
        )?;
        Ok(())
    }

    pub fn list_diagnostic_suppressions(&self) -> Result<Vec<DiagnosticSuppressionRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT id,rule_id,scope,scene_id,finding_fingerprint,reason,created_unix_ms,expires_unix_ms,triggering_fingerprint,status,last_checked_unix_ms FROM diagnostic_suppression ORDER BY created_unix_ms,id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DiagnosticSuppressionRecord {
                id: row.get(0)?,
                rule_id: row.get(1)?,
                scope: row.get(2)?,
                scene_id: row.get(3)?,
                finding_fingerprint: row.get(4)?,
                reason: row.get(5)?,
                created_unix_ms: row.get(6)?,
                expires_unix_ms: row.get(7)?,
                triggering_fingerprint: row.get(8)?,
                status: row.get(9)?,
                last_checked_unix_ms: row.get(10)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn update_diagnostic_suppression_state(
        &self,
        id: &str,
        status: &str,
        checked_unix_ms: i64,
    ) -> Result<()> {
        if !matches!(status, "active" | "stale" | "expired" | "revoked") || checked_unix_ms < 0 {
            return Err(StorageError::Invalid(
                "diagnostic suppression state is invalid".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE diagnostic_suppression SET status=?2,last_checked_unix_ms=?3 WHERE id=?1",
            params![id, status, checked_unix_ms],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!(
                "diagnostic suppression {id}"
            )));
        }
        Ok(())
    }

    /// Applies a historical set of Git-owned files as one recoverable operation.
    /// Prepared and original bytes are fsynced before the first canonical file changes.
    pub fn restore_git_files(&mut self, changes: &[GitRestoreFile]) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        let checkpoint: (i64, i64, i64) =
            self.conn
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
        if checkpoint.0 != 0 {
            return Err(StorageError::RecoveryConflict(
                "SQLite is busy; historical project restore was not started".into(),
            ));
        }
        let recovery_root = self.safe_path(Path::new(".writers-ide/recovery"))?;
        fs::create_dir_all(&recovery_root)?;
        let mut seen = std::collections::BTreeSet::new();
        let transaction_id = Uuid::new_v4().to_string();
        let transaction_dir = recovery_root.join(format!("git-restore-{transaction_id}"));
        let original_dir = transaction_dir.join("original");
        let prepared_dir = transaction_dir.join("prepared");
        fs::create_dir_all(&original_dir)?;
        fs::create_dir_all(&prepared_dir)?;
        let mut journal = GitRestoreJournal {
            format_version: 1,
            committed: false,
            entries: Vec::new(),
        };
        let preparation = (|| {
            for (index, change) in changes.iter().enumerate() {
                if !is_git_restore_path_allowed(&change.relative_path)
                    || !seen.insert(change.relative_path.as_str())
                {
                    return Err(StorageError::Invalid(format!(
                        "invalid or duplicate Git restore path: {:?}",
                        change.relative_path
                    )));
                }
                let relative = Path::new(&change.relative_path);
                let target = self.safe_path(relative)?;
                let old = match fs::symlink_metadata(&target) {
                    Ok(metadata) if metadata.file_type().is_file() => Some(fs::read(&target)?),
                    Ok(_) => {
                        return Err(StorageError::Invalid(format!(
                            "Git restore target is not a regular file: {:?}",
                            change.relative_path
                        )));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                if old.as_deref() == change.contents.as_deref() {
                    continue;
                }
                let suffix = format!("{index:08}.bin");
                let backup_name = format!("original/{suffix}");
                let prepared_name = format!("prepared/{suffix}");
                if let Some(bytes) = old.as_deref() {
                    write_synced(&transaction_dir.join(&backup_name), bytes)?;
                }
                if let Some(bytes) = change.contents.as_deref() {
                    write_synced(&transaction_dir.join(&prepared_name), bytes)?;
                }
                journal.entries.push(GitRestoreJournalEntry {
                    relative_path: change.relative_path.clone(),
                    old_exists: old.is_some(),
                    old_sha256: old.as_deref().map(sha256_bytes),
                    new_sha256: change.contents.as_deref().map(sha256_bytes),
                    backup_name,
                    prepared_name,
                });
            }
            if journal.entries.is_empty() {
                fs::remove_dir_all(&transaction_dir)?;
                return Ok(false);
            }
            sync_dir(&original_dir)?;
            sync_dir(&prepared_dir)?;
            sync_dir(&transaction_dir)?;
            write_git_restore_journal(&transaction_dir, &journal)?;
            sync_dir(transaction_dir.parent().unwrap())?;
            Ok(true)
        })();
        let has_changes = match preparation {
            Ok(has_changes) => has_changes,
            Err(error) => {
                let _ = fs::remove_dir_all(&transaction_dir);
                return Err(error);
            }
        };
        if !has_changes {
            return Ok(());
        }

        let placeholder = Connection::open_in_memory()?;
        let current_connection = std::mem::replace(&mut self.conn, placeholder);
        match current_connection.close() {
            Ok(()) => {}
            Err((connection, error)) => {
                self.conn = connection;
                let _ = fs::remove_dir_all(&transaction_dir);
                return Err(error.into());
            }
        }

        let apply_result = (|| {
            for entry in &journal.entries {
                let target = safe_git_restore_target(&self.root, &entry.relative_path, true)?;
                let observed_hash = match fs::symlink_metadata(&target) {
                    Ok(metadata) if metadata.file_type().is_file() => {
                        Some(sha256_bytes(&fs::read(&target)?))
                    }
                    Ok(_) => {
                        return Err(StorageError::RecoveryConflict(format!(
                            "restore target changed type during operation: {}",
                            entry.relative_path
                        )));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                if observed_hash != entry.old_sha256 {
                    return Err(StorageError::RecoveryConflict(format!(
                        "restore target changed externally during operation: {}; original and proposed versions are preserved",
                        entry.relative_path
                    )));
                }
                match &entry.new_sha256 {
                    Some(expected_hash) => {
                        let prepared = fs::read(transaction_dir.join(&entry.prepared_name))?;
                        if sha256_bytes(&prepared) != *expected_hash {
                            return Err(StorageError::Invalid(
                                "prepared Git restore file failed its SHA-256 check".into(),
                            ));
                        }
                        atomic_write(&target, &prepared)?;
                    }
                    None => match fs::remove_file(&target) {
                        Ok(()) => sync_dir(target.parent().unwrap())?,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    },
                }
            }
            let mut restored = Self::open_without_git_restore_recovery(&self.root)?;
            restored.validate()?;
            journal.committed = true;
            write_git_restore_journal(&transaction_dir, &journal)?;
            self.conn = std::mem::replace(&mut restored.conn, Connection::open_in_memory()?);
            if fs::remove_dir_all(&transaction_dir).is_ok() {
                sync_dir(transaction_dir.parent().unwrap())?;
            }
            Ok(())
        })();
        if let Err(error) = apply_result {
            let rollback = Self::recover_git_restore_transactions(&self.root);
            let reopened = Self::open_without_git_restore_recovery(&self.root);
            let rollback_error = rollback.err();
            match reopened {
                Ok(reopened) => self.conn = reopened.conn,
                Err(reopen_error) => {
                    return Err(StorageError::RecoveryConflict(format!(
                        "Git restore failed ({error}); rollback result: {}; reopening also failed: {reopen_error}",
                        rollback_error
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "ok".into())
                    )));
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(StorageError::RecoveryConflict(format!(
                    "Git restore failed ({error}); rollback needs attention: {rollback_error}"
                )));
            }
            return Err(error);
        }
        Ok(())
    }

    fn recover_git_restore_transactions(root: &Path) -> Result<()> {
        let recovery = root.join(".writers-ide/recovery");
        for directory in [root.join(".writers-ide"), recovery.clone()] {
            match fs::symlink_metadata(&directory) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(StorageError::Invalid(
                        "Git restore recovery directory must be a real directory".into(),
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let entries = match fs::read_dir(&recovery) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("git-restore-")
            {
                continue;
            }
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
                return Err(StorageError::Invalid(
                    "Git restore recovery entry must be a real directory".into(),
                ));
            }
            let journal_path = path.join("journal.json");
            match fs::symlink_metadata(&journal_path) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(StorageError::Invalid(
                        "Git restore journal must be a regular file".into(),
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            }
            let bytes = fs::read(&journal_path)?;
            let journal: GitRestoreJournal = serde_json::from_slice(&bytes).map_err(|error| {
                StorageError::Invalid(format!("invalid Git restore recovery journal: {error}"))
            })?;
            if journal.format_version != 1 {
                return Err(StorageError::UnsupportedSchema(i64::from(
                    journal.format_version,
                )));
            }
            if journal.committed {
                fs::remove_dir_all(&path)?;
                sync_dir(&recovery)?;
                continue;
            }
            for item in journal.entries.iter().rev() {
                if !is_git_restore_path_allowed(&item.relative_path) {
                    return Err(StorageError::Invalid(
                        "recovery journal contains an unsafe project path".into(),
                    ));
                }
                let target = safe_git_restore_target(root, &item.relative_path, false)?;
                let current = match fs::symlink_metadata(&target) {
                    Ok(metadata) if metadata.file_type().is_file() => Some(fs::read(&target)?),
                    Ok(_) => {
                        return Err(StorageError::RecoveryConflict(format!(
                            "restore target became a non-file: {}",
                            item.relative_path
                        )));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                let current_hash = current.as_deref().map(sha256_bytes);
                if current_hash == item.old_sha256 {
                    continue;
                }
                if current_hash != item.new_sha256 {
                    return Err(StorageError::RecoveryConflict(format!(
                        "Git restore recovery found external divergence at {}; both versions are preserved",
                        item.relative_path
                    )));
                }
                if item.old_exists {
                    let original = fs::read(path.join(&item.backup_name))?;
                    if item.old_sha256.as_deref() != Some(sha256_bytes(&original).as_str()) {
                        return Err(StorageError::Invalid(
                            "original Git restore backup failed its SHA-256 check".into(),
                        ));
                    }
                    let destination = safe_git_restore_target(root, &item.relative_path, true)?;
                    atomic_write(&destination, &original)?;
                } else if target.exists() {
                    fs::remove_file(&target)?;
                    sync_dir(target.parent().unwrap())?;
                }
            }
            fs::remove_dir_all(&path)?;
            sync_dir(&recovery)?;
        }
        Ok(())
    }

    /// Recreates a project database from a validated deterministic metadata snapshot.
    /// The caller supplies an already-staged project directory and removes it on failure.
    pub fn restore_metadata_snapshot(root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        for directory in [
            "manuscript",
            "research",
            "research/readings",
            "attachments",
            "templates",
            "exports",
            ".writers-ide/recovery",
        ] {
            fs::create_dir_all(root.join(directory))?;
        }
        let database = root.join("story.db");
        if database.exists() {
            return Err(StorageError::Invalid(
                "restore destination already contains story.db".into(),
            ));
        }
        let sql = fs::read_to_string(root.join("snapshots/database.sql"))?;
        validate_sql_dump(&sql)?;
        let conn = Connection::open(&database)?;
        conn.execute_batch(&sql)?;
        drop(conn);
        fs::File::open(&database)?.sync_all()?;
        fs::File::open(root)?.sync_all()?;
        let store = Self::open(root)?;
        store.validate()?;
        drop(store);
        Ok(())
    }

    pub fn get_import_run_summary(&self, id: ImportRunId) -> Result<ImportRunSummary> {
        self.conn.query_row(
            "SELECT r.source_type,r.source_identifier,r.source_fingerprint,r.status,(SELECT count(*) FROM import_source_item i WHERE i.run_id=r.id),(SELECT count(*) FROM entity_provenance p JOIN import_source_item i ON i.id=p.source_item_id WHERE i.run_id=r.id) FROM import_run r WHERE r.id=?1",
            [id.to_string()],
            |row| Ok(ImportRunSummary {
                id,
                source_type: row.get(0)?,
                source_identifier: row.get(1)?,
                source_fingerprint: row.get(2)?,
                status: row.get(3)?,
                source_items: row.get::<_, i64>(4)? as u64,
                provenance_records: row.get::<_, i64>(5)? as u64,
            }),
        ).map_err(Into::into)
    }
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
            book_type: BookType::Fiction,
            template_id: None,
            target_words: None,
            story_epoch_label: "Day 0".into(),
            story_calendar_id: None,
            language_code: default_language_code(),
            word_count_policy: WordCountPolicy::default(),
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
            book_type: project.book_type,
            template_id: project.template_id.clone(),
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
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_1}\n{MIGRATION_2}\nINSERT INTO schema_migrations(version,applied_at) VALUES(2,'{}');\n{MIGRATION_3}\nINSERT INTO schema_migrations(version,applied_at) VALUES(3,'{}');\n{MIGRATION_4}\nINSERT INTO schema_migrations(version,applied_at) VALUES(4,'{}');\n{MIGRATION_5}\nINSERT INTO schema_migrations(version,applied_at) VALUES(5,'{}');\n{MIGRATION_6}\nINSERT INTO schema_migrations(version,applied_at) VALUES(6,'{}');\n{MIGRATION_7}\nINSERT INTO schema_migrations(version,applied_at) VALUES(7,'{}');\n{MIGRATION_8}\nINSERT INTO schema_migrations(version,applied_at) VALUES(8,'{}');\n{MIGRATION_9}\nINSERT INTO schema_migrations(version,applied_at) VALUES(9,'{}');\nCOMMIT;", now(),now(),now(),now(),now(),now(),now(),now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_10}\nINSERT INTO schema_migrations(version,applied_at) VALUES(10,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_11}\nINSERT INTO schema_migrations(version,applied_at) VALUES(11,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_12}\nINSERT INTO schema_migrations(version,applied_at) VALUES(12,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_13}\nINSERT INTO schema_migrations(version,applied_at) VALUES(13,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_14}\nINSERT INTO schema_migrations(version,applied_at) VALUES(14,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_15}\nINSERT INTO schema_migrations(version,applied_at) VALUES(15,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_16}\nINSERT INTO schema_migrations(version,applied_at) VALUES(16,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_17}\nINSERT INTO schema_migrations(version,applied_at) VALUES(17,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_18}\nINSERT INTO schema_migrations(version,applied_at) VALUES(18,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_19}\nINSERT INTO schema_migrations(version,applied_at) VALUES(19,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_20}\nINSERT INTO schema_migrations(version,applied_at) VALUES(20,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_21}\nINSERT INTO schema_migrations(version,applied_at) VALUES(21,'{}');\n{MIGRATION_22}\nINSERT INTO schema_migrations(version,applied_at) VALUES(22,'{}');\n{MIGRATION_23}\nINSERT INTO schema_migrations(version,applied_at) VALUES(23,'{}');\n{MIGRATION_24}\nINSERT INTO schema_migrations(version,applied_at) VALUES(24,'{}');\n{MIGRATION_25}\nINSERT INTO schema_migrations(version,applied_at) VALUES(25,'{}');\n{MIGRATION_26}\nINSERT INTO schema_migrations(version,applied_at) VALUES(26,'{}');\n{MIGRATION_27}\nINSERT INTO schema_migrations(version,applied_at) VALUES(27,'{}');\n{MIGRATION_28}\nINSERT INTO schema_migrations(version,applied_at) VALUES(28,'{}');\n{MIGRATION_29}\nINSERT INTO schema_migrations(version,applied_at) VALUES(29,'{}');\n{MIGRATION_30}\nINSERT INTO schema_migrations(version,applied_at) VALUES(30,'{}');\n{MIGRATION_31}\nINSERT INTO schema_migrations(version,applied_at) VALUES(31,'{}');\n{MIGRATION_32}\nINSERT INTO schema_migrations(version,applied_at) VALUES(32,'{}');\nCOMMIT;", now(), now(), now(), now(), now(), now(), now(), now(), now(), now(), now(), now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_33}\nINSERT INTO schema_migrations(version,applied_at) VALUES(33,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_34}\nINSERT INTO schema_migrations(version,applied_at) VALUES(34,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_35}\nINSERT INTO schema_migrations(version,applied_at) VALUES(35,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_36}\nINSERT INTO schema_migrations(version,applied_at) VALUES(36,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_37}\nINSERT INTO schema_migrations(version,applied_at) VALUES(37,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_38}\nINSERT INTO schema_migrations(version,applied_at) VALUES(38,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_39}\nINSERT INTO schema_migrations(version,applied_at) VALUES(39,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_40}\nINSERT INTO schema_migrations(version,applied_at) VALUES(40,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_41}\nINSERT INTO schema_migrations(version,applied_at) VALUES(41,'{}');\nCOMMIT;", now()))?;
        conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_42}\nINSERT INTO schema_migrations(version,applied_at) VALUES(42,'{}');\nCOMMIT;", now()))?;
        let now = now();
        conn.execute("INSERT INTO project(singleton,id,title,author,genre,synopsis,target_words,story_epoch_label,story_calendar_id,created_at,updated_at,book_type,template_id,language_code,word_count_policy) VALUES(1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10,?11,?12,?13)",params![cfg.id.to_string(),cfg.title,cfg.author,cfg.genre,cfg.synopsis,cfg.target_words,cfg.story_epoch_label,cfg.story_calendar_id,now,cfg.book_type.as_str(),cfg.template_id,project.language_code,project.word_count_policy.as_str()])?;
        Ok(Self {
            root: root.to_path_buf(),
            conn,
        })
    }
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        Self::recover_git_restore_transactions(root)?;
        Self::open_without_git_restore_recovery(root)
    }

    fn open_without_git_restore_recovery(root: impl AsRef<Path>) -> Result<Self> {
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
        let initial_id: String = conn
            .query_row("SELECT id FROM project WHERE singleton=1", [], |row| {
                row.get(0)
            })
            .map_err(|_| StorageError::Config("database has no project identity".into()))?;
        if cfg.id.to_string() != initial_id {
            return Err(StorageError::IdentityMismatch {
                config: cfg.id.to_string(),
                database: initial_id,
            });
        }
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
        if version > 42 {
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
            current_version = 6;
        }
        if current_version == 6 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_7}\nINSERT INTO schema_migrations(version,applied_at) VALUES(7,'{}');\nCOMMIT;", now()))?;
            current_version = 7;
        }
        if current_version == 7 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_8}\nINSERT INTO schema_migrations(version,applied_at) VALUES(8,'{}');\nCOMMIT;", now()))?;
            current_version = 8;
        }
        if current_version == 8 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_9}\nINSERT INTO schema_migrations(version,applied_at) VALUES(9,'{}');\nCOMMIT;", now()))?;
            current_version = 9;
        }
        if current_version == 9 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_10}\nINSERT INTO schema_migrations(version,applied_at) VALUES(10,'{}');\nCOMMIT;", now()))?;
            current_version = 10;
        }
        if current_version == 10 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_11}\nINSERT INTO schema_migrations(version,applied_at) VALUES(11,'{}');\nCOMMIT;", now()))?;
            current_version = 11;
        }
        if current_version == 11 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_12}\nINSERT INTO schema_migrations(version,applied_at) VALUES(12,'{}');\nCOMMIT;", now()))?;
            current_version = 12;
        }
        if current_version == 12 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_13}\nINSERT INTO schema_migrations(version,applied_at) VALUES(13,'{}');\nCOMMIT;", now()))?;
            current_version = 13;
        }
        if current_version == 13 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_14}\nINSERT INTO schema_migrations(version,applied_at) VALUES(14,'{}');\nCOMMIT;", now()))?;
            current_version = 14;
        }
        if current_version == 14 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_15}\nINSERT INTO schema_migrations(version,applied_at) VALUES(15,'{}');\nCOMMIT;", now()))?;
            current_version = 15;
        }
        if current_version == 15 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_16}\nINSERT INTO schema_migrations(version,applied_at) VALUES(16,'{}');\nCOMMIT;", now()))?;
            current_version = 16;
        }
        if current_version == 16 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_17}\nINSERT INTO schema_migrations(version,applied_at) VALUES(17,'{}');\nCOMMIT;", now()))?;
            current_version = 17;
        }
        if current_version == 17 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_18}\nINSERT INTO schema_migrations(version,applied_at) VALUES(18,'{}');\nCOMMIT;", now()))?;
            current_version = 18;
        }
        if current_version == 18 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_19}\nINSERT INTO schema_migrations(version,applied_at) VALUES(19,'{}');\nCOMMIT;", now()))?;
            current_version = 19;
        }
        if current_version == 19 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_20}\nINSERT INTO schema_migrations(version,applied_at) VALUES(20,'{}');\nCOMMIT;", now()))?;
            current_version = 20;
        }
        if current_version == 20 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_21}\nINSERT INTO schema_migrations(version,applied_at) VALUES(21,'{}');\nCOMMIT;", now()))?;
            current_version = 21;
        }
        if current_version == 21 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_22}\nINSERT INTO schema_migrations(version,applied_at) VALUES(22,'{}');\nCOMMIT;", now()))?;
            current_version = 22;
        }
        if current_version == 22 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_23}\nINSERT INTO schema_migrations(version,applied_at) VALUES(23,'{}');\nCOMMIT;", now()))?;
            current_version = 23;
        }
        if current_version == 23 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_24}\nINSERT INTO schema_migrations(version,applied_at) VALUES(24,'{}');\nCOMMIT;", now()))?;
            current_version = 24;
        }
        if current_version == 24 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_25}\nINSERT INTO schema_migrations(version,applied_at) VALUES(25,'{}');\nCOMMIT;", now()))?;
            current_version = 25;
        }
        if current_version == 25 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_26}\nINSERT INTO schema_migrations(version,applied_at) VALUES(26,'{}');\nCOMMIT;", now()))?;
            current_version = 26;
        }
        if current_version == 26 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_27}\nINSERT INTO schema_migrations(version,applied_at) VALUES(27,'{}');\nCOMMIT;", now()))?;
            current_version = 27;
        }
        if current_version == 27 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_28}\nINSERT INTO schema_migrations(version,applied_at) VALUES(28,'{}');\nCOMMIT;", now()))?;
            current_version = 28;
        }
        if current_version == 28 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_29}\nINSERT INTO schema_migrations(version,applied_at) VALUES(29,'{}');\nCOMMIT;", now()))?;
            current_version = 29;
        }
        if current_version == 29 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_30}\nINSERT INTO schema_migrations(version,applied_at) VALUES(30,'{}');\nCOMMIT;", now()))?;
            current_version = 30;
        }
        if current_version == 30 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_31}\nINSERT INTO schema_migrations(version,applied_at) VALUES(31,'{}');\nCOMMIT;", now()))?;
            current_version = 31;
        }
        if current_version == 31 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_32}\nINSERT INTO schema_migrations(version,applied_at) VALUES(32,'{}');\nCOMMIT;", now()))?;
            current_version = 32;
        }
        if current_version == 32 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_33}\nINSERT INTO schema_migrations(version,applied_at) VALUES(33,'{}');\nCOMMIT;", now()))?;
            current_version = 33;
        }
        if current_version == 33 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_34}\nINSERT INTO schema_migrations(version,applied_at) VALUES(34,'{}');\nCOMMIT;", now()))?;
            current_version = 34;
        }
        if current_version == 34 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_35}\nINSERT INTO schema_migrations(version,applied_at) VALUES(35,'{}');\nCOMMIT;", now()))?;
            current_version = 35;
        }
        if current_version == 35 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_36}\nINSERT INTO schema_migrations(version,applied_at) VALUES(36,'{}');\nCOMMIT;", now()))?;
            current_version = 36;
        }
        if current_version == 36 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_37}\nINSERT INTO schema_migrations(version,applied_at) VALUES(37,'{}');\nCOMMIT;", now()))?;
            current_version = 37;
        }
        if current_version == 37 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_38}\nINSERT INTO schema_migrations(version,applied_at) VALUES(38,'{}');\nCOMMIT;", now()))?;
            current_version = 38;
        }
        if current_version == 38 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_39}\nINSERT INTO schema_migrations(version,applied_at) VALUES(39,'{}');\nCOMMIT;", now()))?;
            current_version = 39;
        }
        if current_version == 39 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_40}\nINSERT INTO schema_migrations(version,applied_at) VALUES(40,'{}');\nCOMMIT;", now()))?;
            current_version = 40;
        }
        if current_version == 40 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_41}\nINSERT INTO schema_migrations(version,applied_at) VALUES(41,'{}');\nCOMMIT;", now()))?;
            current_version = 41;
        }
        if current_version == 41 {
            conn.execute_batch(&format!("BEGIN IMMEDIATE;\n{MIGRATION_42}\nINSERT INTO schema_migrations(version,applied_at) VALUES(42,'{}');\nCOMMIT;", now()))?;
        }
        let dbid: String = conn
            .query_row("SELECT id FROM project LIMIT 1", [], |r| r.get(0))
            .map_err(|_| StorageError::Config("database has no project identity".into()))?;
        let db_book_type: String = conn.query_row(
            "SELECT book_type FROM project WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if BookType::parse(&db_book_type) != Some(cfg.book_type) {
            return Err(StorageError::Config(format!(
                "project book type mismatch: config {}, database {}",
                cfg.book_type.as_str(),
                db_book_type
            )));
        }
        let db_template_id: Option<String> = conn.query_row(
            "SELECT template_id FROM project WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        if cfg.template_id != db_template_id {
            return Err(StorageError::Config(format!(
                "project template mismatch: config {:?}, database {:?}",
                cfg.template_id, db_template_id
            )));
        }
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
        let _ = this
            .recover_pending_writes()
            .map_err(|e| StorageError::Config(format!("scene write recovery failed: {e}")))?;
        let _ = this.recover_pending_content_unit_writes().map_err(|e| {
            StorageError::Config(format!("content-unit write recovery failed: {e}"))
        })?;
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
    pub fn list_proposal_files(&self) -> Result<Vec<PathBuf>> {
        let directory = self.safe_path(Path::new(".writers-ide/proposals"))?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                return Err(StorageError::Invalid(
                    "proposal directory cannot contain symlinks".into(),
                ));
            }
            if metadata.is_file()
                && entry.path().extension().and_then(|x| x.to_str()) == Some("json")
            {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }
    pub fn load_proposal_file(&self, path: &Path) -> Result<Vec<u8>> {
        let relative = self.proposal_relative_path(path)?;
        let target = self.safe_path(&relative)?;
        let metadata = fs::symlink_metadata(&target)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Invalid(
                "proposal must be a regular project file".into(),
            ));
        }
        Ok(fs::read(target)?)
    }
    pub fn update_proposal_file(
        &self,
        path: &Path,
        expected_fingerprint: &str,
        content: &[u8],
    ) -> Result<()> {
        let relative = self.proposal_relative_path(path)?;
        let target = self.safe_path(&relative)?;
        let metadata = fs::symlink_metadata(&target)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::Invalid(
                "proposal must be a regular project file".into(),
            ));
        }
        let current = fs::read(&target)?;
        if hash_bytes(&current) != expected_fingerprint {
            return Err(StorageError::Conflict(
                "proposal changed since it was opened; reload before updating".into(),
            ));
        }
        let temp = target.with_extension(format!("{}.tmp", Uuid::new_v4()));
        write_synced(&temp, content)?;
        fs::rename(&temp, &target)?;
        sync_dir(
            target
                .parent()
                .ok_or_else(|| StorageError::Invalid("proposal has no parent directory".into()))?,
        )?;
        Ok(())
    }
    fn proposal_relative_path(&self, path: &Path) -> Result<PathBuf> {
        let relative = if path.is_absolute() {
            let root = fs::canonicalize(&self.root)?;
            let canonical_path = fs::canonicalize(path)?;
            canonical_path
                .strip_prefix(&root)
                .map_err(|_| StorageError::Invalid("proposal is outside the project".into()))?
                .to_path_buf()
        } else {
            if path.starts_with(Path::new(".writers-ide/proposals")) {
                path.to_path_buf()
            } else if self.root.is_relative() && path.starts_with(&self.root) {
                path.strip_prefix(&self.root)
                    .map_err(|_| {
                        StorageError::Invalid("proposal path is outside the project".into())
                    })?
                    .to_path_buf()
            } else if path.components().count() == 1 {
                Path::new(".writers-ide/proposals").join(path)
            } else {
                path.to_path_buf()
            }
        };
        let components = relative.components().collect::<Vec<_>>();
        if components.len() != 3
            || components[0] != std::path::Component::Normal(".writers-ide".as_ref())
            || components[1] != std::path::Component::Normal("proposals".as_ref())
            || !matches!(components[2], std::path::Component::Normal(_))
            || relative.extension().and_then(|x| x.to_str()) != Some("json")
        {
            return Err(StorageError::Invalid(
                "proposal path must identify a project-local .writers-ide/proposals/*.json file"
                    .into(),
            ));
        }
        Ok(relative)
    }
    pub fn get_project(&self) -> Result<Project> {
        let row: ProjectRow = self.conn.query_row("SELECT id,title,author,genre,synopsis,target_words,story_epoch_label,story_calendar_id,book_type,template_id FROM project WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?)))?;
        let word_count_policy: String = self.conn.query_row(
            "SELECT word_count_policy FROM project WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;
        Ok(Project {
            id: ProjectId(
                Uuid::parse_str(&row.0).map_err(|e| StorageError::Config(e.to_string()))?,
            ),
            title: row.1,
            author: row.2,
            genre: row.3,
            synopsis: row.4,
            book_type: BookType::parse(&row.8).ok_or_else(|| {
                StorageError::Config(format!("invalid project book type {}", row.8))
            })?,
            template_id: row.9,
            target_words: row.5,
            story_epoch_label: row.6,
            story_calendar_id: row.7,
            language_code: self.conn.query_row(
                "SELECT language_code FROM project WHERE singleton=1",
                [],
                |r| r.get(0),
            )?,
            word_count_policy: WordCountPolicy::parse(&word_count_policy).ok_or_else(|| {
                StorageError::Config(format!("invalid word-count policy {word_count_policy}"))
            })?,
        })
    }
    pub fn set_project_word_count_policy(&mut self, policy: WordCountPolicy) -> Result<()> {
        let mut counts = Vec::new();
        for unit in self.list_content_units(true)? {
            let (text, _) = self.load_content_unit_text(unit.id)?;
            counts.push((unit.id, count_words_with_policy(&text, policy)));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE project SET word_count_policy=?1,updated_at=?2 WHERE singleton=1",
            params![policy.as_str(), now()],
        )?;
        for (id, count) in counts {
            tx.execute(
                "UPDATE content_unit SET word_count=?2,updated_at=?3 WHERE id=?1",
                params![id.to_string(), count, now()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn set_project_language(&mut self, language_code: &str) -> Result<()> {
        if !is_valid_language_tag(language_code) {
            return Err(StorageError::Invalid(
                "language must be a valid BCP-47-style language tag (for example en or fr-CA)"
                    .into(),
            ));
        }
        self.conn.execute(
            "UPDATE project SET language_code=?1,updated_at=?2 WHERE singleton=1",
            params![language_code, now()],
        )?;
        Ok(())
    }
    pub fn set_scene_language(
        &mut self,
        scene_id: SceneId,
        language_code: Option<&str>,
    ) -> Result<()> {
        if let Some(language_code) = language_code
            && !is_valid_language_tag(language_code)
        {
            return Err(StorageError::Invalid(
                "scene language must be a valid BCP-47-style tag or inherit the project language"
                    .into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE scene SET language_code=?2,updated_at=?3 WHERE id=?1",
            params![scene_id.to_string(), language_code, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(scene_id.to_string()));
        }
        Ok(())
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

    pub fn backup_archive_to(&mut self, destination: impl AsRef<Path>) -> Result<()> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(StorageError::Invalid(
                "backup archive destination already exists".into(),
            ));
        }
        if destination.starts_with(&self.root) || self.root.starts_with(destination) {
            return Err(StorageError::Invalid(
                "backup archive must be outside the project directory".into(),
            ));
        }
        self.validate()?;
        let parent = destination.parent().ok_or_else(|| {
            StorageError::Invalid("backup archive needs a parent directory".into())
        })?;
        fs::create_dir_all(parent)?;
        let stage = parent.join(format!(".writers-ide-backup-{}", Uuid::new_v4()));
        fs::create_dir(&stage)?;
        let result = (|| -> Result<()> {
            let payload = stage.join("project");
            self.backup_to(&payload)?;
            let mut files = Vec::new();
            collect_regular_files(&payload, &payload, &mut files)?;
            files.sort();
            let mut hashes = std::collections::BTreeMap::new();
            for relative in &files {
                let bytes = fs::read(payload.join(relative))?;
                hashes.insert(
                    relative.to_string_lossy().replace('\\', "/"),
                    hash_bytes(&bytes),
                );
            }
            let project_id = self.get_project()?.id.to_string();
            let manifest = BackupManifest {
                format_version: 1,
                project_id,
                files: hashes,
            };
            let archive_file = File::create(stage.join("archive.zip"))?;
            let mut archive = zip::ZipWriter::new(archive_file);
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for relative in &files {
                let name = relative.to_string_lossy().replace('\\', "/");
                archive.start_file(name, options)?;
                archive.write_all(&fs::read(payload.join(relative))?)?;
            }
            archive.start_file("manifest.json", options)?;
            archive.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|error| {
                StorageError::Invalid(format!("serialize backup manifest: {error}"))
            })?)?;
            let archive_file = archive.finish()?;
            archive_file.sync_all()?;
            fs::rename(stage.join("archive.zip"), destination)?;
            sync_dir(parent)?;
            Ok(())
        })();
        let _ = fs::remove_dir_all(&stage);
        result
    }

    pub fn restore_archive_to(
        archive_path: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let archive_path = archive_path.as_ref();
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(StorageError::Invalid(
                "restore destination already exists".into(),
            ));
        }
        let parent = destination.parent().ok_or_else(|| {
            StorageError::Invalid("restore destination needs a parent directory".into())
        })?;
        fs::create_dir_all(parent)?;
        let stage = parent.join(format!(".writers-ide-restore-{}", Uuid::new_v4()));
        fs::create_dir(&stage)?;
        let result = (|| -> Result<()> {
            let file = File::open(archive_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut observed = std::collections::BTreeMap::new();
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index)?;
                let name = entry.name().to_owned();
                if name == "manifest.json" {
                    continue;
                }
                let relative = safe_archive_path(&name)?;
                if let Some(mode) = entry.unix_mode()
                    && mode & 0o170000 != 0
                    && mode & 0o170000 != 0o100000
                {
                    return Err(StorageError::Invalid(
                        "backup archive contains a non-regular file".into(),
                    ));
                }
                let output = stage.join(&relative);
                let output_parent = output
                    .parent()
                    .ok_or_else(|| StorageError::Invalid("invalid archive file path".into()))?;
                fs::create_dir_all(output_parent)?;
                let mut target = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)?;
                std::io::copy(&mut entry, &mut target)?;
                target.sync_all()?;
                let normalized = relative.to_string_lossy().replace('\\', "/");
                observed.insert(normalized, hash_file(&output)?);
            }
            let manifest: BackupManifest = {
                let mut entry = archive.by_name("manifest.json")?;
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                serde_json::from_slice(&bytes).map_err(|error| {
                    StorageError::Invalid(format!("invalid backup manifest: {error}"))
                })?
            };
            if manifest.format_version != 1 || observed != manifest.files {
                return Err(StorageError::Invalid(
                    "backup manifest contents or hashes do not match the archive".into(),
                ));
            }
            fs::create_dir_all(stage.join(".writers-ide/recovery"))?;
            let project = ProjectStore::open(&stage)?;
            if project.get_project()?.id.to_string() != manifest.project_id {
                return Err(StorageError::IdentityMismatch {
                    config: manifest.project_id,
                    database: project.get_project()?.id.to_string(),
                });
            }
            project.validate()?;
            fs::rename(&stage, destination)?;
            sync_dir(parent)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&stage);
        }
        result
    }
    /// Exports and consumes a single consistent project revision while SQLite's writer barrier
    /// prevents journaled file replacements from crossing the snapshot boundary.
    pub fn with_git_snapshot_barrier<T>(
        &mut self,
        action: impl FnOnce(&Path) -> std::result::Result<T, String>,
    ) -> Result<T> {
        for attempt in 0..3 {
            let conflicts = self.recover_pending_writes()?;
            let content_conflicts = self.recover_pending_content_unit_writes()?;
            if !conflicts.is_empty()
                || !content_conflicts.is_empty()
                || self.pending_conflicts()? > 0
                || self.pending_content_unit_conflicts()? > 0
            {
                return Err(StorageError::RecoveryConflict(
                    "resolve pending manuscript conflicts before creating a Git snapshot".into(),
                ));
            }
            let root = self.root.clone();
            let transaction = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let pending:i64=transaction.query_row("SELECT (SELECT count(*) FROM write_journal)+(SELECT count(*) FROM content_unit_write_journal)",[],|row|row.get(0))?;
            if pending > 0 {
                transaction.rollback()?;
                if attempt == 2 {
                    return Err(StorageError::RecoveryConflict(
                        "manuscript writes remain pending at the snapshot barrier".into(),
                    ));
                }
                continue;
            }
            let dump = deterministic_sql_dump(&transaction)?;
            validate_sql_dump(&dump)?;
            let snapshot = root.join("snapshots/database.sql");
            fs::create_dir_all(snapshot.parent().unwrap())?;
            atomic_write(&snapshot, dump.as_bytes())?;
            let result = action(&root).map_err(StorageError::Invalid)?;
            transaction.commit()?;
            return Ok(result);
        }
        Err(StorageError::RecoveryConflict(
            "could not establish a stable project snapshot".into(),
        ))
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
        let mut expected_paths: std::collections::HashSet<String> =
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
        let content_files: Vec<(String, String, u64, String)> = {
            let mut q = self
                .conn
                .prepare("SELECT id,relative_path,byte_length,sha256 FROM content_unit")?;
            q.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<_, _>>()?
        };
        for (id, relative, length, expected_hash) in content_files {
            expected_paths.insert(relative.clone());
            let path = self.safe_path(Path::new(&relative))?;
            let bytes = fs::read(&path).map_err(|error| {
                StorageError::Conflict(format!("missing/unreadable content unit {id}: {error}"))
            })?;
            if bytes.len() as u64 != length || hash_bytes(&bytes) != expected_hash {
                return Err(StorageError::Conflict(format!(
                    "content unit integrity mismatch for {id}"
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
    pub fn create_content_unit(
        &mut self,
        kind: ContentUnitKind,
        title: &str,
        parent: Option<NodeId>,
        position: u32,
        target_words: Option<u64>,
    ) -> Result<ContentUnitId> {
        let title = title.trim();
        if title.is_empty() {
            return Err(StorageError::Invalid(
                "content unit title cannot be empty".into(),
            ));
        }
        match (kind, parent) {
            (ContentUnitKind::ChapterBody, Some(parent)) => {
                let destination: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT kind FROM structural_node WHERE id=?1",
                        [parent.to_string()],
                        |row| row.get(0),
                    )
                    .optional()?;
                if destination.as_deref() != Some("chapter") {
                    return Err(StorageError::Invalid(
                        "chapter body must belong to an existing chapter".into(),
                    ));
                }
            }
            (ContentUnitKind::SectionBody, Some(parent)) => {
                let destination: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT kind FROM structural_node WHERE id=?1",
                        [parent.to_string()],
                        |row| row.get(0),
                    )
                    .optional()?;
                if destination.as_deref() != Some("section") {
                    return Err(StorageError::Invalid(
                        "section body must belong to an existing section".into(),
                    ));
                }
            }
            (ContentUnitKind::FrontMatter | ContentUnitKind::BackMatter, None) => {}
            _ => {
                return Err(StorageError::Invalid(
                    "content unit kind and parent do not match".into(),
                ));
            }
        }
        let parent_value = parent.map(|id| id.to_string());
        let kind_value = kind.as_str();
        let sibling_count: u32 = if parent.is_some() {
            self.conn.query_row(
                "SELECT count(*) FROM content_unit WHERE parent_node_id=?1",
                [parent_value.as_deref()],
                |row| row.get(0),
            )?
        } else {
            self.conn.query_row(
                "SELECT count(*) FROM content_unit WHERE parent_node_id IS NULL AND kind=?1",
                [kind_value],
                |row| row.get(0),
            )?
        };
        if position > sibling_count {
            return Err(StorageError::Invalid(
                "content unit position beyond sibling count".into(),
            ));
        }
        let id = ContentUnitId::new();
        let relative = PathBuf::from(format!("manuscript/content-{}.md", id));
        let path = self.safe_path(&relative)?;
        let bytes = b"";
        write_synced(&path, bytes)?;
        sync_dir(path.parent().ok_or_else(|| {
            StorageError::Invalid("content unit has no parent directory".into())
        })?)?;
        let now = now();
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let base: i64 = if let Some(parent) = parent_value.as_deref() {
            transaction.query_row(
                "SELECT COALESCE(MAX(position),-1)+1 FROM content_unit WHERE parent_node_id=?1",
                [parent],
                |row| row.get(0),
            )?
        } else {
            transaction.query_row(
                "SELECT COALESCE(MAX(position),-1)+1 FROM content_unit WHERE parent_node_id IS NULL AND kind=?1",
                [kind_value],
                |row| row.get(0),
            )?
        };
        if parent.is_some() {
            transaction.execute(
                "UPDATE content_unit SET position=position+?2 WHERE parent_node_id=?1",
                params![parent_value.as_deref(), base + i64::from(sibling_count) + 2],
            )?;
            let old: Vec<String> = {
                let mut query = transaction.prepare(
                    "SELECT id FROM content_unit WHERE parent_node_id=?1 ORDER BY position,id",
                )?;
                query
                    .query_map([parent_value.as_deref()], |row| row.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for (index, sibling) in old.iter().enumerate() {
                let new_position = index + usize::from(index >= position as usize);
                transaction.execute(
                    "UPDATE content_unit SET position=?2 WHERE id=?1",
                    params![sibling, new_position as i64],
                )?;
            }
        } else {
            transaction.execute(
                "UPDATE content_unit SET position=position+?2 WHERE parent_node_id IS NULL AND kind=?1",
                params![kind_value, base + i64::from(sibling_count) + 2],
            )?;
            let old: Vec<String> = {
                let mut query = transaction.prepare("SELECT id FROM content_unit WHERE parent_node_id IS NULL AND kind=?1 ORDER BY position,id")?;
                query
                    .query_map([kind_value], |row| row.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            };
            for (index, sibling) in old.iter().enumerate() {
                let new_position = index + usize::from(index >= position as usize);
                transaction.execute(
                    "UPDATE content_unit SET position=?2 WHERE id=?1",
                    params![sibling, new_position as i64],
                )?;
            }
        }
        transaction.execute(
            "INSERT INTO content_unit(id,kind,title,parent_node_id,position,target_words,relative_path,sha256,byte_length,word_count,generation,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0,0,0,?9,?9)",
            params![id.to_string(), kind_value, title, parent_value, position, target_words, relative.to_string_lossy(), hash_bytes(bytes), now],
        )?;
        transaction.execute(
            "INSERT INTO content_unit_search_content(content_unit_id,content) VALUES(?1,'')",
            [id.to_string()],
        )?;
        transaction.commit()?;
        Ok(id)
    }
    pub fn get_content_unit(&self, id: ContentUnitId) -> Result<ContentUnitRecord> {
        let row = self.conn.query_row(
            "SELECT kind,title,parent_node_id,position,target_words,word_count,sha256,archived,language_code FROM content_unit WHERE id=?1",
            [id.to_string()],
            |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, Option<String>>(2)?,row.get::<_, u32>(3)?,row.get::<_, Option<u64>>(4)?,row.get::<_, u32>(5)?,row.get::<_, String>(6)?,row.get::<_, bool>(7)?,row.get::<_, Option<String>>(8)?)),
        ).optional()?.ok_or_else(|| StorageError::NotFound(format!("content unit {id}")))?;
        let parent_id = row
            .2
            .map(|value| {
                Uuid::parse_str(&value)
                    .map(NodeId)
                    .map_err(|e| StorageError::Invalid(e.to_string()))
            })
            .transpose()?;
        let kind = ContentUnitKind::parse(&row.0)
            .ok_or_else(|| StorageError::Invalid(format!("unknown content unit kind {}", row.0)))?;
        Ok(ContentUnitRecord {
            id,
            kind,
            title: row.1,
            parent_id,
            position: row.3,
            target_words: row.4,
            actual_words: row.5,
            content_hash: row.6,
            archived: row.7,
            language_code: row.8,
        })
    }
    pub fn set_content_unit_language(
        &mut self,
        id: ContentUnitId,
        language_code: Option<&str>,
    ) -> Result<()> {
        if let Some(language_code) = language_code
            && !is_valid_language_tag(language_code)
        {
            return Err(StorageError::Invalid(
                "language must be a valid BCP-47-style language tag (for example en or fr-CA)"
                    .into(),
            ));
        }
        let updated = self.conn.execute(
            "UPDATE content_unit SET language_code=?2,updated_at=?3 WHERE id=?1",
            params![id.to_string(), language_code, now()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("content unit {id}")));
        }
        Ok(())
    }
    pub fn list_content_units(&self, include_archived: bool) -> Result<Vec<ContentUnitRecord>> {
        let mut query = self.conn.prepare("SELECT id FROM content_unit WHERE (?1 OR archived=0) ORDER BY CASE kind WHEN 'front_matter' THEN 0 WHEN 'section_body' THEN 1 WHEN 'chapter_body' THEN 2 ELSE 3 END,parent_node_id,position,id")?;
        let ids = query
            .query_map([include_archived], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                let id = ContentUnitId(
                    Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                );
                self.get_content_unit(id)
            })
            .collect()
    }
    pub fn rename_content_unit(&mut self, id: ContentUnitId, title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Err(StorageError::Invalid(
                "content unit title cannot be empty".into(),
            ));
        }
        let updated = self.conn.execute(
            "UPDATE content_unit SET title=?2,updated_at=?3 WHERE id=?1",
            params![id.to_string(), title.trim(), now()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("content unit {id}")));
        }
        Ok(())
    }
    pub fn get_scene(&self, id: SceneId) -> Result<SceneRecord> {
        let row=self.conn.query_row("SELECT s.title,s.synopsis,s.status,s.target_words,s.purpose,s.goal,s.outcome,s.start_nominal,s.start_lower,s.start_upper,s.start_precision,s.end_nominal,s.end_lower,s.end_upper,s.end_precision,s.pov_character_id,s.start_location_id,s.end_location_id,s.archived,p.chapter_id,p.position,c.sha256,c.byte_length,s.language_code,s.word_count_policy FROM scene s LEFT JOIN scene_placement p ON p.scene_id=s.id JOIN scene_content c ON c.scene_id=s.id WHERE s.id=?1",[id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<u64>>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,Option<i64>>(7)?,r.get::<_,Option<i64>>(8)?,r.get::<_,Option<i64>>(9)?,r.get::<_,String>(10)?,r.get::<_,Option<i64>>(11)?,r.get::<_,Option<i64>>(12)?,r.get::<_,Option<i64>>(13)?,r.get::<_,String>(14)?,r.get::<_,Option<String>>(15)?,r.get::<_,Option<String>>(16)?,r.get::<_,Option<String>>(17)?,r.get::<_,bool>(18)?,r.get::<_,Option<String>>(19)?,r.get::<_,Option<u32>>(20)?,r.get::<_,String>(21)?,r.get::<_,u64>(22)?,r.get::<_,Option<String>>(23)?,r.get::<_,Option<String>>(24)?))).map_err(|_|StorageError::NotFound(id.to_string()))?;
        let st = read_time(row.7, row.8, row.9, &row.10)?;
        let et = read_time(row.11, row.12, row.13, &row.14)?;
        let content = fs::read(self.safe_path(&self.content_path(id)?)?)?;
        if hash_bytes(&content) != row.21 {
            return Err(StorageError::Conflict(format!(
                "external manuscript modification for scene {id}"
            )));
        }
        let text = String::from_utf8_lossy(&content);
        let project = self.get_project()?;
        let scene_policy = row
            .24
            .as_deref()
            .map(|value| {
                WordCountPolicy::parse(value).ok_or_else(|| {
                    StorageError::Config(format!("invalid scene word-count policy {value}"))
                })
            })
            .transpose()?;
        let actual =
            count_words_with_policy(&text, scene_policy.unwrap_or(project.word_count_policy));
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
                language_code: row.23,
                word_count_policy: scene_policy,
                archived: row.18,
            },
            chapter_id: row.19.and_then(|x| Uuid::parse_str(&x).ok()).map(NodeId),
            position: row.20,
            actual_words: actual,
            content_hash: row.21,
        })
    }
    pub fn scene_interval_boundaries(
        &self,
        id: SceneId,
    ) -> Result<(IntervalBoundary, IntervalBoundary)> {
        let row = self
            .conn
            .query_row(
                "SELECT start_boundary,end_boundary FROM scene_interval_boundary WHERE scene_id=?1",
                [id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match row {
            Some((start, end)) => Ok((parse_boundary(&start)?, parse_boundary(&end)?)),
            None => Ok((IntervalBoundary::Inclusive, IntervalBoundary::Inclusive)),
        }
    }
    pub fn set_scene_interval_boundaries(
        &mut self,
        id: SceneId,
        start: IntervalBoundary,
        end: IntervalBoundary,
    ) -> Result<()> {
        set_interval_boundaries(
            &mut self.conn,
            "scene_interval_boundary",
            "scene_id",
            &id.to_string(),
            start,
            end,
        )
    }
    pub fn update_scene_story_interval(
        &mut self,
        id: SceneId,
        interval: StoryInterval,
    ) -> Result<()> {
        validate_time(interval.start)?;
        validate_time(interval.end)?;
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE scene SET start_nominal=?2,start_lower=?3,start_upper=?4,start_precision=?5,end_nominal=?6,end_lower=?7,end_upper=?8,end_precision=?9,updated_at=?10 WHERE id=?1",
            params![id.to_string(), interval.start.nominal, interval.start.lower, interval.start.upper, precision(interval.start.precision), interval.end.nominal, interval.end.lower, interval.end.upper, precision(interval.end.precision), now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        write_interval_boundaries(
            &tx,
            "scene_interval_boundary",
            "scene_id",
            &id.to_string(),
            interval.start_boundary,
            interval.end_boundary,
        )?;
        tx.commit()?;
        Ok(())
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
        if let Some(language_code) = scene.language_code.as_deref()
            && !is_valid_language_tag(language_code)
        {
            return Err(StorageError::Invalid("invalid scene language tag".into()));
        }
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
        self.conn.execute("UPDATE scene SET title=?2,synopsis=?3,status=?4,target_words=?5,purpose=?6,goal=?7,outcome=?8,start_nominal=?9,start_lower=?10,start_upper=?11,start_precision=?12,end_nominal=?13,end_lower=?14,end_upper=?15,end_precision=?16,pov_character_id=?17,start_location_id=?18,end_location_id=?19,archived=?20,updated_at=?21,language_code=?22,word_count_policy=?23 WHERE id=?1",params![scene.id.to_string(),scene.title,scene.synopsis,scene.status.as_str(),scene.target_words,scene.purpose,scene.goal,scene.outcome,scene.start_time.nominal,scene.start_time.lower,scene.start_time.upper,precision(scene.start_time.precision),scene.end_time.nominal,scene.end_time.lower,scene.end_time.upper,precision(scene.end_time.precision),scene.pov_character.map(|x|x.to_string()),scene.start_location.map(|x|x.to_string()),scene.end_location.map(|x|x.to_string()),scene.archived,now,scene.language_code,scene.word_count_policy.map(|policy| policy.as_str())])?;
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
    pub fn scene_content_relative_path(&self, id: SceneId) -> Result<PathBuf> {
        self.content_path(id)
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
    fn content_unit_path(&self, id: ContentUnitId) -> Result<PathBuf> {
        let relative: String = self
            .conn
            .query_row(
                "SELECT relative_path FROM content_unit WHERE id=?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(format!("content unit {id}")))?;
        Ok(PathBuf::from(relative))
    }
    pub fn content_unit_content_relative_path(&self, id: ContentUnitId) -> Result<PathBuf> {
        let relative = self.content_unit_path(id)?;
        self.safe_path(&relative)?;
        Ok(relative)
    }
    pub fn load_content_unit_text(&self, id: ContentUnitId) -> Result<(String, String)> {
        let relative = self.content_unit_path(id)?;
        let path = self.safe_path(&relative)?;
        let bytes = fs::read(path)?;
        let actual = hash_bytes(&bytes);
        let expected: String = self.conn.query_row(
            "SELECT sha256 FROM content_unit WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )?;
        if actual != expected {
            return Err(StorageError::Conflict(format!(
                "external edit detected for content unit {id}; expected {expected}, found {actual}"
            )));
        }
        Ok((
            String::from_utf8(bytes).map_err(|error| StorageError::Invalid(error.to_string()))?,
            actual,
        ))
    }
    pub fn save_content_unit_text(
        &mut self,
        id: ContentUnitId,
        expected: Option<&str>,
        text: &str,
    ) -> Result<String> {
        self.save_content_unit_text_inner(id, expected, text, None)
    }
    fn save_content_unit_text_inner(
        &mut self,
        id: ContentUnitId,
        expected: Option<&str>,
        text: &str,
        fail: Option<ContentUnitSaveFailpoint>,
    ) -> Result<String> {
        let relative = self.content_unit_path(id)?;
        let target = self.safe_path(&relative)?;
        let current: String = self.conn.query_row(
            "SELECT sha256 FROM content_unit WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )?;
        if expected != Some(current.as_str()) {
            return Err(StorageError::Conflict(
                "stale expected content-unit hash".into(),
            ));
        }
        let actual = if target.exists() {
            hash_file(&target)?
        } else {
            String::new()
        };
        if actual != current {
            return Err(StorageError::Conflict(
                "content unit changed outside Writer's IDE".into(),
            ));
        }
        let bytes = text.as_bytes();
        let new_hash = hash_bytes(bytes);
        let txid = Uuid::new_v4().to_string();
        let temp = target.with_file_name(format!(".content-{id}.{txid}.tmp"));
        if fail == Some(ContentUnitSaveFailpoint::AfterPartialTemp) {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes[..bytes.len() / 2])?;
            file.sync_all()?;
            sync_dir(target.parent().unwrap())?;
            return Err(StorageError::Invalid(
                "injected interruption during content-unit temp write".into(),
            ));
        }
        write_synced(&temp, bytes)?;
        let candidate_relative = PathBuf::from(".writers-ide/recovery")
            .join(&txid)
            .join("candidate.md");
        let candidate = self.safe_path(&candidate_relative)?;
        fs::create_dir_all(candidate.parent().unwrap())?;
        write_synced(&candidate, bytes)?;
        sync_dir(target.parent().unwrap())?;
        sync_dir(candidate.parent().unwrap())?;
        sync_dir(candidate.parent().unwrap().parent().unwrap())?;
        if fail == Some(ContentUnitSaveFailpoint::AfterTempSync) {
            return Err(StorageError::Invalid(
                "injected interruption after content-unit temp sync".into(),
            ));
        }
        let generation: i64 = self.conn.query_row(
            "SELECT generation+1 FROM content_unit WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )?;
        {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute("INSERT INTO content_unit_write_journal(tx_id,content_unit_id,target_path,temp_path,candidate_path,old_hash,new_hash,new_length,next_generation,state,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'prepared',?10)",params![txid,id.to_string(),relative.to_string_lossy(),temp.strip_prefix(&self.root).unwrap_or(&temp).to_string_lossy(),candidate_relative.to_string_lossy(),current,new_hash,bytes.len() as i64,generation,now()])?;
            tx.commit()?;
        }
        if fail == Some(ContentUnitSaveFailpoint::AfterIntent)
            || fail == Some(ContentUnitSaveFailpoint::BeforeRename)
        {
            return Err(StorageError::Invalid(
                "injected interruption after content-unit write intent".into(),
            ));
        }
        if (if target.exists() {
            hash_file(&target)?
        } else {
            String::new()
        }) != current
        {
            let _ = self.recover_pending_content_unit_writes()?;
            return Err(StorageError::Conflict(
                "content unit changed while save was prepared; both versions were preserved".into(),
            ));
        }
        fs::rename(&temp, &target)?;
        sync_dir(target.parent().unwrap())?;
        if fail == Some(ContentUnitSaveFailpoint::AfterRename) {
            return Err(StorageError::Invalid(
                "injected interruption after content-unit replacement".into(),
            ));
        }
        let word_count_policy = self.get_project()?.word_count_policy;
        {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute("UPDATE content_unit SET sha256=?2,byte_length=?3,word_count=?4,generation=?5,updated_at=?6 WHERE id=?1",params![id.to_string(),new_hash,bytes.len() as i64,count_words_with_policy(text,word_count_policy) as i64,generation,now()])?;
            tx.execute(
                "UPDATE content_unit_search_content SET content=?2 WHERE content_unit_id=?1",
                params![id.to_string(), text],
            )?;
            tx.execute(
                "UPDATE content_unit_write_journal SET state='db_committed' WHERE tx_id=?1",
                [&txid],
            )?;
            tx.commit()?;
        }
        if fail == Some(ContentUnitSaveFailpoint::AfterMetadataCommit) {
            return Err(StorageError::Invalid(
                "injected interruption after content-unit metadata commit".into(),
            ));
        }
        self.cleanup_content_unit_journal(&txid, &temp, &candidate)?;
        if fail == Some(ContentUnitSaveFailpoint::AfterJournalCleanup) {
            return Err(StorageError::Invalid(
                "injected interruption during content-unit journal cleanup".into(),
            ));
        }
        Ok(new_hash)
    }
    fn cleanup_content_unit_journal(
        &mut self,
        txid: &str,
        temp: &Path,
        candidate: &Path,
    ) -> Result<()> {
        if temp.exists() {
            fs::remove_file(temp)?;
        }
        if candidate.exists() {
            fs::remove_file(candidate)?;
        }
        if let Some(parent) = candidate.parent() {
            sync_dir(parent).map_err(|e| {
                StorageError::Config(format!(
                    "sync recovery transaction before removal {}: {e}",
                    parent.display()
                ))
            })?;
            let _ = fs::remove_dir(parent);
            if let Some(recovery) = parent.parent() {
                sync_dir(recovery).map_err(|e| {
                    StorageError::Config(format!(
                        "sync recovery directory {}: {e}",
                        recovery.display()
                    ))
                })?;
            }
        }
        if let Some(parent) = temp.parent() {
            sync_dir(parent).map_err(|e| {
                StorageError::Config(format!(
                    "sync manuscript directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        self.conn.execute(
            "DELETE FROM content_unit_write_journal WHERE tx_id=?1 AND state='db_committed'",
            [txid],
        )?;
        Ok(())
    }
    pub fn recover_pending_content_unit_writes(
        &mut self,
    ) -> Result<Vec<ContentUnitRecoveryConflict>> {
        let entries: Vec<ContentUnitJournalRow> = {
            let mut q = self.conn.prepare("SELECT tx_id,content_unit_id,target_path,temp_path,candidate_path,old_hash,new_hash,new_length,next_generation,state FROM content_unit_write_journal ORDER BY created_at")?;
            q.query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                ))
            })?
            .collect::<std::result::Result<_, _>>()?
        };
        let mut conflicts = Vec::new();
        for (
            txid,
            id,
            target_rel,
            temp_rel,
            candidate_rel,
            old_hash,
            new_hash,
            length,
            generation,
            state,
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
            let existing_conflict: Option<String> = self
                .conn
                .query_row(
                    "SELECT tx_id FROM content_unit_write_conflict WHERE tx_id=?1 AND resolved=0",
                    [&txid],
                    |r| r.get(0),
                )
                .optional()?;
            if existing_conflict.is_some() {
                conflicts.push(ContentUnitRecoveryConflict {
                    transaction_id: txid.clone(),
                    content_unit_id: ContentUnitId(
                        Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                    ),
                    recovery_path: self.root.join(".writers-ide/recovery").join(&txid),
                });
            } else if target_hash.as_deref() == Some(new_hash.as_str()) {
                self.update_content_unit_recovery(&txid, &id, &new_hash, length, generation)?;
                self.cleanup_content_unit_journal(&txid, &temp, &candidate)?;
            } else if target_hash.as_deref() == old_hash.as_deref()
                && temp_hash.as_deref() == Some(new_hash.as_str())
            {
                fs::rename(&temp, &target).map_err(|e| {
                    StorageError::Config(format!("replace recovered content unit: {e}"))
                })?;
                sync_dir(target.parent().unwrap()).map_err(|e| {
                    StorageError::Config(format!("sync recovered content directory: {e}"))
                })?;
                self.update_content_unit_recovery(&txid, &id, &new_hash, length, generation)
                    .map_err(|e| {
                        StorageError::Config(format!("update recovered content metadata: {e}"))
                    })?;
                self.cleanup_content_unit_journal(&txid, &temp, &candidate)
                    .map_err(|e| {
                        StorageError::Config(format!("clean recovered content journal: {e}"))
                    })?;
            } else if state == "prepared"
                && target_hash.as_deref() == old_hash.as_deref()
                && temp_hash.is_none()
            {
                self.cleanup_content_unit_journal(&txid, &temp, &candidate)?;
            } else {
                let recovery = self.root.join(".writers-ide/recovery").join(&txid);
                fs::create_dir_all(&recovery)?;
                if target.exists() && !recovery.join("external-version.md").exists() {
                    copy_synced(&target, &recovery.join("external-version.md"))?;
                }
                let prepared = if candidate.exists() {
                    Some(&candidate)
                } else if temp.exists() {
                    Some(&temp)
                } else {
                    None
                };
                if let Some(prepared) = prepared
                    && !recovery.join("prepared-version.md").exists()
                {
                    copy_synced(prepared, &recovery.join("prepared-version.md"))?;
                }
                sync_dir(&recovery)?;
                self.conn.execute("INSERT OR IGNORE INTO content_unit_write_conflict(tx_id,content_unit_id,recovery_path,expected_hash,actual_hash,candidate_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![txid,id,recovery.strip_prefix(&self.root).unwrap().to_string_lossy(),old_hash,target_hash.unwrap_or_default(),candidate_hash.unwrap_or(new_hash),now()])?;
                conflicts.push(ContentUnitRecoveryConflict {
                    transaction_id: txid,
                    content_unit_id: ContentUnitId(
                        Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                    ),
                    recovery_path: recovery,
                });
            }
        }
        Ok(conflicts)
    }
    pub fn pending_content_unit_conflicts(&self) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM content_unit_write_conflict WHERE resolved=0",
            [],
            |row| row.get(0),
        )?)
    }
    pub fn list_content_unit_recovery_conflicts(&self) -> Result<Vec<ContentUnitRecoveryConflict>> {
        let mut q=self.conn.prepare("SELECT tx_id,content_unit_id,recovery_path FROM content_unit_write_conflict WHERE resolved=0 ORDER BY created_at")?;
        let rows = q.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (txid, id, path) = row?;
            Ok(ContentUnitRecoveryConflict {
                transaction_id: txid,
                content_unit_id: ContentUnitId(
                    Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
                ),
                recovery_path: self.safe_path(Path::new(&path))?,
            })
        })
        .collect()
    }
    fn update_content_unit_recovery(
        &mut self,
        txid: &str,
        id: &str,
        hash: &str,
        length: i64,
        generation: i64,
    ) -> Result<()> {
        let relative: String = self.conn.query_row(
            "SELECT relative_path FROM content_unit WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        let content = fs::read(self.safe_path(Path::new(&relative))?)?;
        let content =
            String::from_utf8(content).map_err(|e| StorageError::Invalid(e.to_string()))?;
        let word_count_policy = self.get_project()?.word_count_policy;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE content_unit SET sha256=?2,byte_length=?3,word_count=?4,generation=?5,updated_at=?6 WHERE id=?1",params![id,hash,length,count_words_with_policy(&content,word_count_policy) as i64,generation,now()])?;
        tx.execute(
            "UPDATE content_unit_search_content SET content=?2 WHERE content_unit_id=?1",
            params![id, content],
        )?;
        tx.execute(
            "UPDATE content_unit_write_journal SET state='db_committed' WHERE tx_id=?1",
            [txid],
        )?;
        tx.commit()?;
        Ok(())
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
            refresh_scene_mentions(&tx, id, text, &new_hash)?;
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
            let mut q=self.conn.prepare("SELECT tx_id FROM write_journal UNION SELECT tx_id FROM write_conflict WHERE resolved=0 UNION SELECT tx_id FROM content_unit_write_journal UNION SELECT tx_id FROM content_unit_write_conflict WHERE resolved=0")?;
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
            if let Some(body) = name
                .strip_prefix(".content-")
                .and_then(|s| s.strip_suffix(".tmp"))
                && let Some((content, tx)) = body.split_once('.')
                && Uuid::parse_str(content).is_ok()
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
        refresh_scene_mentions(&tx, scene, &text, new)?;
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
        refresh_scene_mentions(&tx, scene, &text, hash)?;
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
        let name = name.trim();
        if name.is_empty() {
            return Err(StorageError::Invalid(
                "character name cannot be empty".into(),
            ));
        }
        self.conn.execute(
            "INSERT INTO character(id,name,preferred_name) VALUES(?1,?2,?2)",
            params![id.to_string(), name],
        )?;
        Ok(())
    }
    pub fn get_character(&self, id: CharacterId) -> Result<Character> {
        let mut character = self.conn.query_row(
            "SELECT name,preferred_name,description,background,goals,motivations,secrets,internal_conflict,external_conflict,status FROM character WHERE id=?1",
            [id.to_string()],
            |row| Ok(Character { id, full_name: row.get(0)?, preferred_name: row.get(1)?, aliases: Vec::new(), description: row.get(2)?, background: row.get(3)?, goals: row.get(4)?, motivations: row.get(5)?, secrets: row.get(6)?, internal_conflict: row.get(7)?, external_conflict: row.get(8)?, status: row.get(9)? }),
        ).optional()?.ok_or_else(|| StorageError::NotFound(format!("character {id}")))?;
        let mut query = self.conn.prepare(
            "SELECT alias FROM character_alias WHERE character_id=?1 ORDER BY alias COLLATE NOCASE,alias",
        )?;
        character.aliases = query
            .query_map([id.to_string()], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(character)
    }
    pub fn update_character(&mut self, character: &Character) -> Result<()> {
        if character.full_name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "character full name cannot be empty".into(),
            ));
        }
        if character.preferred_name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "character preferred name cannot be empty".into(),
            ));
        }
        let mut aliases = character
            .aliases
            .iter()
            .map(|alias| alias.trim().to_owned())
            .collect::<Vec<_>>();
        if aliases.iter().any(String::is_empty) {
            return Err(StorageError::Invalid(
                "character aliases cannot be empty".into(),
            ));
        }
        let mut unique = std::collections::BTreeSet::new();
        if aliases
            .iter()
            .any(|alias| !unique.insert(alias.to_lowercase()))
        {
            return Err(StorageError::Invalid(
                "character aliases must be unique ignoring case".into(),
            ));
        }
        aliases.sort_by_key(|alias| alias.to_lowercase());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE character SET name=?2,preferred_name=?3,description=?4,background=?5,goals=?6,motivations=?7,secrets=?8,internal_conflict=?9,external_conflict=?10,status=?11 WHERE id=?1",
            params![character.id.to_string(), character.full_name.trim(), character.preferred_name.trim(), character.description, character.background, character.goals, character.motivations, character.secrets, character.internal_conflict, character.external_conflict, character.status],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!(
                "character {}",
                character.id
            )));
        }
        tx.execute(
            "DELETE FROM character_alias WHERE character_id=?1",
            [character.id.to_string()],
        )?;
        for alias in aliases {
            tx.execute(
                "INSERT INTO character_alias(character_id,alias) VALUES(?1,?2)",
                params![character.id.to_string(), alias],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn list_characters(&self) -> Result<Vec<CharacterSummary>> {
        let mut q=self.conn.prepare("SELECT c.id,COALESCE(NULLIF(c.preferred_name,''),c.name),COUNT(DISTINCT sc.scene_id) FROM character c LEFT JOIN scene_character sc ON sc.character_id=c.id GROUP BY c.id,c.name,c.preferred_name ORDER BY COALESCE(NULLIF(c.preferred_name,''),c.name) COLLATE NOCASE,c.id")?;
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

    pub fn create_story_object(&mut self, name: &str, description: &str) -> Result<StoryObjectId> {
        if name.trim().is_empty() {
            return Err(StorageError::Invalid("object name cannot be empty".into()));
        }
        let id = StoryObjectId::new();
        self.conn.execute(
            "INSERT INTO story_object(id,name,description) VALUES(?1,?2,?3)",
            params![id.to_string(), name.trim(), description],
        )?;
        Ok(id)
    }

    fn story_bible_template_path(&self, id: StoryBibleTemplateId) -> Result<PathBuf> {
        self.safe_path(
            &Path::new("templates")
                .join("story-bible")
                .join(format!("{id}.toml")),
        )
    }

    pub fn save_story_bible_template(&self, template: &StoryBibleTemplate) -> Result<()> {
        validate_story_bible_template(template)?;
        let target = self.story_bible_template_path(template.id)?;
        fs::create_dir_all(target.parent().unwrap())?;
        let temporary = target.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let bytes = toml::to_string_pretty(template)
            .map_err(|error| StorageError::Invalid(error.to_string()))?;
        write_synced(&temporary, bytes.as_bytes())?;
        fs::rename(&temporary, &target)?;
        sync_dir(target.parent().unwrap())?;
        Ok(())
    }

    pub fn load_story_bible_template(
        &self,
        id: StoryBibleTemplateId,
    ) -> Result<StoryBibleTemplate> {
        let path = self.story_bible_template_path(id)?;
        let text = fs::read_to_string(path)?;
        let template: StoryBibleTemplate = toml::from_str(&text).map_err(|error| {
            StorageError::Invalid(format!("invalid Story Bible template {id}: {error}"))
        })?;
        if template.id != id {
            return Err(StorageError::Config(
                "Story Bible template filename and stable identity disagree".into(),
            ));
        }
        validate_story_bible_template(&template)?;
        Ok(template)
    }

    pub fn list_story_bible_templates(&self) -> Result<Vec<StoryBibleTemplate>> {
        let directory = self.safe_path(Path::new("templates").join("story-bible").as_path())?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut templates = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "toml")
            {
                let id = StoryBibleTemplateId(
                    Uuid::parse_str(
                        entry
                            .path()
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default(),
                    )
                    .map_err(|error| {
                        StorageError::Invalid(format!(
                            "invalid Story Bible template file name: {error}"
                        ))
                    })?,
                );
                templates.push(self.load_story_bible_template(id)?);
            }
        }
        templates.sort_by(|first, second| {
            first
                .name
                .to_lowercase()
                .cmp(&second.name.to_lowercase())
                .then(first.id.cmp(&second.id))
        });
        Ok(templates)
    }

    fn story_bible_profile_path(
        &self,
        kind: StoryBibleEntityKind,
        entity_id: &str,
    ) -> Result<PathBuf> {
        let entity = Uuid::parse_str(entity_id).map_err(|error| {
            StorageError::Invalid(format!("invalid story-bible entity ID: {error}"))
        })?;
        self.safe_path(
            &Path::new(".writers-ide")
                .join("story-bible-profiles")
                .join(format!("{}-{entity}.json", kind.as_str())),
        )
    }

    pub fn save_story_bible_profile(&mut self, profile: &StoryBibleProfile) -> Result<()> {
        self.validate_story_bible_entity(profile.entity_kind, &profile.entity_id)?;
        let template = self.load_story_bible_template(profile.template_id)?;
        if template.entity_kind != profile.entity_kind {
            return Err(StorageError::Invalid(
                "profile entity kind does not match its template".into(),
            ));
        }
        validate_story_bible_values(&template, &profile.values)?;
        self.validate_story_bible_profile_references(&template, &profile.values)?;
        let path = self.story_bible_profile_path(profile.entity_kind, &profile.entity_id)?;
        fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let bytes = serde_json::to_vec_pretty(profile)
            .map_err(|error| StorageError::Invalid(error.to_string()))?;
        write_synced(&temporary, &bytes)?;
        fs::rename(&temporary, &path)?;
        sync_dir(path.parent().unwrap())?;
        Ok(())
    }

    pub fn load_story_bible_profile(
        &self,
        kind: StoryBibleEntityKind,
        entity_id: &str,
    ) -> Result<StoryBibleProfile> {
        self.validate_story_bible_entity(kind, entity_id)?;
        let path = self.story_bible_profile_path(kind, entity_id)?;
        let bytes = fs::read(path)?;
        let profile: StoryBibleProfile = serde_json::from_slice(&bytes).map_err(|error| {
            StorageError::Invalid(format!("invalid Story Bible profile: {error}"))
        })?;
        if profile.entity_kind != kind || profile.entity_id != entity_id {
            return Err(StorageError::Config(
                "Story Bible profile identity mismatch".into(),
            ));
        }
        let template = self.load_story_bible_template(profile.template_id)?;
        if template.entity_kind != kind {
            return Err(StorageError::Invalid(
                "profile entity kind does not match its template".into(),
            ));
        }
        validate_story_bible_values(&template, &profile.values)?;
        self.validate_story_bible_profile_references(&template, &profile.values)?;
        Ok(profile)
    }

    fn validate_story_bible_profile_references(
        &self,
        template: &StoryBibleTemplate,
        values: &std::collections::BTreeMap<String, StoryBibleFieldValue>,
    ) -> Result<()> {
        for (field_id, value) in values {
            let Some(field) = template.fields.iter().find(|field| field.id == *field_id) else {
                continue;
            };
            if !matches!(
                field.field_type,
                StoryBibleFieldType::EntityRef | StoryBibleFieldType::EntityRefs
            ) {
                continue;
            }
            let targets: Vec<&str> = match value {
                StoryBibleFieldValue::EntityRef(id) => vec![id],
                StoryBibleFieldValue::EntityRefs(ids) => ids.iter().map(String::as_str).collect(),
                _ => Vec::new(),
            };
            let kind = field.reference_kind.ok_or_else(|| {
                StorageError::Invalid("reference field has no target kind".into())
            })?;
            for target in targets {
                self.validate_story_bible_entity(kind, target)?;
            }
        }
        Ok(())
    }

    fn validate_story_bible_entity(&self, kind: StoryBibleEntityKind, id: &str) -> Result<()> {
        let table = match kind {
            StoryBibleEntityKind::Character => "character",
            StoryBibleEntityKind::Location => "location",
            StoryBibleEntityKind::Object => "story_object",
            StoryBibleEntityKind::Plotline => "plotline",
            StoryBibleEntityKind::Event => "story_event",
        };
        let exists: bool = self.conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
            [id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::NotFound(format!("{} {id}", kind.as_str())));
        }
        Ok(())
    }

    pub fn create_planning_board(&mut self, title: &str) -> Result<PlanningBoardId> {
        if title.trim().is_empty() {
            return Err(StorageError::Invalid(
                "planning board title cannot be empty".into(),
            ));
        }
        let id = PlanningBoardId::new();
        let timestamp = now();
        self.conn.execute(
            "INSERT INTO planning_board(id,title,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![id.to_string(), title.trim(), timestamp],
        )?;
        Ok(id)
    }

    pub fn list_planning_boards(&self) -> Result<Vec<PlanningBoardSummary>> {
        let mut statement = self
            .conn
            .prepare("SELECT id,title FROM planning_board ORDER BY title COLLATE NOCASE,id")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, title) = row?;
            Ok::<_, rusqlite::Error>(PlanningBoardSummary {
                id: PlanningBoardId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                title,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
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
        const KINDS: &[&str] = &[
            "scene",
            "character",
            "plotline",
            "location",
            "event",
            "object",
        ];
        if !KINDS.contains(&kind) || !x.is_finite() || !y.is_finite() {
            return Err(StorageError::Invalid(
                "invalid board item kind or coordinates".into(),
            ));
        }
        let id = PlanningBoardItemId::new();
        self.conn.execute("INSERT INTO planning_board_item(id,board_id,entity_kind,entity_id,x,y,note) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![id.to_string(), board.to_string(), kind, entity_id, x, y, note])?;
        Ok(id)
    }

    pub fn list_planning_board_items(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<PlanningBoardItemRecord>> {
        let mut stmt = self.conn.prepare("SELECT id,entity_kind,entity_id,x,y,width,height,note FROM planning_board_item WHERE board_id=?1 ORDER BY y,x,id")?;
        let rows = stmt.query_map([board.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        rows.map(|row| {
            let (id, entity_kind, entity_id, x, y, width, height, note) = row?;
            Ok::<_, rusqlite::Error>(PlanningBoardItemRecord {
                id: PlanningBoardItemId(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?),
                board_id: board,
                entity_kind,
                entity_id,
                x,
                y,
                width,
                height,
                note,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }

    pub fn list_planning_board_connectors(
        &self,
        board: PlanningBoardId,
    ) -> Result<Vec<PlanningBoardConnectorRecord>> {
        let mut stmt = self.conn.prepare("SELECT id,source_item_id,target_item_id,label FROM planning_board_connector WHERE board_id=?1 ORDER BY id")?;
        let rows = stmt.query_map([board.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, source, target, label) = row?;
            let parse = |value: String| {
                Uuid::parse_str(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            };
            Ok::<_, rusqlite::Error>(PlanningBoardConnectorRecord {
                id: PlanningBoardConnectorId(parse(id)?),
                board_id: board,
                source_item_id: PlanningBoardItemId(parse(source)?),
                target_item_id: PlanningBoardItemId(parse(target)?),
                label,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }

    pub fn move_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        x: f64,
        y: f64,
    ) -> Result<()> {
        if !x.is_finite() || !y.is_finite() {
            return Err(StorageError::Invalid(
                "board coordinates must be finite".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE planning_board_item SET x=?2,y=?3 WHERE id=?1",
            params![item.to_string(), x, y],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(item.to_string()));
        }
        Ok(())
    }
    pub fn resize_planning_board_item(
        &mut self,
        item: PlanningBoardItemId,
        width: f64,
        height: f64,
    ) -> Result<()> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(StorageError::Invalid(
                "planning card width and height must be finite positive values".into(),
            ));
        }
        if self.conn.execute(
            "UPDATE planning_board_item SET width=?2,height=?3 WHERE id=?1",
            params![item.to_string(), width, height],
        )? == 0
        {
            return Err(StorageError::NotFound(item.to_string()));
        }
        Ok(())
    }

    pub fn connect_planning_board_items(
        &mut self,
        board: PlanningBoardId,
        source: PlanningBoardItemId,
        target: PlanningBoardItemId,
        label: &str,
    ) -> Result<PlanningBoardConnectorId> {
        if source == target {
            return Err(StorageError::Invalid(
                "board connectors cannot target themselves".into(),
            ));
        }
        let id = PlanningBoardConnectorId::new();
        self.conn.execute("INSERT INTO planning_board_connector(id,board_id,source_item_id,target_item_id,label) VALUES(?1,?2,?3,?4,?5)",params![id.to_string(),board.to_string(),source.to_string(),target.to_string(),label])?;
        Ok(id)
    }

    pub fn create_assembly(&mut self, title: &str) -> Result<AssemblyId> {
        if title.trim().is_empty() {
            return Err(StorageError::Invalid(
                "assembly title cannot be empty".into(),
            ));
        }
        let id = AssemblyId::new();
        let timestamp = now();
        self.conn.execute(
            "INSERT INTO manuscript_assembly(id,title,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![id.to_string(), title.trim(), timestamp],
        )?;
        Ok(id)
    }

    fn assembly_draft_path(&self, id: AssemblyDraftId) -> Result<PathBuf> {
        self.safe_path(
            &Path::new(".writers-ide")
                .join("assembly-drafts")
                .join(format!("{id}.json")),
        )
    }

    fn persist_assembly_draft(&self, draft: &AssemblyDraft) -> Result<()> {
        let target = self.assembly_draft_path(draft.id)?;
        fs::create_dir_all(target.parent().unwrap())?;
        let temp = target.with_extension(format!("{}.tmp", Uuid::new_v4()));
        write_synced(
            &temp,
            &serde_json::to_vec(draft).map_err(|e| StorageError::Invalid(e.to_string()))?,
        )?;
        fs::rename(&temp, &target)?;
        sync_dir(target.parent().unwrap())?;
        Ok(())
    }

    pub fn create_assembly_draft(&mut self, assembly: AssemblyId) -> Result<AssemblyDraft> {
        let project_id = self.get_project()?.id;
        let mut items = Vec::new();
        for item in self.list_assembly_items(assembly)? {
            if let Some(scene_id) = item.scene_id {
                let (text, expected_hash) = self.load_text(scene_id)?;
                items.push(AssemblyDraftItem {
                    scene_id,
                    expected_hash,
                    text,
                });
            }
        }
        if items.is_empty() {
            return Err(StorageError::Invalid(
                "assembly has no scene items to edit".into(),
            ));
        }
        let draft = AssemblyDraft {
            id: AssemblyDraftId::new(),
            assembly_id: assembly,
            project_id,
            created_at: now(),
            items,
        };
        self.persist_assembly_draft(&draft)?;
        Ok(draft)
    }

    pub fn load_assembly_draft(&self, id: AssemblyDraftId) -> Result<AssemblyDraft> {
        let path = self.assembly_draft_path(id)?;
        let bytes = fs::read(path)?;
        let draft: AssemblyDraft = serde_json::from_slice(&bytes)
            .map_err(|e| StorageError::Invalid(format!("invalid assembly draft {id}: {e}")))?;
        if draft.id != id || draft.project_id != self.get_project()?.id {
            return Err(StorageError::Config(
                "assembly draft identity does not match this project".into(),
            ));
        }
        Ok(draft)
    }

    pub fn list_assembly_drafts(&self) -> Result<Vec<AssemblyDraft>> {
        let dir = self.safe_path(&Path::new(".writers-ide").join("assembly-drafts"))?;
        let mut drafts = Vec::new();
        if !dir.exists() {
            return Ok(drafts);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let id = AssemblyDraftId(
                    Uuid::parse_str(
                        entry
                            .path()
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or_default(),
                    )
                    .map_err(|e| {
                        StorageError::Invalid(format!("invalid assembly draft filename: {e}"))
                    })?,
                );
                drafts.push(self.load_assembly_draft(id)?);
            }
        }
        drafts.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(drafts)
    }

    pub fn update_assembly_draft_item(
        &mut self,
        draft_id: AssemblyDraftId,
        scene: SceneId,
        text: &str,
    ) -> Result<()> {
        let mut draft = self.load_assembly_draft(draft_id)?;
        let item = draft
            .items
            .iter_mut()
            .find(|item| item.scene_id == scene)
            .ok_or_else(|| {
                StorageError::NotFound(format!("scene {scene} is not in assembly draft"))
            })?;
        item.text = text.to_owned();
        self.persist_assembly_draft(&draft)
    }

    pub fn replace_assembly_draft_items(
        &mut self,
        id: AssemblyDraftId,
        replacements: &[(SceneId, String)],
    ) -> Result<()> {
        let mut draft = self.load_assembly_draft(id)?;
        if replacements.len() != draft.items.len() {
            return Err(StorageError::Invalid(
                "assembly draft replacement must include every scene exactly once".into(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for (scene, text) in replacements {
            if !seen.insert(*scene) {
                return Err(StorageError::Invalid(
                    "assembly draft contains a duplicate scene".into(),
                ));
            }
            let item = draft
                .items
                .iter_mut()
                .find(|item| item.scene_id == *scene)
                .ok_or_else(|| {
                    StorageError::Invalid(format!("scene {scene} is not in the assembly draft"))
                })?;
            item.text = text.clone();
        }
        self.persist_assembly_draft(&draft)
    }

    /// Applies a durable assembly draft using each scene's normal optimistic journal.
    /// A restart may leave some scene files committed and others pending; the persisted draft
    /// lets this operation safely resume by recognizing either the original or proposed hash.
    pub fn commit_assembly_draft(&mut self, id: AssemblyDraftId) -> Result<()> {
        let draft = self.load_assembly_draft(id)?;
        let mut current = Vec::with_capacity(draft.items.len());
        for item in &draft.items {
            let (_, hash) = self.load_text(item.scene_id)?;
            let proposed = hash_bytes(item.text.as_bytes());
            if hash != item.expected_hash && hash != proposed {
                return Err(StorageError::Conflict(format!(
                    "scene {} changed outside this assembly draft; draft preserved",
                    item.scene_id
                )));
            }
            current.push((hash, proposed));
        }
        for (item, (hash, proposed)) in draft.items.iter().zip(current) {
            if hash != proposed {
                self.save_text(item.scene_id, Some(&hash), &item.text)?;
            }
        }
        let path = self.assembly_draft_path(id)?;
        fs::remove_file(&path)?;
        sync_dir(path.parent().unwrap())?;
        Ok(())
    }

    pub fn list_assemblies(&self) -> Result<Vec<AssemblySummary>> {
        let mut query = self
            .conn
            .prepare("SELECT id,title FROM manuscript_assembly ORDER BY title COLLATE NOCASE,id")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, title) = row?;
            Ok::<AssemblySummary, rusqlite::Error>(AssemblySummary {
                id: AssemblyId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                title,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }

    pub fn list_assembly_items(&self, assembly: AssemblyId) -> Result<Vec<AssemblyItemRecord>> {
        let mut query = self.conn.prepare(
            "SELECT id,position,scene_id,content_unit_id,included FROM assembly_item WHERE assembly_id=?1 ORDER BY position,id",
        )?;
        let rows = query.query_map([assembly.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, position, scene, content, included) = row?;
            let parse_id = |value: String| {
                Uuid::parse_str(&value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            };
            Ok(AssemblyItemRecord {
                id: AssemblyItemId(parse_id(id)?),
                assembly_id: assembly,
                position,
                scene_id: scene.map(parse_id).transpose()?.map(SceneId),
                content_unit_id: content.map(parse_id).transpose()?.map(ContentUnitId),
                included,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }

    pub fn add_assembly_scene(
        &mut self,
        assembly: AssemblyId,
        scene: SceneId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        self.add_assembly_item(assembly, Some(scene), None, position)
    }

    pub fn add_assembly_content_unit(
        &mut self,
        assembly: AssemblyId,
        content: ContentUnitId,
        position: u32,
    ) -> Result<AssemblyItemId> {
        self.add_assembly_item(assembly, None, Some(content), position)
    }

    fn add_assembly_item(
        &mut self,
        assembly: AssemblyId,
        scene: Option<SceneId>,
        content: Option<ContentUnitId>,
        position: u32,
    ) -> Result<AssemblyItemId> {
        if scene.is_some() == content.is_some() {
            return Err(StorageError::Invalid(
                "assembly item must identify exactly one content entity".into(),
            ));
        }
        let id = AssemblyItemId::new();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: u32 = tx.query_row(
            "SELECT count(*) FROM assembly_item WHERE assembly_id=?1",
            [assembly.to_string()],
            |row| row.get(0),
        )?;
        if position > count {
            return Err(StorageError::Invalid(
                "assembly position beyond item count".into(),
            ));
        }
        let max: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position),0) FROM assembly_item WHERE assembly_id=?1",
            [assembly.to_string()],
            |row| row.get(0),
        )?;
        let temporary = max
            .checked_mul(3)
            .and_then(|value| value.checked_add(100))
            .ok_or_else(|| StorageError::Invalid("assembly positions exhausted".into()))?;
        tx.execute(
            "INSERT INTO assembly_item(id,assembly_id,position,scene_id,content_unit_id) VALUES(?1,?2,?3,?4,?5)",
            params![id.to_string(), assembly.to_string(), temporary, scene.map(|value| value.to_string()), content.map(|value| value.to_string())],
        )?;
        let mut ids = query_assembly_item_ids(&tx, assembly)?;
        let inserted = ids
            .iter()
            .position(|value| value == &id.to_string())
            .unwrap();
        let item = ids.remove(inserted);
        ids.insert(position as usize, item);
        compact_assembly_positions(&tx, assembly, &ids)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn move_assembly_item(
        &mut self,
        assembly: AssemblyId,
        item: AssemblyItemId,
        position: u32,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut ids = query_assembly_item_ids(&tx, assembly)?;
        let existing = ids
            .iter()
            .position(|value| value == &item.to_string())
            .ok_or_else(|| StorageError::NotFound(item.to_string()))?;
        let item_id = ids.remove(existing);
        if position as usize > ids.len() {
            return Err(StorageError::Invalid(
                "assembly position beyond item count".into(),
            ));
        }
        ids.insert(position as usize, item_id);
        compact_assembly_positions(&tx, assembly, &ids)?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_assembly_item_included(
        &mut self,
        item: AssemblyItemId,
        included: bool,
    ) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE assembly_item SET included=?2 WHERE id=?1",
            params![item.to_string(), included],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(item.to_string()));
        }
        Ok(())
    }

    pub fn list_story_objects(&self) -> Result<Vec<StoryObjectRecord>> {
        let mut query = self.conn.prepare(
            "SELECT id,name,description FROM story_object ORDER BY name COLLATE NOCASE,id",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, description) = row?;
            Ok(StoryObjectRecord {
                id: StoryObjectId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                name,
                description,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }

    pub fn list_scene_mentions(&self, scene: SceneId) -> Result<Vec<DetectedMentionRecord>> {
        let mut query = self.conn.prepare(
            "SELECT id,start_byte,end_byte,quote,state,entity_kind,entity_id FROM detected_mention WHERE scene_id=?1 ORDER BY start_byte,end_byte,id",
        )?;
        let rows = query.query_map([scene.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (id, start_byte, end_byte, quote, state, entity_kind, entity_id) = row?;
            let id = MentionId(Uuid::parse_str(&id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?);
            let mut candidates_query = self.conn.prepare(
                "SELECT entity_kind,entity_id,matched_name FROM detected_mention_candidate WHERE mention_id=?1 ORDER BY entity_kind,entity_id,matched_name",
            )?;
            let candidates = candidates_query
                .query_map([id.to_string()], |candidate| {
                    Ok(MentionCandidate {
                        entity_kind: candidate.get(0)?,
                        entity_id: candidate.get(1)?,
                        matched_name: candidate.get(2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            records.push(DetectedMentionRecord {
                id,
                scene_id: scene,
                start_byte,
                end_byte,
                quote,
                state,
                entity_kind,
                entity_id,
                candidates,
            });
        }
        Ok(records)
    }

    pub fn reindex_scene_mentions(
        &mut self,
        scene: SceneId,
        text: &str,
        expected_hash: &str,
    ) -> Result<()> {
        let actual: String = self.conn.query_row(
            "SELECT sha256 FROM scene_content WHERE scene_id=?1",
            [scene.to_string()],
            |row| row.get(0),
        )?;
        if actual != expected_hash || hash_bytes(text.as_bytes()) != expected_hash {
            return Err(StorageError::Conflict(
                "cannot index mentions for stale manuscript content".into(),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        refresh_scene_mentions(&tx, scene, text, expected_hash)?;
        tx.commit()?;
        Ok(())
    }

    pub fn confirm_scene_mention(
        &mut self,
        mention: MentionId,
        entity_kind: MentionEntityKind,
        entity_id: &str,
    ) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE detected_mention SET state='confirmed',entity_kind=?2,entity_id=?3,updated_at=?4 WHERE id=?1 AND EXISTS(SELECT 1 FROM detected_mention_candidate WHERE mention_id=?1 AND entity_kind=?2 AND entity_id=?3)",
            params![mention.to_string(), entity_kind.as_str(), entity_id, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::Invalid(
                "the selected entity is not a candidate for this mention".into(),
            ));
        }
        Ok(())
    }

    pub fn dismiss_scene_mention(&mut self, mention: MentionId) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE detected_mention SET state='dismissed',updated_at=?2 WHERE id=?1 AND state IN('detected','ambiguous','needs_review')",
            params![mention.to_string(), now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(mention.to_string()));
        }
        Ok(())
    }

    pub fn create_scratchpad_note(&mut self, title: &str, body: &str) -> Result<ScratchpadId> {
        if title.trim().is_empty() {
            return Err(StorageError::Invalid(
                "scratchpad title cannot be empty".into(),
            ));
        }
        let position: u32 = self.conn.query_row(
            "SELECT COALESCE(MAX(position),-1)+1 FROM scratchpad_note WHERE state='active'",
            [],
            |row| row.get(0),
        )?;
        let id = ScratchpadId::new();
        let timestamp = now();
        self.conn.execute(
            "INSERT INTO scratchpad_note(id,title,body,position,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",
            params![id.to_string(), title.trim(), body, position, timestamp],
        )?;
        Ok(id)
    }

    pub fn list_scratchpad_notes(&self, include_archived: bool) -> Result<Vec<ScratchpadNote>> {
        let mut query = self.conn.prepare(if include_archived {
            "SELECT id,title,body,pinned,position,state,destination_kind,destination_id FROM scratchpad_note ORDER BY pinned DESC,position,id"
        } else {
            "SELECT id,title,body,pinned,position,state,destination_kind,destination_id FROM scratchpad_note WHERE state='active' ORDER BY pinned DESC,position,id"
        })?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        rows.map(|row| {
            let (id, title, body, pinned, position, state, destination_kind, destination_id) = row?;
            Ok(ScratchpadNote {
                id: ScratchpadId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                title,
                body,
                pinned,
                position,
                state,
                destination_kind,
                destination_id,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }

    pub fn update_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        title: &str,
        body: &str,
        pinned: bool,
    ) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE scratchpad_note SET title=?2,body=?3,pinned=?4,updated_at=?5 WHERE id=?1",
            params![id.to_string(), title.trim(), body, pinned, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn promote_scratchpad_note(
        &mut self,
        id: ScratchpadId,
        destination_kind: &str,
        destination_id: &str,
    ) -> Result<()> {
        let exists: bool = match destination_kind {
            "scene" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM scene WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            "character" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM character WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            "location" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM location WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            "object" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM story_object WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            "plotline" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM plotline WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            "research" => self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM research_item WHERE id=?1)",
                [destination_id],
                |row| row.get(0),
            )?,
            _ => {
                return Err(StorageError::Invalid(
                    "unsupported scratchpad destination kind".into(),
                ));
            }
        };
        if !exists {
            return Err(StorageError::NotFound(destination_id.into()));
        }
        let changed = self.conn.execute(
            "UPDATE scratchpad_note SET state='promoted',destination_kind=?2,destination_id=?3,updated_at=?4 WHERE id=?1 AND state='active'",
            params![id.to_string(), destination_kind, destination_id, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }
    pub fn add_location(&mut self, id: LocationId, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO location(id,name) VALUES(?1,?2)",
            params![id.to_string(), name],
        )?;
        Ok(())
    }
    pub fn get_location(&self, id: LocationId) -> Result<Location> {
        self.conn
            .query_row(
                "SELECT id,parent_id,name,description FROM location WHERE id=?1",
                [id.to_string()],
                |row| {
                    let id: String = row.get(0)?;
                    let parent: Option<String> = row.get(1)?;
                    Ok(Location {
                        id: LocationId(Uuid::parse_str(&id).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?),
                        parent: parent
                            .map(|value| Uuid::parse_str(&value).map(LocationId))
                            .transpose()
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }
    pub fn update_location(&mut self, location: &Location) -> Result<()> {
        if location.name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "location name cannot be empty".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE location SET parent_id=?2,name=?3,description=?4 WHERE id=?1",
            params![
                location.id.to_string(),
                location.parent.map(|parent| parent.to_string()),
                location.name.trim(),
                location.description
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(location.id.to_string()));
        }
        Ok(())
    }
    pub fn list_locations(&self) -> Result<Vec<LocationSummary>> {
        let mut query = self
            .conn
            .prepare("SELECT id,name FROM location ORDER BY name COLLATE NOCASE,id")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, name) = row?;
            Ok(LocationSummary {
                id: LocationId(Uuid::parse_str(&id).map_err(|error| {
                    StorageError::Invalid(format!("invalid location UUID: {error}"))
                })?),
                name,
            })
        })
        .collect()
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
    pub fn record_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
        notes: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO scene_character_absence(scene_id,character_id,notes) VALUES(?1,?2,?3)",
            params![scene.to_string(), character.to_string(), notes],
        )?;
        Ok(())
    }
    pub fn clear_character_absence(
        &mut self,
        scene: SceneId,
        character: CharacterId,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM scene_character_absence WHERE scene_id=?1 AND character_id=?2",
            params![scene.to_string(), character.to_string()],
        )?;
        Ok(())
    }
    pub fn scene_character_absences(
        &self,
        scene: SceneId,
    ) -> Result<Vec<(CharacterId, String, String)>> {
        let mut query = self.conn.prepare("SELECT c.id,c.name,a.notes FROM scene_character_absence a JOIN character c ON c.id=a.character_id WHERE a.scene_id=?1 ORDER BY c.name COLLATE NOCASE,c.id")?;
        let rows = query
            .query_map([scene.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, name, notes)| {
                Ok((
                    CharacterId(
                        Uuid::parse_str(&id)
                            .map_err(|error| StorageError::Invalid(error.to_string()))?,
                    ),
                    name,
                    notes,
                ))
            })
            .collect()
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
    pub fn update_story_event(&mut self, event: &StoryEvent) -> Result<()> {
        validate_time(event.start)?;
        validate_time(event.end)?;
        let changed = self.conn.execute("UPDATE story_event SET title=?2,description=?3,time_nominal=?4,time_lower=?5,time_upper=?6,time_precision=?7,end_nominal=?8,end_lower=?9,end_upper=?10,end_precision=?11,source_time_text=?12 WHERE id=?1", params![event.id.to_string(),event.title,event.description,event.start.nominal,event.start.lower,event.start.upper,precision(event.start.precision),event.end.nominal,event.end.lower,event.end.upper,precision(event.end.precision),event.source_time_text])?;
        if changed == 0 {
            return Err(StorageError::NotFound(event.id.to_string()));
        }
        Ok(())
    }
    pub fn update_story_event_time(
        &mut self,
        id: EventId,
        start: StoryTime,
        end: StoryTime,
        source_time_text: &str,
    ) -> Result<()> {
        validate_time(start)?;
        validate_time(end)?;
        let changed = self.conn.execute(
            "UPDATE story_event SET time_nominal=?2,time_lower=?3,time_upper=?4,time_precision=?5,end_nominal=?6,end_lower=?7,end_upper=?8,end_precision=?9,source_time_text=?10 WHERE id=?1",
            params![id.to_string(), start.nominal, start.lower, start.upper, precision(start.precision), end.nominal, end.lower, end.upper, precision(end.precision), source_time_text],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }
    pub fn story_event_interval_boundaries(
        &self,
        id: EventId,
    ) -> Result<(IntervalBoundary, IntervalBoundary)> {
        let row = self.conn.query_row(
            "SELECT start_boundary,end_boundary FROM story_event_interval_boundary WHERE event_id=?1",
            [id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).optional()?;
        match row {
            Some((start, end)) => Ok((parse_boundary(&start)?, parse_boundary(&end)?)),
            None => Ok((IntervalBoundary::Inclusive, IntervalBoundary::Inclusive)),
        }
    }
    pub fn update_story_event_interval(
        &mut self,
        id: EventId,
        interval: StoryInterval,
    ) -> Result<()> {
        validate_time(interval.start)?;
        validate_time(interval.end)?;
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE story_event SET time_nominal=?2,time_lower=?3,time_upper=?4,time_precision=?5,end_nominal=?6,end_lower=?7,end_upper=?8,end_precision=?9 WHERE id=?1",
            params![id.to_string(), interval.start.nominal, interval.start.lower, interval.start.upper, precision(interval.start.precision), interval.end.nominal, interval.end.lower, interval.end.upper, precision(interval.end.precision)],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        write_interval_boundaries(
            &tx,
            "story_event_interval_boundary",
            "event_id",
            &id.to_string(),
            interval.start_boundary,
            interval.end_boundary,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn list_events(&self) -> Result<Vec<EventSummary>> {
        let mut q=self.conn.prepare("SELECT id,title,time_nominal,time_lower,time_upper,time_precision,end_nominal,end_lower,end_upper,end_precision,source_time_text FROM story_event ORDER BY time_nominal IS NULL,time_nominal,id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
            ))
        })?;
        let raw = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        raw.into_iter()
            .map(|row| {
                let start = read_time(row.2, row.3, row.4, &row.5)?;
                let end = read_time(row.6, row.7, row.8, &row.9)?;
                Ok(EventSummary {
                    id: EventId(
                        Uuid::parse_str(&row.0).map_err(|e| {
                            StorageError::Invalid(format!("invalid event UUID: {e}"))
                        })?,
                    ),
                    title: row.1,
                    nominal_time: start.nominal,
                    start,
                    end,
                    source_time: row.10,
                })
            })
            .collect()
    }
    pub fn resolve_content_unit_conflict(&mut self, txid: &str, use_prepared: bool) -> Result<()> {
        let row:Option<(String,String)>=self.conn.query_row("SELECT content_unit_id,recovery_path FROM content_unit_write_conflict WHERE tx_id=?1 AND resolved=0",[txid],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (id, recovery_rel) = row.ok_or_else(|| StorageError::NotFound(txid.into()))?;
        let recovery = self.safe_path(Path::new(&recovery_rel))?;
        let relative = self.content_unit_path(ContentUnitId(
            Uuid::parse_str(&id).map_err(|e| StorageError::Invalid(e.to_string()))?,
        ))?;
        let target = self.safe_path(&relative)?;
        if use_prepared {
            let prepared = recovery.join("prepared-version.md");
            let bytes = fs::read(&prepared).map_err(|e| {
                StorageError::RecoveryConflict(format!("prepared version is unavailable: {e}"))
            })?;
            let temp = target.with_file_name(format!(".resolve-content-{}.tmp", Uuid::new_v4()));
            write_synced(&temp, &bytes)?;
            fs::rename(&temp, &target)?;
            sync_dir(target.parent().unwrap())?;
        }
        let bytes = fs::read(&target)?;
        let hash = hash_bytes(&bytes);
        let text = String::from_utf8(bytes).map_err(|e| StorageError::Invalid(e.to_string()))?;
        let generation: i64 = self.conn.query_row(
            "SELECT generation+1 FROM content_unit WHERE id=?1",
            [&id],
            |r| r.get(0),
        )?;
        let word_count_policy = self.get_project()?.word_count_policy;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE content_unit SET sha256=?2,byte_length=?3,word_count=?4,generation=?5,updated_at=?6 WHERE id=?1",params![id,hash,text.len() as i64,count_words_with_policy(&text,word_count_policy) as i64,generation,now()])?;
        tx.execute(
            "UPDATE content_unit_search_content SET content=?2 WHERE content_unit_id=?1",
            params![id, text],
        )?;
        tx.execute(
            "UPDATE content_unit_write_conflict SET resolved=1,decision=?2 WHERE tx_id=?1",
            params![txid, if use_prepared { 1 } else { 2 }],
        )?;
        tx.execute(
            "DELETE FROM content_unit_write_journal WHERE tx_id=?1",
            [txid],
        )?;
        tx.commit()?;
        // Retain the recovery directory: it contains both versions for later inspection.
        Ok(())
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
    pub fn set_event_characters(
        &mut self,
        event: EventId,
        participants: &[EventCharacterParticipation],
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM story_event_character WHERE event_id=?1",
            [event.to_string()],
        )?;
        for participant in participants {
            tx.execute(
                "INSERT INTO story_event_character(event_id,character_id,role) VALUES(?1,?2,?3)",
                params![
                    event.to_string(),
                    participant.character_id.to_string(),
                    participant.role.as_str()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn event_characters(&self, event: EventId) -> Result<Vec<EventCharacterParticipation>> {
        let mut query = self.conn.prepare(
            "SELECT character_id,role FROM story_event_character WHERE event_id=?1 ORDER BY character_id,role",
        )?;
        let rows = query.query_map([event.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, role) = row?;
            let character_id = CharacterId(Uuid::parse_str(&id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?);
            let role = match role.as_str() {
                "participant" => EventParticipationRole::Participant,
                "witness" => EventParticipationRole::Witness,
                "cause" => EventParticipationRole::Cause,
                "affected" => EventParticipationRole::Affected,
                _ => return Err(StorageError::Invalid(format!("unknown event role: {role}"))),
            };
            Ok(EventCharacterParticipation { character_id, role })
        })
        .collect()
    }
    pub fn set_event_locations(
        &mut self,
        event: EventId,
        locations: &[EventLocationAssociation],
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM story_event_location WHERE event_id=?1",
            [event.to_string()],
        )?;
        for location in locations {
            tx.execute(
                "INSERT INTO story_event_location(event_id,location_id,role) VALUES(?1,?2,?3)",
                params![
                    event.to_string(),
                    location.location_id.to_string(),
                    location.role.as_str()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn set_event_relationships(
        &mut self,
        event: EventId,
        participants: &[EventCharacterParticipation],
        locations: &[EventLocationAssociation],
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM story_event WHERE id=?1)",
            [event.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::NotFound(event.to_string()));
        }
        tx.execute(
            "DELETE FROM story_event_character WHERE event_id=?1",
            [event.to_string()],
        )?;
        tx.execute(
            "DELETE FROM story_event_location WHERE event_id=?1",
            [event.to_string()],
        )?;
        for participant in participants {
            tx.execute(
                "INSERT INTO story_event_character(event_id,character_id,role) VALUES(?1,?2,?3)",
                params![
                    event.to_string(),
                    participant.character_id.to_string(),
                    participant.role.as_str()
                ],
            )?;
        }
        for location in locations {
            tx.execute(
                "INSERT INTO story_event_location(event_id,location_id,role) VALUES(?1,?2,?3)",
                params![
                    event.to_string(),
                    location.location_id.to_string(),
                    location.role.as_str()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn event_locations(&self, event: EventId) -> Result<Vec<EventLocationAssociation>> {
        let mut query = self.conn.prepare(
            "SELECT location_id,role FROM story_event_location WHERE event_id=?1 ORDER BY location_id,role",
        )?;
        let rows = query.query_map([event.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (id, role) = row?;
            let location_id = LocationId(Uuid::parse_str(&id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?);
            let role = match role.as_str() {
                "occurred_at" => EventLocationRole::OccurredAt,
                "starts_at" => EventLocationRole::StartsAt,
                "ends_at" => EventLocationRole::EndsAt,
                _ => {
                    return Err(StorageError::Invalid(format!(
                        "unknown event location role: {role}"
                    )));
                }
            };
            Ok(EventLocationAssociation { location_id, role })
        })
        .collect()
    }
    pub fn add_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        if !matches!(dependency.kind.as_str(), "precedes" | "requires") {
            return Err(StorageError::Invalid(
                "event dependency kind must be precedes or requires".into(),
            ));
        }
        self.conn.execute(
            "INSERT INTO story_event_dependency(predecessor_event_id,successor_event_id,dependency_kind,notes) VALUES(?1,?2,?3,?4)",
            params![dependency.predecessor.to_string(), dependency.successor.to_string(), dependency.kind, dependency.notes],
        )?;
        Ok(())
    }
    pub fn list_event_dependencies(&self) -> Result<Vec<StoryEventDependency>> {
        let mut query = self.conn.prepare("SELECT predecessor_event_id,successor_event_id,dependency_kind,notes FROM story_event_dependency ORDER BY predecessor_event_id,successor_event_id,dependency_kind")?;
        query
            .query_map([], |row| {
                let predecessor: String = row.get(0)?;
                let successor: String = row.get(1)?;
                Ok((
                    predecessor,
                    successor,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .map(|row| {
                let (predecessor, successor, kind, notes) = row?;
                Ok(StoryEventDependency {
                    predecessor: EventId(
                        Uuid::parse_str(&predecessor)
                            .map_err(|error| StorageError::Invalid(error.to_string()))?,
                    ),
                    successor: EventId(
                        Uuid::parse_str(&successor)
                            .map_err(|error| StorageError::Invalid(error.to_string()))?,
                    ),
                    kind,
                    notes,
                })
            })
            .collect()
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
    pub fn tag_passage_language(
        &mut self,
        scene: SceneId,
        start_byte: u64,
        end_byte: u64,
        language_code: &str,
    ) -> Result<AnchorId> {
        if !is_valid_language_tag(language_code) {
            return Err(StorageError::Invalid("invalid passage language tag".into()));
        }
        let (text, hash) = self.load_text(scene)?;
        let start = usize::try_from(start_byte)
            .map_err(|_| StorageError::Invalid("passage offset overflow".into()))?;
        let end = usize::try_from(end_byte)
            .map_err(|_| StorageError::Invalid("passage offset overflow".into()))?;
        if start >= end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(StorageError::Invalid(
                "passage range must select non-empty UTF-8 text".into(),
            ));
        }
        let quote = &text[start..end];
        let prefix = text[..start]
            .graphemes(true)
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        let suffix = text[end..].graphemes(true).take(3).collect::<String>();
        let anchor = AnchorId::new();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO passage_anchor(id,scene_id,start_byte,end_byte,quote,prefix_context,suffix_context,source_sha256,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'current',?9,?9)",
            params![anchor.to_string(),scene.to_string(),start_byte,end_byte,quote,prefix,suffix,hash,now()],
        )?;
        tx.execute(
            "INSERT INTO passage_language_tag(anchor_id,language_code,created_at) VALUES(?1,?2,?3)",
            params![anchor.to_string(), language_code, now()],
        )?;
        tx.commit()?;
        Ok(anchor)
    }
    pub fn list_passage_language_tags(&self, scene: SceneId) -> Result<Vec<PassageLanguageRecord>> {
        let mut query = self.conn.prepare(
            "SELECT a.id,a.start_byte,a.end_byte,a.quote,a.state,a.source_sha256,t.language_code FROM passage_language_tag t JOIN passage_anchor a ON a.id=t.anchor_id WHERE a.scene_id=?1 ORDER BY a.start_byte,a.id",
        )?;
        query
            .query_map([scene.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .map(|row| {
                let (id, start_byte, end_byte, quote, state, source_hash, language_code) = row?;
                Ok(PassageLanguageRecord {
                    anchor: PassageAnchorRecord {
                        id: AnchorId(
                            Uuid::parse_str(&id)
                                .map_err(|error| StorageError::Invalid(error.to_string()))?,
                        ),
                        scene_id: scene,
                        start_byte,
                        end_byte,
                        quote,
                        state,
                        source_hash,
                    },
                    language_code,
                })
            })
            .collect()
    }
    pub fn remove_passage_language_tag(&mut self, anchor: AnchorId) -> Result<()> {
        let removed = self.conn.execute(
            "DELETE FROM passage_language_tag WHERE anchor_id=?1",
            [anchor.to_string()],
        )?;
        if removed == 0 {
            return Err(StorageError::NotFound(format!(
                "passage language tag {anchor}"
            )));
        }
        Ok(())
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
        if matches!(link.kind, LinkKind::Includes | LinkKind::Transcludes)
            && (!matches!(
                link.source,
                EntityRef::Scene(_) | EntityRef::Research(_) | EntityRef::Reading(_)
            ) || !matches!(
                link.target,
                EntityRef::Scene(_) | EntityRef::Research(_) | EntityRef::Reading(_)
            ))
        {
            return Err(StorageError::Invalid(
                "inclusion links require document endpoints".into(),
            ));
        }
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
    pub fn delete_story_link(&mut self, id: LinkId) -> Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM story_link WHERE id=?1", [id.to_string()])?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
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
    pub fn load_research_text(&self, id: ResearchId) -> Result<String> {
        self.conn
            .query_row(
                "SELECT body FROM research_item WHERE id=?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
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
    pub fn create_writing_goal(&mut self, goal: &NewWritingGoal) -> Result<String> {
        let start = parse_goal_date(&goal.start_date)?;
        let end = parse_goal_date(&goal.end_date)?;
        if goal.title.trim().is_empty()
            || goal.target_words <= 0
            || goal.baseline_words < 0
            || start > end
            || goal.word_count_policy.trim().is_empty()
            || !matches!(goal.success_metric.as_str(), "raw" | "real")
        {
            return Err(StorageError::Invalid(
                "invalid writing goal configuration".into(),
            ));
        }
        let offset = parse_goal_timezone_offset(&goal.time_zone)?;
        let _ = offset;
        if let Some(scene_id) = goal.scope_scene_id {
            self.conn
                .query_row(
                    "SELECT 1 FROM scene WHERE id=?1",
                    [scene_id.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .ok_or_else(|| StorageError::NotFound(format!("scene {scene_id}")))?;
        }
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO writing_goal(id,title,scope_type,scope_scene_id,start_date,end_date,target_words,baseline_words,time_zone,word_count_policy,success_metric,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                id,
                goal.title.trim(),
                if goal.scope_scene_id.is_some() { "scene" } else { "project" },
                goal.scope_scene_id.map(|scene| scene.to_string()),
                goal.start_date,
                goal.end_date,
                goal.target_words,
                goal.baseline_words,
                goal.time_zone,
                goal.word_count_policy,
                goal.success_metric,
                now()
            ],
        )?;
        let record = WritingGoalRecord {
            id: id.clone(),
            title: goal.title.trim().into(),
            scope_type: if goal.scope_scene_id.is_some() {
                "scene"
            } else {
                "project"
            }
            .into(),
            scope_scene_id: goal.scope_scene_id,
            start_date: goal.start_date.clone(),
            end_date: goal.end_date.clone(),
            target_words: goal.target_words,
            baseline_words: goal.baseline_words,
            time_zone: goal.time_zone.clone(),
            word_count_policy: goal.word_count_policy.clone(),
            success_metric: goal.success_metric.clone(),
            archived: false,
        };
        refresh_writing_goal_snapshots(&self.conn, &record, unix_now())?;
        Ok(id)
    }
    pub fn update_writing_goal(
        &mut self,
        id: &str,
        title: &str,
        target_words: i64,
        baseline_words: i64,
        success_metric: &str,
    ) -> Result<()> {
        if title.trim().is_empty()
            || target_words <= 0
            || baseline_words < 0
            || !matches!(success_metric, "raw" | "real")
        {
            return Err(StorageError::Invalid(
                "invalid writing goal settings".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE writing_goal SET title=?2,target_words=?3,baseline_words=?4,success_metric=?5 WHERE id=?1 AND archived=0",
            params![id, title.trim(), target_words, baseline_words, success_metric],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!("active writing goal {id}")));
        }
        let goal = self.get_writing_goal(id)?;
        refresh_writing_goal_snapshots(&self.conn, &goal, unix_now())?;
        Ok(())
    }
    pub fn archive_writing_goal(&mut self, id: &str) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE writing_goal SET archived=1 WHERE id=?1 AND archived=0",
            [id],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!("active writing goal {id}")));
        }
        Ok(())
    }
    pub fn list_writing_goals(&self, include_archived: bool) -> Result<Vec<WritingGoalRecord>> {
        let mut query = self.conn.prepare("SELECT id,title,scope_type,scope_scene_id,start_date,end_date,target_words,baseline_words,time_zone,word_count_policy,success_metric,archived FROM writing_goal WHERE (?1=1 OR archived=0) ORDER BY start_date,id")?;
        let rows = query.query_map([include_archived], writing_goal_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn update_event_dependency(&mut self, dependency: &StoryEventDependency) -> Result<()> {
        if !matches!(dependency.kind.as_str(), "precedes" | "requires") {
            return Err(StorageError::Invalid(
                "event dependency kind must be precedes or requires".into(),
            ));
        }
        if self.conn.execute(
            "UPDATE story_event_dependency SET notes=?4 WHERE predecessor_event_id=?1 AND successor_event_id=?2 AND dependency_kind=?3",
            params![dependency.predecessor.to_string(), dependency.successor.to_string(), dependency.kind, dependency.notes],
        )? == 0 {
            return Err(StorageError::NotFound(format!(
                "event dependency {} -> {} ({})",
                dependency.predecessor, dependency.successor, dependency.kind
            )));
        }
        Ok(())
    }
    pub fn remove_event_dependency(
        &mut self,
        predecessor: EventId,
        successor: EventId,
        kind: &str,
    ) -> Result<()> {
        if self.conn.execute(
            "DELETE FROM story_event_dependency WHERE predecessor_event_id=?1 AND successor_event_id=?2 AND dependency_kind=?3",
            params![predecessor.to_string(), successor.to_string(), kind],
        )? == 0 {
            return Err(StorageError::NotFound(format!(
                "event dependency {predecessor} -> {successor} ({kind})"
            )));
        }
        Ok(())
    }
    pub fn add_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        if rule.first_location == rule.second_location {
            return Err(StorageError::Invalid(
                "an incompatibility rule must name two different locations".into(),
            ));
        }
        let (first, second) = if rule.first_location.to_string() < rule.second_location.to_string()
        {
            (rule.first_location, rule.second_location)
        } else {
            (rule.second_location, rule.first_location)
        };
        self.conn.execute(
            "INSERT INTO location_incompatibility_rule(id,first_location_id,second_location_id,description,enabled,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![rule.id.to_string(), first.to_string(), second.to_string(), rule.description, rule.enabled, now()],
        )?;
        Ok(())
    }
    pub fn list_location_incompatibility_rules(&self) -> Result<Vec<LocationIncompatibilityRule>> {
        let mut statement = self.conn.prepare(
            "SELECT id,first_location_id,second_location_id,description,enabled FROM location_incompatibility_rule ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, first, second, description, enabled) = row?;
            Ok::<LocationIncompatibilityRule, rusqlite::Error>(LocationIncompatibilityRule {
                id: ContinuityRuleId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                first_location: LocationId(Uuid::parse_str(&first).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                second_location: LocationId(Uuid::parse_str(&second).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                description,
                enabled,
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(StorageError::from)
    }
    pub fn update_location_incompatibility_rule(
        &mut self,
        rule: &LocationIncompatibilityRule,
    ) -> Result<()> {
        if rule.first_location == rule.second_location {
            return Err(StorageError::Invalid(
                "an incompatibility rule must name two different locations".into(),
            ));
        }
        let (first, second) = if rule.first_location.to_string() < rule.second_location.to_string()
        {
            (rule.first_location, rule.second_location)
        } else {
            (rule.second_location, rule.first_location)
        };
        if self.conn.execute(
            "UPDATE location_incompatibility_rule SET first_location_id=?2,second_location_id=?3,description=?4,enabled=?5 WHERE id=?1",
            params![rule.id.to_string(), first.to_string(), second.to_string(), rule.description, rule.enabled],
        )? == 0 {
            return Err(StorageError::NotFound(rule.id.to_string()));
        }
        Ok(())
    }
    pub fn remove_location_incompatibility_rule(&mut self, id: ContinuityRuleId) -> Result<()> {
        if self.conn.execute(
            "DELETE FROM location_incompatibility_rule WHERE id=?1",
            [id.to_string()],
        )? == 0
        {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }
    pub fn record_writing_goal_event(&mut self, event: &NewWritingGoalEvent) -> Result<String> {
        if !matches!(
            event.origin.as_str(),
            "editor" | "write-or-die" | "dictation" | "import" | "manual" | "reconciliation"
        ) || event.source_identifier.trim().is_empty()
            || event.source_fingerprint.trim().is_empty()
            || event.raw_delta < 0
        {
            return Err(StorageError::Invalid("invalid writing goal event".into()));
        }
        let goal = self.get_writing_goal(&event.goal_id)?;
        if goal.archived {
            return Err(StorageError::Invalid(
                "cannot add activity to an archived writing goal".into(),
            ));
        }
        if let Some(required_scene) = goal.scope_scene_id
            && event.scene_id != Some(required_scene)
        {
            return Err(StorageError::Invalid(
                "event scene is outside the writing goal scope".into(),
            ));
        }
        let offset = parse_goal_timezone_offset(&goal.time_zone)?;
        let local_date = goal_timestamp_date(event.occurred_at, offset)?;
        if local_date < goal.start_date || local_date > goal.end_date {
            return Err(StorageError::Invalid(
                "event timestamp falls outside the inclusive writing goal dates".into(),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id = insert_writing_goal_event(&tx, &goal, event)?;
        tx.commit()?;
        Ok(id)
    }
    pub fn record_writing_activity_for_goals(
        &mut self,
        activity: &NewWritingActivity,
    ) -> Result<usize> {
        let goals = self.list_writing_goals(false)?;
        let mut recorded = 0;
        for goal in goals {
            if goal
                .scope_scene_id
                .is_some_and(|required| Some(required) != activity.scene_id)
            {
                continue;
            }
            let offset = parse_goal_timezone_offset(&goal.time_zone)?;
            let local_date = goal_timestamp_date(activity.occurred_at, offset)?;
            if local_date < goal.start_date || local_date > goal.end_date {
                continue;
            }
            self.record_writing_goal_event(&NewWritingGoalEvent {
                goal_id: goal.id,
                occurred_at: activity.occurred_at,
                origin: activity.origin.clone(),
                source_identifier: activity.source_identifier.clone(),
                source_fingerprint: activity.source_fingerprint.clone(),
                scene_id: activity.scene_id,
                raw_delta: activity.raw_delta,
                real_delta: activity.real_delta,
                detail: activity.detail.clone(),
            })?;
            recorded += 1;
        }
        Ok(recorded)
    }
    pub fn list_writing_goal_events(&self, goal_id: &str) -> Result<Vec<WritingGoalEventRecord>> {
        let mut query = self.conn.prepare("SELECT id,goal_id,occurred_at,origin,source_identifier,source_fingerprint,scene_id,raw_delta,real_delta,reconciliation_state,detail FROM writing_goal_event WHERE goal_id=?1 ORDER BY occurred_at,id")?;
        let rows = query.query_map([goal_id], writing_goal_event_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn reconcile_writing_goal_event(
        &mut self,
        event_id: &str,
        expected_fingerprint: &str,
        corrected_raw_delta: i64,
        corrected_real_delta: i64,
        reason: &str,
    ) -> Result<String> {
        if expected_fingerprint.trim().is_empty()
            || corrected_raw_delta < 0
            || reason.trim().is_empty()
        {
            return Err(StorageError::Invalid(
                "event reconciliation requires an expected fingerprint, non-negative raw count, and reason".into(),
            ));
        }
        let original: WritingGoalEventRecord = self.conn.query_row(
            "SELECT id,goal_id,occurred_at,origin,source_identifier,source_fingerprint,scene_id,raw_delta,real_delta,reconciliation_state,detail FROM writing_goal_event WHERE id=?1",
            [event_id], writing_goal_event_from_row,
        ).optional()?.ok_or_else(|| StorageError::NotFound(format!("writing goal event {event_id}")))?;
        if original.source_fingerprint != expected_fingerprint
            || original.reconciliation_state != "current"
        {
            return Err(StorageError::Conflict(
                "writing goal event changed since it was inspected; reload before reconciliation"
                    .into(),
            ));
        }
        let goal = self.get_writing_goal(&original.goal_id)?;
        let corrected_fingerprint = hash_bytes(
            format!(
                "{}:{}:{}:{}:{}",
                original.source_fingerprint,
                corrected_raw_delta,
                corrected_real_delta,
                reason,
                original.id
            )
            .as_bytes(),
        );
        let replacement_id = Uuid::new_v4().to_string();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE writing_goal_event SET reconciliation_state='superseded' WHERE id=?1 AND source_fingerprint=?2 AND reconciliation_state='current'",
            params![event_id, expected_fingerprint],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict(
                "writing goal event changed during reconciliation".into(),
            ));
        }
        tx.execute(
            "INSERT INTO writing_goal_event(id,goal_id,occurred_at,origin,source_identifier,source_fingerprint,scene_id,raw_delta,real_delta,reconciliation_state,detail,created_at) VALUES(?1,?2,?3,'reconciliation',?4,?5,?6,?7,?8,'current',?9,?10)",
            params![replacement_id,original.goal_id,original.occurred_at,original.id,corrected_fingerprint,original.scene_id.map(|scene| scene.to_string()),corrected_raw_delta,corrected_real_delta,reason.trim(),now()],
        )?;
        refresh_writing_goal_snapshots(&tx, &goal, original.occurred_at)?;
        tx.commit()?;
        Ok(replacement_id)
    }
    pub fn writing_goal_progress(
        &self,
        goal_id: &str,
        as_of_timestamp: i64,
    ) -> Result<Vec<WritingGoalProgressPoint>> {
        let goal = self.get_writing_goal(goal_id)?;
        let offset = parse_goal_timezone_offset(&goal.time_zone)?;
        let as_of = goal_timestamp_date(as_of_timestamp, offset)?;
        let start = parse_goal_date(&goal.start_date)?;
        let end = parse_goal_date(&goal.end_date)?;
        let total_days = i64::from(end.to_julian_day() - start.to_julian_day() + 1);
        let mut points = Vec::new();
        let mut date = start;
        loop {
            let date_text = format_goal_date(date);
            let elapsed_days = i64::from(date.to_julian_day() - start.to_julian_day() + 1);
            let target_range = goal
                .target_words
                .checked_sub(goal.baseline_words)
                .ok_or_else(|| {
                    StorageError::Invalid("writing goal target range overflow".into())
                })?;
            let target = goal
                .baseline_words
                .checked_add(
                    target_range.checked_mul(elapsed_days).ok_or_else(|| {
                        StorageError::Invalid("writing goal target overflow".into())
                    })? / total_days,
                )
                .ok_or_else(|| StorageError::Invalid("writing goal baseline overflow".into()))?;
            let projected = date_text > as_of;
            let (raw_words, real_words) = if projected {
                (None, None)
            } else {
                let end_timestamp = goal_day_start(date, offset)?
                    .checked_add(86_400)
                    .ok_or_else(|| StorageError::Invalid("writing goal day overflow".into()))?;
                let (raw, real): (i64, i64) = self.conn.query_row(
                    "SELECT COALESCE(SUM(raw_delta),0),COALESCE(SUM(real_delta),0) FROM writing_goal_event WHERE goal_id=?1 AND occurred_at < ?2 AND reconciliation_state='current'",
                    params![goal_id,end_timestamp],
                    |row| Ok((row.get(0)?,row.get(1)?)),
                )?;
                (
                    Some(goal.baseline_words.checked_add(raw).ok_or_else(|| {
                        StorageError::Invalid("writing goal raw count overflow".into())
                    })?),
                    Some(goal.baseline_words.checked_add(real).ok_or_else(|| {
                        StorageError::Invalid("writing goal real count overflow".into())
                    })?),
                )
            };
            points.push(WritingGoalProgressPoint {
                date: date_text,
                target_words: target,
                raw_words,
                real_words,
                projected,
            });
            if date >= end {
                break;
            }
            date = date
                .next_day()
                .ok_or_else(|| StorageError::Invalid("writing goal date overflow".into()))?;
        }
        Ok(points)
    }
    fn get_writing_goal(&self, id: &str) -> Result<WritingGoalRecord> {
        self.conn.query_row(
            "SELECT id,title,scope_type,scope_scene_id,start_date,end_date,target_words,baseline_words,time_zone,word_count_policy,success_metric,archived FROM writing_goal WHERE id=?1",
            [id],
            writing_goal_from_row,
        ).optional()?.ok_or_else(|| StorageError::NotFound(format!("writing goal {id}")))
    }
    pub fn start_writing_challenge(
        &mut self,
        target_words: u32,
        duration_seconds: u32,
        mode: &str,
    ) -> Result<String> {
        if target_words == 0
            || duration_seconds == 0
            || !matches!(mode, "steady" | "pressure" | "kamikaze")
        {
            return Err(StorageError::Invalid(
                "challenge requires positive target/duration and mode pressure|kamikaze".into(),
            ));
        }
        let started = unix_now();
        let deadline = started
            .checked_add(i64::from(duration_seconds))
            .ok_or_else(|| StorageError::Invalid("challenge deadline overflow".into()))?;
        let id = Uuid::new_v4().to_string();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE writing_challenge SET status='abandoned',outcome='quit',ended_at=?1 WHERE status='active'",
            [started],
        )?;
        let stored_mode = if mode == "pressure" { "steady" } else { mode };
        tx.execute("INSERT INTO writing_challenge(id,started_at,deadline_at,target_words,mode,status,display_mode) VALUES(?1,?2,?3,?4,?5,'active',?6)", params![id,started,deadline,target_words,stored_mode,mode])?;
        tx.commit()?;
        Ok(id)
    }
    pub fn finish_writing_challenge(
        &mut self,
        id: &str,
        words_written: u32,
        abandoned: bool,
    ) -> Result<()> {
        self.finish_writing_challenge_detailed(
            id,
            words_written,
            0,
            0,
            if abandoned { "quit" } else { "timeout" },
        )
    }
    pub fn finish_writing_challenge_detailed(
        &mut self,
        id: &str,
        words_written: u32,
        words_deleted: u32,
        pauses_consumed: u32,
        outcome: &str,
    ) -> Result<()> {
        self.finish_writing_challenge_with_activity(
            id,
            words_written,
            words_deleted,
            pauses_consumed,
            outcome,
            None,
        )
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
        if !matches!(outcome, "success" | "timeout" | "quit") {
            return Err(StorageError::Invalid("invalid challenge outcome".into()));
        }
        let ended_at = unix_now();
        let status = if outcome == "quit" {
            "abandoned"
        } else {
            "completed"
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE writing_challenge SET ended_at=?2,words_written=?3,words_deleted=?4,pauses_consumed=?5,outcome=?6,status=?7 WHERE id=?1 AND status='active'",
            params![id, ended_at, words_written, words_deleted, pauses_consumed, outcome, status],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(format!(
                "active writing challenge {id}"
            )));
        }
        let goals = {
            let mut query = tx.prepare("SELECT id,title,scope_type,scope_scene_id,start_date,end_date,target_words,baseline_words,time_zone,word_count_policy,success_metric,archived FROM writing_goal WHERE archived=0 ORDER BY start_date,id")?;
            query
                .query_map([], writing_goal_from_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let fingerprint = hash_bytes(
            format!("{id}:{ended_at}:{words_written}:{words_deleted}:{pauses_consumed}:{outcome}")
                .as_bytes(),
        );
        for goal in goals {
            if goal
                .scope_scene_id
                .is_some_and(|required| Some(required) != scene_id)
            {
                continue;
            }
            let offset = parse_goal_timezone_offset(&goal.time_zone)?;
            let date = goal_timestamp_date(ended_at, offset)?;
            if date < goal.start_date || date > goal.end_date {
                continue;
            }
            let event = NewWritingGoalEvent {
                goal_id: goal.id.clone(),
                occurred_at: ended_at,
                origin: "write-or-die".into(),
                source_identifier: id.into(),
                source_fingerprint: fingerprint.clone(),
                scene_id,
                raw_delta: i64::from(words_written) + i64::from(words_deleted),
                real_delta: 0,
                detail: format!("Isolated timed draft finished ({outcome})"),
            };
            let _ = insert_writing_goal_event(&tx, &goal, &event)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn recover_interrupted_writing_challenges(&mut self) -> Result<u64> {
        let changed = self.conn.execute(
            "UPDATE writing_challenge SET status='abandoned',ended_at=?1,outcome='quit' WHERE status='active'",
            [unix_now()],
        )?;
        Ok(changed as u64)
    }
    pub fn list_writing_challenges(&self) -> Result<Vec<WritingChallengeRecord>> {
        let mut query = self.conn.prepare("SELECT id,started_at,deadline_at,target_words,display_mode,ended_at,words_written,status,words_deleted,pauses_consumed,outcome FROM writing_challenge ORDER BY started_at DESC,id DESC")?;
        let rows = query.query_map([], |row| {
            Ok(WritingChallengeRecord {
                id: row.get(0)?,
                started_at: row.get(1)?,
                deadline_at: row.get(2)?,
                target_words: row.get(3)?,
                mode: row.get(4)?,
                ended_at: row.get(5)?,
                words_written: row.get(6)?,
                status: row.get(7)?,
                words_deleted: row.get(8)?,
                pauses_consumed: row.get(9)?,
                outcome: row.get(10)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn start_writing_session(
        &mut self,
        scene_id: Option<SceneId>,
        start_words: u32,
    ) -> Result<String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let started = unix_now();
        tx.execute(
            "UPDATE writing_session SET ended_at=?1,end_words=start_words,words_delta=0,status='abandoned' WHERE status='active'",
            params![started],
        )?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO writing_session(id,started_at,scene_id,start_words,status) VALUES(?1,?2,?3,?4,'active')",
            params![id, started, scene_id.map(|value| value.to_string()), start_words],
        )?;
        tx.commit()?;
        Ok(id)
    }
    pub fn finish_writing_session(
        &mut self,
        id: &str,
        end_words: u32,
        abandoned: bool,
    ) -> Result<()> {
        let status = if abandoned { "abandoned" } else { "completed" };
        let ended_at = unix_now();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (started_at, scene_text, start_words): (i64, Option<String>, u32) = tx
            .query_row(
                "SELECT started_at,scene_id,start_words FROM writing_session WHERE id=?1 AND status='active'",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(format!("active writing session {id}")))?;
        let scene_id = scene_text
            .map(|value| value.parse::<Uuid>().map(SceneId))
            .transpose()
            .map_err(|error| {
                StorageError::Invalid(format!("invalid writing session scene: {error}"))
            })?;
        let delta = i64::from(end_words) - i64::from(start_words);
        tx.execute(
            "UPDATE writing_session SET ended_at=?2,end_words=?3,words_delta=?3-start_words,status=?4 WHERE id=?1 AND status='active'",
            params![id, ended_at, end_words, status],
        )?;
        if !abandoned {
            let raw_delta = delta.max(0);
            if raw_delta != 0 || delta != 0 {
                let goals = {
                    let mut query = tx.prepare("SELECT id,title,scope_type,scope_scene_id,start_date,end_date,target_words,baseline_words,time_zone,word_count_policy,success_metric,archived FROM writing_goal WHERE archived=0 ORDER BY start_date,id")?;
                    query
                        .query_map([], writing_goal_from_row)?
                        .collect::<std::result::Result<Vec<_>, _>>()?
                };
                let fingerprint =
                    hash_bytes(format!("{id}:{started_at}:{ended_at}:{delta}").as_bytes());
                for goal in goals {
                    if goal
                        .scope_scene_id
                        .is_some_and(|required| Some(required) != scene_id)
                    {
                        continue;
                    }
                    let offset = parse_goal_timezone_offset(&goal.time_zone)?;
                    let date = goal_timestamp_date(ended_at, offset)?;
                    if date < goal.start_date || date > goal.end_date {
                        continue;
                    }
                    let event = NewWritingGoalEvent {
                        goal_id: goal.id.clone(),
                        occurred_at: ended_at,
                        origin: "editor".into(),
                        source_identifier: id.into(),
                        source_fingerprint: fingerprint.clone(),
                        scene_id,
                        raw_delta,
                        real_delta: delta,
                        detail: "Completed editor writing session".into(),
                    };
                    let _ = insert_writing_goal_event(&tx, &goal, &event)?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn list_writing_sessions(&self) -> Result<Vec<WritingSessionRecord>> {
        let mut query = self.conn.prepare(
            "SELECT id,started_at,ended_at,scene_id,start_words,end_words,words_delta,status FROM writing_session ORDER BY started_at DESC",
        )?;
        let rows = query.query_map([], |row| {
            let scene_id: Option<String> = row.get(3)?;
            Ok(WritingSessionRecord {
                id: row.get(0)?,
                started_at: row.get(1)?,
                ended_at: row.get(2)?,
                scene_id: scene_id
                    .map(|value| value.parse::<Uuid>().map(SceneId))
                    .transpose()
                    .map_err(|error: uuid::Error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                start_words: row.get(4)?,
                end_words: row.get(5)?,
                words_delta: row.get(6)?,
                status: row.get(7)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
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
    pub fn expand_document_inclusions(&self, root: EntityRef) -> Result<Vec<TranscludedPassage>> {
        fn visit(
            store: &ProjectStore,
            entity: EntityRef,
            depth: usize,
            seen: &mut std::collections::BTreeSet<(String, String)>,
            output: &mut Vec<TranscludedPassage>,
        ) -> Result<()> {
            let key = (entity.kind().to_owned(), entity.id());
            if !seen.insert(key.clone()) {
                return Err(StorageError::Invalid(
                    "transclusion cycle encountered during rendering".into(),
                ));
            }
            let text = match entity {
                EntityRef::Scene(scene) => store.load_text(scene)?.0,
                EntityRef::Research(research) => store.load_research_text(research)?,
                EntityRef::Reading(reading) => store.get_reading_document(reading)?.extracted_text,
                _ => {
                    return Err(StorageError::Invalid(
                        "inclusion target is not a text document".into(),
                    ));
                }
            };
            output.push(TranscludedPassage {
                source: entity,
                text,
                depth,
            });
            let mut q=store.conn.prepare("SELECT target_kind,target_id FROM story_link WHERE source_kind=?1 AND source_id=?2 AND link_kind IN('transcludes','includes') ORDER BY created_at,id")?;
            let targets = q
                .query_map(params![entity.kind(), entity.id()], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (kind, target) in targets {
                let uuid =
                    Uuid::parse_str(&target).map_err(|e| StorageError::Invalid(e.to_string()))?;
                let entity = match kind.as_str() {
                    "scene" => EntityRef::Scene(SceneId(uuid)),
                    "research" => EntityRef::Research(ResearchId(uuid)),
                    "reading" => EntityRef::Reading(ReadingId(uuid)),
                    _ => {
                        return Err(StorageError::Invalid(format!(
                            "unsupported inclusion document kind: {kind}"
                        )));
                    }
                };
                visit(store, entity, depth + 1, seen, output)?;
            }
            seen.remove(&key);
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
    pub fn expand_scene_transclusions(&self, root: SceneId) -> Result<Vec<TranscludedPassage>> {
        self.expand_document_inclusions(EntityRef::Scene(root))
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
        if document
            .language_code
            .as_deref()
            .is_some_and(|tag| !is_valid_language_tag(tag))
        {
            let _ = fs::remove_file(&target);
            return Err(StorageError::Invalid(
                "invalid reading document language tag".into(),
            ));
        }
        let result=self.conn.execute("INSERT INTO reading_document(id,title,media_type,relative_path,source_identifier,source_sha256,extracted_text,imported_at,updated_at,language_code) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,?9)",params![id.to_string(),document.title,document.media_type,relative,source_identifier,expected,document.extracted_text,stamp,document.language_code]);
        if let Err(error) = result {
            let _ = fs::remove_file(&target);
            return Err(error.into());
        }
        Ok(id)
    }
    pub fn get_reading_document(&self, id: ReadingId) -> Result<ReadingDocumentRecord> {
        let row:(String,String,String,String,String,String,Option<String>)=self.conn.query_row("SELECT title,media_type,relative_path,source_identifier,source_sha256,extracted_text,language_code FROM reading_document WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional()?.ok_or_else(||StorageError::NotFound(id.to_string()))?;
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
            language_code: row.6,
        })
    }
    pub fn set_reading_document_language(
        &mut self,
        id: ReadingId,
        language_code: Option<&str>,
    ) -> Result<()> {
        if language_code.is_some_and(|tag| !is_valid_language_tag(tag)) {
            return Err(StorageError::Invalid(
                "invalid reading document language tag".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE reading_document SET language_code=?2,updated_at=?3 WHERE id=?1",
            params![id.to_string(), language_code, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
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
        tx.execute("UPDATE reading_document SET title=?2,media_type=?3,relative_path=?4,source_sha256=?5,extracted_text=?6,language_code=?7,updated_at=?8 WHERE id=?1",params![id.to_string(),document.title,document.media_type,relative,expected,document.extracted_text,document.language_code,now()])?;
        if existing.source_sha256 != expected {
            let annotations: Vec<(String, String, String, String)> = {
                let mut q =
                    tx.prepare("SELECT id,quote,prefix_context,suffix_context FROM reading_annotation WHERE document_id=?1")?;
                q.query_map([id.to_string()], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })?
                .collect::<std::result::Result<_, _>>()?
            };
            for (annotation, quote, prefix, suffix) in annotations {
                let found = unique_contextual_occurrence(
                    &document.extracted_text,
                    &quote,
                    &prefix,
                    &suffix,
                );
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
    pub fn create_personal_comment(
        &mut self,
        scene: SceneId,
        start: u64,
        end: u64,
        quote: &str,
        body: &str,
    ) -> Result<PersonalCommentId> {
        if body.trim().is_empty() {
            return Err(StorageError::Invalid("comment text cannot be empty".into()));
        }
        let (text, hash) = self.load_text(scene)?;
        let start_usize = usize::try_from(start)
            .map_err(|_| StorageError::Invalid("comment offset overflow".into()))?;
        let end_usize = usize::try_from(end)
            .map_err(|_| StorageError::Invalid("comment offset overflow".into()))?;
        if quote.is_empty()
            || start_usize > end_usize
            || end_usize > text.len()
            || !text.is_char_boundary(start_usize)
            || !text.is_char_boundary(end_usize)
            || text.get(start_usize..end_usize) != Some(quote)
        {
            return Err(StorageError::Invalid(
                "comment offsets must select the supplied UTF-8 passage".into(),
            ));
        }
        let prefix = &text[..start_usize];
        let suffix = &text[end_usize..];
        let prefix = prefix
            .chars()
            .rev()
            .take(32)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        let suffix = suffix.chars().take(32).collect::<String>();
        let anchor_id = AnchorId::new();
        let comment_id = PersonalCommentId::new();
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO passage_anchor(id,scene_id,start_byte,end_byte,quote,prefix_context,suffix_context,source_sha256,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'current',?9,?9)",
            params![anchor_id.to_string(), scene.to_string(), start, end, quote, prefix, suffix, hash, now()],
        )?;
        transaction.execute(
            "INSERT INTO personal_comment(id,scene_id,anchor_id,body,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",
            params![comment_id.to_string(), scene.to_string(), anchor_id.to_string(), body.trim(), now()],
        )?;
        transaction.commit()?;
        Ok(comment_id)
    }

    pub fn list_scene_comments(&self, scene: SceneId) -> Result<Vec<PersonalCommentRecord>> {
        let mut query = self.conn.prepare(
            "SELECT c.id,c.anchor_id,c.body,c.state,a.quote,a.start_byte,a.end_byte,a.state FROM personal_comment c LEFT JOIN passage_anchor a ON a.id=c.anchor_id WHERE c.scene_id=?1 ORDER BY COALESCE(a.start_byte,0),c.created_at,c.id",
        )?;
        let rows = query.query_map([scene.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<u64>>(5)?,
                row.get::<_, Option<u64>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        rows.map(|row| {
            let (id, anchor, body, state, quote, start_byte, end_byte, anchor_state) = row?;
            Ok(PersonalCommentRecord {
                id: PersonalCommentId(Uuid::parse_str(&id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?),
                scene_id: scene,
                anchor_id: anchor
                    .map(|value| {
                        Uuid::parse_str(&value).map(AnchorId).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })
                    })
                    .transpose()?,
                quote,
                start_byte,
                end_byte,
                anchor_state,
                body,
                state,
            })
        })
        .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()
        .map_err(StorageError::from)
    }

    pub fn set_personal_comment_state(
        &mut self,
        comment: PersonalCommentId,
        state: &str,
    ) -> Result<()> {
        if !matches!(state, "open" | "resolved") {
            return Err(StorageError::Invalid(
                "comment state must be open or resolved".into(),
            ));
        }
        let changed = self.conn.execute(
            "UPDATE personal_comment SET state=?2,updated_at=?3 WHERE id=?1",
            params![comment.to_string(), state, now()],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound(comment.to_string()));
        }
        Ok(())
    }
    pub fn outgoing_links(&self, source: EntityRef) -> Result<Vec<StoryLink>> {
        let mut q = self.conn.prepare("SELECT id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label FROM story_link WHERE source_kind=?1 AND source_id=?2 ORDER BY created_at,id")?;
        let rows = q.query_map(params![source.kind(), source.id()], read_story_link)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn broken_links(&self) -> Result<Vec<BrokenLink>> {
        let mut q = self.conn.prepare(
            "SELECT l.id,
             CASE WHEN (l.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.source_id))
                    OR (l.source_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.source_id))
                    OR (l.source_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.source_id))
                    OR (l.source_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.source_id))
                    OR (l.source_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=l.source_id)) THEN 'missing source'
                  WHEN (l.target_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.target_id))
                    OR (l.target_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.target_id))
                    OR (l.target_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.target_id))
                    OR (l.target_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.target_id))
                    OR (l.target_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=l.target_id)) THEN 'missing target'
                  ELSE 'missing passage anchor' END
             FROM story_link l
             WHERE (l.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.source_id))
                OR (l.source_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.source_id))
                OR (l.source_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.source_id))
                OR (l.source_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.source_id))
                OR (l.source_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=l.source_id))
                OR (l.target_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=l.target_id))
                OR (l.target_kind='character' AND NOT EXISTS(SELECT 1 FROM character WHERE id=l.target_id))
                OR (l.target_kind='plotline' AND NOT EXISTS(SELECT 1 FROM plotline WHERE id=l.target_id))
                OR (l.target_kind='research' AND NOT EXISTS(SELECT 1 FROM research_item WHERE id=l.target_id))
                OR (l.target_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=l.target_id))
                OR (l.source_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=l.source_anchor_id))
                OR (l.target_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=l.target_anchor_id))
             ORDER BY l.id",
        )?;
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
        for filter in &query.filters {
            validate_custom_metadata_filter(&self.conn, filter)?;
            if let SceneQueryFilter::CustomMetadataContainsEntity {
                field_id,
                target_kind,
                target_id,
            } = filter
            {
                let (kind, reference_kind): (String, Option<String>) = self
                    .conn
                    .query_row(
                        "SELECT field_type,reference_kind FROM custom_field_definition WHERE id=?1",
                        [field_id.to_string()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(|_| StorageError::NotFound(field_id.to_string()))?;
                if kind != "entity_refs" || reference_kind.as_deref() != Some(target_kind.as_str())
                {
                    return Err(StorageError::Invalid(
                        "entity membership filter must match an entity-reference-list field kind"
                            .into(),
                    ));
                }
                validate_entity_ref(&self.conn, Some(target_kind), target_id)?;
            } else if let SceneQueryFilter::CustomMetadataEquals { field_id, value } = filter {
                if matches!(value, FieldValue::EntityRefs(_)) {
                    return Err(StorageError::Invalid(
                        "entity-reference lists cannot be compared for equality".into(),
                    ));
                }
                let (field_type, enum_values, reference_kind): (String, String, Option<String>) = self.conn.query_row(
                    "SELECT field_type,enum_values,reference_kind FROM custom_field_definition WHERE id=?1",
                    [field_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                ).map_err(|_| StorageError::NotFound(field_id.to_string()))?;
                let definition = CustomFieldDefinition {
                    id: *field_id,
                    name: String::new(),
                    field_type: parse_field_type(&field_type)?,
                    required: false,
                    default_value: None,
                    enum_values: enum_values.lines().map(str::to_owned).collect(),
                    reference_kind,
                };
                definition
                    .validate(value)
                    .map_err(|error| StorageError::Invalid(error.to_string()))?;
            }
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
                SceneQueryFilter::ExcludeStatus(status) => {
                    ("exclude_status", status.as_str().to_owned())
                }
                SceneQueryFilter::Character(id) => ("character", id.to_string()),
                SceneQueryFilter::CharacterPresent(id) => ("character_present", id.to_string()),
                SceneQueryFilter::CharacterMentioned(id) => ("character_mentioned", id.to_string()),
                SceneQueryFilter::CharacterAbsent(id) => ("character_absent", id.to_string()),
                SceneQueryFilter::AnyCharacter(ids) => {
                    if ids.is_empty() {
                        return Err(StorageError::Invalid(
                            "character-any query filter cannot be empty".into(),
                        ));
                    }
                    (
                        "character_any",
                        ids.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    )
                }
                SceneQueryFilter::Plotline(id) => ("plotline", id.to_string()),
                SceneQueryFilter::AnyPlotline(ids) => {
                    if ids.is_empty() {
                        return Err(StorageError::Invalid(
                            "plotline-any query filter cannot be empty".into(),
                        ));
                    }
                    (
                        "plotline_any",
                        ids.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    )
                }
                SceneQueryFilter::Location(id) => ("location", id.to_string()),
                SceneQueryFilter::Event(id) => ("event", id.to_string()),
                SceneQueryFilter::Assembly(id) => ("assembly", id.to_string()),
                SceneQueryFilter::IssueStatus(status) => {
                    if !valid_issue_status(status) {
                        return Err(StorageError::Invalid(format!(
                            "invalid issue status query filter: {status}"
                        )));
                    }
                    ("issue_status", status.clone())
                }
                SceneQueryFilter::Chapter(id) => ("chapter", id.clone()),
                SceneQueryFilter::TextContains(text) => {
                    if text.trim().is_empty() {
                        return Err(StorageError::Invalid("text query cannot be empty".into()));
                    }
                    ("text_contains", text.clone())
                }
                SceneQueryFilter::CustomMetadataEquals { field_id, value } => (
                    "custom_metadata_equals",
                    serde_json::to_string(&(field_id, value))
                        .map_err(|error| StorageError::Invalid(error.to_string()))?,
                ),
                SceneQueryFilter::CustomMetadataContainsEntity {
                    field_id,
                    target_kind,
                    target_id,
                } => (
                    "custom_metadata_contains_entity",
                    serde_json::to_string(&(field_id, target_kind, target_id))
                        .map_err(|error| StorageError::Invalid(error.to_string()))?,
                ),
                SceneQueryFilter::CustomMetadataExists { field_id } => {
                    ("custom_metadata_exists", field_id.to_string())
                }
                SceneQueryFilter::CustomMetadataTextContains { field_id, text } => (
                    "custom_metadata_text_contains",
                    serde_json::to_string(&(field_id, text))
                        .map_err(|error| StorageError::Invalid(error.to_string()))?,
                ),
                SceneQueryFilter::CustomMetadataNumberCompare {
                    field_id,
                    comparison,
                    value,
                } => (
                    "custom_metadata_number_compare",
                    serde_json::to_string(&(field_id, comparison, value))
                        .map_err(|error| StorageError::Invalid(error.to_string()))?,
                ),
                SceneQueryFilter::TimeBefore(tick) => ("time_before", tick.to_string()),
                SceneQueryFilter::TimeAfter(tick) => ("time_after", tick.to_string()),
                SceneQueryFilter::TimeBeforeDefinitely(tick) => {
                    ("time_before_definitely", tick.to_string())
                }
                SceneQueryFilter::TimeBeforePossibly(tick) => {
                    ("time_before_possibly", tick.to_string())
                }
                SceneQueryFilter::TimeAfterDefinitely(tick) => {
                    ("time_after_definitely", tick.to_string())
                }
                SceneQueryFilter::TimeAfterPossibly(tick) => {
                    ("time_after_possibly", tick.to_string())
                }
                SceneQueryFilter::UnresolvedIssue => ("unresolved_issue", "1".into()),
            };
            tx.execute("INSERT INTO saved_query_filter(query_id,ordinal,filter_kind,value) VALUES(?1,?2,?3,?4)",params![query.id.to_string(),index as i64,kind,value])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn set_scene_query_expression(
        &mut self,
        query: QueryId,
        expression: Option<&QueryExpression>,
    ) -> Result<()> {
        if let Some(expression) = expression {
            validate_query_expression(expression)?;
            validate_custom_metadata_expression(&self.conn, expression)?;
        }
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM saved_query WHERE id=?1)",
            [query.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::NotFound(query.to_string()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(expression) = expression {
            let encoded = serde_json::to_string(expression)
                .map_err(|error| StorageError::Invalid(error.to_string()))?;
            tx.execute(
                "INSERT INTO saved_query_expression(query_id,expression_json,updated_at) VALUES(?1,?2,?3) ON CONFLICT(query_id) DO UPDATE SET expression_json=excluded.expression_json,updated_at=excluded.updated_at",
                params![query.to_string(), encoded, now()],
            )?;
        } else {
            tx.execute(
                "DELETE FROM saved_query_expression WHERE query_id=?1",
                [query.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_scene_query_expression(&self, query: QueryId) -> Result<Option<QueryExpression>> {
        let encoded: Option<String> = self
            .conn
            .query_row(
                "SELECT expression_json FROM saved_query_expression WHERE query_id=?1",
                [query.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        encoded
            .map(|encoded| {
                let expression: QueryExpression = serde_json::from_str(&encoded)
                    .map_err(|error| StorageError::Invalid(error.to_string()))?;
                validate_query_expression(&expression)?;
                Ok(expression)
            })
            .transpose()
    }
    pub fn set_query_assertions(
        &mut self,
        query: QueryId,
        assertions: &[QueryAssertion],
    ) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM saved_query WHERE id=?1)",
            [query.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::NotFound(query.to_string()));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM saved_query_assertion WHERE query_id=?1",
            [query.to_string()],
        )?;
        for (ordinal, assertion) in assertions.iter().enumerate() {
            let (kind, threshold) = match assertion {
                QueryAssertion::AtLeast(value) => (
                    "at_least",
                    Some(i64::try_from(*value).map_err(|_| {
                        StorageError::Invalid(
                            "assertion threshold exceeds SQLite integer range".into(),
                        )
                    })?),
                ),
                QueryAssertion::AtMost(value) => (
                    "at_most",
                    Some(i64::try_from(*value).map_err(|_| {
                        StorageError::Invalid(
                            "assertion threshold exceeds SQLite integer range".into(),
                        )
                    })?),
                ),
                QueryAssertion::Exactly(value) => (
                    "exactly",
                    Some(i64::try_from(*value).map_err(|_| {
                        StorageError::Invalid(
                            "assertion threshold exceeds SQLite integer range".into(),
                        )
                    })?),
                ),
                QueryAssertion::NonEmpty => ("non_empty", None),
                QueryAssertion::Empty => ("empty", None),
            };
            tx.execute("INSERT INTO saved_query_assertion(query_id,ordinal,assertion_kind,threshold) VALUES(?1,?2,?3,?4)", params![query.to_string(), ordinal as i64, kind, threshold])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_query_assertions(&self, query: QueryId) -> Result<Vec<QueryAssertion>> {
        let mut statement = self.conn.prepare("SELECT assertion_kind,threshold FROM saved_query_assertion WHERE query_id=?1 ORDER BY ordinal")?;
        let rows = statement.query_map([query.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })?;
        rows.map(|row| {
            let (kind, threshold) = row?;
            let numeric = || {
                threshold
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or_else(|| {
                        StorageError::Invalid(format!("invalid assertion threshold for {kind}"))
                    })
            };
            Ok(match kind.as_str() {
                "at_least" => QueryAssertion::AtLeast(numeric()?),
                "at_most" => QueryAssertion::AtMost(numeric()?),
                "exactly" => QueryAssertion::Exactly(numeric()?),
                "non_empty" if threshold.is_none() => QueryAssertion::NonEmpty,
                "empty" if threshold.is_none() => QueryAssertion::Empty,
                _ => {
                    return Err(StorageError::Invalid(format!(
                        "invalid saved query assertion {kind}"
                    )));
                }
            })
        })
        .collect()
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
                "exclude_status" => { sql.push_str(" AND s.status<>?"); values.push(value); }
                "character" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=?)"); values.push(value); }
                "character_present" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=? AND sc.participation_role IN('pov','major','minor','off_camera'))"); values.push(value); }
                "character_mentioned" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id=? AND sc.participation_role='mentioned')"); values.push(value); }
                "character_absent" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_character_absence a WHERE a.scene_id=s.id AND a.character_id=?)"); values.push(value); }
                "character_any" => {
                    let ids = value.split(',').filter(|id| !id.is_empty()).collect::<Vec<_>>();
                    if ids.is_empty() { return Err(StorageError::Invalid("empty character-any query filter".into())); }
                    sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM scene_character sc WHERE sc.scene_id=s.id AND sc.character_id IN ({}))", vec!["?"; ids.len()].join(",")));
                    values.extend(ids.into_iter().map(str::to_owned));
                }
                "plotline" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_plotline sp WHERE sp.scene_id=s.id AND sp.plotline_id=?)"); values.push(value); }
                "location" => { sql.push_str(" AND (s.start_location_id=? OR s.end_location_id=?)"); values.push(value.clone()); values.push(value); }
                "event" => { sql.push_str(" AND EXISTS(SELECT 1 FROM scene_event se WHERE se.scene_id=s.id AND se.event_id=?)"); values.push(value); }
                "assembly" => { sql.push_str(" AND EXISTS(SELECT 1 FROM assembly_item ai WHERE ai.scene_id=s.id AND ai.assembly_id=? AND ai.included=1)"); values.push(value); }
                "issue_status" => {
                    if !valid_issue_status(&value) { return Err(StorageError::Invalid(format!("invalid issue status query filter: {value}"))); }
                    sql.push_str(" AND EXISTS(SELECT 1 FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=s.id AND i.status=?)"); values.push(value);
                }
                "chapter" => { sql.push_str(" AND p.chapter_id=?"); values.push(value); }
                "text_contains" => {
                    let pattern = format!("%{}%", value.replace('%', "\\%").replace('_', "\\_"));
                    sql.push_str(" AND (s.title LIKE ? ESCAPE '\\' OR s.synopsis LIKE ? ESCAPE '\\' OR EXISTS(SELECT 1 FROM scene_search_content sc WHERE sc.scene_id=s.id AND sc.content LIKE ? ESCAPE '\\'))");
                    values.extend([pattern.clone(), pattern.clone(), pattern]);
                }
                "custom_metadata_equals" => {
                    let (field_id, value): (FieldId, FieldValue) = serde_json::from_str(&value)
                        .map_err(|error| StorageError::Invalid(format!("invalid custom metadata query: {error}")))?;
                    let expression = QueryExpression::Filter(SceneQueryFilter::CustomMetadataEquals { field_id, value });
                    sql.push_str(" AND ");
                    sql.push_str(&compile_query_expression(&expression, &mut values)?);
                }
                "custom_metadata_contains_entity" => {
                    let (field_id, target_kind, target_id) = serde_json::from_str(&value)
                        .map_err(|error| StorageError::Invalid(format!("invalid custom entity query: {error}")))?;
                    let expression = QueryExpression::Filter(SceneQueryFilter::CustomMetadataContainsEntity { field_id, target_kind, target_id });
                    sql.push_str(" AND ");
                    sql.push_str(&compile_query_expression(&expression, &mut values)?);
                }
                "custom_metadata_exists" => {
                    let field_id = FieldId(Uuid::parse_str(&value)
                        .map_err(|error| StorageError::Invalid(format!("invalid custom field id: {error}")))?);
                    let expression = QueryExpression::Filter(SceneQueryFilter::CustomMetadataExists { field_id });
                    sql.push_str(" AND ");
                    sql.push_str(&compile_query_expression(&expression, &mut values)?);
                }
                "custom_metadata_text_contains" => {
                    let (field_id, text): (FieldId, String) = serde_json::from_str(&value)
                        .map_err(|error| StorageError::Invalid(format!("invalid custom metadata text query: {error}")))?;
                    let expression = QueryExpression::Filter(SceneQueryFilter::CustomMetadataTextContains { field_id, text });
                    sql.push_str(" AND ");
                    sql.push_str(&compile_query_expression(&expression, &mut values)?);
                }
                "custom_metadata_number_compare" => {
                    let (field_id, comparison, value): (FieldId, MetadataComparison, FieldValue) = serde_json::from_str(&value)
                        .map_err(|error| StorageError::Invalid(format!("invalid custom metadata numeric query: {error}")))?;
                    let expression = QueryExpression::Filter(SceneQueryFilter::CustomMetadataNumberCompare { field_id, comparison, value });
                    sql.push_str(" AND ");
                    sql.push_str(&compile_query_expression(&expression, &mut values)?);
                }
                "plotline_any" => {
                    let ids = value.split(',').filter(|id| !id.is_empty()).collect::<Vec<_>>();
                    if ids.is_empty() { return Err(StorageError::Invalid("empty plotline-any query filter".into())); }
                    sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM scene_plotline sp WHERE sp.scene_id=s.id AND sp.plotline_id IN ({}))", vec!["?"; ids.len()].join(",")));
                    values.extend(ids.into_iter().map(str::to_owned));
                }
                "time_before" => { sql.push_str(" AND s.start_nominal IS NOT NULL AND s.start_nominal<?"); values.push(value); }
                "time_after" => { sql.push_str(" AND s.start_nominal IS NOT NULL AND s.start_nominal>?"); values.push(value); }
                "time_before_definitely" => { sql.push_str(" AND s.start_upper IS NOT NULL AND s.start_upper<?"); values.push(value); }
                "time_before_possibly" => { sql.push_str(" AND s.start_lower IS NOT NULL AND s.start_lower<?"); values.push(value); }
                "time_after_definitely" => { sql.push_str(" AND s.start_lower IS NOT NULL AND s.start_lower>?"); values.push(value); }
                "time_after_possibly" => { sql.push_str(" AND s.start_upper IS NOT NULL AND s.start_upper>?"); values.push(value); }
                "unresolved_issue" => sql.push_str(" AND EXISTS(SELECT 1 FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=s.id AND i.status IN('open','in_progress'))"),
                _ => return Err(StorageError::Invalid(format!("unknown saved query filter: {kind}"))),
            }
        }
        if let Some(expression) = self.get_scene_query_expression(query)? {
            let compiled = compile_query_expression(&expression, &mut values)?;
            sql.push_str(" AND ");
            sql.push_str(&compiled);
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
    /// Return the stable story-model sources attached to a scene that currently
    /// satisfies the saved query. Positive literal text predicates additionally
    /// receive byte ranges in the verified canonical manuscript file.
    pub fn explain_scene_query_match(
        &self,
        query: QueryId,
        scene: SceneId,
    ) -> Result<Vec<QueryEvidence>> {
        if !self.run_scene_query(query)?.contains(&scene) {
            return Err(StorageError::Invalid(
                "scene does not currently match the saved query".into(),
            ));
        }
        let (prose, _current_hash) = self.load_text(scene)?;
        let mut statement = self.conn.prepare(
            "SELECT 'scene',s.id,'title: '||s.title||'; status: '||s.status||'; sha256: '||c.sha256
               FROM scene s JOIN scene_content c ON c.scene_id=s.id WHERE s.id=?1
             UNION ALL SELECT 'character',c.id,c.name||' ('||sc.participation_role||')'
               FROM scene_character sc JOIN character c ON c.id=sc.character_id WHERE sc.scene_id=?1
             UNION ALL SELECT 'plotline',p.id,p.name||' ('||sp.role||')'
               FROM scene_plotline sp JOIN plotline p ON p.id=sp.plotline_id WHERE sp.scene_id=?1
             UNION ALL SELECT 'event',e.id,e.title FROM scene_event se JOIN story_event e ON e.id=se.event_id WHERE se.scene_id=?1
             UNION ALL SELECT 'location',l.id,'start: '||l.name FROM scene s JOIN location l ON l.id=s.start_location_id WHERE s.id=?1
             UNION ALL SELECT 'location',l.id,'end: '||l.name FROM scene s JOIN location l ON l.id=s.end_location_id WHERE s.id=?1
             UNION ALL SELECT 'issue',i.id,i.title||' ('||i.status||')'
               FROM scene_issue si JOIN story_issue i ON i.id=si.issue_id WHERE si.scene_id=?1
             UNION ALL SELECT 'custom-field',d.id,d.name||': '||COALESCE(v.text_value,CAST(v.integer_value AS TEXT),v.decimal_value,CAST(v.boolean_value AS TEXT),v.entity_ref,'(reference list)')
               FROM custom_field_value v JOIN custom_field_definition d ON d.id=v.field_id WHERE v.scene_id=?1
             UNION ALL SELECT 'custom-reference',r.target_id,d.name||' references '||r.target_kind
               FROM custom_field_entity_ref r JOIN custom_field_definition d ON d.id=r.field_id WHERE r.scene_id=?1
             ORDER BY 1,2,3",
        )?;
        let rows = statement.query_map([scene.to_string()], |row| {
            Ok(QueryEvidence {
                source_kind: row.get(0)?,
                source_id: row.get(1)?,
                detail: row.get(2)?,
                passage_bytes: None,
                field_bytes: None,
            })
        })?;
        let mut evidence = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        let mut terms = {
            let mut statement = self.conn.prepare(
                "SELECT value FROM saved_query_filter WHERE query_id=?1 AND filter_kind='text_contains' ORDER BY ordinal",
            )?;
            statement
                .query_map([query.to_string()], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        if let Some(expression) = self.get_scene_query_expression(query)? {
            positive_text_query_terms(&expression, false, &mut terms);
        }
        terms.sort();
        terms.dedup();
        if !terms.is_empty() {
            let (title, synopsis) = self.conn.query_row(
                "SELECT title,synopsis FROM scene WHERE id=?1",
                [scene.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            for term in terms {
                for (field_name, field_text) in [("title", &title), ("synopsis", &synopsis)] {
                    for (start, end) in find_ascii_case_insensitive(field_text, &term) {
                        evidence.push(QueryEvidence {
                            source_kind: "scene-field".into(),
                            source_id: scene.to_string(),
                            detail: format!("matched literal query text {term:?}"),
                            passage_bytes: None,
                            field_bytes: Some((field_name.to_owned(), start, end)),
                        });
                    }
                }
                for passage_bytes in find_ascii_case_insensitive(&prose, &term) {
                    evidence.push(QueryEvidence {
                        source_kind: "scene-passage".into(),
                        source_id: scene.to_string(),
                        detail: format!("matched literal query text {term:?}"),
                        passage_bytes: Some(passage_bytes),
                        field_bytes: None,
                    });
                }
            }
        }
        Ok(evidence)
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
    pub fn add_approved_knowledge_claim(&mut self, claim: &ApprovedKnowledgeClaim) -> Result<()> {
        let source_length = i64::try_from(claim.source_length_bytes).map_err(|_| {
            StorageError::Invalid("source length exceeds SQLite integer range".into())
        })?;
        let start = i64::try_from(claim.evidence_start_byte).map_err(|_| {
            StorageError::Invalid("evidence offset exceeds SQLite integer range".into())
        })?;
        let end = i64::try_from(claim.evidence_end_byte).map_err(|_| {
            StorageError::Invalid("evidence offset exceeds SQLite integer range".into())
        })?;
        self.conn.execute(
            "INSERT INTO approved_knowledge_claim(id,kind,name,claim,approximate_story_time,source_kind,source_id,source_fingerprint,source_length_bytes,evidence_quote,evidence_start_byte,evidence_end_byte,model,approved_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![claim.id.to_string(),claim.kind,claim.name,claim.claim,claim.approximate_story_time,claim.source_kind,claim.source_id,claim.source_fingerprint,source_length,claim.evidence_quote,start,end,claim.model,claim.approved_at],
        )?;
        Ok(())
    }
    pub fn get_approved_knowledge_claim(
        &self,
        id: KnowledgeClaimId,
    ) -> Result<ApprovedKnowledgeClaim> {
        self.list_approved_knowledge_claims()?
            .into_iter()
            .find(|claim| claim.id == id)
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }
    pub fn list_approved_knowledge_claims(&self) -> Result<Vec<ApprovedKnowledgeClaim>> {
        let mut statement = self.conn.prepare(
            "SELECT id,kind,name,claim,approximate_story_time,source_kind,source_id,source_fingerprint,source_length_bytes,evidence_quote,evidence_start_byte,evidence_end_byte,model,approved_at FROM approved_knowledge_claim ORDER BY approved_at,id",
        )?;
        let rows = statement.query_map([], |row| {
            let parse_id = |value: String| -> rusqlite::Result<KnowledgeClaimId> {
                Uuid::parse_str(&value)
                    .map(KnowledgeClaimId)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
            };
            Ok(ApprovedKnowledgeClaim {
                id: parse_id(row.get(0)?)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                claim: row.get(3)?,
                approximate_story_time: row.get(4)?,
                source_kind: row.get(5)?,
                source_id: row.get(6)?,
                source_fingerprint: row.get(7)?,
                source_length_bytes: usize::try_from(row.get::<_, i64>(8)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                evidence_quote: row.get(9)?,
                evidence_start_byte: usize::try_from(row.get::<_, i64>(10)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        10,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                evidence_end_byte: usize::try_from(row.get::<_, i64>(11)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        11,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                model: row.get(12)?,
                approved_at: row.get(13)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
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
            let hash: String = self.conn.query_row(
                "SELECT sha256 FROM scene_content WHERE scene_id=?1",
                [scene.to_string()],
                |row| row.get(0),
            )?;
            refresh_scene_mentions(&self.conn, scene, &text, &hash)?;
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
    pub fn list_custom_field_definitions(&self) -> Result<Vec<CustomFieldDefinition>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,name,field_type,required,default_text,default_integer,default_decimal,default_boolean,enum_values,reference_kind FROM custom_field_definition ORDER BY name,id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<bool>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    name,
                    raw_type,
                    required,
                    default_text,
                    default_integer,
                    default_decimal,
                    default_boolean,
                    enum_values,
                    reference_kind,
                )| {
                    let field_type = parse_field_type(&raw_type)?;
                    let default_value = match field_type {
                        FieldType::Text | FieldType::LongText => default_text.map(FieldValue::Text),
                        FieldType::Integer => default_integer.map(FieldValue::Integer),
                        FieldType::Decimal => default_decimal.map(FieldValue::Decimal),
                        FieldType::Boolean => default_boolean.map(FieldValue::Boolean),
                        FieldType::DateTime => default_text.map(FieldValue::DateTime),
                        FieldType::Enum => default_text.map(FieldValue::Enum),
                        FieldType::EntityRef => default_text.map(FieldValue::EntityRef),
                        FieldType::EntityRefs => None,
                    };
                    Ok(CustomFieldDefinition {
                        id: FieldId(
                            Uuid::parse_str(&id)
                                .map_err(|error| StorageError::Invalid(error.to_string()))?,
                        ),
                        name,
                        field_type,
                        required,
                        default_value,
                        enum_values: enum_values.lines().map(str::to_owned).collect(),
                        reference_kind,
                    })
                },
            )
            .collect()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentUnitSaveFailpoint {
    AfterPartialTemp,
    AfterTempSync,
    AfterIntent,
    BeforeRename,
    AfterRename,
    AfterMetadataCommit,
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
            "reading" => Ok(EntityRef::Reading(ReadingId(id))),
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

#[derive(Clone)]
struct MentionTerm {
    kind: String,
    id: String,
    name: String,
}

fn refresh_scene_mentions(
    conn: &Connection,
    scene: SceneId,
    text: &str,
    content_hash: &str,
) -> Result<()> {
    let previous: Vec<(String, u64, u64, String, String)> = {
        let mut query = conn.prepare(
            "SELECT id,start_byte,end_byte,quote,state FROM detected_mention WHERE scene_id=?1 AND state IN('confirmed','dismissed')",
        )?;
        query
            .query_map([scene.to_string()], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<std::result::Result<_, _>>()?
    };
    let mut retained = std::collections::BTreeSet::new();
    for (id, start, end, quote, state) in previous {
        let current = usize::try_from(start)
            .ok()
            .zip(usize::try_from(end).ok())
            .and_then(|(start, end)| text.get(start..end));
        if current == Some(quote.as_str()) {
            conn.execute(
                "UPDATE detected_mention SET source_sha256=?2,updated_at=?3 WHERE id=?1",
                params![id, content_hash, now()],
            )?;
            retained.insert((start, end));
        } else if state == "confirmed" {
            conn.execute(
                "UPDATE detected_mention SET state='needs_review',source_sha256=?2,updated_at=?3 WHERE id=?1",
                params![id, content_hash, now()],
            )?;
        } else {
            conn.execute("DELETE FROM detected_mention WHERE id=?1", [&id])?;
        }
    }
    conn.execute(
        "DELETE FROM detected_mention WHERE scene_id=?1 AND state IN('detected','ambiguous')",
        [scene.to_string()],
    )?;

    let mut terms = Vec::new();
    {
        let mut query = conn.prepare("SELECT id,name FROM character UNION SELECT id,preferred_name FROM character WHERE preferred_name<>'' UNION SELECT character_id,alias FROM character_alias")?;
        terms.extend(
            query
                .query_map([], |row| {
                    Ok(MentionTerm {
                        kind: "character".into(),
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        );
    }
    {
        let mut query = conn.prepare("SELECT id,name FROM location")?;
        terms.extend(
            query
                .query_map([], |row| {
                    Ok(MentionTerm {
                        kind: "location".into(),
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        );
    }
    {
        let mut query = conn.prepare("SELECT id,name FROM story_object")?;
        terms.extend(
            query
                .query_map([], |row| {
                    Ok(MentionTerm {
                        kind: "object".into(),
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        );
    }
    let mut matches: std::collections::BTreeMap<(usize, usize), Vec<MentionCandidate>> =
        std::collections::BTreeMap::new();
    let starts = text
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for term in terms
        .into_iter()
        .filter(|term| !term.name.trim().is_empty())
    {
        let needle = term.name.trim();
        let folded = needle.to_lowercase();
        for &start in &starts {
            let Some(end) = start.checked_add(needle.len()) else {
                continue;
            };
            let Some(candidate) = text.get(start..end) else {
                continue;
            };
            if candidate.to_lowercase() != folded {
                continue;
            }
            let before_ok = text[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric());
            let after_ok = text[end..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric());
            if !before_ok || !after_ok {
                continue;
            }
            let entry = matches.entry((start, end)).or_default();
            let found = MentionCandidate {
                entity_kind: term.kind.clone(),
                entity_id: term.id.clone(),
                matched_name: needle.to_owned(),
            };
            if !entry.contains(&found) {
                entry.push(found);
            }
        }
    }
    for ((start, end), mut candidates) in matches {
        let (Ok(start_i64), Ok(end_i64)) = (i64::try_from(start), i64::try_from(end)) else {
            continue;
        };
        let (Ok(start_u64), Ok(end_u64)) = (u64::try_from(start), u64::try_from(end)) else {
            continue;
        };
        if retained.contains(&(start_u64, end_u64)) {
            continue;
        }
        candidates.sort_by(|a, b| {
            (&a.entity_kind, &a.entity_id, &a.matched_name).cmp(&(
                &b.entity_kind,
                &b.entity_id,
                &b.matched_name,
            ))
        });
        let id = MentionId::new();
        let state = if candidates
            .iter()
            .map(|candidate| (&candidate.entity_kind, &candidate.entity_id))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1
        {
            "ambiguous"
        } else {
            "detected"
        };
        conn.execute(
            "INSERT INTO detected_mention(id,scene_id,start_byte,end_byte,quote,source_sha256,state,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
            params![id.to_string(), scene.to_string(), start_i64, end_i64, &text[start..end], content_hash, state, now()],
        )?;
        for candidate in candidates {
            conn.execute(
                "INSERT INTO detected_mention_candidate(mention_id,entity_kind,entity_id,matched_name) VALUES(?1,?2,?3,?4)",
                params![id.to_string(), candidate.entity_kind, candidate.entity_id, candidate.matched_name],
            )?;
        }
    }
    Ok(())
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
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}
fn hash_bytes(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

fn writing_goal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WritingGoalRecord> {
    let scene_id: Option<String> = row.get(3)?;
    Ok(WritingGoalRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        scope_type: row.get(2)?,
        scope_scene_id: scene_id
            .map(|value| value.parse::<Uuid>().map(SceneId))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        start_date: row.get(4)?,
        end_date: row.get(5)?,
        target_words: row.get(6)?,
        baseline_words: row.get(7)?,
        time_zone: row.get(8)?,
        word_count_policy: row.get(9)?,
        success_metric: row.get(10)?,
        archived: row.get::<_, i64>(11)? != 0,
    })
}

fn writing_goal_event_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WritingGoalEventRecord> {
    let scene_id: Option<String> = row.get(6)?;
    Ok(WritingGoalEventRecord {
        id: row.get(0)?,
        goal_id: row.get(1)?,
        occurred_at: row.get(2)?,
        origin: row.get(3)?,
        source_identifier: row.get(4)?,
        source_fingerprint: row.get(5)?,
        scene_id: scene_id
            .map(|value| value.parse::<Uuid>().map(SceneId))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        raw_delta: row.get(7)?,
        real_delta: row.get(8)?,
        reconciliation_state: row.get(9)?,
        detail: row.get(10)?,
    })
}

fn parse_goal_date(value: &str) -> Result<time::Date> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .ok_or_else(|| StorageError::Invalid("dates must use YYYY-MM-DD".into()))?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .and_then(|month| time::Month::try_from(month).ok())
        .ok_or_else(|| StorageError::Invalid("invalid month in date".into()))?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or_else(|| StorageError::Invalid("invalid day in date".into()))?;
    if parts.next().is_some() {
        return Err(StorageError::Invalid("dates must use YYYY-MM-DD".into()));
    }
    let date = time::Date::from_calendar_date(year, month, day)
        .map_err(|error| StorageError::Invalid(error.to_string()))?;
    if format_goal_date(date) != value {
        return Err(StorageError::Invalid(
            "dates must use zero-padded YYYY-MM-DD".into(),
        ));
    }
    Ok(date)
}

fn format_goal_date(date: time::Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn parse_goal_timezone_offset(value: &str) -> Result<i64> {
    if matches!(value, "UTC" | "Z") {
        return Ok(0);
    }
    let (sign, value) = if let Some(rest) = value.strip_prefix('+') {
        (1i64, rest)
    } else if let Some(rest) = value.strip_prefix('-') {
        (-1i64, rest)
    } else {
        return Err(StorageError::Invalid(
            "time zone must be UTC or a fixed offset such as +01:30".into(),
        ));
    };
    let (hours, minutes) = value
        .split_once(':')
        .ok_or_else(|| StorageError::Invalid("fixed time zone offsets use ±HH:MM".into()))?;
    let hours = hours
        .parse::<i64>()
        .map_err(|_| StorageError::Invalid("invalid time zone hours".into()))?;
    let minutes = minutes
        .parse::<i64>()
        .map_err(|_| StorageError::Invalid("invalid time zone minutes".into()))?;
    if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
        return Err(StorageError::Invalid(
            "time zone offset is out of range".into(),
        ));
    }
    Ok(sign * (hours * 3600 + minutes * 60))
}

fn goal_timestamp_date(timestamp: i64, offset: i64) -> Result<String> {
    let local = timestamp
        .checked_add(offset)
        .and_then(|timestamp| time::OffsetDateTime::from_unix_timestamp(timestamp).ok())
        .ok_or_else(|| StorageError::Invalid("writing goal timestamp overflow".into()))?;
    Ok(format_goal_date(local.date()))
}

fn goal_day_start(date: time::Date, offset: i64) -> Result<i64> {
    date.midnight()
        .assume_utc()
        .unix_timestamp()
        .checked_sub(offset)
        .ok_or_else(|| StorageError::Invalid("writing goal day boundary overflow".into()))
}

fn insert_writing_goal_event(
    conn: &Connection,
    goal: &WritingGoalRecord,
    event: &NewWritingGoalEvent,
) -> Result<String> {
    if let Some((existing_id, existing_fingerprint)) = conn
        .query_row(
            "SELECT id,source_fingerprint FROM writing_goal_event WHERE goal_id=?1 AND origin=?2 AND source_identifier=?3",
            params![event.goal_id, event.origin, event.source_identifier],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if existing_fingerprint == event.source_fingerprint {
            return Ok(existing_id);
        }
        return Err(StorageError::Conflict(format!(
            "source {:?} changed fingerprint; reconcile it explicitly",
            event.source_identifier
        )));
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO writing_goal_event(id,goal_id,occurred_at,origin,source_identifier,source_fingerprint,scene_id,raw_delta,real_delta,detail,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![id,event.goal_id,event.occurred_at,event.origin,event.source_identifier,event.source_fingerprint,event.scene_id.map(|scene| scene.to_string()),event.raw_delta,event.real_delta,event.detail,now()],
    )?;
    refresh_writing_goal_snapshots(conn, goal, event.occurred_at)?;
    Ok(id)
}

fn refresh_writing_goal_snapshots(
    conn: &Connection,
    goal: &WritingGoalRecord,
    recorded_at: i64,
) -> Result<()> {
    let offset = parse_goal_timezone_offset(&goal.time_zone)?;
    let timestamps = {
        let mut query = conn.prepare(
            "SELECT occurred_at FROM writing_goal_event WHERE goal_id=?1 AND reconciliation_state='current' ORDER BY occurred_at",
        )?;
        query
            .query_map([&goal.id], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut dates = timestamps
        .into_iter()
        .map(|timestamp| goal_timestamp_date(timestamp, offset))
        .collect::<Result<std::collections::BTreeSet<_>>>()?;
    if dates.is_empty() {
        dates.insert(goal.start_date.clone());
    }
    conn.execute(
        "DELETE FROM writing_goal_snapshot WHERE goal_id=?1",
        [&goal.id],
    )?;
    for date in dates {
        upsert_writing_goal_snapshot(conn, goal, &date, recorded_at)?;
    }
    Ok(())
}

fn upsert_writing_goal_snapshot(
    conn: &Connection,
    goal: &WritingGoalRecord,
    snapshot_date: &str,
    recorded_at: i64,
) -> Result<()> {
    let offset = parse_goal_timezone_offset(&goal.time_zone)?;
    let start = parse_goal_date(&goal.start_date)?;
    let end = parse_goal_date(&goal.end_date)?;
    let date = parse_goal_date(snapshot_date)?;
    if date < start || date > end {
        return Ok(());
    }
    let total_days = i64::from(end.to_julian_day() - start.to_julian_day() + 1);
    let day_index = i64::from(date.to_julian_day() - start.to_julian_day() + 1);
    let target_range = goal
        .target_words
        .checked_sub(goal.baseline_words)
        .ok_or_else(|| StorageError::Invalid("writing goal target range overflow".into()))?;
    let target = goal
        .baseline_words
        .checked_add(
            target_range
                .checked_mul(day_index)
                .ok_or_else(|| StorageError::Invalid("writing goal target overflow".into()))?
                / total_days,
        )
        .ok_or_else(|| StorageError::Invalid("writing goal baseline overflow".into()))?;
    let boundary = goal_day_start(date, offset)?
        .checked_add(86_400)
        .ok_or_else(|| StorageError::Invalid("writing goal day boundary overflow".into()))?;
    let (raw, real): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(raw_delta),0),COALESCE(SUM(real_delta),0) FROM writing_goal_event WHERE goal_id=?1 AND occurred_at < ?2 AND reconciliation_state='current'",
        params![goal.id,boundary],
        |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    conn.execute(
        "INSERT INTO writing_goal_snapshot(id,goal_id,snapshot_date,recorded_at,target_words,raw_words,real_words,success_metric) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(goal_id,snapshot_date) DO UPDATE SET recorded_at=excluded.recorded_at,target_words=excluded.target_words,raw_words=excluded.raw_words,real_words=excluded.real_words,success_metric=excluded.success_metric",
        params![Uuid::new_v4().to_string(),goal.id,format_goal_date(date),recorded_at,target,goal.baseline_words.checked_add(raw).ok_or_else(|| StorageError::Invalid("writing goal raw count overflow".into()))?,goal.baseline_words.checked_add(real).ok_or_else(|| StorageError::Invalid("writing goal real count overflow".into()))?,goal.success_metric],
    )?;
    Ok(())
}

fn validate_story_bible_template(template: &StoryBibleTemplate) -> Result<()> {
    if template.name.trim().is_empty() || template.fields.is_empty() {
        return Err(StorageError::Invalid(
            "Story Bible templates need a name and at least one field".into(),
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let mut labels = std::collections::HashSet::new();
    for field in &template.fields {
        if field.id.trim().is_empty()
            || field.id.chars().any(char::is_whitespace)
            || field.label.trim().is_empty()
        {
            return Err(StorageError::Invalid("template field IDs must be nonempty and contain no whitespace; labels are required".into()));
        }
        if !ids.insert(field.id.to_lowercase()) || !labels.insert(field.label.to_lowercase()) {
            return Err(StorageError::Invalid(
                "template field IDs and labels must be unique".into(),
            ));
        }
        match field.field_type {
            StoryBibleFieldType::Enum if field.enum_values.is_empty() => {
                return Err(StorageError::Invalid(format!(
                    "enum field {} needs values",
                    field.id
                )));
            }
            StoryBibleFieldType::Enum => {
                let mut choices = std::collections::HashSet::new();
                if field.enum_values.iter().any(|choice| {
                    choice.trim().is_empty() || !choices.insert(choice.to_lowercase())
                }) {
                    return Err(StorageError::Invalid(format!(
                        "enum field {} has an empty or duplicate choice",
                        field.id
                    )));
                }
            }
            _ if !field.enum_values.is_empty() => {
                return Err(StorageError::Invalid(format!(
                    "non-enum field {} cannot have enum choices",
                    field.id
                )));
            }
            _ => {}
        }
        if matches!(
            field.field_type,
            StoryBibleFieldType::EntityRef | StoryBibleFieldType::EntityRefs
        ) != field.reference_kind.is_some()
        {
            return Err(StorageError::Invalid(format!(
                "entity-reference field {} must declare exactly one target kind",
                field.id
            )));
        }
    }
    Ok(())
}

fn validate_story_bible_values(
    template: &StoryBibleTemplate,
    values: &std::collections::BTreeMap<String, StoryBibleFieldValue>,
) -> Result<()> {
    for field in &template.fields {
        let value = values.get(&field.id);
        if field.required && value.is_none() {
            return Err(StorageError::Invalid(format!(
                "required Story Bible field {} is missing",
                field.id
            )));
        }
        let Some(value) = value else { continue };
        let matches = match (field.field_type, value) {
            (StoryBibleFieldType::Text, StoryBibleFieldValue::Text(text)) => {
                !field.required || !text.trim().is_empty()
            }
            (StoryBibleFieldType::LongText, StoryBibleFieldValue::LongText(text)) => {
                !field.required || !text.trim().is_empty()
            }
            (StoryBibleFieldType::Integer, StoryBibleFieldValue::Integer(_)) => true,
            (StoryBibleFieldType::Decimal, StoryBibleFieldValue::Decimal(number)) => {
                number.parse::<f64>().is_ok_and(f64::is_finite)
            }
            (StoryBibleFieldType::Boolean, StoryBibleFieldValue::Boolean(_)) => true,
            (StoryBibleFieldType::DateTime, StoryBibleFieldValue::DateTime(value)) => {
                (!field.required && value.trim().is_empty())
                    || time::OffsetDateTime::parse(
                        value,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .is_ok()
            }
            (StoryBibleFieldType::Enum, StoryBibleFieldValue::Enum(choice)) => {
                field.enum_values.contains(choice)
            }
            (StoryBibleFieldType::EntityRef, StoryBibleFieldValue::EntityRef(id)) => {
                !id.trim().is_empty()
            }
            (StoryBibleFieldType::EntityRefs, StoryBibleFieldValue::EntityRefs(ids)) => {
                !field.required || !ids.is_empty()
            }
            _ => false,
        };
        if !matches {
            return Err(StorageError::Invalid(format!(
                "value type or value does not satisfy Story Bible field {}",
                field.id
            )));
        }
    }
    if values
        .keys()
        .any(|key| !template.fields.iter().any(|field| &field.id == key))
    {
        return Err(StorageError::Invalid(
            "Story Bible profile contains an unknown template field".into(),
        ));
    }
    Ok(())
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

/// Reanchors a passage only when its exact quote and saved neighboring context identify one
/// location. If context does not disambiguate duplicate text, the annotation remains reviewable.
fn unique_contextual_occurrence(
    text: &str,
    quote: &str,
    prefix: &str,
    suffix: &str,
) -> Option<(usize, usize)> {
    if quote.is_empty() {
        return None;
    }
    let mut matches = text.match_indices(quote).filter_map(|(start, _)| {
        let end = start.checked_add(quote.len())?;
        let before = &text[..start];
        let after = &text[end..];
        (before.ends_with(prefix) && after.starts_with(suffix)).then_some((start, end))
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
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
fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    format!("{:x}", hash.finalize())
}
fn write_git_restore_journal(directory: &Path, journal: &GitRestoreJournal) -> Result<()> {
    let bytes = serde_json::to_vec(journal).map_err(|error| {
        StorageError::Invalid(format!("serialize Git restore journal: {error}"))
    })?;
    atomic_write(&directory.join("journal.json"), &bytes)
}
fn is_git_restore_path_allowed(relative: &str) -> bool {
    let path = Path::new(relative);
    let safe = !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    safe && (relative == "story.db"
        || relative == ".gitignore"
        || relative == "project.toml"
        || relative == "snapshots/database.sql"
        || ["manuscript/", "research/", "attachments/", "templates/"]
            .iter()
            .any(|prefix| relative.starts_with(prefix)))
}
fn safe_git_restore_target(root: &Path, relative: &str, create_parent: bool) -> Result<PathBuf> {
    if !is_git_restore_path_allowed(relative) {
        return Err(StorageError::Invalid("unsafe Git restore path".into()));
    }
    let root = fs::canonicalize(root)?;
    let mut target = root.clone();
    let components = Path::new(relative)
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => Ok(part.to_owned()),
            _ => Err(StorageError::Invalid("unsafe Git restore path".into())),
        })
        .collect::<Result<Vec<_>>>()?;
    for (index, component) in components.iter().enumerate() {
        target.push(component);
        let is_parent = index + 1 < components.len();
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(StorageError::Invalid(
                    "Git restore refuses to traverse project symlinks".into(),
                ));
            }
            Ok(metadata) if is_parent && !metadata.is_dir() => {
                return Err(StorageError::Invalid(
                    "Git restore parent path is not a directory".into(),
                ));
            }
            Ok(_) => {}
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && is_parent && create_parent =>
            {
                fs::create_dir(&target)?;
                if let Some(parent) = target.parent() {
                    sync_dir(parent)?;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if is_parent && create_parent && !target.exists() {
            fs::create_dir(&target)?;
            sync_dir(target.parent().unwrap())?;
        }
    }
    Ok(target)
}
fn collect_regular_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(StorageError::Invalid(
                "project backup contains a symbolic link".into(),
            ));
        }
        if metadata.is_dir() {
            collect_regular_files(root, &path, files)?;
        } else if metadata.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|_| StorageError::Invalid("backup path escaped project".into()))?
                    .to_path_buf(),
            );
        } else {
            return Err(StorageError::Invalid(
                "project backup contains a non-regular file".into(),
            ));
        }
    }
    Ok(())
}

fn safe_archive_path(value: &str) -> Result<PathBuf> {
    if value.is_empty() || value.contains(['\\', ':']) {
        return Err(StorageError::Invalid("unsafe backup archive path".into()));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || !path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err(StorageError::Invalid("unsafe backup archive path".into()));
    }
    Ok(path.to_path_buf())
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
fn deterministic_sql_dump(conn: &Connection) -> Result<String> {
    let mut schema = Vec::<(String, String, String)>::new();
    {
        let mut query=conn.prepare("SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1 WHEN 'view' THEN 2 WHEN 'trigger' THEN 3 ELSE 4 END,name")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let entry = row?;
            if is_transient_or_fts_shadow(&entry.1) {
                continue;
            }
            schema.push(entry);
        }
    }
    let virtual_tables: std::collections::HashSet<String> = schema
        .iter()
        .filter(|(kind, _, sql)| {
            kind == "table" && sql.to_ascii_uppercase().contains("CREATE VIRTUAL TABLE")
        })
        .map(|(_, name, _)| name.clone())
        .collect();
    let mut output = String::from(
        "-- Writer's IDE deterministic metadata snapshot v1\nPRAGMA foreign_keys=OFF;\nBEGIN IMMEDIATE;\n",
    );
    for (_, _, sql) in &schema {
        output.push_str(sql.trim_end());
        output.push_str(";\n");
    }
    let mut data_tables = schema
        .iter()
        .filter(|(kind, name, _)| {
            kind == "table" && !virtual_tables.contains(name) && !excluded_snapshot_table(name)
        })
        .collect::<Vec<_>>();
    // Some integrity triggers validate parent kinds even while foreign keys are
    // disabled in the portable dump. Insert those parents before dependent data.
    data_tables.sort_by_key(|(_, name, _)| match name.as_str() {
        "structural_node" => (0, name.as_str()),
        "content_unit" => (1, name.as_str()),
        _ => (2, name.as_str()),
    });
    for (_, name, _) in data_tables {
        let table_quote = quote_identifier(name);
        let mut columns_stmt = conn.prepare(&format!("PRAGMA table_info({table_quote})"))?;
        let columns: Vec<(String, i64)> = columns_stmt
            .query_map([], |row| Ok((row.get(1)?, row.get(5)?)))?
            .collect::<std::result::Result<_, _>>()?;
        if columns.is_empty() {
            continue;
        }
        let names = columns
            .iter()
            .map(|(name, _)| quote_identifier(name))
            .collect::<Vec<_>>();
        let pk = columns
            .iter()
            .filter(|(_, order)| *order > 0)
            .collect::<Vec<_>>();
        let order = if pk.is_empty() {
            names.clone()
        } else {
            pk.iter().map(|(name, _)| quote_identifier(name)).collect()
        };
        let sql = format!(
            "SELECT {} FROM {table_quote} ORDER BY {}",
            names.join(","),
            order.join(",")
        );
        let mut statement = conn.prepare(&sql)?;
        let count = statement.column_count();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let values = (0..count)
                .map(|index| row.get_ref(index).map(sql_literal))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            output.push_str("INSERT INTO ");
            output.push_str(&table_quote);
            output.push_str(" (");
            output.push_str(&names.join(","));
            output.push_str(") VALUES (");
            output.push_str(&values.join(","));
            output.push_str(");\n");
        }
    }
    output.push_str("COMMIT;\nPRAGMA foreign_keys=ON;\n");
    Ok(output)
}
fn excluded_snapshot_table(name: &str) -> bool {
    matches!(
        name,
        "scene_snapshot"
            | "write_journal"
            | "write_conflict"
            | "content_unit_write_journal"
            | "content_unit_write_conflict"
            | "project_search_content"
            | "scene_search_content"
            | "content_unit_search_content"
    )
}
fn is_transient_or_fts_shadow(name: &str) -> bool {
    name.starts_with("sqlite_")
        || ["scene_fts_", "reading_fts_", "project_search_fts_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}
fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn sql_literal(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(value) => value.to_string(),
        ValueRef::Real(value) => value.to_string(),
        ValueRef::Text(value) => {
            format!("'{}'", String::from_utf8_lossy(value).replace('\'', "''"))
        }
        ValueRef::Blob(value) => format!(
            "X'{}'",
            value
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
    }
}
fn validate_sql_dump(sql: &str) -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(sql)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(StorageError::Invalid(format!(
            "generated SQL snapshot failed integrity_check: {integrity}"
        )));
    }
    let foreign_key_errors: i64 =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if foreign_key_errors != 0 {
        return Err(StorageError::Invalid(format!(
            "generated SQL snapshot has {foreign_key_errors} foreign-key violations"
        )));
    }
    Ok(())
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

fn boundary_name(boundary: IntervalBoundary) -> &'static str {
    match boundary {
        IntervalBoundary::Inclusive => "inclusive",
        IntervalBoundary::Exclusive => "exclusive",
    }
}

fn parse_boundary(value: &str) -> Result<IntervalBoundary> {
    match value {
        "inclusive" => Ok(IntervalBoundary::Inclusive),
        "exclusive" => Ok(IntervalBoundary::Exclusive),
        other => Err(StorageError::Config(format!(
            "invalid story interval boundary {other}"
        ))),
    }
}

fn write_interval_boundaries(
    conn: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
    start: IntervalBoundary,
    end: IntervalBoundary,
) -> Result<()> {
    let delete_sql = match table {
        "scene_interval_boundary" => "DELETE FROM scene_interval_boundary WHERE scene_id=?1",
        "story_event_interval_boundary" => {
            "DELETE FROM story_event_interval_boundary WHERE event_id=?1"
        }
        _ => {
            return Err(StorageError::Invalid(
                "unknown interval-boundary table".into(),
            ));
        }
    };
    if start == IntervalBoundary::Inclusive && end == IntervalBoundary::Inclusive {
        conn.execute(delete_sql, [id])?;
        return Ok(());
    }
    let insert_sql = match (table, id_column) {
        ("scene_interval_boundary", "scene_id") => {
            "INSERT INTO scene_interval_boundary(scene_id,start_boundary,end_boundary) VALUES(?1,?2,?3) ON CONFLICT(scene_id) DO UPDATE SET start_boundary=excluded.start_boundary,end_boundary=excluded.end_boundary"
        }
        ("story_event_interval_boundary", "event_id") => {
            "INSERT INTO story_event_interval_boundary(event_id,start_boundary,end_boundary) VALUES(?1,?2,?3) ON CONFLICT(event_id) DO UPDATE SET start_boundary=excluded.start_boundary,end_boundary=excluded.end_boundary"
        }
        _ => {
            return Err(StorageError::Invalid(
                "unknown interval-boundary entity key".into(),
            ));
        }
    };
    conn.execute(
        insert_sql,
        params![id, boundary_name(start), boundary_name(end)],
    )?;
    Ok(())
}

fn set_interval_boundaries(
    conn: &mut Connection,
    table: &str,
    id_column: &str,
    id: &str,
    start: IntervalBoundary,
    end: IntervalBoundary,
) -> Result<()> {
    let parent_table = match (table, id_column) {
        ("scene_interval_boundary", "scene_id") => "scene",
        ("story_event_interval_boundary", "event_id") => "story_event",
        _ => {
            return Err(StorageError::Invalid(
                "unknown interval-boundary entity".into(),
            ));
        }
    };
    let tx = conn.transaction()?;
    let exists: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {parent_table} WHERE id=?1)"),
        [id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(StorageError::NotFound(id.to_owned()));
    }
    write_interval_boundaries(&tx, table, id_column, id, start, end)?;
    tx.commit()?;
    Ok(())
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
fn query_assembly_item_ids(
    transaction: &rusqlite::Transaction<'_>,
    assembly: AssemblyId,
) -> Result<Vec<String>> {
    let mut query = transaction
        .prepare("SELECT id FROM assembly_item WHERE assembly_id=?1 ORDER BY position,id")?;
    Ok(query
        .query_map([assembly.to_string()], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn compact_assembly_positions(
    transaction: &rusqlite::Transaction<'_>,
    assembly: AssemblyId,
    ids: &[String],
) -> Result<()> {
    let max = ids.len() as i64;
    let offset = max
        .checked_mul(4)
        .and_then(|value| value.checked_add(100))
        .ok_or_else(|| StorageError::Invalid("assembly positions exhausted".into()))?;
    transaction.execute(
        "UPDATE assembly_item SET position=position+?2 WHERE assembly_id=?1",
        params![assembly.to_string(), offset],
    )?;
    for (index, id) in ids.iter().enumerate() {
        transaction.execute(
            "UPDATE assembly_item SET position=?2 WHERE id=?1",
            params![id, offset * 2 + index as i64],
        )?;
    }
    for (index, id) in ids.iter().enumerate() {
        transaction.execute(
            "UPDATE assembly_item SET position=?2 WHERE id=?1",
            params![id, index as i64],
        )?;
    }
    Ok(())
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
    fn story_interval_boundaries_migrate_persist_and_reject_invalid_sql_values() {
        let directory = tempdir().unwrap();
        let project = directory.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Opening").unwrap();
        let event = EventId::new();
        store
            .add_story_event(&StoryEvent {
                id: event,
                title: "Train crash".into(),
                description: String::new(),
                start: StoryTime::exact(15 * 60 + 13),
                end: StoryTime::exact(15 * 60 + 13),
                source_time_text: "15:13".into(),
            })
            .unwrap();
        let interval = StoryInterval {
            start: StoryTime::exact(9 * 60),
            end: StoryTime::exact(16 * 60),
            start_boundary: IntervalBoundary::Inclusive,
            end_boundary: IntervalBoundary::Exclusive,
        };
        store.update_scene_story_interval(scene, interval).unwrap();
        store
            .update_story_event_interval(
                event,
                StoryInterval::closed(
                    StoryTime::exact(15 * 60 + 13),
                    StoryTime::exact(15 * 60 + 13),
                ),
            )
            .unwrap();
        assert_eq!(
            store.scene_interval_boundaries(scene).unwrap(),
            (IntervalBoundary::Inclusive, IntervalBoundary::Exclusive)
        );
        assert!(
            store
                .conn
                .execute(
                    "UPDATE scene_interval_boundary SET end_boundary='sometimes' WHERE scene_id=?1",
                    [scene.to_string()],
                )
                .is_err()
        );
        drop(store);

        let reopened = ProjectStore::open(&project).unwrap();
        assert_eq!(
            reopened.scene_interval_boundaries(scene).unwrap(),
            (IntervalBoundary::Inclusive, IntervalBoundary::Exclusive)
        );
        assert_eq!(
            reopened.story_event_interval_boundaries(event).unwrap(),
            (IntervalBoundary::Inclusive, IntervalBoundary::Inclusive)
        );
        let current = reopened.get_scene(scene).unwrap().scene;
        assert_eq!(current.start_time, interval.start);
        assert_eq!(current.end_time, interval.end);
    }
    #[test]
    fn schema_thirty_nine_projects_migrate_interval_boundary_tables() {
        let directory = tempdir().unwrap();
        let project = directory.path().join("Book");
        let store = ProjectStore::create(&project, "Book").unwrap();
        store.conn.execute_batch(
            "DROP TABLE diagnostic_suppression; DROP TABLE story_event_character; DROP TABLE story_event_location; DROP TABLE story_event_interval_boundary; DROP TABLE scene_interval_boundary; DELETE FROM schema_migrations WHERE version>=40;",
        ).unwrap();
        drop(store);

        let mut migrated = ProjectStore::open(&project).unwrap();
        let chapter = migrated
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = migrated.create_scene(chapter, 0, "Opening").unwrap();
        migrated
            .set_scene_interval_boundaries(
                scene,
                IntervalBoundary::Exclusive,
                IntervalBoundary::Inclusive,
            )
            .unwrap();
        assert_eq!(
            migrated.scene_interval_boundaries(scene).unwrap(),
            (IntervalBoundary::Exclusive, IntervalBoundary::Inclusive)
        );
        let version: i64 = migrated
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 42);
    }
    #[test]
    fn event_character_and_location_relationships_are_typed_atomic_and_persistent() {
        let directory = tempdir().unwrap();
        let project = directory.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        let character = CharacterId::new();
        store.add_character(character, "Mara").unwrap();
        let location = LocationId::new();
        store.add_location(location, "North Station").unwrap();
        let event = EventId::new();
        store
            .add_story_event(&StoryEvent {
                id: event,
                title: "Train arrives".into(),
                description: String::new(),
                start: StoryTime::exact(120),
                end: StoryTime::exact(121),
                source_time_text: "day 1".into(),
            })
            .unwrap();
        let participants = [EventCharacterParticipation {
            character_id: character,
            role: EventParticipationRole::Witness,
        }];
        store.set_event_characters(event, &participants).unwrap();
        let locations = [EventLocationAssociation {
            location_id: location,
            role: EventLocationRole::OccurredAt,
        }];
        store.set_event_locations(event, &locations).unwrap();
        assert!(
            store
                .set_event_characters(
                    event,
                    &[EventCharacterParticipation {
                        character_id: CharacterId::new(),
                        role: EventParticipationRole::Participant,
                    }]
                )
                .is_err()
        );
        assert_eq!(store.event_characters(event).unwrap(), participants);
        assert_eq!(store.event_locations(event).unwrap(), locations);
        assert!(store.conn.execute(
            "INSERT INTO story_event_character(event_id,character_id,role) VALUES(?1,?2,'telepathically_present')",
            rusqlite::params![event.to_string(), character.to_string()],
        ).is_err());
        drop(store);
        let reopened = ProjectStore::open(&project).unwrap();
        assert_eq!(reopened.event_characters(event).unwrap(), participants);
        assert_eq!(reopened.event_locations(event).unwrap(), locations);
        let schema_version: i64 = reopened
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(schema_version, 42);
    }
    #[test]
    fn writing_challenges_are_validated_persisted_and_completed_once() {
        let d = tempdir().unwrap();
        let project = d.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        assert!(store.start_writing_challenge(0, 60, "steady").is_err());
        assert!(store.start_writing_challenge(100, 0, "steady").is_err());
        let id = store
            .start_writing_challenge(500, 3600, "kamikaze")
            .unwrap();
        let row = store.list_writing_challenges().unwrap().remove(0);
        assert_eq!(row.target_words, 500);
        assert_eq!(row.status, "active");
        store
            .finish_writing_challenge_detailed(&id, 120, 17, 2, "success")
            .unwrap();
        assert_eq!(
            store.list_writing_challenges().unwrap()[0].words_written,
            Some(120)
        );
        let finished = &store.list_writing_challenges().unwrap()[0];
        assert_eq!(finished.words_deleted, 17);
        assert_eq!(finished.pauses_consumed, 2);
        assert_eq!(finished.outcome, "success");
        assert!(store.finish_writing_challenge(&id, 120, false).is_err());
        let interrupted = store.start_writing_challenge(50, 300, "pressure").unwrap();
        drop(store);
        let mut reopened = ProjectStore::open(project).unwrap();
        reopened.recover_interrupted_writing_challenges().unwrap();
        let interrupted_record = reopened
            .list_writing_challenges()
            .unwrap()
            .into_iter()
            .find(|record| record.id == interrupted)
            .unwrap();
        assert_eq!(interrupted_record.mode, "pressure");
        assert_eq!(interrupted_record.outcome, "quit");
    }

    #[test]
    fn timed_challenge_completion_and_goal_activity_commit_atomically() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(root.path().join("Book"), "Book").unwrap();
        let today = goal_timestamp_date(unix_now(), 0).unwrap();
        let goal = store
            .create_writing_goal(&NewWritingGoal {
                title: "Timed draft day".into(),
                scope_scene_id: None,
                start_date: today.clone(),
                end_date: today,
                target_words: 100,
                baseline_words: 0,
                time_zone: "UTC".into(),
                word_count_policy: "unicode-word-boundaries".into(),
                success_metric: "raw".into(),
            })
            .unwrap();
        let challenge = store.start_writing_challenge(10, 60, "steady").unwrap();
        assert!(
            store
                .finish_writing_challenge_with_activity(
                    &challenge,
                    5,
                    2,
                    0,
                    "timeout",
                    Some(SceneId(Uuid::new_v4())),
                )
                .is_err()
        );
        assert_eq!(store.list_writing_challenges().unwrap()[0].status, "active");
        assert!(store.list_writing_goal_events(&goal).unwrap().is_empty());
        store
            .finish_writing_challenge_with_activity(&challenge, 5, 2, 0, "timeout", None)
            .unwrap();
        assert_eq!(
            store.list_writing_challenges().unwrap()[0].status,
            "completed"
        );
        let events = store.list_writing_goal_events(&goal).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].raw_delta, 7);
        assert_eq!(events[0].real_delta, 0);
    }

    #[test]
    fn writing_goal_ledger_is_idempotent_scoped_and_projects_daily_pace() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let goal = store
            .create_writing_goal(&NewWritingGoal {
                title: "January draft".into(),
                scope_scene_id: None,
                start_date: "2025-01-01".into(),
                end_date: "2025-01-03".into(),
                target_words: 900,
                baseline_words: 100,
                time_zone: "UTC".into(),
                word_count_policy: "unicode-word-boundaries".into(),
                success_metric: "real".into(),
            })
            .unwrap();
        assert!(
            store
                .create_writing_goal(&NewWritingGoal {
                    title: "Invalid range".into(),
                    scope_scene_id: None,
                    start_date: "2025-01-03".into(),
                    end_date: "2025-01-01".into(),
                    target_words: 900,
                    baseline_words: 0,
                    time_zone: "UTC".into(),
                    word_count_policy: "unicode-word-boundaries".into(),
                    success_metric: "real".into(),
                })
                .is_err()
        );

        // 2025-01-01 12:00 UTC.
        let day_one = 1_735_732_800;
        let event_value = NewWritingGoalEvent {
            goal_id: goal.clone(),
            occurred_at: day_one,
            origin: "editor".into(),
            source_identifier: "session-1".into(),
            source_fingerprint: "fingerprint-1".into(),
            scene_id: Some(scene),
            raw_delta: 60,
            real_delta: 55,
            detail: "First writing session".into(),
        };
        let original_event_id = store.record_writing_goal_event(&event_value).unwrap();
        assert_eq!(
            store.record_writing_goal_event(&event_value).unwrap(),
            original_event_id
        );
        let mut changed = event_value.clone();
        changed.source_fingerprint = "changed-fingerprint".into();
        assert!(store.record_writing_goal_event(&changed).is_err());
        let series = store.writing_goal_progress(&goal, day_one).unwrap();
        assert_eq!(series.len(), 3);
        assert_eq!(series[0].target_words, 366);
        assert_eq!(series[0].raw_words, Some(160));
        assert_eq!(series[0].real_words, Some(155));
        assert_eq!(series[1].target_words, 633);
        assert!(series[1].projected);
        let scoped_goal = NewWritingGoal {
            title: "Scene goal".into(),
            scope_scene_id: Some(scene),
            start_date: "2025-01-01".into(),
            end_date: "2025-01-03".into(),
            target_words: 100,
            baseline_words: 0,
            time_zone: "UTC".into(),
            word_count_policy: "unicode-word-boundaries".into(),
            success_metric: "real".into(),
        };
        let scene_goal = store.create_writing_goal(&scoped_goal).unwrap();
        let wrong_scene = NewWritingGoalEvent {
            goal_id: scene_goal,
            occurred_at: day_one,
            origin: "manual".into(),
            source_identifier: "wrong-scene".into(),
            source_fingerprint: "fingerprint".into(),
            scene_id: None,
            raw_delta: 1,
            real_delta: 1,
            detail: "wrong scope".into(),
        };
        assert!(store.record_writing_goal_event(&wrong_scene).is_err());
        let replacement_id = store
            .reconcile_writing_goal_event(
                &original_event_id,
                "fingerprint-1",
                62,
                54,
                "corrected totals",
            )
            .unwrap();
        assert_ne!(replacement_id, original_event_id);
        let ledger = store.list_writing_goal_events(&goal).unwrap();
        assert_eq!(ledger.len(), 2);
        let superseded = ledger
            .iter()
            .find(|ledger_event| ledger_event.id == original_event_id)
            .expect("original source event remains traceable");
        let current = ledger
            .iter()
            .find(|ledger_event| ledger_event.id == replacement_id)
            .expect("replacement event is recorded");
        assert_eq!(superseded.reconciliation_state, "superseded");
        assert_eq!(current.reconciliation_state, "current");
        assert_eq!(current.raw_delta, 62);
        assert_eq!(current.real_delta, 54);
        let snapshot: (i64, i64, i64) = store
            .conn
            .query_row(
                "SELECT target_words,raw_words,real_words FROM writing_goal_snapshot WHERE goal_id=?1 AND snapshot_date='2025-01-01'",
                [&goal],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(snapshot, (366, 162, 154));
        drop(store);
        let reopened = ProjectStore::open(project).unwrap();
        assert_eq!(reopened.list_writing_goal_events(&goal).unwrap().len(), 2);
    }

    #[test]
    fn completed_editor_sessions_feed_matching_goal_ledger_once() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(root.path().join("Book"), "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let today = goal_timestamp_date(unix_now(), 0).unwrap();
        let goal = store
            .create_writing_goal(&NewWritingGoal {
                title: "Current work".into(),
                scope_scene_id: Some(scene),
                start_date: today.clone(),
                end_date: today,
                target_words: 500,
                baseline_words: 200,
                time_zone: "UTC".into(),
                word_count_policy: "unicode-word-boundaries".into(),
                success_metric: "real".into(),
            })
            .unwrap();
        let session = store.start_writing_session(Some(scene), 200).unwrap();
        store.finish_writing_session(&session, 215, false).unwrap();
        let events = store.list_writing_goal_events(&goal).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].origin, "editor");
        assert_eq!(events[0].source_identifier, session);
        assert_eq!(events[0].raw_delta, 15);
        assert_eq!(events[0].real_delta, 15);
        store
            .finish_writing_session(&session, 215, false)
            .unwrap_err();
        assert_eq!(store.list_writing_goal_events(&goal).unwrap().len(), 1);
    }

    #[test]
    fn writing_sessions_persist_scene_word_deltas_and_recover_interrupted_sessions() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(root.path().join("Book"), "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let id = store.start_writing_session(Some(scene), 100).unwrap();
        store.finish_writing_session(&id, 145, false).unwrap();
        let sessions = store.list_writing_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].scene_id, Some(scene));
        assert_eq!(sessions[0].words_delta, Some(45));
        assert_eq!(sessions[0].status, "completed");
        let interrupted = store.start_writing_session(Some(scene), 145).unwrap();
        let next = store.start_writing_session(Some(scene), 150).unwrap();
        let sessions = store.list_writing_sessions().unwrap();
        assert_eq!(
            sessions
                .iter()
                .find(|row| row.id == interrupted)
                .unwrap()
                .status,
            "abandoned"
        );
        assert_eq!(
            sessions
                .iter()
                .find(|row| row.id == interrupted)
                .unwrap()
                .words_delta,
            Some(0)
        );
        store.finish_writing_session(&next, 152, true).unwrap();
        assert_eq!(
            store
                .list_writing_sessions()
                .unwrap()
                .iter()
                .find(|row| row.id == next)
                .unwrap()
                .words_delta,
            Some(2)
        );
    }
    #[test]
    fn spatial_planning_board_keeps_positions_separate_from_manuscript_order() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("Book"), "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let board = store.create_planning_board("Story map").unwrap();
        let first = store
            .add_planning_board_item(board, "scene", &scene.to_string(), 5.5, 2.0, "Opening")
            .unwrap();
        let object = store.create_story_object("Key", "").unwrap();
        let second = store
            .add_planning_board_item(board, "object", &object.to_string(), 18.0, 4.0, "")
            .unwrap();
        store
            .connect_planning_board_items(board, first, second, "meets")
            .unwrap();
        store.move_planning_board_item(first, 8.0, -1.25).unwrap();
        assert_eq!(store.list_planning_board_items(board).unwrap()[0].x, 8.0);
        store.resize_planning_board_item(first, 32.0, 10.0).unwrap();
        assert_eq!(
            store.list_planning_board_items(board).unwrap()[0].width,
            32.0
        );
        assert!(store.resize_planning_board_item(first, 0.0, 10.0).is_err());
        assert_eq!(store.get_scene(scene).unwrap().position, Some(0));
        assert!(
            store
                .add_planning_board_item(board, "scene", "missing", 0.0, 0.0, "")
                .is_err()
        );
        let other_board = store.create_planning_board("Other").unwrap();
        let other_item = store
            .add_planning_board_item(other_board, "scene", &scene.to_string(), 0.0, 0.0, "")
            .unwrap();
        assert!(
            store
                .connect_planning_board_items(board, first, other_item, "invalid")
                .is_err()
        );
        let duplicate = store.conn.execute("INSERT INTO planning_board_connector(id,board_id,source_item_id,target_item_id,label) VALUES(?1,?2,?3,?4,'')", params![PlanningBoardConnectorId::new().to_string(),board.to_string(),first.to_string(),other_item.to_string()]);
        assert!(
            duplicate.is_err(),
            "direct SQL cannot connect items on different boards"
        );
    }

    #[test]
    fn assembly_draft_resumes_mixed_scene_commits_and_preserves_external_conflicts() {
        let d = tempdir().unwrap();
        let project = d.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let first = store.create_scene(chapter, 0, "First").unwrap();
        let second = store.create_scene(chapter, 1, "Second").unwrap();
        let assembly = store.create_assembly("Edition").unwrap();
        store.add_assembly_scene(assembly, first, 0).unwrap();
        store.add_assembly_scene(assembly, second, 1).unwrap();
        let draft = store.create_assembly_draft(assembly).unwrap();
        store
            .update_assembly_draft_item(draft.id, first, "Drafted first scene.")
            .unwrap();
        store
            .update_assembly_draft_item(draft.id, second, "Drafted second scene.")
            .unwrap();

        // Simulate interruption after the first file's independently journaled save.
        let first_old = draft
            .items
            .iter()
            .find(|item| item.scene_id == first)
            .unwrap()
            .expected_hash
            .clone();
        store
            .save_text(first, Some(&first_old), "Drafted first scene.")
            .unwrap();
        drop(store);

        let mut reopened = ProjectStore::open(&project).unwrap();
        reopened.commit_assembly_draft(draft.id).unwrap();
        assert_eq!(reopened.load_text(first).unwrap().0, "Drafted first scene.");
        assert_eq!(
            reopened.load_text(second).unwrap().0,
            "Drafted second scene."
        );
        assert!(reopened.list_assembly_drafts().unwrap().is_empty());

        let conflict_draft = reopened.create_assembly_draft(assembly).unwrap();
        reopened
            .update_assembly_draft_item(conflict_draft.id, second, "Proposed text.")
            .unwrap();
        let (_, current_hash) = reopened.load_text(second).unwrap();
        reopened
            .save_text(second, Some(&current_hash), "Independent external edit.")
            .unwrap();
        assert!(reopened.commit_assembly_draft(conflict_draft.id).is_err());
        assert_eq!(
            reopened.load_text(second).unwrap().0,
            "Independent external edit."
        );
        assert_eq!(
            reopened
                .load_assembly_draft(conflict_draft.id)
                .unwrap()
                .items
                .iter()
                .find(|item| item.scene_id == second)
                .unwrap()
                .text,
            "Proposed text."
        );
    }

    #[test]
    fn story_bible_templates_are_portable_typed_and_validate_profile_values() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("Book");
        let mut store = ProjectStore::create(&project, "Book").unwrap();
        let character = CharacterId::new();
        let location = LocationId::new();
        store
            .conn
            .execute(
                "INSERT INTO character(id,name) VALUES(?1,'Mara')",
                [character.to_string()],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO location(id,name) VALUES(?1,'Harbor')",
                [location.to_string()],
            )
            .unwrap();
        let template = StoryBibleTemplate {
            id: StoryBibleTemplateId::new(),
            name: "Investigator".into(),
            entity_kind: StoryBibleEntityKind::Character,
            fields: vec![
                StoryBibleFieldDefinition {
                    id: "profession".into(),
                    label: "Profession".into(),
                    field_type: StoryBibleFieldType::Text,
                    required: true,
                    enum_values: vec![],
                    reference_kind: None,
                },
                StoryBibleFieldDefinition {
                    id: "temperament".into(),
                    label: "Temperament".into(),
                    field_type: StoryBibleFieldType::Enum,
                    required: true,
                    enum_values: vec!["cautious".into(), "impulsive".into()],
                    reference_kind: None,
                },
                StoryBibleFieldDefinition {
                    id: "home".into(),
                    label: "Home".into(),
                    field_type: StoryBibleFieldType::EntityRef,
                    required: false,
                    enum_values: vec![],
                    reference_kind: Some(StoryBibleEntityKind::Location),
                },
                StoryBibleFieldDefinition {
                    id: "last_seen".into(),
                    label: "Last seen".into(),
                    field_type: StoryBibleFieldType::DateTime,
                    required: false,
                    enum_values: vec![],
                    reference_kind: None,
                },
            ],
        };
        store.save_story_bible_template(&template).unwrap();
        assert_eq!(
            store.list_story_bible_templates().unwrap(),
            vec![template.clone()]
        );
        let mut values = std::collections::BTreeMap::new();
        values.insert(
            "profession".into(),
            StoryBibleFieldValue::Text("Investigator".into()),
        );
        values.insert(
            "temperament".into(),
            StoryBibleFieldValue::Enum("reckless".into()),
        );
        let mut profile = StoryBibleProfile {
            entity_kind: StoryBibleEntityKind::Character,
            entity_id: character.to_string(),
            template_id: template.id,
            values: values.clone(),
        };
        assert!(store.save_story_bible_profile(&profile).is_err());
        values.insert(
            "temperament".into(),
            StoryBibleFieldValue::Enum("cautious".into()),
        );
        values.insert(
            "home".into(),
            StoryBibleFieldValue::EntityRef(Uuid::new_v4().to_string()),
        );
        profile.values = values.clone();
        assert!(store.save_story_bible_profile(&profile).is_err());
        values.insert(
            "home".into(),
            StoryBibleFieldValue::EntityRef(location.to_string()),
        );
        values.insert(
            "last_seen".into(),
            StoryBibleFieldValue::DateTime("not a date".into()),
        );
        profile.values = values.clone();
        assert!(store.save_story_bible_profile(&profile).is_err());
        values.insert(
            "last_seen".into(),
            StoryBibleFieldValue::DateTime("2026-11-21T12:30:00Z".into()),
        );
        profile.values = values;
        store.save_story_bible_profile(&profile).unwrap();
        drop(store);
        let store = ProjectStore::open(&project).unwrap();
        assert_eq!(
            store.load_story_bible_template(template.id).unwrap(),
            template
        );
        assert_eq!(
            store
                .load_story_bible_profile(StoryBibleEntityKind::Character, &character.to_string())
                .unwrap(),
            profile
        );
    }

    #[test]
    fn chapter_content_units_persist_and_compile_without_scenes() {
        let d = tempdir().unwrap();
        let project = d.path().join("Book");
        let mut st = ProjectStore::create_for(
            &project,
            &Project {
                id: ProjectId::new(),
                title: "Book".into(),
                author: String::new(),
                genre: String::new(),
                synopsis: String::new(),
                book_type: BookType::Nonfiction,
                template_id: None,
                target_words: None,
                story_epoch_label: "Day 0".into(),
                story_calendar_id: None,
                language_code: default_language_code(),
                word_count_policy: WordCountPolicy::default(),
            },
        )
        .unwrap();
        let chapter = st
            .create_node(None, NodeKind::Chapter, "Chapter 1", 0)
            .unwrap();
        let unit = st
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Chapter 1",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let (_, hash) = st.load_content_unit_text(unit).unwrap();
        st.save_content_unit_text(unit, Some(&hash), "A chapter with prose and no scenes.")
            .unwrap();
        assert_eq!(st.get_content_unit(unit).unwrap().actual_words, 7);
        drop(st);
        let st = ProjectStore::open(&project).unwrap();
        assert_eq!(st.get_project().unwrap().book_type, BookType::Nonfiction);
        assert_eq!(
            st.load_content_unit_text(unit).unwrap().0,
            "A chapter with prose and no scenes."
        );
        st.validate().unwrap();
    }

    #[test]
    fn content_unit_constraints_reject_wrong_parent_kind_and_id_collisions_in_sql() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("Book"), "Book").unwrap();
        let section = store
            .create_node(None, NodeKind::Section, "Part", 0)
            .unwrap();
        let chapter = store
            .create_node(Some(section), NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let _section_body = store
            .create_content_unit(
                ContentUnitKind::SectionBody,
                "Part introduction",
                Some(section),
                0,
                None,
            )
            .unwrap();
        let unit = store
            .create_content_unit(ContentUnitKind::ChapterBody, "Body", Some(chapter), 0, None)
            .unwrap();
        assert!(
            store
                .conn
                .execute(
                    "UPDATE structural_node SET kind='group' WHERE id=?1",
                    [section.to_string()]
                )
                .is_err()
        );
        assert!(
            store
                .conn
                .execute(
                    "UPDATE content_unit SET kind='section_body' WHERE id=?1",
                    [unit.to_string()]
                )
                .is_err()
        );
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        assert!(
            store
                .conn
                .execute(
                    "UPDATE scene SET id=?2 WHERE id=?1",
                    params![scene.to_string(), unit.to_string()]
                )
                .is_err()
        );
    }

    #[test]
    fn metadata_sql_snapshot_is_deterministic_consistent_and_replayable() {
        let d = tempdir().unwrap();
        let project = d.path().join("Novel");
        let mut store = ProjectStore::create(&project, "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "One", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Quoted ' title").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        store
            .save_text(scene, Some(&hash), "Élodie said: 'hello'.")
            .unwrap();
        let first = store
            .with_git_snapshot_barrier(|root| {
                fs::read(root.join("snapshots/database.sql")).map_err(|e| e.to_string())
            })
            .unwrap();
        let second = store
            .with_git_snapshot_barrier(|root| {
                fs::read(root.join("snapshots/database.sql")).map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(first, second);
        let dump = String::from_utf8(first).unwrap();
        assert!(!dump.contains("CREATE VIRTUAL TABLE \"scene_fts\"_data"));
        let restored = Connection::open_in_memory().unwrap();
        restored.execute_batch(&dump).unwrap();
        let integrity: String = restored
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        let title: String = restored
            .query_row(
                "SELECT title FROM scene WHERE id=?1",
                [scene.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Quoted ' title");
        let transient_rows: i64 = restored
            .query_row("SELECT count(*) FROM write_journal", [], |r| r.get(0))
            .unwrap();
        assert_eq!(transient_rows, 0);
    }

    #[test]
    fn content_unit_write_journal_recovers_interrupted_replacements() {
        let failpoints = [
            ContentUnitSaveFailpoint::AfterPartialTemp,
            ContentUnitSaveFailpoint::AfterTempSync,
            ContentUnitSaveFailpoint::AfterIntent,
            ContentUnitSaveFailpoint::BeforeRename,
            ContentUnitSaveFailpoint::AfterRename,
            ContentUnitSaveFailpoint::AfterMetadataCommit,
        ];
        for failpoint in failpoints {
            let d = tempdir().unwrap();
            let project = d.path().join("Book");
            let mut st = ProjectStore::create(&project, "Book").unwrap();
            let chapter = st.create_node(None, NodeKind::Chapter, "Ch", 0).unwrap();
            let unit = st
                .create_content_unit(ContentUnitKind::ChapterBody, "Body", Some(chapter), 0, None)
                .unwrap();
            let (_, hash) = st.load_content_unit_text(unit).unwrap();
            assert!(
                st.save_content_unit_text_inner(
                    unit,
                    Some(&hash),
                    "Prepared prose.",
                    Some(failpoint)
                )
                .is_err()
            );
            drop(st);
            let st = ProjectStore::open(&project)
                .unwrap_or_else(|error| panic!("reopen after {failpoint:?}: {error}"));
            let expected = if matches!(
                failpoint,
                ContentUnitSaveFailpoint::AfterPartialTemp
                    | ContentUnitSaveFailpoint::AfterTempSync
            ) {
                ""
            } else {
                "Prepared prose."
            };
            assert_eq!(st.load_content_unit_text(unit).unwrap().0, expected);
            assert_eq!(st.pending_content_unit_conflicts().unwrap(), 0);
            st.validate().unwrap();
        }
    }

    #[test]
    fn content_unit_recovery_preserves_external_conflicts() {
        let d = tempdir().unwrap();
        let project = d.path().join("Book");
        let mut st = ProjectStore::create(&project, "Book").unwrap();
        let chapter = st.create_node(None, NodeKind::Chapter, "Ch", 0).unwrap();
        let unit = st
            .create_content_unit(ContentUnitKind::ChapterBody, "Body", Some(chapter), 0, None)
            .unwrap();
        let (_, hash) = st.load_content_unit_text(unit).unwrap();
        assert!(
            st.save_content_unit_text_inner(
                unit,
                Some(&hash),
                "Prepared version",
                Some(ContentUnitSaveFailpoint::AfterIntent)
            )
            .is_err()
        );
        let path = st.safe_path(&st.content_unit_path(unit).unwrap()).unwrap();
        fs::write(&path, "External version").unwrap();
        drop(st);
        let mut st = ProjectStore::open(&project).unwrap();
        assert_eq!(st.pending_content_unit_conflicts().unwrap(), 1);
        let conflicts = st.list_content_unit_recovery_conflicts().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(fs::read_to_string(path).unwrap(), "External version");
        assert_eq!(
            fs::read_to_string(conflicts[0].recovery_path.join("prepared-version.md")).unwrap(),
            "Prepared version"
        );
        st.resolve_content_unit_conflict(&conflicts[0].transaction_id, true)
            .unwrap();
        assert_eq!(
            st.load_content_unit_text(unit).unwrap().0,
            "Prepared version"
        );
        assert_eq!(
            fs::read_to_string(conflicts[0].recovery_path.join("external-version.md")).unwrap(),
            "External version"
        );
    }
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
                .map(|p| p.source)
                .collect::<Vec<_>>(),
            vec![EntityRef::Scene(b), EntityRef::Scene(a)]
        );
        let research = st
            .create_research_item(
                "Research excerpt",
                "A brass compass points toward the buried city.",
                "text/markdown",
                None,
                None,
            )
            .unwrap();
        st.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(a),
            source_anchor: None,
            target: EntityRef::Research(research),
            target_anchor: None,
            kind: LinkKind::Includes,
            label: "document excerpt".into(),
        })
        .unwrap();
        let source_bytes = b"A field guide to obsidian doors.".to_vec();
        let reading_doc = story_reading::ReferenceDocument {
            title: "Field Guide".into(),
            media_type: "text/plain".into(),
            sha256: format!("{:x}", Sha256::digest(&source_bytes)),
            extracted_text: "A field guide to obsidian doors.".into(),
            source_bytes,
            language_code: Some("en-GB".into()),
        };
        let reading_id = st
            .import_reading_document(&reading_doc, "fixture:field-guide")
            .unwrap();
        let imported = st.get_reading_document(reading_id).unwrap();
        assert_eq!(imported.language_code.as_deref(), Some("en-GB"));
        st.set_reading_document_language(reading_id, Some("it-IT"))
            .unwrap();
        assert_eq!(
            st.get_reading_document(reading_id)
                .unwrap()
                .language_code
                .as_deref(),
            Some("it-IT")
        );
        assert!(
            st.set_reading_document_language(reading_id, Some("en--US"))
                .is_err()
        );
        assert_eq!(
            fs::read(st.root.join(imported.relative_path)).unwrap(),
            reading_doc.source_bytes.as_slice()
        );
        st.create_story_link(&StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(a),
            source_anchor: None,
            target: EntityRef::Reading(reading_id),
            target_anchor: None,
            kind: LinkKind::Transcludes,
            label: "reference text".into(),
        })
        .unwrap();
        let expanded = st.expand_scene_transclusions(b).unwrap();
        assert_eq!(expanded.len(), 4);
        let research_passage = expanded
            .iter()
            .find(|passage| passage.source == EntityRef::Research(research))
            .unwrap();
        assert_eq!(
            research_passage.text,
            "A brass compass points toward the buried city."
        );
        let reading_passage = expanded
            .iter()
            .find(|passage| passage.source == EntityRef::Reading(reading_id))
            .unwrap();
        assert_eq!(reading_passage.text, reading_doc.extracted_text);
        assert_eq!(
            st.backlinks(EntityRef::Reading(reading_id)).unwrap().len(),
            1
        );
        assert!(
            st.create_story_link(&StoryLink {
                id: LinkId::new(),
                source: EntityRef::Reading(reading_id),
                source_anchor: None,
                target: EntityRef::Scene(b),
                target_anchor: None,
                kind: LinkKind::Includes,
                label: String::new(),
            })
            .is_err()
        );
        assert!(
            st.create_story_link(&StoryLink {
                id: LinkId::new(),
                source: EntityRef::Research(research),
                source_anchor: None,
                target: EntityRef::Scene(b),
                target_anchor: None,
                kind: LinkKind::Includes,
                label: String::new(),
            })
            .is_err()
        );
        let character = CharacterId::new();
        st.add_character(character, "Not a document").unwrap();
        let sql_link = LinkId::new();
        assert!(st.conn.execute(
            "INSERT INTO story_link(id,source_kind,source_id,target_kind,target_id,link_kind,label,created_at) VALUES(?1,'scene',?2,'character',?3,'includes','',?4)",
            params![sql_link.to_string(), a.to_string(), character.to_string(), now()]
        ).is_err());
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
        let imported_reading = st.get_reading_document(reading_id).unwrap();
        fs::write(
            st.root.join(&imported_reading.relative_path),
            "externally changed source",
        )
        .unwrap();
        assert!(matches!(
            st.expand_document_inclusions(EntityRef::Scene(b)),
            Err(StorageError::Conflict(_))
        ));
        st.move_scene(b, other_chapter, 0).unwrap();
        let renamed = Scene {
            title: "Renamed B".into(),
            ..st.get_scene(b).unwrap().scene
        };
        st.update_scene(&renamed).unwrap();
        assert_eq!(st.backlinks(EntityRef::Scene(b)).unwrap()[0].id, link.id);
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
        let alternate_character = CharacterId::new();
        st.add_character(alternate_character, "Ivo").unwrap();
        st.link_character(second, character, ParticipationRole::Pov)
            .unwrap();
        st.link_character(first, alternate_character, ParticipationRole::Major)
            .unwrap();
        let query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Mara scenes".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::Character(character)],
        };
        st.save_scene_query(&query).unwrap();
        let assertions = [
            QueryAssertion::AtLeast(1),
            QueryAssertion::AtMost(3),
            QueryAssertion::NonEmpty,
        ];
        st.set_query_assertions(query.id, &assertions).unwrap();
        assert_eq!(st.get_query_assertions(query.id).unwrap(), assertions);
        assert!(st.set_query_assertions(QueryId::new(), &[]).is_err());
        assert!(
            st.set_query_assertions(query.id, &[QueryAssertion::Exactly(u64::MAX)])
                .is_err()
        );
        assert_eq!(st.list_saved_queries().unwrap()[0].name, "Mara scenes");
        assert_eq!(st.run_scene_query(query.id).unwrap(), vec![second]);
        st.link_character(first, character, ParticipationRole::Minor)
            .unwrap();
        assert_eq!(st.run_scene_query(query.id).unwrap(), vec![first, second]);
        let plot_a = PlotlineId::new();
        let plot_b = PlotlineId::new();
        st.add_plotline(plot_a, "Political").unwrap();
        st.add_plotline(plot_b, "Personal").unwrap();
        st.link_plotline(first, plot_a, "main", None).unwrap();
        let multi = SavedSceneQuery {
            id: QueryId::new(),
            name: "Any thread participant, excluding complete scenes".into(),
            description: String::new(),
            filters: vec![
                SceneQueryFilter::AnyCharacter(vec![character, alternate_character]),
                SceneQueryFilter::AnyPlotline(vec![plot_a, plot_b]),
                SceneQueryFilter::ExcludeStatus(SceneStatus::Complete),
            ],
        };
        st.save_scene_query(&multi).unwrap();
        assert_eq!(st.run_scene_query(multi.id).unwrap(), vec![first]);
        assert!(st
            .conn
            .execute(
                "INSERT INTO saved_query_filter(query_id,ordinal,filter_kind,value) VALUES(?1,99,'invalid','x')",
                [multi.id.to_string()],
            )
            .is_err());
        let empty_any = SavedSceneQuery {
            id: QueryId::new(),
            name: "Invalid empty OR".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::AnyCharacter(Vec::new())],
        };
        assert!(st.save_scene_query(&empty_any).is_err());
        let order = query_scene_ids(&st.conn, &chapter.to_string()).unwrap();
        assert_eq!(order, vec![first.to_string(), second.to_string()]);
        drop(st);
        let reopened = ProjectStore::open(d.path().join("N")).unwrap();
        assert_eq!(reopened.get_query_assertions(query.id).unwrap(), assertions);
    }

    #[test]
    fn inspection_queries_distinguish_confirmed_presence_mentions_and_story_facts() {
        let d = tempdir().unwrap();
        let mut store = ProjectStore::create(d.path().join("Inspect"), "Inspect").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let present = store.create_scene(chapter, 0, "The meeting").unwrap();
        let mentioned = store.create_scene(chapter, 1, "A report").unwrap();
        let mara = CharacterId::new();
        store.add_character(mara, "Mara").unwrap();
        store
            .link_character(present, mara, ParticipationRole::Major)
            .unwrap();
        store
            .link_character(mentioned, mara, ParticipationRole::Mentioned)
            .unwrap();
        store
            .record_character_absence(mentioned, mara, "Confirmed elsewhere")
            .unwrap();
        let (_, initial_hash) = store.load_text(present).unwrap();
        store
            .save_text(present, Some(&initial_hash), "Mara entered the station.")
            .unwrap();
        let location = LocationId::new();
        store.add_location(location, "Station").unwrap();
        let field = FieldId::new();
        store
            .define_custom_field(&CustomFieldDefinition {
                id: field,
                name: "Weather".into(),
                field_type: FieldType::Enum,
                required: false,
                default_value: None,
                enum_values: vec!["Rain".into(), "Clear".into()],
                reference_kind: None,
            })
            .unwrap();
        store
            .set_custom_field(present, field, &FieldValue::Enum("Rain".into()))
            .unwrap();
        let mut scene = store.get_scene(present).unwrap().scene;
        scene.start_location = Some(location);
        store.update_scene(&scene).unwrap();

        let query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Present at station".into(),
            description: String::new(),
            filters: vec![],
        };
        store.save_scene_query(&query).unwrap();
        store
            .set_scene_query_expression(
                query.id,
                Some(&QueryExpression::All(vec![
                    QueryExpression::Filter(SceneQueryFilter::CharacterPresent(mara)),
                    QueryExpression::Filter(SceneQueryFilter::Location(location)),
                    QueryExpression::Filter(SceneQueryFilter::TextContains("entered".into())),
                    QueryExpression::Filter(SceneQueryFilter::Chapter(chapter.to_string())),
                ])),
            )
            .unwrap();
        assert_eq!(store.run_scene_query(query.id).unwrap(), vec![present]);
        store
            .set_scene_query_expression(
                query.id,
                Some(&QueryExpression::Filter(
                    SceneQueryFilter::CharacterMentioned(mara),
                )),
            )
            .unwrap();
        assert_eq!(store.run_scene_query(query.id).unwrap(), vec![mentioned]);

        let direct = SavedSceneQuery {
            id: QueryId::new(),
            name: "direct inspection filters".into(),
            description: String::new(),
            filters: vec![
                SceneQueryFilter::CharacterPresent(mara),
                SceneQueryFilter::Location(location),
                SceneQueryFilter::TextContains("entered".into()),
            ],
        };
        store.save_scene_query(&direct).unwrap();
        assert_eq!(store.run_scene_query(direct.id).unwrap(), vec![present]);
        let metadata = SavedSceneQuery {
            id: QueryId::new(),
            name: "rain scenes".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::CustomMetadataEquals {
                field_id: field,
                value: FieldValue::Enum("Rain".into()),
            }],
        };
        store.save_scene_query(&metadata).unwrap();
        assert_eq!(store.run_scene_query(metadata.id).unwrap(), vec![present]);
        let notes = FieldId::new();
        store
            .define_custom_field(&CustomFieldDefinition {
                id: notes,
                name: "Author notes".into(),
                field_type: FieldType::LongText,
                required: false,
                default_value: None,
                enum_values: vec![],
                reference_kind: None,
            })
            .unwrap();
        store
            .set_custom_field(
                present,
                notes,
                &FieldValue::Text("A red key appears".into()),
            )
            .unwrap();
        let priority = FieldId::new();
        store
            .define_custom_field(&CustomFieldDefinition {
                id: priority,
                name: "Priority".into(),
                field_type: FieldType::Integer,
                required: false,
                default_value: None,
                enum_values: vec![],
                reference_kind: None,
            })
            .unwrap();
        store
            .set_custom_field(present, priority, &FieldValue::Integer(7))
            .unwrap();
        for (name, filter) in [
            (
                "metadata exists",
                SceneQueryFilter::CustomMetadataExists { field_id: notes },
            ),
            (
                "metadata text",
                SceneQueryFilter::CustomMetadataTextContains {
                    field_id: notes,
                    text: "red key".into(),
                },
            ),
            (
                "metadata numeric",
                SceneQueryFilter::CustomMetadataNumberCompare {
                    field_id: priority,
                    comparison: MetadataComparison::GreaterThanOrEqual,
                    value: FieldValue::Integer(7),
                },
            ),
        ] {
            let query = SavedSceneQuery {
                id: QueryId::new(),
                name: name.into(),
                description: String::new(),
                filters: vec![filter],
            };
            store.save_scene_query(&query).unwrap();
            assert_eq!(store.run_scene_query(query.id).unwrap(), vec![present]);
        }
        let bad_numeric = SavedSceneQuery {
            id: QueryId::new(),
            name: "invalid numeric metadata".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::CustomMetadataNumberCompare {
                field_id: notes,
                comparison: MetadataComparison::Equal,
                value: FieldValue::Integer(1),
            }],
        };
        assert!(store.save_scene_query(&bad_numeric).is_err());
        let absence = SavedSceneQuery {
            id: QueryId::new(),
            name: "explicit absence only".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::CharacterAbsent(mara)],
        };
        store.save_scene_query(&absence).unwrap();
        assert_eq!(store.run_scene_query(absence.id).unwrap(), vec![mentioned]);
        assert_eq!(
            store.scene_character_absences(mentioned).unwrap()[0].2,
            "Confirmed elsewhere"
        );
        assert!(
            store
                .link_character(mentioned, mara, ParticipationRole::Major)
                .is_err()
        );
    }

    #[test]
    fn event_prerequisites_are_persistent_and_sql_prevents_cycles() {
        let directory = tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Events"), "Events").unwrap();
        let first = EventId::new();
        let second = EventId::new();
        let third = EventId::new();
        store.add_event(first, "The message", "day 1").unwrap();
        store.add_event(second, "The response", "day 2").unwrap();
        store.add_event(third, "The consequence", "day 3").unwrap();
        let dependency = StoryEventDependency {
            predecessor: first,
            successor: second,
            kind: "precedes".into(),
            notes: "The response follows delivery".into(),
        };
        store.add_event_dependency(&dependency).unwrap();
        let mut updated_dependency = dependency.clone();
        updated_dependency.notes = "The message reaches the city first".into();
        store.update_event_dependency(&updated_dependency).unwrap();
        store
            .add_event_dependency(&StoryEventDependency {
                predecessor: second,
                successor: third,
                kind: "requires".into(),
                notes: String::new(),
            })
            .unwrap();
        assert_eq!(store.list_event_dependencies().unwrap().len(), 2);
        assert!(
            store
                .add_event_dependency(&StoryEventDependency {
                    predecessor: third,
                    successor: first,
                    kind: "precedes".into(),
                    notes: String::new()
                })
                .is_err()
        );
        assert!(store.conn.execute("INSERT INTO story_event_dependency(predecessor_event_id,successor_event_id,dependency_kind) VALUES(?1,?2,'precedes')", params![third.to_string(), first.to_string()]).is_err());
        drop(store);
        let reopened = ProjectStore::open(directory.path().join("Events")).unwrap();
        let reopened_dependencies = reopened.list_event_dependencies().unwrap();
        assert_eq!(reopened_dependencies.len(), 2);
        assert!(reopened_dependencies.contains(&updated_dependency));
        let final_dependency = StoryEventDependency {
            predecessor: second,
            successor: third,
            kind: "requires".into(),
            notes: String::new(),
        };
        assert!(reopened_dependencies.contains(&final_dependency));
        let mut reopened = reopened;
        reopened
            .remove_event_dependency(second, third, "requires")
            .unwrap();
        assert_eq!(
            reopened.list_event_dependencies().unwrap(),
            vec![updated_dependency]
        );
        reopened
            .remove_event_dependency(first, second, "precedes")
            .unwrap();
        assert!(reopened.list_event_dependencies().unwrap().is_empty());
    }

    #[test]
    fn location_incompatibility_rules_persist_and_reject_invalid_sql() {
        let directory = tempdir().unwrap();
        let project = directory.path().join("Location Rules");
        let mut store = ProjectStore::create(&project, "Location Rules").unwrap();
        let left = LocationId::new();
        let right = LocationId::new();
        store.add_location(left, "The bridge").unwrap();
        store.add_location(right, "The sealed room").unwrap();
        let rule = LocationIncompatibilityRule {
            id: ContinuityRuleId::new(),
            first_location: left,
            second_location: right,
            description: "cannot be occupied simultaneously".into(),
            enabled: true,
        };
        store.add_location_incompatibility_rule(&rule).unwrap();
        assert_eq!(
            store.list_location_incompatibility_rules().unwrap().len(),
            1
        );
        assert!(store.add_location_incompatibility_rule(&rule).is_err());
        let bad_id = ContinuityRuleId::new();
        assert!(store.conn.execute(
            "INSERT INTO location_incompatibility_rule(id,first_location_id,second_location_id,description,enabled,created_at) VALUES(?1,?2,?2,'self',1,'now')",
            params![bad_id.to_string(), left.to_string()]
        ).is_err());
        assert!(store.conn.execute(
            "INSERT INTO location_incompatibility_rule(id,first_location_id,second_location_id,description,enabled,created_at) VALUES(?1,?2,?3,'missing',1,'now')",
            params![bad_id.to_string(), left.to_string(), Uuid::new_v4().to_string()]
        ).is_err());
        store.remove_location_incompatibility_rule(rule.id).unwrap();
        drop(store);
        assert!(
            ProjectStore::open(project)
                .unwrap()
                .list_location_incompatibility_rules()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn location_profiles_validate_hierarchy_and_index_description() {
        let directory = tempdir().unwrap();
        let project = directory.path().join("Location Profiles");
        let mut store = ProjectStore::create(&project, "Location Profiles").unwrap();
        let parent = LocationId::new();
        let child = LocationId::new();
        store.add_location(parent, "The coast").unwrap();
        store.add_location(child, "The harbour").unwrap();
        let mut profile = store.get_location(child).unwrap();
        profile.parent = Some(parent);
        profile.description = "A narrow harbour below the cliffs".into();
        store.update_location(&profile).unwrap();
        let stored = store.get_location(child).unwrap();
        assert_eq!(stored.id, profile.id);
        assert_eq!(stored.parent, profile.parent);
        assert_eq!(stored.name, profile.name);
        assert_eq!(stored.description, profile.description);
        assert_eq!(
            store.search_project("narrow harbour", 10).unwrap()[0].entity_id,
            child.to_string()
        );
        assert!(
            store
                .conn
                .execute(
                    "UPDATE location SET parent_id=?2 WHERE id=?1",
                    params![parent.to_string(), child.to_string()]
                )
                .is_err()
        );
        assert!(
            store
                .conn
                .execute(
                    "UPDATE location SET parent_id=?2 WHERE id=?1",
                    params![child.to_string(), Uuid::new_v4().to_string()]
                )
                .is_err()
        );
        drop(store);
        let reopened = ProjectStore::open(project)
            .unwrap()
            .get_location(child)
            .unwrap();
        assert_eq!(reopened.id, profile.id);
        assert_eq!(reopened.parent, profile.parent);
        assert_eq!(reopened.name, profile.name);
        assert_eq!(reopened.description, profile.description);
    }

    #[test]
    fn query_filters_cover_assembly_membership_and_all_issue_states() {
        let directory = tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Queries"), "Queries").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let other = store.create_scene(chapter, 1, "Other").unwrap();
        let assembly = store.create_assembly("Selected edition").unwrap();
        store.add_assembly_scene(assembly, scene, 0).unwrap();
        let issue = store.create_issue("Check the clue").unwrap();
        store.link_issue(scene, issue).unwrap();

        let in_assembly = SavedSceneQuery {
            id: QueryId::new(),
            name: "In edition".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::Assembly(assembly)],
        };
        store.save_scene_query(&in_assembly).unwrap();
        assert_eq!(store.run_scene_query(in_assembly.id).unwrap(), vec![scene]);

        let issue_query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Resolved clue".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::IssueStatus("resolved".into())],
        };
        store.save_scene_query(&issue_query).unwrap();
        assert!(store.run_scene_query(issue_query.id).unwrap().is_empty());
        store.set_issue_status(issue, "resolved").unwrap();
        assert_eq!(store.run_scene_query(issue_query.id).unwrap(), vec![scene]);
        store.set_issue_status(issue, "deferred").unwrap();
        assert!(store.run_scene_query(issue_query.id).unwrap().is_empty());
        assert!(
            store
                .run_scene_query(in_assembly.id)
                .unwrap()
                .contains(&scene)
        );
        assert!(
            !store
                .run_scene_query(in_assembly.id)
                .unwrap()
                .contains(&other)
        );
        assert!(
            store
                .save_scene_query(&SavedSceneQuery {
                    id: QueryId::new(),
                    name: "Bad status".into(),
                    description: String::new(),
                    filters: vec![SceneQueryFilter::IssueStatus("maybe".into())]
                })
                .is_err()
        );
    }

    #[test]
    fn uncertainty_aware_time_query_filters_only_assert_supported_relations() {
        let directory = tempdir().unwrap();
        let mut store =
            ProjectStore::create(directory.path().join("Chronology"), "Chronology").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let exact_before = store.create_scene(chapter, 0, "Exact before").unwrap();
        let bounded = store.create_scene(chapter, 1, "Possibly before").unwrap();
        let unknown = store.create_scene(chapter, 2, "Unknown").unwrap();
        let mut scene = store.get_scene(exact_before).unwrap().scene;
        scene.start_time = StoryTime::exact(-5);
        store.update_scene(&scene).unwrap();
        let mut scene = store.get_scene(bounded).unwrap().scene;
        scene.start_time =
            StoryTime::new(Some(10), Some(4), Some(16), TimePrecision::Bounded).unwrap();
        store.update_scene(&scene).unwrap();
        let cutoff = 10;
        let make_query = |name: &str, filter| SavedSceneQuery {
            id: QueryId::new(),
            name: name.into(),
            description: String::new(),
            filters: vec![filter],
        };
        let before_definite = make_query(
            "before definitely",
            SceneQueryFilter::TimeBeforeDefinitely(cutoff),
        );
        let before_possible = make_query(
            "before possibly",
            SceneQueryFilter::TimeBeforePossibly(cutoff),
        );
        let after_definite = make_query(
            "after definitely",
            SceneQueryFilter::TimeAfterDefinitely(cutoff),
        );
        let after_possible = make_query(
            "after possibly",
            SceneQueryFilter::TimeAfterPossibly(cutoff),
        );
        for query in [
            &before_definite,
            &before_possible,
            &after_definite,
            &after_possible,
        ] {
            store.save_scene_query(query).unwrap();
        }
        assert_eq!(
            store.run_scene_query(before_definite.id).unwrap(),
            vec![exact_before]
        );
        assert_eq!(
            store.run_scene_query(before_possible.id).unwrap(),
            vec![exact_before, bounded]
        );
        assert!(store.run_scene_query(after_definite.id).unwrap().is_empty());
        assert_eq!(
            store.run_scene_query(after_possible.id).unwrap(),
            vec![bounded]
        );
        assert!(
            !store
                .run_scene_query(before_possible.id)
                .unwrap()
                .contains(&unknown)
        );
    }

    #[test]
    fn nested_saved_query_expressions_are_validated_persistent_and_dynamic() {
        let directory = tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let first = store.create_scene(chapter, 0, "First").unwrap();
        let second = store.create_scene(chapter, 1, "Second").unwrap();
        let third = store.create_scene(chapter, 2, "Third").unwrap();
        let mara = CharacterId::new();
        let ivo = CharacterId::new();
        store.add_character(mara, "Mara").unwrap();
        store.add_character(ivo, "Ivo").unwrap();
        store
            .link_character(first, mara, ParticipationRole::Major)
            .unwrap();
        store
            .link_character(second, ivo, ParticipationRole::Major)
            .unwrap();
        let query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Active scenes for either character".into(),
            description: String::new(),
            filters: Vec::new(),
        };
        store.save_scene_query(&query).unwrap();
        let expression = QueryExpression::All(vec![
            QueryExpression::Any(vec![
                QueryExpression::Filter(SceneQueryFilter::Character(mara)),
                QueryExpression::Filter(SceneQueryFilter::Character(ivo)),
            ]),
            QueryExpression::Not(Box::new(QueryExpression::Filter(SceneQueryFilter::Status(
                SceneStatus::Complete,
            )))),
        ]);
        store
            .set_scene_query_expression(query.id, Some(&expression))
            .unwrap();
        assert_eq!(
            store.get_scene_query_expression(query.id).unwrap(),
            Some(expression)
        );
        assert_eq!(
            store.run_scene_query(query.id).unwrap(),
            vec![first, second]
        );
        store
            .conn
            .execute(
                "UPDATE scene SET status='complete' WHERE id=?1",
                [second.to_string()],
            )
            .unwrap();
        assert_eq!(store.run_scene_query(query.id).unwrap(), vec![first]);
        assert!(
            store
                .set_scene_query_expression(query.id, Some(&QueryExpression::All(vec![])))
                .is_err()
        );
        assert!(
            store
                .set_scene_query_expression(QueryId::new(), None)
                .is_err()
        );
        drop(store);
        let reopened = ProjectStore::open(directory.path().join("Novel")).unwrap();
        assert_eq!(reopened.run_scene_query(query.id).unwrap(), vec![first]);
        assert!(!reopened.run_scene_query(query.id).unwrap().contains(&third));
        reopened
            .conn
            .query_row(
                "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|version| assert_eq!(version, 42))
            .unwrap();
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
    fn multilingual_fts_is_caseless_accent_insensitive_and_preserves_prose() {
        let directory = tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Café").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        let source = "Élodie likes café and déjà-vu; 한국어도 씁니다.";
        store.save_text(scene, Some(&hash), source).unwrap();

        // SQLite FTS5 unicode61 uses its default simple Unicode case folding
        // and diacritic folding. This affects only the search index, never the
        // canonical UTF-8 manuscript file.
        assert_eq!(store.search_scenes("ELODIE").unwrap(), vec![scene]);
        assert_eq!(store.search_scenes("E\u{301}LODIE").unwrap(), vec![scene]);
        assert_eq!(store.search_scenes("cafe").unwrap(), vec![scene]);
        assert_eq!(store.search_scenes("deja").unwrap(), vec![scene]);
        assert_eq!(store.search_scenes("한국어도").unwrap(), vec![scene]);
        assert_eq!(store.load_text(scene).unwrap().0, source);
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
    fn approved_knowledge_requires_existing_source_and_is_search_indexed() {
        let dir = tempdir().unwrap();
        let mut store = ProjectStore::create(dir.path().join("Novel"), "Novel").unwrap();
        let invalid = store.conn.execute(
            "INSERT INTO approved_knowledge_claim(id,kind,name,claim,source_kind,source_id,source_fingerprint,source_length_bytes,evidence_quote,evidence_start_byte,evidence_end_byte,model,approved_at) VALUES(?1,'event','Signal','The signal happened','scene',?2,?3,10,'the signal',0,10,'fixture','2026-01-01')",
            params![KnowledgeClaimId::new().to_string(), Uuid::new_v4().to_string(), "a".repeat(64)],
        );
        assert!(
            invalid
                .unwrap_err()
                .to_string()
                .contains("source scene does not exist")
        );

        let chapter = store
            .create_node(None, NodeKind::Chapter, "One", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "The Signal").unwrap();
        let id = KnowledgeClaimId::new();
        store.conn.execute(
            "INSERT INTO approved_knowledge_claim(id,kind,name,claim,source_kind,source_id,source_fingerprint,source_length_bytes,evidence_quote,evidence_start_byte,evidence_end_byte,model,approved_at) VALUES(?1,'event','Signal','The signal happened','scene',?2,?3,10,'the signal',0,10,'fixture','2026-01-01')",
            params![id.to_string(), scene.to_string(), "a".repeat(64)],
        ).unwrap();
        let claim = store.get_approved_knowledge_claim(id).unwrap();
        assert_eq!(claim.source_id, scene.to_string());
        assert_eq!(claim.evidence_quote, "the signal");
        assert!(
            store
                .search_project("signal happened", 10)
                .unwrap()
                .iter()
                .any(|hit| hit.entity_kind == "knowledge_claim" && hit.entity_id == id.to_string())
        );
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
                    prefix_context: "Mara finds a ".into(),
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
        fs::write(
            &changed,
            "Earlier text. Mara finds a silver key. Mara hides the silver key.",
        )
        .unwrap();
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
        conn.execute_batch("DROP TRIGGER reading_document_language_insert; DROP TRIGGER reading_document_language_update; ALTER TABLE reading_document DROP COLUMN language_code; DROP TRIGGER location_parent_insert; DROP TRIGGER location_parent_update; DROP TRIGGER project_search_location_profile_update; ALTER TABLE location DROP COLUMN description; ALTER TABLE location DROP COLUMN parent_id; DROP TRIGGER story_event_dependency_no_cycle; DROP TABLE story_event_dependency; DROP TRIGGER character_absence_conflicts_with_presence; DROP TRIGGER character_presence_conflicts_with_absence; DROP TRIGGER character_presence_update_conflicts_with_absence; DROP TABLE scene_character_absence; DROP TABLE writing_goal_snapshot; DROP TABLE writing_goal_event; DROP TABLE writing_goal; DROP TABLE saved_query_expression")
            .unwrap();
        conn.execute_batch("DROP TABLE diagnostic_suppression; DROP TABLE writing_session; DROP TABLE saved_query_assertion; DROP TABLE writing_challenge; DROP TRIGGER planning_board_connector_scope_insert; DROP TRIGGER planning_board_entity_insert; DROP TRIGGER planning_board_entity_update; DROP TABLE planning_board_connector; DROP TABLE planning_board_item; DROP TABLE planning_board; DROP TRIGGER scratchpad_scene_delete; DROP TRIGGER scratchpad_character_delete; DROP TRIGGER scratchpad_location_delete; DROP TRIGGER scratchpad_object_delete; DROP TRIGGER scratchpad_plotline_delete; DROP TRIGGER scratchpad_research_delete; DROP TABLE assembly_item; DROP TABLE manuscript_assembly; DROP TABLE personal_comment; DROP TABLE scratchpad_note; DROP TABLE detected_mention_candidate; DROP TABLE detected_mention; DROP TABLE story_object; DROP TRIGGER structural_node_kind_not_content_unit; DROP TRIGGER scene_update_id_not_content_unit; DROP TRIGGER content_unit_kind_insert; DROP TRIGGER content_unit_kind_update; DROP TRIGGER content_unit_search_insert; DROP TRIGGER content_unit_search_update; DROP TRIGGER content_unit_search_delete; DROP TRIGGER content_unit_search_text_insert; DROP TRIGGER content_unit_search_text_update; DROP TRIGGER scene_id_not_content_unit; DROP TABLE content_unit_write_conflict; DROP TABLE content_unit_write_journal; DROP TABLE content_unit_search_content; DROP TABLE content_unit; ALTER TABLE project DROP COLUMN template_id; ALTER TABLE project DROP COLUMN book_type; DROP TRIGGER story_link_inclusion_documents_insert; DROP TRIGGER story_link_inclusion_documents_update; DROP TRIGGER project_search_custom_field_insert; DROP TRIGGER project_search_custom_field_update; DROP TRIGGER project_search_custom_field_delete; DROP TRIGGER project_search_content_insert; DROP TRIGGER project_search_content_update; DROP TRIGGER project_search_content_delete; DROP TRIGGER project_search_scene_insert; DROP TRIGGER project_search_scene_update; DROP TRIGGER project_search_scene_delete; DROP TRIGGER project_search_scene_text_insert; DROP TRIGGER project_search_scene_text_update; DROP TRIGGER project_search_scene_text_delete; DROP TRIGGER project_search_character_insert; DROP TRIGGER project_search_character_update; DROP TRIGGER project_search_character_delete; DROP TRIGGER project_search_character_profile_update; DROP TRIGGER project_search_character_alias_insert; DROP TRIGGER project_search_character_alias_delete; DROP TRIGGER project_search_location_insert; DROP TRIGGER project_search_location_update; DROP TRIGGER project_search_location_delete; DROP TRIGGER project_search_plotline_insert; DROP TRIGGER project_search_plotline_update; DROP TRIGGER project_search_plotline_delete; DROP TRIGGER project_search_event_insert; DROP TRIGGER project_search_event_update; DROP TRIGGER project_search_event_delete; DROP TRIGGER project_search_research_insert; DROP TRIGGER project_search_research_update; DROP TRIGGER project_search_research_delete; DROP TRIGGER project_search_issue_insert; DROP TRIGGER project_search_issue_update; DROP TRIGGER project_search_issue_delete; DROP TRIGGER project_search_reading_insert; DROP TRIGGER project_search_reading_update; DROP TRIGGER project_search_reading_delete; DROP TRIGGER project_search_knowledge_insert; DROP TRIGGER project_search_knowledge_update; DROP TRIGGER project_search_knowledge_delete; DROP TRIGGER approved_knowledge_source_insert; DROP TRIGGER approved_knowledge_source_update; DROP TABLE character_alias; ALTER TABLE character DROP COLUMN status; ALTER TABLE character DROP COLUMN external_conflict; ALTER TABLE character DROP COLUMN internal_conflict; ALTER TABLE character DROP COLUMN secrets; ALTER TABLE character DROP COLUMN motivations; ALTER TABLE character DROP COLUMN goals; ALTER TABLE character DROP COLUMN background; ALTER TABLE character DROP COLUMN description; ALTER TABLE character DROP COLUMN preferred_name; DROP TABLE approved_knowledge_claim; DROP TABLE project_search_fts; DROP TABLE project_search_content; ALTER TABLE project DROP COLUMN language_code; ALTER TABLE project DROP COLUMN word_count_policy; DROP TABLE passage_language_tag; ALTER TABLE scene DROP COLUMN word_count_policy; ALTER TABLE scene DROP COLUMN language_code; DROP TABLE location_incompatibility_rule; DROP TABLE story_event_character; DROP TABLE story_event_location; DROP TABLE story_event_interval_boundary; DROP TABLE scene_interval_boundary; DELETE FROM schema_migrations WHERE version>4;").unwrap();
        drop(conn);
        let st = ProjectStore::open(&project).unwrap();
        assert_eq!(st.search_scenes("obsidian").unwrap(), vec![scene]);
        let version: i64 = st
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 42);
        let hits = st.search_project("obsidian", 10).unwrap();
        assert_eq!(hits[0].entity_id, scene.to_string());
    }

    #[test]
    fn version_eight_story_links_migrate_to_reading_document_endpoints() {
        let d = tempdir().unwrap();
        let project = d.path().join("N");
        let mut store = ProjectStore::create(&project, "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let first = store.create_scene(chapter, 0, "First").unwrap();
        let second = store.create_scene(chapter, 1, "Second").unwrap();
        let old_link = StoryLink {
            id: LinkId::new(),
            source: EntityRef::Scene(first),
            source_anchor: None,
            target: EntityRef::Scene(second),
            target_anchor: None,
            kind: LinkKind::Related,
            label: "preserve me".into(),
        };
        store.create_story_link(&old_link).unwrap();
        drop(store);

        let conn = Connection::open(project.join("story.db")).unwrap();
        conn.execute_batch(
            "DROP TRIGGER reading_document_language_insert; DROP TRIGGER reading_document_language_update; ALTER TABLE reading_document DROP COLUMN language_code;
             DROP TRIGGER location_parent_insert; DROP TRIGGER location_parent_update; DROP TRIGGER project_search_location_profile_update; ALTER TABLE location DROP COLUMN description; ALTER TABLE location DROP COLUMN parent_id; DROP TRIGGER story_event_dependency_no_cycle; DROP TABLE story_event_dependency; DROP TRIGGER character_absence_conflicts_with_presence; DROP TRIGGER character_presence_conflicts_with_absence; DROP TRIGGER character_presence_update_conflicts_with_absence; DROP TABLE scene_character_absence; DROP TABLE writing_goal_snapshot; DROP TABLE writing_goal_event; DROP TABLE writing_goal; DROP TABLE writing_session; DROP TABLE saved_query_assertion; DROP TABLE writing_challenge; DROP TRIGGER planning_board_connector_scope_insert; DROP TRIGGER planning_board_entity_insert; DROP TRIGGER planning_board_entity_update; DROP TABLE planning_board_connector; DROP TABLE planning_board_item; DROP TABLE planning_board; DROP TRIGGER scratchpad_scene_delete; DROP TRIGGER scratchpad_character_delete; DROP TRIGGER scratchpad_location_delete; DROP TRIGGER scratchpad_object_delete; DROP TRIGGER scratchpad_plotline_delete; DROP TRIGGER scratchpad_research_delete; DROP TABLE assembly_item; DROP TABLE manuscript_assembly; DROP TABLE personal_comment; DROP TABLE scratchpad_note; DROP TABLE detected_mention_candidate; DROP TABLE detected_mention; DROP TABLE story_object;
             DROP TRIGGER story_link_source_exists_insert;
             DROP TRIGGER structural_node_kind_not_content_unit; DROP TRIGGER scene_update_id_not_content_unit; DROP TRIGGER content_unit_kind_insert; DROP TRIGGER content_unit_kind_update;
             DROP TRIGGER content_unit_search_insert; DROP TRIGGER content_unit_search_update;
             DROP TRIGGER content_unit_search_delete; DROP TRIGGER content_unit_search_text_insert;
             DROP TRIGGER content_unit_search_text_update; DROP TRIGGER scene_id_not_content_unit;
             DROP TABLE content_unit_write_conflict; DROP TABLE content_unit_write_journal;
             DROP TABLE content_unit_search_content; DROP TABLE content_unit;
             ALTER TABLE project DROP COLUMN template_id; ALTER TABLE project DROP COLUMN book_type;
             DROP TRIGGER story_link_source_exists_update;
             DROP TRIGGER story_link_no_transclusion_cycle;
             DROP TRIGGER story_link_inclusion_documents_insert;
             DROP TRIGGER story_link_inclusion_documents_update;
             DROP INDEX story_link_source;
             DROP INDEX story_link_target;
             CREATE TABLE story_link_v8(
               id TEXT PRIMARY KEY,
               source_kind TEXT NOT NULL CHECK(source_kind IN('scene','character','plotline','research')),
               source_id TEXT NOT NULL,
               source_anchor_id TEXT,
               target_kind TEXT NOT NULL CHECK(target_kind IN('scene','character','plotline','research')),
               target_id TEXT NOT NULL,
               target_anchor_id TEXT,
               link_kind TEXT NOT NULL CHECK(link_kind IN('related','cites','transcludes','includes','navigates')),
               label TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
               CHECK(source_kind<>target_kind OR source_id<>target_id OR link_kind NOT IN('transcludes','includes'))
             );
             INSERT INTO story_link_v8 SELECT * FROM story_link;
             DROP TABLE story_link;
             ALTER TABLE story_link_v8 RENAME TO story_link;
             CREATE INDEX story_link_source ON story_link(source_kind,source_id);
             CREATE INDEX story_link_target ON story_link(target_kind,target_id);
             CREATE TRIGGER story_link_source_exists_insert BEFORE INSERT ON story_link BEGIN SELECT 1; END;
             CREATE TRIGGER story_link_source_exists_update BEFORE UPDATE ON story_link BEGIN SELECT 1; END;
             CREATE TRIGGER story_link_no_transclusion_cycle BEFORE INSERT ON story_link BEGIN SELECT 1; END;
             CREATE TRIGGER story_link_inclusion_documents_insert BEFORE INSERT ON story_link BEGIN SELECT 1; END;
             CREATE TRIGGER story_link_inclusion_documents_update BEFORE UPDATE ON story_link BEGIN SELECT 1; END;
             DROP TRIGGER project_search_character_profile_update;
             DROP TRIGGER project_search_character_alias_insert;
             DROP TRIGGER project_search_character_alias_delete;
             DROP TABLE character_alias;
             ALTER TABLE character DROP COLUMN status; ALTER TABLE character DROP COLUMN external_conflict; ALTER TABLE character DROP COLUMN internal_conflict;
             ALTER TABLE character DROP COLUMN secrets;
             ALTER TABLE character DROP COLUMN motivations;
             ALTER TABLE character DROP COLUMN goals;
             ALTER TABLE character DROP COLUMN background;
             ALTER TABLE character DROP COLUMN description;
             ALTER TABLE character DROP COLUMN preferred_name;
             ALTER TABLE project DROP COLUMN language_code;
             ALTER TABLE project DROP COLUMN word_count_policy;
             DROP TABLE passage_language_tag;
             ALTER TABLE scene DROP COLUMN word_count_policy;
             ALTER TABLE scene DROP COLUMN language_code;
             DROP TABLE saved_query_expression;
             DROP TABLE location_incompatibility_rule;
             DROP TABLE story_event_character;
             DROP TABLE story_event_location;
             DROP TABLE story_event_interval_boundary;
             DROP TABLE scene_interval_boundary;
             DROP TABLE diagnostic_suppression;
             DELETE FROM schema_migrations WHERE version>=9;",
        )
        .unwrap();
        drop(conn);

        let mut migrated = ProjectStore::open(&project).unwrap();
        assert_eq!(
            migrated.outgoing_links(EntityRef::Scene(first)).unwrap(),
            vec![old_link]
        );
        let bytes = b"Reference included from a scene.".to_vec();
        let reading = story_reading::ReferenceDocument {
            title: "Reference".into(),
            media_type: "text/plain".into(),
            extracted_text: String::from_utf8(bytes.clone()).unwrap(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            source_bytes: bytes,
            language_code: None,
        };
        let reading_id = migrated
            .import_reading_document(&reading, "fixture:migration")
            .unwrap();
        migrated
            .create_story_link(&StoryLink {
                id: LinkId::new(),
                source: EntityRef::Scene(first),
                source_anchor: None,
                target: EntityRef::Reading(reading_id),
                target_anchor: None,
                kind: LinkKind::Includes,
                label: String::new(),
            })
            .unwrap();
        assert!(
            migrated
                .expand_document_inclusions(EntityRef::Scene(first))
                .unwrap()
                .iter()
                .any(|passage| passage.source == EntityRef::Reading(reading_id))
        );
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
            language_code: None,
            word_count_policy: None,
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
    fn sustained_editing_preserves_large_unicode_buffer_and_scene_identity() {
        let d = tempdir().unwrap();
        let project = d.path().join("Writing");
        let mut store = ProjectStore::create(&project, "Writing").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Long scene").unwrap();
        let mut expected = String::new();
        let mut hash = store.load_text(scene).unwrap().1;
        for revision in 0..24 {
            expected = format!(
                "Revision {revision}: {}\n",
                "e\u{301} 👩‍🚀 你好. ".repeat(2_000)
            );
            hash = store.save_text(scene, Some(&hash), &expected).unwrap();
            if revision % 6 == 5 {
                drop(store);
                store = ProjectStore::open(&project).unwrap();
                assert_eq!(store.load_text(scene).unwrap().0, expected);
            }
        }
        let record = store.get_scene(scene).unwrap();
        assert_eq!(record.content_hash, hash);
        assert_eq!(record.scene.id, scene);
        assert_eq!(store.load_text(scene).unwrap().0, expected);
        assert_eq!(record.actual_words, count_words(&expected));
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
            book_type: BookType::Nonfiction,
            template_id: None,
            target_words: Some(90000),
            story_epoch_label: "Before the Sundering".into(),
            story_calendar_id: Some("twelve-moons".into()),
            language_code: "fr-CA".into(),
            word_count_policy: WordCountPolicy::default(),
        };
        let s = ProjectStore::create_for(&p, &project).unwrap();
        let got = s.get_project().unwrap();
        assert_eq!(got.id, project.id);
        assert_eq!(got.book_type, BookType::Nonfiction);
        assert_eq!(got.target_words, Some(90000));
        assert_eq!(got.story_epoch_label, "Before the Sundering");
        assert_eq!(got.story_calendar_id.as_deref(), Some("twelve-moons"));
        assert_eq!(got.language_code, "fr-CA");
        let mut s = s;
        s.set_project_language("ja-JP").unwrap();
        assert!(s.set_project_language("bad tag").is_err());
        drop(s);
        assert_eq!(
            ProjectStore::open(&p)
                .unwrap()
                .get_project()
                .unwrap()
                .language_code,
            "ja-JP"
        );
        let s = ProjectStore::open(&p).unwrap();
        assert!(s.conn.execute("UPDATE project SET id='00000000-0000-0000-0000-000000000000' WHERE singleton=1",[]).is_err());
        assert!(s.conn.execute("INSERT INTO project(singleton,id,title,created_at,updated_at) VALUES(1,?1,'Other','0','0')",[Uuid::new_v4().to_string()]).is_err());
    }
    #[test]
    fn project_word_count_policy_recounts_saved_prose_and_persists() {
        let d = tempdir().unwrap();
        let root = d.path().join("Japanese Novel");
        let mut store = ProjectStore::create(&root, "Japanese Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "第一章", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "猫").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        store.save_text(scene, Some(&hash), "한국어").unwrap();
        let front = store
            .create_content_unit(ContentUnitKind::FrontMatter, "献辞", None, 0, None)
            .unwrap();
        let (_, hash) = store.load_content_unit_text(front).unwrap();
        store
            .save_content_unit_text(front, Some(&hash), "한국어")
            .unwrap();
        let unicode_words = store.get_scene(scene).unwrap().actual_words;
        assert!(unicode_words < 3);
        assert_eq!(
            store.get_content_unit(front).unwrap().actual_words,
            unicode_words
        );
        store
            .set_project_word_count_policy(WordCountPolicy::UnspacedScriptGraphemes)
            .unwrap();
        assert_eq!(store.get_scene(scene).unwrap().actual_words, 3);
        assert_eq!(store.get_content_unit(front).unwrap().actual_words, 3);
        drop(store);
        let reopened = ProjectStore::open(&root).unwrap();
        assert_eq!(
            reopened.get_project().unwrap().word_count_policy,
            WordCountPolicy::UnspacedScriptGraphemes
        );
        assert_eq!(reopened.get_scene(scene).unwrap().actual_words, 3);
        assert_eq!(reopened.get_content_unit(front).unwrap().actual_words, 3);
    }

    #[test]
    fn scene_language_override_inherits_project_default_and_survives_reopen() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("Languages");
        let mut store = ProjectStore::create(&root, "Languages").unwrap();
        store.set_project_language("it-IT").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Capitolo", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scena").unwrap();
        let content_hash = store.load_text(scene).unwrap().1;

        let project = store.get_project().unwrap();
        let record = store.get_scene(scene).unwrap();
        assert_eq!(record.scene.language_code, None);
        assert_eq!(record.scene.effective_language_code(&project), "it-IT");

        store.set_scene_language(scene, Some("fr-FR")).unwrap();
        let project = store.get_project().unwrap();
        let record = store.get_scene(scene).unwrap();
        assert_eq!(record.scene.language_code.as_deref(), Some("fr-FR"));
        assert_eq!(record.scene.effective_language_code(&project), "fr-FR");
        assert!(store.set_scene_language(scene, Some("not a tag")).is_err());

        store.set_scene_language(scene, None).unwrap();
        store.set_project_language("ko-KR").unwrap();
        let project = store.get_project().unwrap();
        assert_eq!(
            store
                .get_scene(scene)
                .unwrap()
                .scene
                .effective_language_code(&project),
            "ko-KR"
        );
        assert_eq!(store.load_text(scene).unwrap().1, content_hash);
        drop(store);

        let reopened = ProjectStore::open(&root).unwrap();
        let project = reopened.get_project().unwrap();
        let record = reopened.get_scene(scene).unwrap();
        assert_eq!(record.scene.language_code, None);
        assert_eq!(record.scene.effective_language_code(&project), "ko-KR");
    }

    #[test]
    fn prose_document_language_inherits_and_overrides_project_default() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("Documents");
        let mut store = ProjectStore::create(&root, "Documents").unwrap();
        store.set_project_language("it-IT").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let unit = store
            .create_content_unit(
                ContentUnitKind::ChapterBody,
                "Chapter prose",
                Some(chapter),
                0,
                None,
            )
            .unwrap();
        let project = store.get_project().unwrap();
        let record = store.get_content_unit(unit).unwrap();
        assert_eq!(record.language_code, None);
        assert_eq!(record.effective_language_code(&project), "it-IT");

        store
            .set_content_unit_language(unit, Some("fr-FR"))
            .unwrap();
        assert!(
            store
                .set_content_unit_language(unit, Some("not a tag"))
                .is_err()
        );
        let project = store.get_project().unwrap();
        assert_eq!(
            store
                .get_content_unit(unit)
                .unwrap()
                .effective_language_code(&project),
            "fr-FR"
        );
        store.set_content_unit_language(unit, None).unwrap();
        store.set_project_language("ko-KR").unwrap();
        drop(store);

        let reopened = ProjectStore::open(&root).unwrap();
        let project = reopened.get_project().unwrap();
        let record = reopened.get_content_unit(unit).unwrap();
        assert_eq!(record.language_code, None);
        assert_eq!(record.effective_language_code(&project), "ko-KR");
    }

    #[test]
    fn passage_language_tags_use_reanchored_utf8_passage_anchors() {
        let directory = tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Mixed"), "Mixed").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Dialogue").unwrap();
        let original = "Mara smiled. Bonjour, mon ami!";
        let (_, hash) = store.load_text(scene).unwrap();
        store.save_text(scene, Some(&hash), original).unwrap();
        let start = original.find("Bonjour").unwrap() as u64;
        let end = start + "Bonjour, mon ami!".len() as u64;
        let anchor = store
            .tag_passage_language(scene, start, end, "fr-FR")
            .unwrap();
        let tag = store.list_passage_language_tags(scene).unwrap().remove(0);
        assert_eq!(tag.anchor.id, anchor);
        assert_eq!(tag.anchor.quote, "Bonjour, mon ami!");
        assert_eq!(tag.language_code, "fr-FR");
        assert_eq!(tag.anchor.state, "current");

        let (current, current_hash) = store.load_text(scene).unwrap();
        store
            .save_text(scene, Some(&current_hash), &format!("{current} Encore."))
            .unwrap();
        let tag = store.list_passage_language_tags(scene).unwrap().remove(0);
        assert_eq!(tag.anchor.id, anchor);
        assert_eq!(tag.anchor.state, "current");
        assert_eq!(tag.anchor.start_byte, start);
        assert_eq!(tag.language_code, "fr-FR");

        assert!(store.tag_passage_language(scene, 1, 2, "bad tag").is_err());
        assert!(store.tag_passage_language(scene, 0, 9999, "fr-FR").is_err());
        store.remove_passage_language_tag(anchor).unwrap();
        assert!(store.list_passage_language_tags(scene).unwrap().is_empty());
        assert_eq!(store.list_scene_anchors(scene).unwrap().len(), 1);
    }

    #[test]
    fn scene_word_count_policy_overrides_project_counts_and_persists() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("Korean Counts");
        let mut store = ProjectStore::create(&root, "Korean Counts").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        store.save_text(scene, Some(&hash), "한국어").unwrap();
        assert_eq!(store.get_scene(scene).unwrap().actual_words, 1);

        let mut metadata = store.get_scene(scene).unwrap().scene;
        metadata.word_count_policy = Some(WordCountPolicy::UnspacedScriptGraphemes);
        store.update_scene(&metadata).unwrap();
        assert_eq!(store.get_scene(scene).unwrap().actual_words, 3);
        assert_eq!(
            store.get_project().unwrap().word_count_policy,
            WordCountPolicy::UnicodeWords
        );
        assert!(
            store
                .conn
                .execute(
                    "UPDATE scene SET word_count_policy='invented' WHERE id=?1",
                    [scene.to_string()],
                )
                .is_err()
        );

        drop(store);
        let reopened = ProjectStore::open(root).unwrap();
        let record = reopened.get_scene(scene).unwrap();
        assert_eq!(record.actual_words, 3);
        assert_eq!(
            record.scene.word_count_policy,
            Some(WordCountPolicy::UnspacedScriptGraphemes)
        );
        assert_eq!(reopened.load_text(scene).unwrap().0, "한국어");
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
    fn verified_single_file_backup_restores_to_relocated_project_without_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let project_path = directory.path().join("Original");
        let mut project = ProjectStore::create(&project_path, "Portable Novel").unwrap();
        let chapter = project
            .create_node(None, NodeKind::Chapter, "Chapter One", 0)
            .unwrap();
        let scene = project.create_scene(chapter, 0, "Opening").unwrap();
        let (_, hash) = project.load_text(scene).unwrap();
        project
            .save_text(scene, Some(&hash), "Portable manuscript text.")
            .unwrap();
        fs::create_dir_all(project_path.join("attachments")).unwrap();
        fs::write(
            project_path.join("attachments/source.txt"),
            b"research source",
        )
        .unwrap();
        let archive = directory.path().join("novel.wide.zip");
        project.backup_archive_to(&archive).unwrap();
        let pending: i64 = project
            .conn
            .query_row("SELECT count(*) FROM write_journal", [], |row| row.get(0))
            .unwrap();
        assert_eq!(pending, 0);
        drop(project);

        let relocated = directory.path().join("Relocated");
        ProjectStore::restore_archive_to(&archive, &relocated).unwrap();
        let restored = ProjectStore::open(&relocated).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.get_project().unwrap().title, "Portable Novel");
        assert_eq!(
            restored.load_text(scene).unwrap().0,
            "Portable manuscript text."
        );
        assert_eq!(
            fs::read(relocated.join("attachments/source.txt")).unwrap(),
            b"research source"
        );
        assert!(ProjectStore::restore_archive_to(&archive, &relocated).is_err());
    }

    #[test]
    fn git_restore_file_set_reopens_and_interrupted_restore_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Novel");
        let store = ProjectStore::create(&project, "Novel").unwrap();
        std::fs::create_dir_all(project.join("attachments")).unwrap();
        std::fs::write(project.join("attachments/cover.bin"), b"old cover").unwrap();
        drop(store);

        let mut store = ProjectStore::open(&project).unwrap();
        store
            .restore_git_files(&[
                GitRestoreFile {
                    relative_path: "attachments/cover.bin".into(),
                    contents: Some(b"restored cover".to_vec()),
                },
                GitRestoreFile {
                    relative_path: "attachments/obsolete.bin".into(),
                    contents: None,
                },
            ])
            .unwrap();
        assert_eq!(
            std::fs::read(project.join("attachments/cover.bin")).unwrap(),
            b"restored cover"
        );
        drop(store);
        let store = ProjectStore::open(&project).unwrap();
        store.validate().unwrap();
        drop(store);

        let old = b"restored cover".to_vec();
        let proposed = b"interrupted proposal".to_vec();
        let recovery = project.join(".writers-ide/recovery/git-restore-interrupted");
        std::fs::create_dir_all(recovery.join("original")).unwrap();
        std::fs::create_dir_all(recovery.join("prepared")).unwrap();
        write_synced(&recovery.join("original/00000000.bin"), &old).unwrap();
        write_synced(&recovery.join("prepared/00000000.bin"), &proposed).unwrap();
        std::fs::write(project.join("attachments/cover.bin"), &proposed).unwrap();
        let journal = GitRestoreJournal {
            format_version: 1,
            committed: false,
            entries: vec![GitRestoreJournalEntry {
                relative_path: "attachments/cover.bin".into(),
                old_exists: true,
                old_sha256: Some(sha256_bytes(&old)),
                new_sha256: Some(sha256_bytes(&proposed)),
                backup_name: "original/00000000.bin".into(),
                prepared_name: "prepared/00000000.bin".into(),
            }],
        };
        write_git_restore_journal(&recovery, &journal).unwrap();
        let reopened = ProjectStore::open(&project).unwrap();
        assert_eq!(
            std::fs::read(project.join("attachments/cover.bin")).unwrap(),
            old
        );
        assert!(!recovery.exists());
        reopened.validate().unwrap();
    }

    #[test]
    fn git_restore_recovery_preserves_an_external_conflict() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("Novel");
        let store = ProjectStore::create(&project, "Novel").unwrap();
        std::fs::create_dir_all(project.join("attachments")).unwrap();
        drop(store);
        let original = b"original";
        let prepared = b"prepared";
        let external = b"external divergence";
        let recovery = project.join(".writers-ide/recovery/git-restore-conflict");
        std::fs::create_dir_all(recovery.join("original")).unwrap();
        std::fs::create_dir_all(recovery.join("prepared")).unwrap();
        write_synced(&recovery.join("original/00000000.bin"), original).unwrap();
        write_synced(&recovery.join("prepared/00000000.bin"), prepared).unwrap();
        std::fs::write(project.join("attachments/file.bin"), external).unwrap();
        let journal = GitRestoreJournal {
            format_version: 1,
            committed: false,
            entries: vec![GitRestoreJournalEntry {
                relative_path: "attachments/file.bin".into(),
                old_exists: true,
                old_sha256: Some(sha256_bytes(original)),
                new_sha256: Some(sha256_bytes(prepared)),
                backup_name: "original/00000000.bin".into(),
                prepared_name: "prepared/00000000.bin".into(),
            }],
        };
        write_git_restore_journal(&recovery, &journal).unwrap();

        assert!(matches!(
            ProjectStore::open(&project),
            Err(StorageError::RecoveryConflict(_))
        ));
        assert_eq!(
            std::fs::read(project.join("attachments/file.bin")).unwrap(),
            external
        );
        assert!(recovery.join("original/00000000.bin").is_file());
        assert!(recovery.join("prepared/00000000.bin").is_file());
    }

    #[test]
    fn archive_restore_rejects_path_traversal_before_publishing() {
        let directory = tempfile::tempdir().unwrap();
        let archive_path = directory.path().join("malicious.zip");
        let mut writer = zip::ZipWriter::new(File::create(&archive_path).unwrap());
        writer
            .start_file("../outside.txt", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(b"do not extract").unwrap();
        writer.finish().unwrap();
        let destination = directory.path().join("Restored");
        assert!(ProjectStore::restore_archive_to(&archive_path, &destination).is_err());
        assert!(!destination.exists());
        assert!(!directory.path().join("outside.txt").exists());
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
        let sc = s.create_scene(ch, 0, "Mara Arrival").unwrap();
        s.conn
            .execute(
                "UPDATE scene SET synopsis='Mara enters the old station' WHERE id=?1",
                [sc.to_string()],
            )
            .unwrap();
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
        let refs_field = FieldId::new();
        s.define_custom_field(&CustomFieldDefinition {
            id: refs_field,
            name: "Referenced people".into(),
            field_type: FieldType::EntityRefs,
            required: false,
            default_value: None,
            enum_values: vec![],
            reference_kind: Some("character".into()),
        })
        .unwrap();
        s.set_custom_field(
            sc,
            refs_field,
            &FieldValue::EntityRefs(vec![char.to_string()]),
        )
        .unwrap();
        let entity_query = SavedSceneQuery {
            id: QueryId::new(),
            name: "scenes referencing Mara in metadata".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::CustomMetadataContainsEntity {
                field_id: refs_field,
                target_kind: "character".into(),
                target_id: char.to_string(),
            }],
        };
        s.save_scene_query(&entity_query).unwrap();
        assert_eq!(s.run_scene_query(entity_query.id).unwrap(), vec![sc]);
        let evidence = s.explain_scene_query_match(entity_query.id, sc).unwrap();
        assert!(
            evidence
                .iter()
                .any(|item| item.source_kind == "character" && item.source_id == char.to_string())
        );
        assert!(
            s.explain_scene_query_match(entity_query.id, SceneId::new())
                .is_err()
        );
        let (before_text, text_hash) = s.load_text(sc).unwrap();
        s.save_text(
            sc,
            Some(&text_hash),
            &format!("Mara opens a door. {before_text}"),
        )
        .unwrap();
        let text_query = SavedSceneQuery {
            id: QueryId::new(),
            name: "Mara passage".into(),
            description: String::new(),
            filters: vec![SceneQueryFilter::TextContains("MARA".into())],
        };
        s.save_scene_query(&text_query).unwrap();
        let passage_evidence = s.explain_scene_query_match(text_query.id, sc).unwrap();
        assert!(passage_evidence.iter().any(|item| {
            item.source_kind == "scene-passage" && item.passage_bytes == Some((0, 4))
        }));
        assert!(passage_evidence.iter().any(|item| {
            item.source_kind == "scene-field" && item.field_bytes == Some(("title".into(), 0, 4))
        }));
        assert!(passage_evidence.iter().any(|item| {
            item.source_kind == "scene-field" && item.field_bytes == Some(("synopsis".into(), 0, 4))
        }));
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
        let query = SavedSceneQuery {
            id: QueryId::new(),
            name: "all".into(),
            description: String::new(),
            filters: vec![],
        };
        s.save_scene_query(&query).unwrap();
        fs::write(p.join("manuscript").join(format!("{sc}.md")), "external").unwrap();
        assert!(s.save_text(sc, Some(&h), "ours").is_err());
        assert!(s.explain_scene_query_match(query.id, sc).is_err());
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
    fn proposal_updates_are_project_local_atomic_and_optimistic() {
        let d = tempdir().unwrap();
        let store = ProjectStore::create(d.path().join("Novel"), "Novel").unwrap();
        assert!(store.list_proposal_files().unwrap().is_empty());
        let path = store
            .save_proposal_file("candidate.json", br#"{"claim":"candidate"}"#)
            .unwrap();
        let original = store.load_proposal_file(&path).unwrap();
        let fingerprint = hash_bytes(&original);
        assert_eq!(store.list_proposal_files().unwrap(), vec![path.clone()]);
        store
            .update_proposal_file(&path, &fingerprint, br#"{"dismissed":true}"#)
            .unwrap();
        assert_eq!(
            store.load_proposal_file(&path).unwrap(),
            br#"{"dismissed":true}"#
        );
        assert!(
            store
                .update_proposal_file(&path, &fingerprint, b"stale")
                .is_err()
        );
        assert!(store.load_proposal_file(d.path()).is_err());
        assert!(
            store
                .update_proposal_file(&d.path().join("outside.json"), "", b"unsafe")
                .is_err()
        );
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

    #[test]
    fn character_profiles_and_aliases_migrate_persist_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Novel");
        let mut store = ProjectStore::create(&path, "Novel").unwrap();
        let id = CharacterId::new();
        store.add_character(id, "Mara Venn").unwrap();
        let mut character = store.get_character(id).unwrap();
        assert_eq!(character.preferred_name, "Mara Venn");
        character.preferred_name = "Mara".into();
        character.aliases = vec!["The Cartographer".into(), "M.V.".into()];
        character.description = "A reluctant mapmaker".into();
        character.background = "Raised in the flood district".into();
        character.goals = "Find the missing atlas".into();
        character.motivations = "Protect her sister".into();
        character.secrets = "She destroyed the first map".into();
        character.internal_conflict = "Fear of being known".into();
        character.external_conflict = "The guild wants the atlas".into();
        character.status = "missing".into();
        store.update_character(&character).unwrap();
        assert!(
            store
                .update_character(&Character {
                    aliases: vec!["same".into(), "SAME".into()],
                    ..character.clone()
                })
                .is_err()
        );
        drop(store);

        let store = ProjectStore::open(&path).unwrap();
        let mut expected = character.clone();
        expected.aliases.sort_by_key(|alias| alias.to_lowercase());
        assert_eq!(store.get_character(id).unwrap(), expected);
        assert!(
            store
                .search_project("Cartographer", 10)
                .unwrap()
                .iter()
                .any(|hit| { hit.entity_kind == "character" && hit.entity_id == id.to_string() })
        );
        assert!(
            store
                .search_project("flood", 10)
                .unwrap()
                .iter()
                .any(|hit| { hit.entity_kind == "character" && hit.entity_id == id.to_string() })
        );
        assert_eq!(store.list_characters().unwrap()[0].name, "Mara");
    }

    #[test]
    fn mention_detection_is_separate_from_participation_and_updates_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter 1", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Arrival").unwrap();
        let character = CharacterId::new();
        store.add_character(character, "Mara").unwrap();
        store.add_location(LocationId::new(), "Mara").unwrap();
        store
            .add_location(LocationId::new(), "Glass House")
            .unwrap();
        store
            .create_story_object("Brass Key", "A small key")
            .unwrap();
        let (_, initial_hash) = store.load_text(scene).unwrap();
        let hash = store
            .save_text(
                scene,
                Some(&initial_hash),
                "Mara passed the Glass House and picked up the Brass Key. A Marathian waited.",
            )
            .unwrap();
        let mentions = store.list_scene_mentions(scene).unwrap();
        assert_eq!(mentions.len(), 3);
        assert!(mentions.iter().any(|mention| mention.quote == "Mara"));
        assert!(
            mentions
                .iter()
                .any(|mention| mention.quote == "Glass House")
        );
        assert!(mentions.iter().any(|mention| mention.quote == "Brass Key"));
        assert_eq!(
            mentions
                .iter()
                .find(|mention| mention.quote == "Mara")
                .unwrap()
                .state,
            "ambiguous"
        );
        assert!(store.scene_character_analytics(scene).unwrap().is_empty());

        let mara = mentions
            .iter()
            .find(|mention| mention.quote == "Mara")
            .unwrap();
        store
            .confirm_scene_mention(
                mara.id,
                MentionEntityKind::Character,
                &character.to_string(),
            )
            .unwrap();
        let shifted = format!("Preface. {}", store.load_text(scene).unwrap().0);
        store.save_text(scene, Some(&hash), &shifted).unwrap();
        assert!(
            store
                .list_scene_mentions(scene)
                .unwrap()
                .iter()
                .any(|mention| mention.state == "needs_review")
        );
        let key = store
            .list_scene_mentions(scene)
            .unwrap()
            .into_iter()
            .find(|mention| mention.quote == "Brass Key")
            .unwrap();
        store.dismiss_scene_mention(key.id).unwrap();
        let (current, current_hash) = store.load_text(scene).unwrap();
        store
            .reindex_scene_mentions(scene, &current, &current_hash)
            .unwrap();
        assert!(
            store
                .list_scene_mentions(scene)
                .unwrap()
                .iter()
                .any(|mention| { mention.quote == "Brass Key" && mention.state == "dismissed" })
        );
    }

    #[test]
    fn scratchpad_notes_persist_pin_and_promote_without_losing_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(dir.path().join("Novel"), "Novel").unwrap();
        let character = CharacterId::new();
        store.add_character(character, "Mara").unwrap();
        let note = store
            .create_scratchpad_note("Remember", "The atlas is hidden under the bridge.")
            .unwrap();
        store
            .update_scratchpad_note(
                note,
                "Atlas clue",
                "The atlas is hidden under the bridge.",
                true,
            )
            .unwrap();
        store
            .promote_scratchpad_note(note, "character", &character.to_string())
            .unwrap();
        let note = &store.list_scratchpad_notes(true).unwrap()[0];
        assert_eq!(note.state, "promoted");
        assert!(note.pinned);
        assert_eq!(note.body, "The atlas is hidden under the bridge.");
        assert_eq!(note.destination_id, Some(character.to_string()));
        assert!(store.list_scratchpad_notes(false).unwrap().is_empty());
    }

    #[test]
    fn personal_comments_follow_passage_anchors_and_report_orphans() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let scene = store.create_scene(chapter, 0, "Scene").unwrap();
        let (_, hash) = store.load_text(scene).unwrap();
        let text = "Élodie said hello.\n";
        store.save_text(scene, Some(&hash), text).unwrap();
        let start = text.find("said").unwrap() as u64;
        let comment = store
            .create_personal_comment(
                scene,
                start,
                start + 4,
                "said",
                "Clarify who she addresses.",
            )
            .unwrap();
        store
            .set_personal_comment_state(comment, "resolved")
            .unwrap();
        let (current, hash) = store.load_text(scene).unwrap();
        let hash = store
            .save_text(scene, Some(&hash), &format!("Before. {current}"))
            .unwrap();
        let record = store.list_scene_comments(scene).unwrap().remove(0);
        assert_eq!(record.state, "resolved");
        assert_eq!(record.anchor_state.as_deref(), Some("current"));
        assert_eq!(record.quote.as_deref(), Some("said"));
        let text = "Before. Élodie greeted everyone.\n";
        store.save_text(scene, Some(&hash), text).unwrap();
        let record = store.list_scene_comments(scene).unwrap().remove(0);
        assert_eq!(record.state, "resolved");
        assert_ne!(record.anchor_state.as_deref(), Some("current"));
        assert!(record.anchor_id.is_some());
    }

    #[test]
    fn assemblies_reorder_and_filter_content_without_touching_canonical_placement() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = ProjectStore::create(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = store
            .create_node(None, NodeKind::Chapter, "Chapter", 0)
            .unwrap();
        let first = store.create_scene(chapter, 0, "First").unwrap();
        let second = store.create_scene(chapter, 1, "Second").unwrap();
        let assembly = store.create_assembly("Alternate edition").unwrap();
        let first_item = store.add_assembly_scene(assembly, first, 0).unwrap();
        let second_item = store.add_assembly_scene(assembly, second, 1).unwrap();
        store.move_assembly_item(assembly, second_item, 0).unwrap();
        store.set_assembly_item_included(first_item, false).unwrap();
        let items = store.list_assembly_items(assembly).unwrap();
        assert_eq!(items[0].id, second_item);
        assert_eq!(items[0].position, 0);
        assert!(items[0].included);
        assert_eq!(items[1].id, first_item);
        assert_eq!(items[1].position, 1);
        assert!(!items[1].included);
        assert!(store.add_assembly_scene(assembly, first, 2).is_err());
        let other = store.create_assembly("Reading copy").unwrap();
        assert!(store.add_assembly_scene(other, first, 0).is_ok());
        let canonical = store.list_scenes(false).unwrap();
        assert_eq!(
            canonical.iter().map(|row| row.scene.id).collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(
            canonical
                .iter()
                .map(|row| row.position.unwrap())
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn diagnostic_suppression_storage_enforces_scope_reason_and_lifecycle_states() {
        let directory = tempdir().unwrap();
        let store = ProjectStore::create(directory.path().join("Novel"), "Novel").unwrap();
        let valid = DiagnosticSuppressionRecord {
            id: Uuid::new_v4().to_string(),
            rule_id: "PUB004".into(),
            scope: "finding".into(),
            scene_id: Some(Uuid::new_v4().to_string()),
            finding_fingerprint: Some("fingerprint".into()),
            reason: "known intentional placeholder".into(),
            created_unix_ms: 100,
            expires_unix_ms: Some(200),
            triggering_fingerprint: Some("project-fingerprint".into()),
            status: "active".into(),
            last_checked_unix_ms: 100,
        };
        store.create_diagnostic_suppression(&valid).unwrap();
        let listed = store.list_diagnostic_suppressions().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], valid);
        store
            .update_diagnostic_suppression_state(&valid.id, "stale", 150)
            .unwrap();
        assert_eq!(
            store.list_diagnostic_suppressions().unwrap()[0].status,
            "stale"
        );
        store
            .update_diagnostic_suppression_state(&valid.id, "active", 175)
            .unwrap();
        assert_eq!(
            store.list_diagnostic_suppressions().unwrap()[0].status,
            "active"
        );

        let invalid_scope = DiagnosticSuppressionRecord {
            id: Uuid::new_v4().to_string(),
            scope: "scene".into(),
            scene_id: None,
            finding_fingerprint: Some("orphan".into()),
            reason: "invalid scope shape".into(),
            ..valid.clone()
        };
        assert!(store.create_diagnostic_suppression(&invalid_scope).is_err());
        let invalid_expiry = DiagnosticSuppressionRecord {
            id: Uuid::new_v4().to_string(),
            created_unix_ms: 100,
            expires_unix_ms: Some(100),
            last_checked_unix_ms: 100,
            ..valid.clone()
        };
        assert!(
            store
                .create_diagnostic_suppression(&invalid_expiry)
                .is_err()
        );
        assert!(store.conn.execute(
            "INSERT INTO diagnostic_suppression(id,rule_id,scope,reason,created_unix_ms,last_checked_unix_ms,status) VALUES('bad','PUB004','finding','bad',1,1,'active')",
            [],
        ).is_err());
    }
}
