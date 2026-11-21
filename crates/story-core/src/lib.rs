use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
id_type!(ProjectId);
id_type!(NodeId);
id_type!(SceneId);
id_type!(CharacterId);
id_type!(PlotlineId);
id_type!(EventId);
id_type!(LocationId);
id_type!(FieldId);
id_type!(ImportRunId);
id_type!(SourceItemId);
id_type!(ProvenanceId);
id_type!(ResearchId);
id_type!(LinkId);
id_type!(AnchorId);
id_type!(QueryId);
id_type!(IssueId);
id_type!(ReadingId);
id_type!(AnnotationId);
id_type!(BookmarkId);
id_type!(ReadingTermId);
id_type!(SnapshotId);
id_type!(KnowledgeClaimId);
id_type!(ContentUnitId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRestoreSelection {
    pub current_start: usize,
    pub current_end: usize,
    pub snapshot_start: usize,
    pub snapshot_end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadingAnnotationDraft {
    pub kind: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub body: String,
    pub prefix_context: String,
    pub suffix_context: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntityRef {
    Scene(SceneId),
    Character(CharacterId),
    Plotline(PlotlineId),
    Research(ResearchId),
    Reading(ReadingId),
}
impl EntityRef {
    pub fn kind(self) -> &'static str {
        match self {
            Self::Scene(_) => "scene",
            Self::Character(_) => "character",
            Self::Plotline(_) => "plotline",
            Self::Research(_) => "research",
            Self::Reading(_) => "reading",
        }
    }
    pub fn id(self) -> String {
        match self {
            Self::Scene(id) => id.to_string(),
            Self::Character(id) => id.to_string(),
            Self::Plotline(id) => id.to_string(),
            Self::Research(id) => id.to_string(),
            Self::Reading(id) => id.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinkKind {
    Related,
    Cites,
    Transcludes,
    Includes,
    Navigates,
}
impl LinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Related => "related",
            Self::Cites => "cites",
            Self::Transcludes => "transcludes",
            Self::Includes => "includes",
            Self::Navigates => "navigates",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PassageAnchor {
    pub id: AnchorId,
    pub scene_id: SceneId,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub prefix_context: String,
    pub suffix_context: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryLink {
    pub id: LinkId,
    pub source: EntityRef,
    pub source_anchor: Option<AnchorId>,
    pub target: EntityRef,
    pub target_anchor: Option<AnchorId>,
    pub kind: LinkKind,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SceneQueryFilter {
    Status(SceneStatus),
    Character(CharacterId),
    Plotline(PlotlineId),
    TimeBefore(i64),
    TimeAfter(i64),
    UnresolvedIssue,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SavedSceneQuery {
    pub id: QueryId,
    pub name: String,
    pub description: String,
    pub filters: Vec<SceneQueryFilter>,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid story time: {0}")]
    InvalidStoryTime(String),
    #[error("invalid word allocation: {0}")]
    InvalidAllocation(String),
    #[error("invalid custom field value: {0}")]
    InvalidField(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimePrecision {
    Exact,
    Approximate,
    Bounded,
    Unknown,
}
pub const STORY_TICKS_PER_DAY: i64 = 86_400;

/// Story epoch is project-defined; each tick is one second. Unknown is not tick zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryTime {
    pub nominal: Option<i64>,
    pub lower: Option<i64>,
    pub upper: Option<i64>,
    pub precision: TimePrecision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeOrdering {
    Before,
    After,
    Same,
    Ambiguous,
}
impl StoryTime {
    pub fn unknown() -> Self {
        Self {
            nominal: None,
            lower: None,
            upper: None,
            precision: TimePrecision::Unknown,
        }
    }
    pub fn exact(tick: i64) -> Self {
        Self {
            nominal: Some(tick),
            lower: Some(tick),
            upper: Some(tick),
            precision: TimePrecision::Exact,
        }
    }
    pub fn new(
        nominal: Option<i64>,
        lower: Option<i64>,
        upper: Option<i64>,
        precision: TimePrecision,
    ) -> Result<Self, DomainError> {
        match precision {
            TimePrecision::Unknown if nominal.is_some() || lower.is_some() || upper.is_some() => {
                return Err(DomainError::InvalidStoryTime(
                    "unknown time must not have ticks".into(),
                ));
            }
            TimePrecision::Unknown => (),
            TimePrecision::Exact if nominal.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "exact time requires nominal tick".into(),
                ));
            }
            TimePrecision::Approximate if nominal.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "approximate time requires nominal tick".into(),
                ));
            }
            TimePrecision::Bounded if lower.is_none() || upper.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "bounded time requires both bounds".into(),
                ));
            }
            _ => (),
        }
        if let (Some(lo), Some(hi)) = (lower, upper)
            && lo > hi
        {
            return Err(DomainError::InvalidStoryTime(
                "lower bound exceeds upper bound".into(),
            ));
        }
        if let Some(n) = nominal
            && (lower.is_some_and(|x| n < x) || upper.is_some_and(|x| n > x))
        {
            return Err(DomainError::InvalidStoryTime(
                "nominal is outside bounds".into(),
            ));
        }
        if precision == TimePrecision::Exact && (lower != nominal || upper != nominal) {
            return Err(DomainError::InvalidStoryTime(
                "exact time bounds must equal nominal".into(),
            ));
        }
        Ok(Self {
            nominal,
            lower,
            upper,
            precision,
        })
    }
    pub fn compare(self, other: Self) -> TimeOrdering {
        let (Some((al, ah)), Some((bl, bh))) = (self.interval(), other.interval()) else {
            return TimeOrdering::Ambiguous;
        };
        if ah < bl {
            TimeOrdering::Before
        } else if bh < al {
            TimeOrdering::After
        } else if al == ah && bl == bh && al == bl {
            TimeOrdering::Same
        } else {
            TimeOrdering::Ambiguous
        }
    }
    fn interval(self) -> Option<(i64, i64)> {
        match self.precision {
            TimePrecision::Exact => self.nominal.map(|n| (n, n)),
            TimePrecision::Bounded => Some((self.lower?, self.upper?)),
            TimePrecision::Approximate => Some((self.lower?, self.upper?)),
            TimePrecision::Unknown => None,
        }
    }
    pub fn duration_until(self, end: Self) -> Result<Option<i64>, DomainError> {
        match (self.nominal, end.nominal) {
            (Some(a), Some(b)) => b
                .checked_sub(a)
                .map(Some)
                .ok_or_else(|| DomainError::InvalidStoryTime("duration overflow".into())),
            _ => Ok(None),
        }
    }
    pub fn duration_bounds_until(self, end: Self) -> Result<Option<(i64, i64)>, DomainError> {
        let (Some((start_lo, start_hi)), Some((end_lo, end_hi))) =
            (self.interval(), end.interval())
        else {
            return Ok(None);
        };
        let min = end_lo
            .checked_sub(start_hi)
            .ok_or_else(|| DomainError::InvalidStoryTime("minimum duration overflow".into()))?;
        let max = end_hi
            .checked_sub(start_lo)
            .ok_or_else(|| DomainError::InvalidStoryTime("maximum duration overflow".into()))?;
        Ok(Some((min, max)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Section,
    Chapter,
    Group,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentUnitKind {
    FrontMatter,
    SectionBody,
    ChapterBody,
    BackMatter,
}

impl ContentUnitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FrontMatter => "front_matter",
            Self::SectionBody => "section_body",
            Self::ChapterBody => "chapter_body",
            Self::BackMatter => "back_matter",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "front_matter" => Some(Self::FrontMatter),
            "section_body" => Some(Self::SectionBody),
            "chapter_body" => Some(Self::ChapterBody),
            "back_matter" => Some(Self::BackMatter),
            _ => None,
        }
    }
}
impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Chapter => "chapter",
            Self::Group => "group",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SceneStatus {
    Idea,
    Outlined,
    ReadyToDraft,
    Drafting,
    DraftComplete,
    Revising,
    Complete,
    Archived,
}
impl SceneStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Outlined => "outlined",
            Self::ReadyToDraft => "ready_to_draft",
            Self::Drafting => "drafting",
            Self::DraftComplete => "draft_complete",
            Self::Revising => "revising",
            Self::Complete => "complete",
            Self::Archived => "archived",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub author: String,
    pub genre: String,
    pub synopsis: String,
    #[serde(default)]
    pub book_type: BookType,
    #[serde(default)]
    pub template_id: Option<String>,
    pub target_words: Option<u64>,
    pub story_epoch_label: String,
    pub story_calendar_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinTemplate {
    BlankFiction,
    Novel,
    ShortStory,
    BlankNonfiction,
    NonfictionBook,
}

impl BuiltinTemplate {
    pub fn id(self) -> &'static str {
        match self {
            Self::BlankFiction => "blank-fiction",
            Self::Novel => "novel",
            Self::ShortStory => "short-story",
            Self::BlankNonfiction => "blank-nonfiction",
            Self::NonfictionBook => "nonfiction-book",
        }
    }
    pub fn book_type(self) -> BookType {
        match self {
            Self::BlankNonfiction | Self::NonfictionBook => BookType::Nonfiction,
            _ => BookType::Fiction,
        }
    }
    pub fn all() -> [Self; 5] {
        [
            Self::BlankFiction,
            Self::Novel,
            Self::ShortStory,
            Self::BlankNonfiction,
            Self::NonfictionBook,
        ]
    }
    pub fn parse(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|template| template.id() == id)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookType {
    #[default]
    Fiction,
    Nonfiction,
    Custom,
}

impl BookType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fiction => "fiction",
            Self::Nonfiction => "nonfiction",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fiction" => Some(Self::Fiction),
            "nonfiction" | "non-fiction" => Some(Self::Nonfiction),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuralNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub kind: NodeKind,
    pub title: String,
    pub position: u32,
    pub target_words: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub title: String,
    pub synopsis: String,
    pub status: SceneStatus,
    pub target_words: Option<u64>,
    pub purpose: String,
    pub goal: String,
    pub outcome: String,
    pub start_time: StoryTime,
    pub end_time: StoryTime,
    pub pov_character: Option<CharacterId>,
    pub start_location: Option<LocationId>,
    pub end_location: Option<LocationId>,
    pub archived: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenePlacement {
    pub scene_id: SceneId,
    pub chapter_id: NodeId,
    pub position: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ParticipationRole {
    Pov,
    Major,
    Minor,
    OffCamera,
    Mentioned,
}
impl ParticipationRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pov => "pov",
            Self::Major => "major",
            Self::Minor => "minor",
            Self::OffCamera => "off_camera",
            Self::Mentioned => "mentioned",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneCharacter {
    pub character_id: CharacterId,
    pub role: ParticipationRole,
    pub notes: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenePlotline {
    pub plotline_id: PlotlineId,
    pub role: String,
    pub share_basis_points: Option<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneEvent {
    pub event_id: EventId,
    pub relationship_kind: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Character {
    pub id: CharacterId,
    pub full_name: String,
    pub preferred_name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub background: String,
    pub goals: String,
    pub motivations: String,
    pub secrets: String,
    pub internal_conflict: String,
    pub external_conflict: String,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plotline {
    pub id: PlotlineId,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub priority: i32,
    pub beginning_state: String,
    pub intended_development: String,
    pub intended_resolution: String,
    pub status: String,
    pub target_words: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub id: LocationId,
    pub parent: Option<LocationId>,
    pub name: String,
    pub description: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoryEvent {
    pub id: EventId,
    pub title: String,
    pub description: String,
    pub start: StoryTime,
    pub end: StoryTime,
    pub source_time_text: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WordAttributionPolicy {
    Overlap,
    EqualShare,
    AuthorShares,
}
pub fn validate_shares(links: &[ScenePlotline]) -> Result<(), DomainError> {
    let unique: std::collections::BTreeSet<_> = links.iter().map(|x| x.plotline_id).collect();
    if unique.len() != links.len() {
        return Err(DomainError::InvalidAllocation(
            "duplicate plotline allocation".into(),
        ));
    }
    if links.iter().any(|x| x.share_basis_points.is_none()) {
        return Err(DomainError::InvalidAllocation(
            "every plotline needs a share".into(),
        ));
    }
    let total: u32 = links
        .iter()
        .map(|x| u32::from(x.share_basis_points.unwrap()))
        .sum();
    if total != 10_000 {
        return Err(DomainError::InvalidAllocation(format!(
            "shares total {total}, expected 10000"
        )));
    }
    Ok(())
}
/// Largest-remainder apportionment; ties use lexicographic UUID for stable results.
pub fn allocate_words(
    words: u64,
    links: &[ScenePlotline],
) -> Result<BTreeMap<PlotlineId, u64>, DomainError> {
    validate_shares(links)?;
    let mut rows: Vec<_> = links
        .iter()
        .map(|x| {
            let n = u128::from(words) * u128::from(x.share_basis_points.unwrap());
            (x.plotline_id, (n / 10_000) as u64, (n % 10_000) as u64)
        })
        .collect();
    let assigned: u64 = rows.iter().map(|x| x.1).sum();
    let mut left = words - assigned;
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
    });
    for row in &mut rows {
        if left == 0 {
            break;
        }
        row.1 += 1;
        left -= 1;
    }
    Ok(rows.into_iter().map(|x| (x.0, x.1)).collect())
}

pub fn attribute_words(
    words: u64,
    links: &[ScenePlotline],
    policy: WordAttributionPolicy,
) -> Result<BTreeMap<PlotlineId, u64>, DomainError> {
    if links
        .iter()
        .map(|l| l.plotline_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != links.len()
    {
        return Err(DomainError::InvalidAllocation(
            "duplicate plotline membership".into(),
        ));
    }
    match policy {
        WordAttributionPolicy::Overlap => {
            Ok(links.iter().map(|l| (l.plotline_id, words)).collect())
        }
        WordAttributionPolicy::AuthorShares => allocate_words(words, links),
        WordAttributionPolicy::EqualShare => {
            if links.is_empty() {
                return Ok(BTreeMap::new());
            }
            let base = 10_000 / (links.len() as u32);
            let rem = 10_000 % (links.len() as u32);
            let mut equal = links.to_vec();
            equal.sort_by_key(|l| l.plotline_id.to_string());
            for (i, l) in equal.iter_mut().enumerate() {
                l.share_basis_points = Some((base + u32::from((i as u32) < rem)) as u16);
            }
            allocate_words(words, &equal)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    LongText,
    Integer,
    Decimal,
    Boolean,
    DateTime,
    Enum,
    EntityRef,
    EntityRefs,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FieldValue {
    Text(String),
    Integer(i64),
    Decimal(String),
    Boolean(bool),
    DateTime(String),
    Enum(String),
    EntityRef(String),
    EntityRefs(Vec<String>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomFieldDefinition {
    pub id: FieldId,
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<FieldValue>,
    pub enum_values: Vec<String>,
    pub reference_kind: Option<String>,
}
impl CustomFieldDefinition {
    pub fn validate(&self, value: &FieldValue) -> Result<(), DomainError> {
        let okay = matches!(
            (self.field_type, value),
            (FieldType::Text, FieldValue::Text(_))
                | (FieldType::LongText, FieldValue::Text(_))
                | (FieldType::Integer, FieldValue::Integer(_))
                | (FieldType::Decimal, FieldValue::Decimal(_))
                | (FieldType::Boolean, FieldValue::Boolean(_))
                | (FieldType::DateTime, FieldValue::DateTime(_))
                | (FieldType::Enum, FieldValue::Enum(_))
                | (FieldType::EntityRef, FieldValue::EntityRef(_))
                | (FieldType::EntityRefs, FieldValue::EntityRefs(_))
        );
        if !okay {
            return Err(DomainError::InvalidField(
                "value type does not match field".into(),
            ));
        }
        if let FieldValue::Enum(v) = value
            && !self.enum_values.contains(v)
        {
            return Err(DomainError::InvalidField(
                "value is outside enum options".into(),
            ));
        }
        if let FieldValue::Decimal(v) = value
            && !matches!(v.parse::<f64>(), Ok(n) if n.is_finite())
        {
            return Err(DomainError::InvalidField("invalid decimal value".into()));
        }
        if let FieldValue::DateTime(v) = value
            && !valid_datetime(v)
        {
            return Err(DomainError::InvalidField(
                "datetime must be RFC 3339".into(),
            ));
        }
        Ok(())
    }
}

fn valid_datetime(v: &str) -> bool {
    let b = v.as_bytes();
    if !v.is_ascii()
        || b.len() < 20
        || b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || b.get(10) != Some(&b'T')
        || b.get(13) != Some(&b':')
        || b.get(16) != Some(&b':')
    {
        return false;
    }
    if ![0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
        .iter()
        .all(|r| b[r.clone()].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let hour = (b[11] - b'0') as u32 * 10 + (b[12] - b'0') as u32;
    let minute = (b[14] - b'0') as u32 * 10 + (b[15] - b'0') as u32;
    let second = (b[17] - b'0') as u32 * 10 + (b[18] - b'0') as u32;
    if hour > 23 || minute > 59 || second > 60 {
        return false;
    }
    let mut rest = &v[19..];
    if rest.starts_with('.') {
        let fraction_end = rest[1..]
            .find(['Z', '+', '-'])
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        if fraction_end <= 1 || !rest[1..fraction_end].bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        rest = &rest[fraction_end..];
    }
    if rest == "Z" {
        return true;
    }
    if let Some(zone) = rest.strip_prefix('+').or_else(|| rest.strip_prefix('-')) {
        let z = zone.as_bytes();
        return z.len() == 5
            && z[2] == b':'
            && z[..2].iter().all(u8::is_ascii_digit)
            && z[3..].iter().all(u8::is_ascii_digit)
            && z[..2]
                .iter()
                .fold(0u32, |n, d| n * 10 + u32::from(*d - b'0'))
                <= 23
            && z[3..]
                .iter()
                .fold(0u32, |n, d| n * 10 + u32::from(*d - b'0'))
                <= 59;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_is_not_zero_and_overlap_is_ambiguous() {
        assert_eq!(
            StoryTime::unknown().compare(StoryTime::exact(0)),
            TimeOrdering::Ambiguous
        );
        let a = StoryTime::new(Some(5), Some(0), Some(10), TimePrecision::Bounded).unwrap();
        assert_eq!(a.compare(StoryTime::exact(7)), TimeOrdering::Ambiguous);
    }
    #[test]
    fn duration_overflow_is_checked() {
        assert!(
            StoryTime::exact(i64::MIN)
                .duration_until(StoryTime::exact(i64::MAX))
                .is_err()
        );
    }
    #[test]
    fn bounded_times_allow_no_nominal_and_duration_propagates_uncertainty() {
        let a = StoryTime::new(None, Some(-20), Some(-10), TimePrecision::Bounded).unwrap();
        let b = StoryTime::new(None, Some(5), Some(15), TimePrecision::Bounded).unwrap();
        assert_eq!(a.compare(b), TimeOrdering::Before);
        assert_eq!(a.duration_bounds_until(b).unwrap(), Some((15, 35)));
        let approximate = StoryTime::new(Some(2), None, None, TimePrecision::Approximate).unwrap();
        assert_eq!(
            approximate.compare(StoryTime::exact(3)),
            TimeOrdering::Ambiguous
        );
        assert_eq!(STORY_TICKS_PER_DAY, 86400);
    }
    #[test]
    fn shares_reject_incomplete_and_apportion_exactly() {
        let a = PlotlineId::new();
        let b = PlotlineId::new();
        let links = vec![
            ScenePlotline {
                plotline_id: a,
                role: "advances".into(),
                share_basis_points: Some(3333),
            },
            ScenePlotline {
                plotline_id: b,
                role: "advances".into(),
                share_basis_points: Some(6667),
            },
        ];
        let out = allocate_words(7, &links).unwrap();
        assert_eq!(out.values().sum::<u64>(), 7);
        assert!(validate_shares(&links[..1]).is_err());
    }
    #[test]
    fn attribution_policies_are_explicit_and_equal_share_totals_match() {
        let a = PlotlineId::new();
        let b = PlotlineId::new();
        let links = vec![
            ScenePlotline {
                plotline_id: a,
                role: "advances".into(),
                share_basis_points: None,
            },
            ScenePlotline {
                plotline_id: b,
                role: "advances".into(),
                share_basis_points: None,
            },
        ];
        let overlap = attribute_words(9, &links, WordAttributionPolicy::Overlap).unwrap();
        assert_eq!(overlap[&a], 9);
        assert_eq!(overlap[&b], 9);
        let equal = attribute_words(9, &links, WordAttributionPolicy::EqualShare).unwrap();
        assert_eq!(equal.values().sum::<u64>(), 9);
        assert!(equal.values().all(|n| *n == 4 || *n == 5));
    }
    #[test]
    fn datetime_custom_values_are_type_checked() {
        let d = CustomFieldDefinition {
            id: FieldId::new(),
            name: "When".into(),
            field_type: FieldType::DateTime,
            required: false,
            default_value: None,
            enum_values: vec![],
            reference_kind: None,
        };
        assert!(
            d.validate(&FieldValue::DateTime("2026-11-20T13:45:00Z".into()))
                .is_ok()
        );
        assert!(
            d.validate(&FieldValue::DateTime("tomorrow".into()))
                .is_err()
        );
    }
}
