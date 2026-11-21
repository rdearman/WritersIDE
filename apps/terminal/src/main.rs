use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
        canvas::{Canvas, Rectangle},
    },
};
use story_app::WriterApp;
use story_core::{SceneId, SceneStatus};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Workspace {
    Dashboard,
    Outline,
    Cards,
    Editor,
    PlotGrid,
    Characters,
    StoryBible,
    Timeline,
    Analytics,
    Research,
    Issues,
    Search,
    Reading,
    Revisions,
    PlanningBoards,
}
impl Workspace {
    const ALL: [Self; 15] = [
        Self::Dashboard,
        Self::Outline,
        Self::Cards,
        Self::Editor,
        Self::PlotGrid,
        Self::Characters,
        Self::StoryBible,
        Self::Timeline,
        Self::Analytics,
        Self::Research,
        Self::Issues,
        Self::Search,
        Self::Reading,
        Self::Revisions,
        Self::PlanningBoards,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Outline => "Outline",
            Self::Cards => "Scene Cards",
            Self::Editor => "Manuscript Editor",
            Self::PlotGrid => "Plot Grid",
            Self::Characters => "Characters",
            Self::StoryBible => "Story Bible",
            Self::Timeline => "Timeline",
            Self::Analytics => "Analytics",
            Self::Research => "Research",
            Self::Issues => "Issues",
            Self::Search => "Search",
            Self::Reading => "Reading",
            Self::Revisions => "Revisions",
            Self::PlanningBoards => "Planning Boards",
        }
    }
}
struct AppState {
    app: WriterApp,
    workspace: Workspace,
    scenes: Vec<(SceneId, String, SceneStatus, u32)>,
    selected: usize,
    editor: Vec<String>,
    editing_scene: Option<SceneId>,
    cursor: (usize, usize),
    saved_hash: String,
    dirty: bool,
    mode: Option<char>,
    prompt: String,
    message: String,
    history: Vec<Workspace>,
    history_index: usize,
    width: usize,
    editor_focus_mode: bool,
    horizontal_split_ratio: u16,
    vertical_split_ratio: u16,
    assembly_draft: Option<story_core::AssemblyDraftId>,
    planning_board_id: Option<story_core::PlanningBoardId>,
    writing_stats_enabled: bool,
    typed_characters: u64,
    pasted_characters: u64,
    successful_saves: u64,
    writing_session: Option<ActiveWritingSession>,
    scroll: usize,
    reading_document: Option<story_core::ReadingId>,
    reading_text: String,
    reading_offset: u64,
    reading_focus: bool,
    reading_selection_start: Option<u64>,
    function_bindings: Vec<(u8, Workspace)>,
    command_bindings: CommandBindings,
    revision_diff: Option<story_core::SnapshotId>,
    revision_scene: Option<SceneId>,
    revision_index: usize,
    revision_hunk: usize,
    search_hits: Vec<story_storage::ProjectSearchHit>,
    search_locations: Vec<(SceneId, usize, usize)>,
    search_location_index: usize,
    query_view: Option<story_core::QueryId>,
    proposal_review_active: bool,
    proposal_review_rows: Vec<String>,
    proposal_review_index: usize,
    editor_selection: Option<(usize, usize)>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    clipboard: String,
    snippets: BTreeMap<String, String>,
    registers: BTreeMap<String, String>,
    macros: BTreeMap<String, Vec<story_core::EditMacroOp>>,
    recording_macro: Option<(String, Vec<story_core::EditMacroOp>)>,
    command_macros: BTreeMap<String, Vec<RecordedCommand>>,
    recording_command_macro: Option<(String, Vec<RecordedCommand>)>,
    text_expansion: TextExpansionConfig,
    editor_preferences: EditorPreferences,
    bookmarks: BTreeMap<String, EditorBookmark>,
    kill_ring: Vec<String>,
    kill_ring_index: usize,
    last_yank: Option<LastYank>,
    macro_preview: Option<MacroPreview>,
    extra_cursors: Vec<(usize, usize)>,
    active_challenge: Option<ActiveWritingChallenge>,
    menu: Option<MenuState>,
    reference_panes: Vec<ReferencePane>,
    active_reference: usize,
    reference_focused: bool,
}

#[derive(Clone, Debug)]
struct ActiveWritingChallenge {
    id: String,
    initial_words: u32,
    target_words: u32,
    duration_seconds: u32,
    deadline: Instant,
    mode: String,
    last_activity: Instant,
    kamikaze_idle_seconds: u64,
    previous_editor: Vec<String>,
    previous_cursor: (usize, usize),
    previous_dirty: bool,
    previous_scene: Option<SceneId>,
    previous_hash: String,
    previous_selection: Option<(usize, usize)>,
    previous_workspace: Workspace,
    grace_pauses_remaining: u8,
    penalties_applied: u32,
    deletion_words_per_penalty: u32,
    words_deleted: u32,
    pauses_consumed: u32,
    failure_taunt: String,
    destination_scene: Option<SceneId>,
    started_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChallengeDraftFile {
    version: u16,
    challenge_id: String,
    text: String,
    target_words: u32,
    mode: String,
    duration_seconds: u32,
    words_deleted: u32,
    pauses_consumed: u32,
    #[serde(default)]
    destination_scene: Option<SceneId>,
    started_at: i64,
    updated_at: i64,
}

#[derive(Clone, Debug)]
struct ActiveWritingSession {
    id: String,
    scene_id: Option<SceneId>,
    initial_words: u32,
}

#[derive(Clone, Debug)]
struct ReferencePane {
    source_key: String,
    scene_id: Option<SceneId>,
    title: String,
    content: String,
    scroll: usize,
}

#[derive(Clone, Debug)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor: (usize, usize),
    selection: Option<(usize, usize)>,
}

#[derive(Clone, Copy)]
enum CursorMotion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MenuState {
    top: usize,
    selected: usize,
}

const MENU_CATEGORIES: [&str; 9] = [
    "File", "Edit", "Insert", "Scene", "Plot", "View", "Compile", "Tools", "Help",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionSnapshot {
    workspace: String,
    selected_scene: Option<String>,
    editing_scene: Option<String>,
    cursor: (usize, usize),
    scroll: usize,
    selected: usize,
    #[serde(default)]
    narrowed_chapter: Option<String>,
    reading_document: Option<String>,
    reading_offset: u64,
    #[serde(default)]
    editor_focus_mode: bool,
    #[serde(default = "default_split_ratio")]
    horizontal_split_ratio: u16,
    #[serde(default = "default_split_ratio")]
    vertical_split_ratio: u16,
    #[serde(default)]
    assembly_draft: Option<String>,
    #[serde(default)]
    planning_board_id: Option<String>,
    #[serde(default)]
    writing_stats_enabled: bool,
    #[serde(default)]
    typed_characters: u64,
    #[serde(default)]
    pasted_characters: u64,
    #[serde(default)]
    successful_saves: u64,
    #[serde(default)]
    reference_scene: Option<String>,
    #[serde(default)]
    reference_scroll: usize,
    #[serde(default)]
    reference_scenes: Vec<String>,
    #[serde(default)]
    reference_sources: Vec<String>,
    #[serde(default)]
    active_reference: usize,
    #[serde(default)]
    reference_scrolls: Vec<usize>,
    #[serde(default)]
    reference_focused: bool,
}

#[derive(Clone, Debug)]
enum TimelineItem {
    Scene(story_storage::SceneRecord),
    Event(story_storage::EventSummary),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RevisionHunk {
    diff_lines: std::ops::Range<usize>,
    selection: story_core::SnapshotRestoreSelection,
}

#[derive(Deserialize, Serialize)]
struct KeymapFile {
    #[serde(default)]
    bindings: BTreeMap<String, String>,
    #[serde(default)]
    commands: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct EditorToolsFile {
    version: u16,
    #[serde(default)]
    snippets: BTreeMap<String, String>,
    #[serde(default)]
    registers: BTreeMap<String, String>,
    #[serde(default)]
    macros: BTreeMap<String, Vec<story_core::EditMacroOp>>,
    #[serde(default)]
    command_macros: BTreeMap<String, Vec<RecordedCommand>>,
    #[serde(default)]
    recording_command_macro: Option<(String, Vec<RecordedCommand>)>,
    #[serde(default)]
    bookmarks: BTreeMap<String, EditorBookmark>,
    #[serde(default)]
    kill_ring: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecordedCommand {
    command_id: String,
    invocation: String,
}

#[derive(Clone, Debug)]
struct LastYank {
    scene_id: Option<SceneId>,
    range: std::ops::Range<usize>,
    text: String,
}

#[derive(Clone, Debug)]
struct MacroPreview {
    name: String,
    repetitions: usize,
    source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EditorBookmark {
    scene_id: SceneId,
    offset: usize,
    quote: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TextExpansionFile {
    text_expansion: TextExpansionConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EditorPreferencesFile {
    editor: EditorPreferences,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct TextExpansionConfig {
    enabled: bool,
    triggers: Vec<String>,
    mappings: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct EditorPreferences {
    line_ending: String,
    indentation: String,
    indent_width: usize,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            line_ending: "lf".into(),
            indentation: "tabs".into(),
            indent_width: 4,
        }
    }
}

impl Default for TextExpansionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            triggers: vec!["space".into(), "enter".into()],
            mappings: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CoverageConfig {
    version: u16,
    #[serde(default)]
    requirements: Vec<CoverageConfigEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CoverageConfigEntry {
    id: String,
    label: String,
    entity_id: String,
    dimension: String,
    minimum_scenes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChallengeConfig {
    version: u16,
    #[serde(default = "default_kamikaze_idle_seconds")]
    kamikaze_idle_seconds: u64,
    #[serde(default = "default_challenge_pauses")]
    grace_pauses: u8,
    #[serde(default = "default_challenge_deletion_rate")]
    deletion_words_per_penalty: u32,
    #[serde(default = "default_challenge_taunt")]
    failure_taunt: String,
}

struct ChallengeStart {
    target_words: u32,
    duration_seconds: u32,
    mode: String,
    destination_scene: Option<SceneId>,
}

fn default_kamikaze_idle_seconds() -> u64 {
    10
}

fn default_challenge_pauses() -> u8 {
    2
}

fn default_challenge_deletion_rate() -> u32 {
    1
}

fn default_challenge_taunt() -> String {
    "Apparently {words} words was too much to ask.".into()
}

impl Default for ChallengeConfig {
    fn default() -> Self {
        Self {
            version: 1,
            kamikaze_idle_seconds: default_kamikaze_idle_seconds(),
            grace_pauses: default_challenge_pauses(),
            deletion_words_per_penalty: default_challenge_deletion_rate(),
            failure_taunt: default_challenge_taunt(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandAction {
    Workspace(Workspace),
    OpenMainMenu,
    Save,
    Help,
    RestoreRevisionHunk,
    Process { expand_transclusions: bool },
    ProcessSelected { expand_transclusions: bool },
    LegacyPalette,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandSpec {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: &'static str,
    mnemonic: Option<char>,
    palette_aliases: &'static [&'static str],
    palette_prefix: Option<&'static str>,
    takes_arguments: bool,
    action: CommandAction,
}

fn command_registry() -> Vec<CommandSpec> {
    let mut commands = Workspace::ALL
        .iter()
        .copied()
        .map(|workspace| CommandSpec {
            id: match workspace {
                Workspace::Dashboard => "view.dashboard",
                Workspace::Outline => "view.outline",
                Workspace::Cards => "view.cards",
                Workspace::Editor => "view.editor",
                Workspace::PlotGrid => "view.plot-grid",
                Workspace::Characters => "view.characters",
                Workspace::StoryBible => "view.story-bible",
                Workspace::Timeline => "view.timeline",
                Workspace::Analytics => "view.analytics",
                Workspace::Research => "view.research",
                Workspace::Issues => "view.issues",
                Workspace::Search => "view.search",
                Workspace::Reading => "view.reading",
                Workspace::Revisions => "view.revisions",
                Workspace::PlanningBoards => "view.planning-boards",
            },
            label: workspace.title(),
            description: "Switch to this workspace",
            category: "View",
            mnemonic: None,
            palette_aliases: &[],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::Workspace(workspace),
        })
        .collect::<Vec<_>>();
    commands.extend([
        CommandSpec {
            id: "menu.main",
            label: "Main Menu",
            description: "Open the hierarchical command menu",
            category: "Menu",
            mnemonic: Some('m'),
            palette_aliases: &[],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::OpenMainMenu,
        },
        CommandSpec {
            id: "file.save",
            label: "Save Manuscript Buffer",
            description: "Save the current scene or resumable multi-scene assembly buffer",
            category: "File",
            mnemonic: Some('s'),
            palette_aliases: &["save"],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::Save,
        },
        CommandSpec {
            id: "help.context",
            label: "Context Help",
            description: "Show keyboard and command help",
            category: "Help",
            mnemonic: Some('h'),
            palette_aliases: &["help"],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::Help,
        },
        CommandSpec {
            id: "revision.restore-hunk",
            label: "Restore Selected Revision Hunk",
            description: "Restore the highlighted snapshot diff hunk",
            category: "File",
            mnemonic: Some('r'),
            palette_aliases: &[],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::RestoreRevisionHunk,
        },
        CommandSpec {
            id: "compile.run",
            label: "Compile",
            description: "Run the manuscript processing pipeline",
            category: "Compile",
            mnemonic: Some('c'),
            palette_aliases: &["process", "compile"],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::Process {
                expand_transclusions: false,
            },
        },
        CommandSpec {
            id: "compile.expand-transclusions",
            label: "Compile with Transclusions",
            description: "Compile a derived view with scene inclusions expanded",
            category: "Compile",
            mnemonic: None,
            palette_aliases: &["process-expand", "compile-expand"],
            palette_prefix: None,
            takes_arguments: false,
            action: CommandAction::Process {
                expand_transclusions: true,
            },
        },
        CommandSpec {
            id: "compile.selected",
            label: "Compile Selected Stages",
            description: "Run selected processing stages and their dependencies",
            category: "Compile",
            mnemonic: None,
            palette_aliases: &["process-only"],
            palette_prefix: None,
            takes_arguments: true,
            action: CommandAction::ProcessSelected {
                expand_transclusions: false,
            },
        },
        CommandSpec {
            id: "compile.selected-expand-transclusions",
            label: "Compile Selected Stages with Transclusions",
            description: "Run selected stages with scene inclusions expanded",
            category: "Compile",
            mnemonic: None,
            palette_aliases: &["process-expand-only"],
            palette_prefix: None,
            takes_arguments: true,
            action: CommandAction::ProcessSelected {
                expand_transclusions: true,
            },
        },
    ]);
    commands.extend([
        legacy_command(
            "compile.format",
            "Compile to a Document Format",
            "Compile the manuscript to Markdown, TeX, HTML, EPUB, DOCX, or PDF",
            "Compile",
            "compile-format",
            true,
        ),
        legacy_command(
            "review.merge",
            "Merge Editorial Revision",
            "Three-way merge a reviewed manuscript into a scene and preserve conflicts for inspection",
            "Edit",
            "review-merge",
            true,
        ),
        legacy_command(
            "review.resolve",
            "Resolve Editorial Conflicts",
            "Apply explicit base, local, or reviewed choices to each overlapping hunk",
            "Edit",
            "review-resolve",
            true,
        ),
        legacy_command(
            "review.docx",
            "Accept or Reject DOCX Revisions",
            "Choose a tracked-changes view and three-way merge it into a scene",
            "Edit",
            "review-docx",
            true,
        ),
        legacy_command(
            "review.inspect",
            "Inspect DOCX Review Markup",
            "List tracked edits and comments without modifying the source file",
            "Edit",
            "review-inspect",
            true,
        ),
        legacy_command(
            "scene.create",
            "New Scene",
            "Create a scene in a chapter",
            "Insert",
            "scene",
            true,
        ),
        legacy_command(
            "scene.move",
            "Move Scene",
            "Move a scene to a chapter and position",
            "Scene",
            "move",
            true,
        ),
        legacy_command(
            "scene.rename",
            "Rename Scene",
            "Rename a scene without changing its identity",
            "Edit",
            "rename",
            true,
        ),
        legacy_command(
            "scene.status",
            "Set Scene Status",
            "Change a scene's drafting status",
            "Edit",
            "scene-status",
            true,
        ),
        legacy_command(
            "scene.field",
            "Edit Scene Planning Field",
            "Edit a scene synopsis or purpose field",
            "Edit",
            "scene-field",
            true,
        ),
        legacy_command(
            "scene.subchapter",
            "New Chapter in Section",
            "Create a chapter under a section",
            "Insert",
            "subchapter",
            true,
        ),
        legacy_command(
            "structure.section",
            "New Section",
            "Create a top-level section",
            "Insert",
            "section",
            true,
        ),
        legacy_command(
            "structure.chapter",
            "New Chapter",
            "Create a top-level chapter",
            "Insert",
            "chapter",
            true,
        ),
        legacy_command(
            "character.create",
            "New Character",
            "Create a character record",
            "Plot",
            "character",
            true,
        ),
        legacy_command(
            "character.profile",
            "Show Character Profile",
            "Inspect structured character information",
            "Plot",
            "character-profile",
            true,
        ),
        legacy_command(
            "character.set",
            "Edit Character Field",
            "Update a structured character profile field",
            "Plot",
            "character-set",
            true,
        ),
        legacy_command(
            "character.alias",
            "Add Character Alias",
            "Add a searchable character alias",
            "Plot",
            "character-alias",
            true,
        ),
        legacy_command(
            "character.un-alias",
            "Remove Character Alias",
            "Remove an alias from a character profile",
            "Plot",
            "character-un-alias",
            true,
        ),
        legacy_command(
            "character.link",
            "Link Character to Scene",
            "Record character participation in a scene",
            "Plot",
            "character-link",
            true,
        ),
        legacy_command(
            "location.create",
            "New Location",
            "Create a location record",
            "Plot",
            "location",
            true,
        ),
        legacy_command(
            "plotline.create",
            "New Plotline",
            "Create a plotline",
            "Plot",
            "plotline",
            true,
        ),
        legacy_command(
            "plotline.link",
            "Link Plotline to Scene",
            "Assign a plotline and attribution policy",
            "Plot",
            "plotline-link",
            true,
        ),
        legacy_command(
            "plotline.shares",
            "Set Plotline Word Shares",
            "Set complete basis-point allocations",
            "Plot",
            "plotline-shares",
            true,
        ),
        legacy_command(
            "event.create",
            "New Story Event",
            "Create a story event",
            "Plot",
            "event",
            true,
        ),
        legacy_command(
            "event.create-timed",
            "New Timed Story Event",
            "Create an event with start and end intervals",
            "Plot",
            "event-time",
            true,
        ),
        legacy_command(
            "event.edit-time",
            "Edit Event Chronology",
            "Update story-time bounds while preserving manuscript order",
            "Plot",
            "event-time-set",
            true,
        ),
        legacy_command(
            "event.link",
            "Link Event to Scene",
            "Associate a story event with a scene",
            "Plot",
            "event-link",
            true,
        ),
        legacy_command(
            "issue.create",
            "New Writing Issue",
            "Create an issue",
            "Tools",
            "issue",
            true,
        ),
        legacy_command(
            "issue.link",
            "Link Issue to Scene",
            "Associate an issue with a scene",
            "Tools",
            "issue-link",
            true,
        ),
        legacy_command(
            "issue.status",
            "Change Issue Status",
            "Update an issue's workflow status",
            "Tools",
            "issue-status",
            true,
        ),
        legacy_command(
            "research.create",
            "New Research Note",
            "Create a research note",
            "Tools",
            "research",
            true,
        ),
        legacy_command(
            "research.import",
            "Import Reading Document",
            "Import a text, Markdown, or EPUB reference",
            "Tools",
            "read-import",
            true,
        ),
        legacy_command(
            "research.open",
            "Open Reading Document",
            "Open a reference by stable ID",
            "Tools",
            "read",
            true,
        ),
        legacy_command(
            "research.highlight",
            "Add Reading Highlight",
            "Anchor a highlight to a reference passage",
            "Tools",
            "highlight",
            true,
        ),
        legacy_command(
            "research.annotation",
            "Add Reading Note",
            "Anchor an annotation to a reference passage",
            "Tools",
            "note",
            true,
        ),
        legacy_command(
            "research.bookmark",
            "Add Reading Bookmark",
            "Bookmark the current reading position",
            "Tools",
            "bookmark",
            true,
        ),
        legacy_command(
            "research.term",
            "Add Reading Term",
            "Add a character, species, or term index entry",
            "Tools",
            "term",
            true,
        ),
        legacy_command(
            "research.entity-link",
            "Link Reference to Story Entity",
            "Link a reading source to a story entity",
            "Tools",
            "entity-link",
            true,
        ),
        legacy_command(
            "research.follow-link",
            "Follow Reading Link",
            "Navigate to a linked story entity",
            "Tools",
            "reading-follow",
            true,
        ),
        legacy_command(
            "search.project",
            "Search Project",
            "Search all indexed project content",
            "View",
            "search-all",
            true,
        ),
        legacy_command(
            "search.saved-query",
            "Run Saved Query",
            "Open a saved dynamic scene collection",
            "View",
            "query",
            true,
        ),
        legacy_command(
            "search.save-query",
            "Save Scene Query",
            "Save structured scene filters",
            "View",
            "query-save",
            true,
        ),
        legacy_command(
            "search.query-expression",
            "Set Nested Query Expression",
            "Set a JSON AND/OR/NOT expression on a saved query",
            "View",
            "query-expression-set",
            true,
        ),
        legacy_command(
            "search.list-queries",
            "List Saved Queries",
            "List saved structured queries",
            "View",
            "query-list",
            false,
        ),
        legacy_command(
            "settings.keybinding-set",
            "Bind Command Shortcut",
            "Assign a validated modified-key chord to a stable command ID",
            "Tools",
            "keybinding-set",
            true,
        ),
        legacy_command(
            "settings.keybinding-remove",
            "Remove Command Shortcut",
            "Remove a command chord by its key combination",
            "Tools",
            "keybinding-remove",
            true,
        ),
        legacy_command(
            "settings.keybinding-list",
            "List Command Shortcuts",
            "Inspect project-local command shortcuts",
            "Tools",
            "keybinding-list",
            false,
        ),
        legacy_command(
            "search.query-evidence",
            "Inspect Query Match Evidence",
            "Show stable story-model sources attached to a query result",
            "View",
            "query-evidence",
            true,
        ),
        legacy_command(
            "search.clear-query",
            "Clear Query View",
            "Leave the current virtual collection",
            "View",
            "query-clear",
            false,
        ),
        legacy_command("search.query-assert-save", "Save Query Assertions", "Persist count assertions for a saved query", "View", "query-assert-save", true),
        legacy_command("search.query-assert-check", "Check Query Assertions", "Evaluate saved count assertions against refreshed results", "View", "query-assert-check", true),
        legacy_command("analysis.coverage-add", "Add Coverage Rule", "Add a project-local character or plotline appearance requirement", "Plot", "coverage-add", true),
        legacy_command("analysis.event-dependency-add", "Add Event Dependency", "Record a story-time precedence or prerequisite assertion", "Plot", "event-dependency-add", true),
        legacy_command("analysis.event-dependency-list", "List Event Dependencies", "Show saved event-order assertions", "Plot", "event-dependency-list", false),
        legacy_command("analysis.event-dependency-check", "Check Event Dependencies", "Evaluate saved event-order assertions with uncertainty", "Plot", "event-dependency-check", false),
        legacy_command("analysis.event-dependency-update", "Update Event Dependency", "Edit the note on a saved event assertion", "Plot", "event-dependency-update", true),
        legacy_command("analysis.event-dependency-remove", "Remove Event Dependency", "Delete a saved event-order assertion", "Plot", "event-dependency-remove", true),
        legacy_command("analysis.location-rule-add", "Add Location Conflict Rule", "Record two locations that cannot overlap in time", "Plot", "location-rule-add", true),
        legacy_command("analysis.location-rule-list", "List Location Conflict Rules", "Show author-defined location incompatibilities", "Plot", "location-rule-list", false),
        legacy_command("analysis.location-rule-update", "Update Location Conflict Rule", "Enable, disable or rename a location incompatibility", "Plot", "location-rule-update", true),
        legacy_command("analysis.location-rule-check", "Check Location Conflict Rules", "Inspect scenes against location rules", "Plot", "location-rule-check", false),
        legacy_command("analysis.location-rule-remove", "Remove Location Conflict Rule", "Delete an author-defined location rule", "Plot", "location-rule-remove", true),
        legacy_command("analysis.coverage-check", "Check Coverage Rules", "Evaluate saved coverage requirements without changing story data", "Plot", "coverage-check", false),
        legacy_command("analysis.coverage-list", "List Coverage Rules", "List project-local coverage requirements", "Plot", "coverage-list", false),
        legacy_command("analysis.coverage-remove", "Remove Coverage Rule", "Remove a coverage requirement by ID", "Plot", "coverage-remove", true),
        legacy_command("analysis.spellcheck", "Spellcheck Manuscript", "Run local Aspell without changing prose", "Tools", "spellcheck", true),
        legacy_command(
            "link.create",
            "Create Story Link",
            "Create a typed link between stable entities",
            "Edit",
            "link",
            true,
        ),
        legacy_command(
            "link.outgoing",
            "Show Outgoing Links",
            "List outgoing links from an entity",
            "Edit",
            "outgoing",
            true,
        ),
        legacy_command(
            "link.backlinks",
            "Show Backlinks",
            "List links that target an entity",
            "Edit",
            "backlinks",
            true,
        ),
        legacy_command(
            "link.broken",
            "Find Broken Links",
            "Report links with missing entities or anchors",
            "Tools",
            "broken-links",
            false,
        ),
        legacy_command(
            "anchor.list",
            "List Passage Anchors",
            "Inspect anchors in the selected scene",
            "Edit",
            "anchors",
            false,
        ),
        legacy_command(
            "anchor.move",
            "Reattach Passage Anchor",
            "Explicitly reattach an anchor to selected bytes",
            "Edit",
            "anchor-move",
            true,
        ),
        legacy_command(
            "compile.preview-inclusions",
            "Preview Document Inclusions",
            "Preview a derived scene inclusion view",
            "Compile",
            "transclude",
            true,
        ),
        legacy_command(
            "revision.snapshot",
            "Create Snapshot",
            "Create a labelled scene snapshot",
            "File",
            "snapshot",
            true,
        ),
        legacy_command(
            "revision.history",
            "Show Revision History",
            "List snapshots for a scene",
            "File",
            "history",
            false,
        ),
        legacy_command(
            "revision.diff",
            "Compare Snapshot",
            "Compare current text to a snapshot",
            "File",
            "diff",
            true,
        ),
        legacy_command(
            "revision.restore",
            "Restore Snapshot Range",
            "Restore selected snapshot text",
            "File",
            "restore",
            true,
        ),
        legacy_command(
            "revision.restore-lines",
            "Restore Selected Snapshot Lines",
            "Restore a line range from a snapshot through the durable save journal",
            "File",
            "restore-lines",
            true,
        ),
        legacy_command(
            "git.snapshot",
            "Create Project Snapshot",
            "Commit consistent prose and SQL metadata to local Git history",
            "File",
            "git-snapshot",
            true,
        ),
        legacy_command(
            "git.history",
            "Show Project History",
            "List local project snapshots",
            "File",
            "git-history",
            false,
        ),
        legacy_command(
            "git.diff",
            "Compare Project Snapshots",
            "Compare two local project snapshots",
            "File",
            "git-diff",
            true,
        ),
        legacy_command(
            "git.restore-new",
            "Restore Snapshot as New Project",
            "Reconstruct a validated historical project in a new directory",
            "File",
            "git-restore-new",
            true,
        ),
        legacy_command(
            "git.restore-scene",
            "Restore Scene from Snapshot",
            "Restore one scene's historical prose through the journal after snapshotting current state",
            "File",
            "git-restore-scene",
            true,
        ),
        legacy_command(
            "extract.propose",
            "Extract Story Candidates",
            "Create provisional Ollama candidates",
            "Tools",
            "extract",
            true,
        ),
        legacy_command(
            "extract.approve",
            "Approve Candidate",
            "Approve a candidate after evidence validation",
            "Tools",
            "approve",
            true,
        ),
        legacy_command(
            "extract.dismiss",
            "Dismiss Candidate",
            "Dismiss a provisional claim while retaining its evidence",
            "Tools",
            "dismiss",
            true,
        ),
        legacy_command(
            "extract.review",
            "Review Candidate Proposals",
            "List provisional, approved, and dismissed candidate claims",
            "Tools",
            "proposals",
            false,
        ),
        legacy_command(
            "extract.knowledge",
            "List Approved Knowledge",
            "Show evidence-linked approved claims",
            "Story Bible",
            "knowledge",
            false,
        ),
        legacy_command(
            "reading.jump",
            "Jump to Reading Offset",
            "Navigate to a source byte position",
            "Tools",
            "jump",
            true,
        ),
        legacy_command(
            "reading.list-bookmarks",
            "List Reading Bookmarks",
            "List bookmarks in the current document",
            "Tools",
            "bookmarks",
            false,
        ),
        legacy_command(
            "reading.list-annotations",
            "List Reading Annotations",
            "List annotations in the current document",
            "Tools",
            "annotations",
            false,
        ),
        legacy_command(
            "reading.annotation-jump",
            "Jump to Annotation",
            "Navigate to a saved annotation anchor",
            "Tools",
            "annotation-jump",
            true,
        ),
        legacy_command(
            "reading.bookmark-jump",
            "Jump to Bookmark",
            "Navigate to a saved bookmark anchor",
            "Tools",
            "bookmark-jump",
            true,
        ),
        legacy_command(
            "reading.list-terms",
            "List Reading Terms",
            "List the current document's terminology index",
            "Tools",
            "terms",
            false,
        ),
        legacy_command(
            "reading.links",
            "List Reading Links",
            "List entity links from the current document",
            "Tools",
            "reading-links",
            false,
        ),
        legacy_command(
            "workspace.open",
            "Open Workspace",
            "Switch to a named workspace",
            "View",
            "workspace",
            true,
        ),
        legacy_command(
            "scene.open",
            "Open Scene",
            "Open a scene by stable ID",
            "View",
            "open",
            true,
        ),
        legacy_command(
            "scratchpad.create",
            "Quick Capture",
            "Capture a note without leaving the current scene",
            "Tools",
            "scratchpad",
            true,
        ),
        legacy_command(
            "scratchpad.list",
            "Scratchpad Notes",
            "List active and promoted scratchpad notes",
            "Tools",
            "scratchpad-list",
            false,
        ),
        legacy_command(
            "scratchpad.pin",
            "Pin Scratchpad Note",
            "Pin or unpin a quick-capture note",
            "Tools",
            "scratchpad-pin",
            true,
        ),
        legacy_command(
            "scratchpad.promote",
            "Promote Scratchpad Note",
            "Link a note to a scene or story-bible entity",
            "Tools",
            "scratchpad-promote",
            true,
        ),
        legacy_command(
            "mention.list",
            "Scene Mentions",
            "Inspect candidate story-bible mentions in the current scene",
            "Scene",
            "mentions",
            false,
        ),
        legacy_command(
            "mention.jump",
            "Jump to Story Mention",
            "Navigate to a detected passage",
            "Scene",
            "mention-jump",
            true,
        ),
        legacy_command(
            "mention.appearances",
            "Entity Mention Appearances",
            "List confirmed mention backlinks",
            "Plot",
            "mention-appearances",
            true,
        ),
        legacy_command(
            "mention.confirm",
            "Confirm Entity Mention",
            "Confirm a detected story-bible reference",
            "Scene",
            "mention-confirm",
            true,
        ),
        legacy_command(
            "mention.dismiss",
            "Dismiss Entity Mention",
            "Dismiss a false positive mention",
            "Scene",
            "mention-dismiss",
            true,
        ),
        legacy_command(
            "object.create",
            "New Story Object",
            "Create an object for mention detection",
            "Plot",
            "object",
            true,
        ),
        legacy_command(
            "comment.create",
            "Comment on Selection",
            "Attach a personal note to selected manuscript text",
            "Edit",
            "comment",
            true,
        ),
        legacy_command(
            "comment.list",
            "Scene Comments",
            "List personal notes and anchor status",
            "Edit",
            "comments",
            false,
        ),
        legacy_command(
            "comment.state",
            "Resolve or Reopen Comment",
            "Change personal comment state",
            "Edit",
            "comment-state",
            true,
        ),
        legacy_command(
            "assembly.create",
            "Create Manuscript Assembly",
            "Create an independent edition ordering",
            "Compile",
            "assembly",
            true,
        ),
        legacy_command(
            "assembly.list",
            "List Manuscript Assemblies",
            "Inspect alternate manuscript arrangements",
            "Compile",
            "assembly-list",
            false,
        ),
        legacy_command(
            "assembly.edit",
            "Edit Assembly Scenes",
            "Open an editable buffer spanning every scene in an assembly",
            "Edit",
            "assembly-edit",
            true,
        ),
        legacy_command(
            "assembly.resume",
            "Resume Assembly Draft",
            "Resume a durable multi-scene edit buffer by draft ID",
            "Edit",
            "assembly-resume",
            true,
        ),
        legacy_command(
            "assembly.drafts",
            "List Assembly Drafts",
            "Inspect interrupted or pending assembly edits",
            "Edit",
            "assembly-drafts",
            false,
        ),
        legacy_command(
            "assembly.add",
            "Add Scene to Assembly",
            "Add a scene without changing canonical placement",
            "Compile",
            "assembly-add",
            true,
        ),
        legacy_command(
            "assembly.move",
            "Reorder Assembly Item",
            "Reorder content in an assembly only",
            "Compile",
            "assembly-move",
            true,
        ),
        legacy_command(
            "assembly.include",
            "Include or Exclude Assembly Item",
            "Control edition membership",
            "Compile",
            "assembly-include",
            true,
        ),
        legacy_command(
            "assembly.compile",
            "Compile Assembly",
            "Render an assembly through Compile IR",
            "Compile",
            "assembly-compile",
            true,
        ),
        legacy_command(
            "board.create",
            "Create Planning Board",
            "Create a spatial planning canvas",
            "Plan",
            "board",
            true,
        ),
        legacy_command(
            "board.open",
            "Open Planning Board",
            "Open a board to inspect and move its cards",
            "Plan",
            "board-open",
            true,
        ),
        legacy_command(
            "board.back",
            "Board List",
            "Return to planning board list",
            "Plan",
            "board-back",
            false,
        ),
        legacy_command(
            "board.list",
            "List Board Items",
            "Inspect entities placed on a planning board",
            "Plan",
            "board-list",
            true,
        ),
        legacy_command(
            "board.export",
            "Export Planning Board",
            "Render a portable Markdown table of this board's cards",
            "Plan",
            "board-export",
            true,
        ),
        legacy_command(
            "board.move",
            "Move Planning Card",
            "Change a card's canvas coordinates without affecting story order",
            "Plan",
            "board-move",
            true,
        ),
        legacy_command(
            "board.resize",
            "Resize Planning Card",
            "Change a planning card's canvas width and height",
            "Plan",
            "board-resize",
            true,
        ),
        legacy_command(
            "board.connect",
            "Connect Planning Cards",
            "Add a labeled relationship between cards on one board",
            "Plan",
            "board-connect",
            true,
        ),
        legacy_command(
            "board.add-scene",
            "Place Scene on Board",
            "Place a scene without changing manuscript order",
            "Plan",
            "board-add-scene",
            true,
        ),
        legacy_command(
            "board.add",
            "Add Planning Card",
            "Place a supported story entity on a planning board",
            "Plan",
            "board-add",
            true,
        ),
        legacy_command(
            "writing.stats",
            "Writing Statistics",
            "Opt in to local aggregate counts without recording manuscript text",
            "Tools",
            "writing-stats",
            true,
        ),
        legacy_command("edit.snippet-save", "Define Session Snippet", "Store a reusable snippet with an optional ${cursor} marker", "Insert", "snippet", true),
        legacy_command("edit.snippet-insert", "Insert Session Snippet", "Expand and insert a named session snippet", "Insert", "snippet-insert", true),
        legacy_command("edit.register-save", "Save Editor Register", "Save the selection or current line in a named register", "Edit", "register-save", true),
        legacy_command("edit.register-insert", "Insert Editor Register", "Insert a named editor register", "Edit", "register-insert", true),
        legacy_command("edit.register-rotate", "Rotate Editor Registers", "Rotate values through two or more named registers", "Edit", "register-rotate", true),
        legacy_command("edit.multi-insert", "Multi-cursor Insert", "Insert text at multiple original UTF-8 byte offsets", "Edit", "multi-insert", true),
        legacy_command("edit.macro", "Run Edit Macro", "Apply an undoable selection or line edit macro", "Edit", "macro", true),
        legacy_command("edit.macro-define", "Define Edit Macro", "Save a reusable, data-only edit macro", "Edit", "macro-define", true),
        legacy_command("edit.macro-run", "Run Named Edit Macro", "Apply a saved macro once or a bounded number of times", "Edit", "macro-run", true),
        legacy_command("edit.macro-preview", "Preview Named Edit Macro", "Preview a macro and repetition count without changing manuscript text", "Edit", "macro-preview", true),
        legacy_command("edit.macro-record", "Record Edit Macro", "Record editor insertions, deletions, cursor movement, and selection changes", "Edit", "macro-record", true),
        legacy_command("edit.macro-stop", "Stop Macro Recording", "Save the currently recorded edit macro", "Edit", "macro-stop", false),
        legacy_command("command-macro.record", "Record Command Sequence", "Record stable command-registry actions for later replay", "Edit", "command-macro-record", true),
        legacy_command("command-macro.stop", "Stop Command Recording", "Save the currently recorded command sequence", "Edit", "command-macro-stop", false),
        legacy_command("command-macro.run", "Run Command Sequence", "Replay a saved command sequence through the shared dispatcher", "Edit", "command-macro-run", true),
        legacy_command("command-macro.list", "List Command Sequences", "List saved command sequences", "Edit", "command-macro-list", false),
        legacy_command("edit.text-expansion-set", "Set Text Expansion", "Add a literal abbreviation and replacement to the project dictionary", "Insert", "text-expansion-set", true),
        legacy_command("edit.text-expansion", "Text Expansion Settings", "Enable, disable, or list project text expansions", "Insert", "text-expansion", true),
        legacy_command("edit.text-expand", "Expand Abbreviation", "Expand the abbreviation immediately before the editor cursor", "Insert", "text-expand", false),
        legacy_command("edit.preferences", "Editor Preferences", "Set project-local line endings and indentation without rewriting until save", "Edit", "editor-preferences", true),
        legacy_command("edit.bookmark-save", "Save Passage Bookmark", "Save a named stable-ID bookmark for the current passage", "Navigate", "manuscript-bookmark-save", true),
        legacy_command("edit.bookmark-list", "List Passage Bookmarks", "List saved manuscript passage bookmarks", "Navigate", "manuscript-bookmark-list", false),
        legacy_command("edit.bookmark-jump", "Jump to Passage Bookmark", "Navigate to a saved passage, following stable scene identity", "Navigate", "manuscript-bookmark-jump", true),
        legacy_command("edit.kill-ring-list", "List Kill Ring", "Inspect retained copied and cut manuscript text", "Edit", "kill-ring-list", false),
        legacy_command("edit.kill-ring-insert", "Insert Kill Ring Entry", "Insert a retained text entry by its number", "Edit", "kill-ring-insert", true),
        legacy_command("edit.yank-pop", "Cycle Previous Yank", "Replace the last yank with the next retained kill-ring entry", "Edit", "yank-pop", false),
        legacy_command("edit.cursor-add", "Add Multi-cursor", "Add an editing cursor at grapheme row and column", "Edit", "cursor-add", true),
        legacy_command("edit.cursor-clear", "Clear Multi-cursors", "Return to the primary editor cursor", "Edit", "cursor-clear", false),
        legacy_command("challenge.start", "Start Writing Challenge", "Start a timed word target; opt-in Kamikaze mode reacts to inactivity", "Tools", "challenge-start", true),
        legacy_command("challenge.finish", "Finish Writing Challenge", "Record words written and finish the active timed challenge", "Tools", "challenge-finish", false),
        legacy_command("challenge.idle", "Set Kamikaze Idle Threshold", "Set the project-local inactivity threshold for Kamikaze mode", "Tools", "challenge-idle", true),
        legacy_command(
            "story-template-list",
            "List Story Bible Templates",
            "Inspect reusable profile templates",
            "Story Bible",
            "story-template-list",
            false,
        ),
        legacy_command(
            "story-template-import",
            "Import Story Bible Template",
            "Add a validated TOML profile template",
            "Story Bible",
            "story-template-import",
            true,
        ),
        legacy_command(
            "story-profile-show",
            "Show Story Bible Profile",
            "Inspect typed values for a story entity",
            "Story Bible",
            "story-profile-show",
            true,
        ),
        legacy_command(
            "story-profile-import",
            "Import Story Bible Profile",
            "Validate and store a typed profile from JSON",
            "Story Bible",
            "story-profile-import",
            true,
        ),
    ]);
    commands
}

fn legacy_command(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: &'static str,
    alias: &'static str,
    takes_arguments: bool,
) -> CommandSpec {
    CommandSpec {
        id,
        label,
        description,
        category,
        mnemonic: None,
        palette_aliases: &[],
        palette_prefix: Some(alias),
        takes_arguments,
        action: CommandAction::LegacyPalette,
    }
}

fn command_by_id(id: &str) -> Option<CommandSpec> {
    command_registry()
        .into_iter()
        .find(|command| command.id == id)
}

fn command_for_workspace(workspace: Workspace) -> Option<CommandSpec> {
    command_registry()
        .into_iter()
        .find(|command| command.action == CommandAction::Workspace(workspace))
}

fn command_palette_invocation(text: &str) -> Option<(CommandSpec, Option<&str>)> {
    for command in command_registry() {
        for alias in command
            .palette_aliases
            .iter()
            .copied()
            .chain(command.palette_prefix)
        {
            if text == alias {
                return Some((command, None));
            }
            if command.takes_arguments
                && let Some(arguments) = text
                    .strip_prefix(alias)
                    .and_then(|remaining| remaining.strip_prefix(' '))
            {
                return Some((command, Some(arguments)));
            }
        }
    }
    None
}

fn menu_category(category: &str) -> &'static str {
    match category {
        "File" => "File",
        "Edit" => "Edit",
        "Insert" => "Insert",
        "Scene" => "Scene",
        "Plot" => "Plot",
        "View" => "View",
        "Compile" => "Compile",
        "Help" => "Help",
        _ => "Tools",
    }
}

fn is_menu_shortcut(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('/') | KeyCode::Char('_'))
}

fn editor_position_byte(lines: &[String], row: usize, column: usize) -> usize {
    let prefix_bytes = lines
        .iter()
        .take(row)
        .map(|line| line.len() + 1)
        .sum::<usize>();
    let line = lines.get(row).map(String::as_str).unwrap_or_default();
    prefix_bytes + grapheme_byte(line, column)
}

fn grapheme_byte(text: &str, column: usize) -> usize {
    text.grapheme_indices(true)
        .nth(column)
        .map(|(offset, _)| offset)
        .unwrap_or(text.len())
}

fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

fn editor_position_from_byte(text: &str, byte: usize) -> Option<(usize, usize)> {
    let prefix = text.get(..byte)?;
    Some((
        prefix.matches('\n').count(),
        grapheme_count(prefix.rsplit('\n').next().unwrap_or_default()),
    ))
}

fn parse_processing_stages(names: &str) -> Result<Vec<story_pipeline::Stage>, String> {
    if names.trim().is_empty() {
        return Err("provide at least one processing stage".into());
    }
    names
        .split(',')
        .map(|name| {
            story_pipeline::Stage::parse_name(name.trim())
                .ok_or_else(|| format!("unknown processing stage '{name}'"))
        })
        .collect()
}

fn parse_find_mode(value: &str) -> Result<story_core::FindMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "normal" => Ok(story_core::FindMode::Normal),
        "extended" => Ok(story_core::FindMode::Extended),
        "regex" => Ok(story_core::FindMode::Regex),
        _ => Err("search mode must be normal, extended, or regex".into()),
    }
}

fn load_editor_tools(root: &Path) -> Result<EditorToolsFile, Box<dyn std::error::Error>> {
    let path = root.join(".writers-ide/editor-tools.json");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(EditorToolsFile {
                version: 1,
                ..EditorToolsFile::default()
            });
        }
        Err(error) => return Err(error.into()),
    };
    let tools: EditorToolsFile = serde_json::from_slice(&bytes)?;
    if tools.version != 1 {
        return Err(format!("unsupported editor-tools version {}", tools.version).into());
    }
    Ok(tools)
}

fn load_text_expansion(root: &Path) -> Result<TextExpansionConfig, Box<dyn std::error::Error>> {
    let path = root.join(".writers-ide/text-expansion.toml");
    match fs::read_to_string(path) {
        Ok(text) => {
            let config: TextExpansionFile = toml::from_str(&text)?;
            if config
                .text_expansion
                .triggers
                .iter()
                .any(|trigger| !matches!(trigger.as_str(), "space" | "enter" | "tab"))
            {
                return Err("text expansion triggers must be space or enter".into());
            }
            if config
                .text_expansion
                .mappings
                .iter()
                .any(|(key, _)| key.is_empty() || key.chars().any(char::is_whitespace))
            {
                return Err(
                    "text expansion abbreviations must be non-empty and contain no whitespace"
                        .into(),
                );
            }
            Ok(config.text_expansion)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(TextExpansionConfig::default()),
        Err(error) => Err(error.into()),
    }
}

fn save_text_expansion(
    root: &Path,
    config: &TextExpansionConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join(".writers-ide");
    fs::create_dir_all(&directory)?;
    let path = directory.join("text-expansion.toml");
    let text = toml::to_string_pretty(&TextExpansionFile {
        text_expansion: config.clone(),
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn load_editor_preferences(root: &Path) -> Result<EditorPreferences, Box<dyn std::error::Error>> {
    let path = root.join(".writers-ide/editor-preferences.toml");
    match fs::read_to_string(path) {
        Ok(text) => {
            let file: EditorPreferencesFile = toml::from_str(&text)?;
            validate_editor_preferences(&file.editor)?;
            Ok(file.editor)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(EditorPreferences::default()),
        Err(error) => Err(error.into()),
    }
}

fn save_editor_preferences(
    root: &Path,
    preferences: &EditorPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_editor_preferences(preferences)?;
    let directory = root.join(".writers-ide");
    fs::create_dir_all(&directory)?;
    let path = directory.join("editor-preferences.toml");
    let text = toml::to_string_pretty(&EditorPreferencesFile {
        editor: preferences.clone(),
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn validate_editor_preferences(
    preferences: &EditorPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(preferences.line_ending.as_str(), "lf" | "crlf")
        || !matches!(preferences.indentation.as_str(), "tabs" | "spaces")
        || !(1..=8).contains(&preferences.indent_width)
    {
        return Err("editor preferences require line_ending=lf|crlf, indentation=tabs|spaces, and indent_width=1..8".into());
    }
    Ok(())
}

fn parse_macro_recipe(
    recipe: &str,
) -> Result<Vec<story_core::EditMacroOp>, Box<dyn std::error::Error>> {
    recipe
        .split('|')
        .map(|part| {
            let part = part.trim();
            if part == "upper" {
                Ok(story_core::EditMacroOp::UppercaseSelection)
            } else if part == "lower" {
                Ok(story_core::EditMacroOp::LowercaseSelection)
            } else if let Some(text) = part
                .strip_prefix("insert:")
                .or_else(|| part.strip_prefix("insert "))
            {
                Ok(story_core::EditMacroOp::Insert(text.to_owned()))
            } else if let Some(pair) = part
                .strip_prefix("replace:")
                .or_else(|| part.strip_prefix("replace "))
            {
                let (from, to) = pair
                    .split_once("=>")
                    .ok_or("macro replace must use FROM=>TO")?;
                Ok(story_core::EditMacroOp::Replace {
                    from: from.to_owned(),
                    to: to.to_owned(),
                })
            } else {
                Err(format!("unknown edit macro operation: {part}").into())
            }
        })
        .collect()
}

fn parse_macro_run_arguments(input: &str) -> Result<(&str, usize), String> {
    let mut parts = input.split_whitespace();
    let name = parts.next().ok_or("use macro-run NAME [REPETITIONS]")?;
    let repetitions = parts
        .next()
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|_| "macro repetitions must be a positive integer")?
        .unwrap_or(1);
    if parts.next().is_some() || !(1..=100).contains(&repetitions) {
        return Err("use macro-run NAME [REPETITIONS], with repetitions from 1 to 100".into());
    }
    Ok((name, repetitions))
}

fn repeated_macro_operations(
    operations: &[story_core::EditMacroOp],
    repetitions: usize,
) -> Result<Vec<story_core::EditMacroOp>, String> {
    if operations.is_empty() || !(1..=100).contains(&repetitions) {
        return Err("macro must contain operations and repetitions must be from 1 to 100".into());
    }
    Ok((0..repetitions)
        .flat_map(|_| operations.iter().cloned())
        .collect())
}

fn is_destructive_macro_op(operation: &story_core::EditMacroOp) -> bool {
    matches!(
        operation,
        story_core::EditMacroOp::Backspace
            | story_core::EditMacroOp::DeleteForward
            | story_core::EditMacroOp::Replace { .. }
            | story_core::EditMacroOp::UppercaseSelection
            | story_core::EditMacroOp::LowercaseSelection
    )
}

fn parse_terminal_query_assertion(
    value: &str,
) -> Result<story_core::QueryAssertion, Box<dyn std::error::Error>> {
    match value {
        "non-empty" => Ok(story_core::QueryAssertion::NonEmpty),
        "empty" => Ok(story_core::QueryAssertion::Empty),
        _ => {
            let (kind, value) = value
                .split_once(':')
                .ok_or("numeric assertion must be kind:threshold")?;
            let value = value.parse::<u64>()?;
            Ok(match kind {
                "at-least" => story_core::QueryAssertion::AtLeast(value),
                "at-most" => story_core::QueryAssertion::AtMost(value),
                "exactly" => story_core::QueryAssertion::Exactly(value),
                _ => {
                    return Err(
                        "assertion kind must be at-least, at-most, exactly, empty, or non-empty"
                            .into(),
                    );
                }
            })
        }
    }
}

fn default_bindings() -> Vec<(u8, Workspace)> {
    Workspace::ALL
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, workspace)| {
            if index == 9 {
                None
            } else {
                let function_key = (index + 1) as u8;
                Some((function_key, workspace))
            }
        })
        .collect()
}

fn infer_narrowed_chapter(
    visible: &[(SceneId, String, SceneStatus, u32)],
    all: &[story_storage::SceneRecord],
) -> Option<story_core::NodeId> {
    if visible.is_empty() || visible.len() >= all.len() {
        return None;
    }
    let visible_ids = visible.iter().map(|scene| scene.0).collect::<BTreeSet<_>>();
    let chapter = all
        .iter()
        .filter(|scene| visible_ids.contains(&scene.scene.id))
        .map(|scene| scene.chapter_id)
        .collect::<Option<BTreeSet<_>>>()?;
    if chapter.len() != 1 {
        return None;
    }
    let chapter = *chapter.first()?;
    let chapter_ids = all
        .iter()
        .filter(|scene| scene.chapter_id == Some(chapter))
        .map(|scene| scene.scene.id)
        .collect::<BTreeSet<_>>();
    (chapter_ids == visible_ids).then_some(chapter)
}

fn load_bindings(project: &Path) -> Result<Vec<(u8, Workspace)>, Box<dyn std::error::Error>> {
    let path = project.join(".writers-ide/keybindings.toml");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(default_bindings()),
        Err(error) => return Err(error.into()),
    };
    let config: KeymapFile = toml::from_str(&text)?;
    let mut result = default_bindings();
    for (key, name) in config.bindings {
        let number: u8 = key
            .strip_prefix('F')
            .ok_or("keymap keys must use F1 through F14")?
            .parse()?;
        if !(1..=14).contains(&number) {
            return Err(format!("unsupported function key {key}").into());
        }
        if number == 10 {
            return Err("F10 is reserved for the hierarchical menu fallback".into());
        }
        let workspace = Workspace::ALL
            .iter()
            .copied()
            .find(|w| w.title().eq_ignore_ascii_case(&name))
            .ok_or_else(|| format!("unknown workspace in keymap: {name}"))?;
        if let Some(binding) = result.iter_mut().find(|(function, _)| *function == number) {
            *binding = (number, workspace);
        } else {
            result.push((number, workspace));
            result.sort_by_key(|(function, _)| *function);
        }
    }
    Ok(result)
}

type CommandBindings = Vec<(KeyEvent, String)>;

fn parse_shortcut(value: &str) -> Result<KeyEvent, Box<dyn std::error::Error>> {
    let parts = value.split('+').collect::<Vec<_>>();
    let key = parts.last().copied().ok_or("empty key chord")?;
    let mut modifiers = KeyModifiers::empty();
    for modifier in &parts[..parts.len().saturating_sub(1)] {
        let flag = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return Err(format!("unsupported key modifier: {modifier}").into()),
        };
        if modifiers.contains(flag) {
            return Err(format!("duplicate key modifier: {modifier}").into());
        }
        modifiers |= flag;
    }
    if modifiers.is_empty() {
        return Err("command shortcuts must include Ctrl, Alt, or Shift".into());
    }
    let code = if let Some(number) = key.strip_prefix('F').or_else(|| key.strip_prefix('f')) {
        let number: u8 = number.parse()?;
        if !(1..=14).contains(&number) || number == 10 {
            return Err("function shortcut must be F1–F9 or F11–F14".into());
        }
        KeyCode::F(number)
    } else {
        let mut chars = key.chars();
        let Some(character) = chars.next() else {
            return Err("missing shortcut key".into());
        };
        if chars.next().is_some() || !character.is_ascii() {
            return Err("shortcut key must be one ASCII character or F-key".into());
        }
        KeyCode::Char(character.to_ascii_lowercase())
    };
    Ok(KeyEvent::new(code, modifiers))
}

fn is_reserved_command_shortcut(event: KeyEvent) -> bool {
    let ctrl = KeyModifiers::CONTROL;
    let exact = |code, modifiers| event.code == code && event.modifiers == modifiers;
    exact(KeyCode::F(10), KeyModifiers::NONE)
        || exact(KeyCode::Char('/'), ctrl)
        || exact(KeyCode::Char('_'), ctrl)
        || exact(KeyCode::Char('s'), ctrl)
        || exact(KeyCode::Char('z'), ctrl)
        || exact(KeyCode::Char('y'), ctrl)
        || exact(KeyCode::Char('z'), ctrl | KeyModifiers::SHIFT)
        || exact(KeyCode::Char('g'), ctrl)
        || exact(KeyCode::Tab, ctrl)
        || exact(KeyCode::F(3), KeyModifiers::NONE)
        || exact(KeyCode::F(3), KeyModifiers::SHIFT)
}

fn same_keycode(left: KeyCode, right: KeyCode) -> bool {
    match (left, right) {
        (KeyCode::Char(a), KeyCode::Char(b)) => a.eq_ignore_ascii_case(&b),
        _ => left == right,
    }
}

fn load_command_bindings(project: &Path) -> Result<CommandBindings, Box<dyn std::error::Error>> {
    let path = project.join(".writers-ide/keybindings.toml");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let config: KeymapFile = toml::from_str(&text)?;
    validate_command_binding_config(&config)
}

fn validate_command_binding_config(
    config: &KeymapFile,
) -> Result<CommandBindings, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    for (chord, id) in &config.commands {
        let event = parse_shortcut(chord)?;
        if is_reserved_command_shortcut(event) {
            return Err(format!("{chord} conflicts with a reserved application shortcut").into());
        }
        if result.iter().any(|(existing, _): &(KeyEvent, String)| {
            existing.code == event.code && existing.modifiers == event.modifiers
        }) {
            return Err(format!("duplicate command shortcut: {chord}").into());
        }
        let command = command_by_id(id).ok_or_else(|| format!("unknown command ID: {id}"))?;
        if command.takes_arguments {
            return Err(
                format!("command {id} requires arguments and cannot be bound directly").into(),
            );
        }
        result.push((event, id.clone()));
    }
    Ok(result)
}

fn save_keymap(project: &Path, config: &KeymapFile) -> Result<(), Box<dyn std::error::Error>> {
    validate_command_binding_config(config)?;
    let directory = project.join(".writers-ide");
    fs::create_dir_all(&directory)?;
    let path = directory.join("keybindings.toml");
    let bytes = toml::to_string_pretty(config)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temporary.write_all(bytes.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn read_keymap(project: &Path) -> Result<KeymapFile, Box<dyn std::error::Error>> {
    let path = project.join(".writers-ide/keybindings.toml");
    match fs::read_to_string(path) {
        Ok(text) => Ok(toml::from_str(&text)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(KeymapFile {
            bindings: BTreeMap::new(),
            commands: BTreeMap::new(),
        }),
        Err(error) => Err(error.into()),
    }
}
impl AppState {
    fn new(app: WriterApp, function_bindings: Vec<(u8, Workspace)>) -> Self {
        let draft_notice = list_challenge_drafts(app.project_root())
            .ok()
            .filter(|notice| notice != "No recoverable challenge drafts")
            .map(|_| {
                "Recoverable isolated challenge draft(s) found; use challenge-list-drafts"
                    .to_owned()
            });
        let (tools, tools_error) = match load_editor_tools(app.project_root()) {
            Ok(tools) => (tools, String::new()),
            Err(error) => (
                EditorToolsFile::default(),
                format!("Editor tools not loaded: {error}"),
            ),
        };
        let (text_expansion, expansion_error) = match load_text_expansion(app.project_root()) {
            Ok(config) => (config, String::new()),
            Err(error) => (
                TextExpansionConfig::default(),
                format!("Text expansions not loaded: {error}"),
            ),
        };
        let (editor_preferences, preferences_error) =
            match load_editor_preferences(app.project_root()) {
                Ok(preferences) => (preferences, String::new()),
                Err(error) => (
                    EditorPreferences::default(),
                    format!("Editor preferences not loaded: {error}"),
                ),
            };
        let writing_session = app
            .list_writing_sessions()
            .ok()
            .and_then(|sessions| {
                sessions
                    .into_iter()
                    .find(|session| session.status == "active")
            })
            .map(|session| ActiveWritingSession {
                id: session.id,
                scene_id: session.scene_id,
                initial_words: session.start_words,
            });
        Self {
            app,
            workspace: Workspace::Dashboard,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: draft_notice.unwrap_or({
                if tools_error.is_empty() {
                    if expansion_error.is_empty() {
                        preferences_error
                    } else {
                        expansion_error
                    }
                } else {
                    tools_error
                }
            }),
            history: vec![Workspace::Dashboard],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: writing_session.is_some(),
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings,
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: tools.snippets,
            registers: tools.registers,
            macros: tools.macros,
            recording_macro: None,
            command_macros: tools.command_macros,
            recording_command_macro: tools.recording_command_macro,
            text_expansion,
            editor_preferences,
            bookmarks: tools.bookmarks,
            kill_ring: tools.kill_ring,
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        }
    }
    fn session_path(&self) -> std::path::PathBuf {
        self.app.project_root().join(".writers-ide/session.json")
    }
    fn persist_editor_tools(&self) -> Result<(), Box<dyn std::error::Error>> {
        let directory = self.app.project_root().join(".writers-ide");
        fs::create_dir_all(&directory)?;
        let path = directory.join("editor-tools.json");
        let bytes = serde_json::to_vec_pretty(&EditorToolsFile {
            version: 1,
            snippets: self.snippets.clone(),
            registers: self.registers.clone(),
            macros: self.macros.clone(),
            command_macros: self.command_macros.clone(),
            recording_command_macro: self.recording_command_macro.clone(),
            bookmarks: self.bookmarks.clone(),
            kill_ring: self.kill_ring.clone(),
        })?;
        let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
        use std::io::Write;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path)?;
        fs::File::open(directory)?.sync_all()?;
        Ok(())
    }
    fn record_command_macro_step(
        &mut self,
        command: CommandSpec,
        invocation: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some((_, steps)) = self.recording_command_macro.as_mut() else {
            return Ok(());
        };
        if command.id.starts_with("command-macro.") {
            return Ok(());
        }
        if invocation.len() > 4096 {
            return Err("command macro invocation exceeds the 4096-byte limit".into());
        }
        if steps.len() >= 256 {
            return Err("command macro reached its 256-step limit".into());
        }
        steps.push(RecordedCommand {
            command_id: command.id.to_owned(),
            invocation: invocation.to_owned(),
        });
        self.persist_editor_tools()?;
        Ok(())
    }
    fn replay_command_macro(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.recording_command_macro.is_some() {
            return Err("stop command recording before replaying a command macro".into());
        }
        let steps = self
            .command_macros
            .get(name)
            .ok_or("unknown saved command macro")?
            .clone();
        if steps.is_empty() {
            return Err("saved command macro contains no steps".into());
        }
        let mut resolved = Vec::with_capacity(steps.len());
        for (index, step) in steps.into_iter().enumerate() {
            let spec = command_by_id(&step.command_id).ok_or_else(|| {
                format!(
                    "command macro step {} uses unknown command ID {}",
                    index + 1,
                    step.command_id
                )
            })?;
            if spec.id.starts_with("command-macro.") {
                return Err(format!(
                    "command macro step {} recursively invokes command recording",
                    index + 1
                )
                .into());
            }
            let (invoked_spec, arguments) = command_palette_invocation(&step.invocation)
                .ok_or_else(|| {
                    format!("command macro step {} has an invalid invocation", index + 1)
                })?;
            if invoked_spec.id != spec.id {
                return Err(format!(
                    "command macro step {} no longer matches registry ID {}",
                    index + 1,
                    spec.id
                )
                .into());
            }
            if let Some(reason) = self.command_disabled_reason(spec) {
                return Err(
                    format!("command macro step {} is unavailable: {reason}", index + 1).into(),
                );
            }
            let dispatch_arguments = if spec.action == CommandAction::LegacyPalette {
                Some(step.invocation.clone())
            } else {
                arguments.map(str::to_owned)
            };
            resolved.push((spec.action, dispatch_arguments));
        }
        for (index, (action, arguments)) in resolved.into_iter().enumerate() {
            self.dispatch_registered_with_args(action, arguments.as_deref())
                .map_err(|error| {
                    format!(
                        "command macro {name} stopped at step {}: {error}",
                        index + 1
                    )
                })?;
        }
        self.message = format!(
            "Replayed command macro {name} ({} steps)",
            self.command_macros[name].len()
        );
        Ok(())
    }
    fn remember_kill(&mut self, text: String) -> Result<(), Box<dyn std::error::Error>> {
        self.last_yank = None;
        if text.is_empty() {
            return Ok(());
        }
        if self.kill_ring.first() != Some(&text) {
            self.kill_ring.insert(0, text);
            self.kill_ring.truncate(32);
        }
        self.kill_ring_index = 0;
        self.persist_editor_tools()
    }
    fn insert_kill_ring_entry(&mut self, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let text = self
            .kill_ring
            .get(index)
            .ok_or("kill-ring index is out of range")?
            .clone();
        self.kill_ring_index = index;
        self.clipboard = text.clone();
        self.push_undo();
        self.paste_text(&text);
        Ok(())
    }
    fn yank_pop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let last = self
            .last_yank
            .clone()
            .ok_or("yank-pop requires a recent manuscript paste")?;
        if last.scene_id != self.editing_scene {
            return Err("yank-pop cannot cross a scene change".into());
        }
        if self.kill_ring.len() < 2 {
            return Err("yank-pop needs at least two retained entries".into());
        }
        let next_index = (self.kill_ring_index + 1) % self.kill_ring.len();
        let replacement = self.kill_ring[next_index].clone();
        let mut text = self.editor.join("\n");
        if text.get(last.range.clone()) != Some(last.text.as_str()) {
            return Err("the last yanked passage changed; yank-pop was cancelled".into());
        }
        self.push_undo();
        text.replace_range(last.range.clone(), &replacement);
        self.editor = text.split('\n').map(str::to_owned).collect();
        self.cursor = editor_position_from_byte(&text, last.range.start + replacement.len())
            .ok_or("replacement cursor is not a valid UTF-8 position")?;
        self.dirty = true;
        self.kill_ring_index = next_index;
        self.last_yank = Some(LastYank {
            scene_id: self.editing_scene,
            range: last.range.start..last.range.start + replacement.len(),
            text: replacement,
        });
        Ok(())
    }
    fn save_session(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.session_path();
        let parent = path.parent().ok_or("session file has no parent")?;
        fs::create_dir_all(parent)?;
        let narrowed_chapter = infer_narrowed_chapter(&self.scenes, &self.app.list_scenes(false)?)
            .map(|chapter| chapter.to_string());
        let snapshot = SessionSnapshot {
            workspace: self.workspace.title().to_owned(),
            selected_scene: self.active_scene().map(|id| id.to_string()),
            editing_scene: self.editing_scene.map(|id| id.to_string()),
            cursor: self.cursor,
            scroll: self.scroll,
            selected: self.selected,
            narrowed_chapter,
            reading_document: self.reading_document.map(|id| id.to_string()),
            reading_offset: self.reading_offset,
            editor_focus_mode: self.editor_focus_mode,
            horizontal_split_ratio: self.horizontal_split_ratio,
            vertical_split_ratio: self.vertical_split_ratio,
            assembly_draft: self.assembly_draft.map(|id| id.to_string()),
            planning_board_id: self.planning_board_id.map(|id| id.to_string()),
            writing_stats_enabled: self.writing_stats_enabled,
            typed_characters: self.typed_characters,
            pasted_characters: self.pasted_characters,
            successful_saves: self.successful_saves,
            reference_scene: self
                .active_reference_pane()
                .and_then(|pane| pane.scene_id)
                .map(|id| id.to_string()),
            reference_scroll: self.active_reference_pane().map_or(0, |pane| pane.scroll),
            reference_scenes: self
                .reference_panes
                .iter()
                .filter_map(|pane| pane.scene_id.map(|id| id.to_string()))
                .collect(),
            reference_sources: self
                .reference_panes
                .iter()
                .map(|pane| pane.source_key.clone())
                .collect(),
            reference_scrolls: self
                .reference_panes
                .iter()
                .map(|pane| pane.scroll)
                .collect(),
            active_reference: self.active_reference,
            reference_focused: self.reference_focused,
        };
        let bytes = serde_json::to_vec_pretty(&snapshot)?;
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&temp, &path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
    fn restore_session(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.session_path();
        let Ok(bytes) = fs::read(&path) else {
            return Ok(());
        };
        let snapshot: SessionSnapshot = match serde_json::from_slice(&bytes) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.message = format!("Previous session could not be restored: {error}");
                return Ok(());
            }
        };
        self.refresh()?;
        let mut stale_reference = false;
        if let Some(chapter) = snapshot
            .narrowed_chapter
            .as_deref()
            .and_then(|value| value.parse::<uuid::Uuid>().ok())
            .map(story_core::NodeId)
        {
            let all = self.app.list_scenes(false)?;
            if all.iter().any(|scene| scene.chapter_id == Some(chapter)) {
                self.scenes = all
                    .into_iter()
                    .filter(|scene| scene.chapter_id == Some(chapter))
                    .map(|record| {
                        (
                            record.scene.id,
                            record.scene.title,
                            record.scene.status,
                            record.actual_words,
                        )
                    })
                    .collect();
            } else {
                stale_reference = true;
            }
        }
        self.selected = snapshot.selected.min(self.scenes.len().saturating_sub(1));
        if let Some(scene_id) = snapshot
            .selected_scene
            .as_deref()
            .and_then(|value| value.parse::<uuid::Uuid>().ok())
            .map(SceneId)
            && let Some(index) = self.scenes.iter().position(|scene| scene.0 == scene_id)
        {
            self.selected = index;
        }
        if let Some(scene_id) = snapshot
            .editing_scene
            .as_deref()
            .and_then(|value| value.parse::<uuid::Uuid>().ok())
            .map(SceneId)
        {
            if self.scenes.iter().any(|scene| scene.0 == scene_id) {
                self.open_scene(scene_id)?;
                self.cursor.0 = snapshot.cursor.0.min(self.editor.len().saturating_sub(1));
                self.cursor.1 = snapshot
                    .cursor
                    .1
                    .min(grapheme_count(&self.editor[self.cursor.0]));
            } else {
                stale_reference = true;
                self.editing_scene = None;
            }
        }
        self.scroll = snapshot.scroll;
        self.editor_focus_mode = snapshot.editor_focus_mode;
        self.planning_board_id = match snapshot.planning_board_id.as_deref() {
            Some(value) => match value.parse::<uuid::Uuid>() {
                Ok(id) => {
                    let id = story_core::PlanningBoardId(id);
                    if self
                        .app
                        .list_planning_boards()?
                        .iter()
                        .any(|board| board.id == id)
                    {
                        Some(id)
                    } else {
                        stale_reference = true;
                        None
                    }
                }
                Err(_) => {
                    stale_reference = true;
                    None
                }
            },
            None => None,
        };
        self.horizontal_split_ratio = snapshot.horizontal_split_ratio.clamp(20, 80);
        self.vertical_split_ratio = snapshot.vertical_split_ratio.clamp(20, 80);
        self.writing_stats_enabled =
            snapshot.writing_stats_enabled && self.writing_session.is_some();
        self.typed_characters = snapshot.typed_characters;
        self.pasted_characters = snapshot.pasted_characters;
        self.successful_saves = snapshot.successful_saves;
        let mut saved_references = if snapshot.reference_sources.is_empty() {
            snapshot
                .reference_scenes
                .iter()
                .map(|id| format!("scene:{id}"))
                .collect::<Vec<_>>()
        } else {
            snapshot.reference_sources.clone()
        };
        if let Some(legacy) = snapshot.reference_scene.as_ref()
            && !saved_references.contains(&format!("scene:{legacy}"))
        {
            saved_references.push(format!("scene:{legacy}"));
        }
        for (index, value) in saved_references.iter().enumerate() {
            let (source_kind, source_id) = value.split_once(':').unwrap_or(("scene", value));
            let loaded = match source_kind {
                "scene" => source_id
                    .parse::<uuid::Uuid>()
                    .map(SceneId)
                    .ok()
                    .and_then(|id| {
                        let row = self.app.get_scene(id).ok()?;
                        let (content, _) = self.app.load_scene_text(id).ok()?;
                        Some((Some(id), row.scene.title, content))
                    }),
                "research" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::ResearchId)
                    .and_then(|id| {
                        let row = self
                            .app
                            .list_research_items()
                            .ok()?
                            .into_iter()
                            .find(|row| row.id == id)?;
                        let content = self.app.load_research_text(id).ok()?;
                        Some((None, row.title, content))
                    }),
                "character" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::CharacterId)
                    .and_then(|id| {
                        let character = self.app.get_character(id).ok()?;
                        Some((
                            None,
                            character.preferred_name.clone(),
                            format_character_reference(&character),
                        ))
                    }),
                "location" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::LocationId)
                    .and_then(|id| {
                        let location = self.app.get_location(id).ok()?;
                        Some((
                            None,
                            location.name.clone(),
                            format_location_reference(&location),
                        ))
                    }),
                "plotline" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::PlotlineId)
                    .and_then(|id| {
                        let row = self
                            .app
                            .list_plotlines()
                            .ok()?
                            .into_iter()
                            .find(|row| row.id == id)?;
                        Some((
                            None,
                            row.name.clone(),
                            format!("Plotline: {}\nScenes: {}", row.name, row.scene_count),
                        ))
                    }),
                "event" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::EventId)
                    .and_then(|id| {
                        let row = self
                            .app
                            .list_events()
                            .ok()?
                            .into_iter()
                            .find(|row| row.id == id)?;
                        Some((
                            None,
                            row.title.clone(),
                            format!(
                                "Event: {}\nSource time: {}\nStart: {:?}\nEnd: {:?}",
                                row.title, row.source_time, row.start, row.end
                            ),
                        ))
                    }),
                "object" => source_id
                    .parse::<uuid::Uuid>()
                    .ok()
                    .map(story_core::StoryObjectId)
                    .and_then(|id| {
                        let row = self
                            .app
                            .list_story_objects()
                            .ok()?
                            .into_iter()
                            .find(|row| row.id == id)?;
                        Some((
                            None,
                            row.name.clone(),
                            format!("Object: {}\n{}", row.name, row.description),
                        ))
                    }),
                _ => None,
            };
            if let Some((scene_id, title, content)) = loaded {
                let fallback_scroll = if Some(source_id) == snapshot.reference_scene.as_deref() {
                    snapshot.reference_scroll
                } else {
                    0
                };
                let requested = snapshot
                    .reference_scrolls
                    .get(index)
                    .copied()
                    .unwrap_or(fallback_scroll);
                let scroll = requested.min(content.lines().count().saturating_sub(1));
                self.reference_panes.push(ReferencePane {
                    source_key: value.clone(),
                    scene_id,
                    title,
                    content,
                    scroll,
                });
            } else {
                stale_reference = true;
            }
        }
        if !self.reference_panes.is_empty() {
            self.active_reference = snapshot
                .active_reference
                .min(self.reference_panes.len() - 1);
            self.reference_focused = snapshot.reference_focused;
        }
        if let Some(id) = snapshot
            .assembly_draft
            .as_deref()
            .and_then(|v| v.parse::<uuid::Uuid>().ok())
            .map(story_core::AssemblyDraftId)
        {
            match self.app.load_assembly_draft(id) {
                Ok(draft) => {
                    self.editor = format_assembly_draft(&draft)
                        .lines()
                        .map(str::to_owned)
                        .collect();
                    if self.editor.is_empty() {
                        self.editor.push(String::new());
                    }
                    self.assembly_draft = Some(id);
                    self.editing_scene = None;
                }
                Err(_) => {
                    stale_reference = true;
                    self.assembly_draft = None;
                }
            }
        }
        if let Some(reading_id) = snapshot
            .reading_document
            .as_deref()
            .and_then(|value| value.parse::<uuid::Uuid>().ok())
            .map(story_core::ReadingId)
        {
            if let Some(document) = self
                .app
                .list_reading_documents()?
                .into_iter()
                .find(|document| document.id == reading_id)
            {
                self.reading_document = Some(reading_id);
                self.reading_text = document.extracted_text;
                let requested = usize::try_from(snapshot.reading_offset)?;
                let safe_offset = self
                    .reading_text
                    .floor_char_boundary(requested.min(self.reading_text.len()));
                self.reading_offset = safe_offset as u64;
            } else {
                stale_reference = true;
                self.reading_document = None;
                self.reading_text.clear();
            }
        }
        self.workspace = Workspace::ALL
            .iter()
            .copied()
            .find(|workspace| workspace.title().eq_ignore_ascii_case(&snapshot.workspace))
            .unwrap_or(Workspace::Dashboard);
        if stale_reference {
            self.message =
                "Previous session restored; one removed scene or reading reference was skipped"
                    .into();
        }
        Ok(())
    }
    fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scenes = self
            .app
            .list_scenes(false)?
            .into_iter()
            .map(|r| (r.scene.id, r.scene.title, r.scene.status, r.actual_words))
            .collect();
        if !self.scenes.is_empty() {
            self.selected = self.selected.min(self.scenes.len() - 1);
        }
        Ok(())
    }
    fn active_scene(&self) -> Option<SceneId> {
        self.scenes.get(self.selected).map(|x| x.0)
    }
    fn current_writing_words(
        &self,
        scene_id: Option<SceneId>,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        if let Some(scene_id) = scene_id {
            let text = if self.editing_scene == Some(scene_id) {
                self.editor.join("\n")
            } else {
                self.app.load_scene_text(scene_id)?.0
            };
            return Ok(story_core::count_words(&text));
        }
        let mut total = self.scenes.iter().map(|row| u64::from(row.3)).sum::<u64>();
        if let Some(scene_id) = self.editing_scene
            && let Some(row) = self.scenes.iter().find(|row| row.0 == scene_id)
        {
            total = total.saturating_sub(u64::from(row.3));
            total =
                total.saturating_add(u64::from(story_core::count_words(&self.editor.join("\n"))));
        }
        Ok(total.min(u64::from(u32::MAX)) as u32)
    }
    fn scenes_for_scope(
        &self,
        scope: &str,
    ) -> Result<Vec<story_storage::SceneRecord>, Box<dyn std::error::Error>> {
        let scenes = self.app.list_scenes(false)?;
        if scope == "all" {
            return Ok(scenes);
        }
        if let Some(id) = scope.strip_prefix("chapter:") {
            let chapter = story_core::NodeId(id.parse()?);
            return Ok(scenes
                .into_iter()
                .filter(|record| record.chapter_id == Some(chapter))
                .collect());
        }
        let id = if scope == "current" {
            self.editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?
        } else {
            SceneId(scope.parse()?)
        };
        Ok(scenes
            .into_iter()
            .filter(|record| record.scene.id == id)
            .collect())
    }
    fn active_reference_pane(&self) -> Option<&ReferencePane> {
        self.reference_panes.get(self.active_reference)
    }
    fn active_reference_pane_mut(&mut self) -> Option<&mut ReferencePane> {
        self.reference_panes.get_mut(self.active_reference)
    }
    fn open_reference_buffer(
        &mut self,
        source_key: String,
        scene_id: Option<SceneId>,
        title: String,
        content: String,
    ) {
        if let Some(index) = self
            .reference_panes
            .iter()
            .position(|pane| pane.source_key == source_key)
        {
            let scroll = self.reference_panes[index].scroll;
            self.reference_panes[index] = ReferencePane {
                source_key,
                scene_id,
                title,
                content,
                scroll,
            };
            self.active_reference = index;
        } else {
            self.reference_panes.push(ReferencePane {
                source_key,
                scene_id,
                title,
                content,
                scroll: 0,
            });
            self.active_reference = self.reference_panes.len() - 1;
        }
    }
    fn navigate_search_match(&mut self, forward: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.search_locations.is_empty() {
            self.message = "No active find results".into();
            return Ok(());
        }
        let index = if forward {
            (self.search_location_index + 1) % self.search_locations.len()
        } else {
            (self.search_location_index + self.search_locations.len() - 1)
                % self.search_locations.len()
        };
        let (scene, start, end) = self.search_locations[index];
        if self.editing_scene != Some(scene) && self.dirty {
            self.message =
                "Save the dirty scene before navigating to a match in another scene".into();
            return Ok(());
        }
        self.search_location_index = index;
        if self.editing_scene != Some(scene) {
            self.open_scene(scene)?;
        }
        let text = self.editor.join("\n");
        let (start_row, start_col) = editor_position_from_byte(&text, start)
            .ok_or("match no longer aligns with manuscript UTF-8")?;
        let (end_row, end_col) = editor_position_from_byte(&text, end)
            .ok_or("match no longer aligns with manuscript UTF-8")?;
        self.editor_selection = Some((start_row, start_col));
        self.move_cursor(end_row, end_col, true);
        self.switch(Workspace::Editor);
        self.message = format!(
            "Match {}/{}",
            self.search_location_index + 1,
            self.search_locations.len()
        );
        Ok(())
    }
    fn open_reading_selected(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let docs = self.app.list_reading_documents()?;
        if let Some(doc) = docs.get(self.selected) {
            self.reading_document = Some(doc.id);
            self.reading_text = doc.extracted_text.clone();
            self.reading_offset = self.app.reading_checkpoint(doc.id)?.unwrap_or(0);
            let offset = usize::try_from(self.reading_offset)?;
            self.scroll = self
                .reading_text
                .get(..offset)
                .ok_or("stored reading checkpoint no longer matches document")?
                .matches('\n')
                .count();
            self.message = format!("{} · {}", doc.title, doc.media_type);
        }
        Ok(())
    }
    fn open_selected(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.workspace == Workspace::Search && !self.search_hits.is_empty() {
            let hit = self
                .search_hits
                .get(self.selected)
                .cloned()
                .ok_or("search selection is out of range")?;
            self.search_hits.clear();
            match hit.entity_kind.as_str() {
                "scene" => {
                    let id = SceneId(hit.entity_id.parse()?);
                    self.selected = self
                        .scenes
                        .iter()
                        .position(|scene| scene.0 == id)
                        .ok_or("search scene no longer exists")?;
                    return self.open_scene(id);
                }
                "reading" => {
                    let id = story_core::ReadingId(hit.entity_id.parse()?);
                    let docs = self.app.list_reading_documents()?;
                    self.selected = docs
                        .iter()
                        .position(|doc| doc.id == id)
                        .ok_or("search reference no longer exists")?;
                    self.switch(Workspace::Reading);
                    return self.open_reading_selected();
                }
                "character" => {
                    let id = story_core::CharacterId(hit.entity_id.parse()?);
                    self.selected = self
                        .app
                        .list_characters()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("search character no longer exists")?;
                    self.switch(Workspace::StoryBible);
                    return Ok(());
                }
                "plotline" => {
                    let id = story_core::PlotlineId(hit.entity_id.parse()?);
                    let plotline_index = self
                        .app
                        .list_plotlines()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("search plotline no longer exists")?;
                    self.selected = self.app.list_characters()?.len()
                        + self.app.list_locations()?.len()
                        + plotline_index;
                    self.switch(Workspace::StoryBible);
                    return Ok(());
                }
                "location" => {
                    let id = story_core::LocationId(hit.entity_id.parse()?);
                    let location_index = self
                        .app
                        .list_locations()?
                        .iter()
                        .position(|location| location.id == id)
                        .ok_or("search location no longer exists")?;
                    self.selected = self.app.list_characters()?.len() + location_index;
                    self.switch(Workspace::StoryBible);
                    return Ok(());
                }
                "research" => {
                    let id = story_core::ResearchId(hit.entity_id.parse()?);
                    self.selected = self
                        .app
                        .list_research_items()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("search research item no longer exists")?;
                    self.switch(Workspace::Research);
                    return Ok(());
                }
                "event" => {
                    let id = story_core::EventId(hit.entity_id.parse()?);
                    let event_index = self
                        .app
                        .list_events()?
                        .iter()
                        .position(|event| event.id == id)
                        .ok_or("search event no longer exists")?;
                    self.selected = self.app.list_characters()?.len()
                        + self.app.list_locations()?.len()
                        + self.app.list_plotlines()?.len()
                        + event_index;
                    self.switch(Workspace::StoryBible);
                    return Ok(());
                }
                "knowledge_claim" => {
                    let id = story_core::KnowledgeClaimId(hit.entity_id.parse()?);
                    let knowledge_index = self
                        .app
                        .list_approved_knowledge_claims()?
                        .iter()
                        .position(|claim| claim.id == id)
                        .ok_or("approved knowledge claim no longer exists")?;
                    self.selected = self.app.list_characters()?.len()
                        + self.app.list_locations()?.len()
                        + self.app.list_plotlines()?.len()
                        + self.app.list_events()?.len()
                        + knowledge_index;
                    self.switch(Workspace::StoryBible);
                    return Ok(());
                }
                "issue" => {
                    let id = story_core::IssueId(hit.entity_id.parse()?);
                    self.selected = self
                        .app
                        .list_issues()?
                        .iter()
                        .position(|issue| issue.id == id)
                        .ok_or("search issue no longer exists")?;
                    self.switch(Workspace::Issues);
                    return Ok(());
                }
                kind => return Err(format!("unsupported search entity kind: {kind}").into()),
            }
        }
        if self.workspace == Workspace::Search
            && let Some(query_id) = self.query_view
        {
            let matches = self.app.run_scene_query(query_id)?;
            if matches.is_empty() {
                self.selected = 0;
                self.message = "Saved query has no matching scenes".into();
                return Ok(());
            }
            self.selected = self.selected.min(matches.len() - 1);
            let id = matches[self.selected];
            self.selected = self
                .scenes
                .iter()
                .position(|scene| scene.0 == id)
                .ok_or("query scene no longer exists")?;
            return self.open_scene(id);
        }
        if self.workspace == Workspace::PlanningBoards {
            if self.planning_board_id.is_none() {
                let boards = self.app.list_planning_boards()?;
                let board = boards
                    .get(self.selected)
                    .ok_or("planning board selection is out of range")?;
                self.planning_board_id = Some(board.id);
                self.selected = 0;
                self.message = format!(
                    "{} · Ctrl+arrows move cards · Esc returns to boards",
                    board.title
                );
            } else {
                let board = self.planning_board_id.unwrap();
                let items = self.app.list_planning_board_items(board)?;
                let item = items
                    .get(self.selected)
                    .ok_or("planning board card selection is out of range")?;
                if item.entity_kind == "scene" {
                    let id = SceneId(item.entity_id.parse()?);
                    self.selected = self
                        .scenes
                        .iter()
                        .position(|scene| scene.0 == id)
                        .ok_or("board scene no longer exists")?;
                    return self.open_scene(id);
                }
                self.message = format!("{} {} · {}", item.entity_kind, item.entity_id, item.note);
            }
            return Ok(());
        }
        if matches!(
            self.workspace,
            Workspace::StoryBible | Workspace::Issues | Workspace::Research
        ) {
            if let Some(row) = workspace_items(self).get(self.selected) {
                self.message = row.clone();
            }
            return Ok(());
        }
        if let Some(id) = self.active_scene() {
            self.open_scene(id)?;
        }
        Ok(())
    }
    fn open_scene(&mut self, id: SceneId) -> Result<(), Box<dyn std::error::Error>> {
        if self.dirty && self.editing_scene != Some(id) {
            self.message =
                "Unsaved text is open. Save with Ctrl-S before opening another scene.".into();
            return Ok(());
        }
        let (text, hash) = self.app.load_scene_text(id)?;
        self.app.reindex_scene_mentions(id, &text, &hash)?;
        // Preserve the final empty editor row when prose ends in a newline so
        // restored end-of-document cursors remain after the last paragraph.
        self.editor = text.split('\n').map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        self.cursor = (0, 0);
        self.saved_hash = hash;
        self.editing_scene = Some(id);
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.editor_selection = None;
        self.last_yank = None;
        self.assembly_draft = None;
        self.dirty = false;
        self.workspace = Workspace::Editor;
        let mentions = self.app.list_scene_mentions(id)?;
        let detected = mentions
            .iter()
            .filter(|mention| matches!(mention.state.as_str(), "detected" | "ambiguous"))
            .count();
        if detected > 0 {
            self.message =
                format!("{detected} possible story-bible mention(s) · use :mentions to review");
        }
        Ok(())
    }
    fn open_assembly_draft(
        &mut self,
        id: story_core::AssemblyDraftId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.dirty {
            self.message = "Save the current editor buffer before opening an assembly".into();
            return Ok(());
        }
        let draft = self.app.load_assembly_draft(id)?;
        self.editor = format_assembly_draft(&draft)
            .lines()
            .map(str::to_owned)
            .collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        self.cursor = (0, 0);
        self.scroll = 0;
        self.editing_scene = None;
        self.assembly_draft = Some(id);
        self.dirty = false;
        self.workspace = Workspace::Editor;
        self.message = format!(
            "Assembly buffer: {} scene files · scene markers must remain intact",
            draft.items.len()
        );
        Ok(())
    }
    fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.active_challenge.is_some() {
            self.persist_challenge_draft()?;
            self.dirty = false;
            self.message =
                "Challenge draft checkpointed separately; it has not changed manuscript prose"
                    .into();
            return Ok(());
        }
        if let Some(draft_id) = self.assembly_draft {
            let draft = self.app.load_assembly_draft(draft_id)?;
            match parse_assembly_draft(&draft, &self.editor.join("\n")) {
                Ok(items) => match self.app.replace_assembly_draft_items(draft_id, &items) {
                    Ok(()) => match self.app.commit_assembly_draft(draft_id) {
                        Ok(()) => {
                            self.assembly_draft = None;
                            self.dirty = false;
                            if self.writing_stats_enabled {
                                self.successful_saves = self.successful_saves.saturating_add(1);
                            }
                            self.message =
                                "Saved all assembly scene files through journaled writes".into();
                            self.refresh()?;
                            self.switch(Workspace::Outline);
                        }
                        Err(error) => {
                            self.message =
                                format!("Assembly save can be resumed; draft retained: {error}")
                        }
                    },
                    Err(error) => {
                        self.message = format!("Assembly draft was not replaced: {error}")
                    }
                },
                Err(error) => {
                    self.message =
                        format!("Assembly markers are invalid; no files changed: {error}")
                }
            }
            return Ok(());
        }
        if let Some(id) = self.editing_scene {
            let canonical_text = self.editor.join("\n");
            let text = if self.editor_preferences.line_ending == "crlf" {
                canonical_text.replace('\n', "\r\n")
            } else {
                canonical_text
            };
            match self.app.save_scene_text(id, Some(&self.saved_hash), &text) {
                Ok(hash) => {
                    self.saved_hash = hash;
                    self.dirty = false;
                    if self.writing_stats_enabled {
                        self.successful_saves = self.successful_saves.saturating_add(1);
                    }
                    self.message = "Saved".into();
                }
                Err(e) => self.message = format!("Save conflict/error: {e}"),
            }
        }
        Ok(())
    }
    fn persist_challenge_draft(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(challenge) = &self.active_challenge else {
            return Ok(());
        };
        let directory = self.app.project_root().join(".writers-ide/recovery");
        fs::create_dir_all(&directory)?;
        let path = challenge_draft_path(&directory, &challenge.id);
        let draft = ChallengeDraftFile {
            version: 1,
            challenge_id: challenge.id.clone(),
            text: self.editor.join("\n"),
            target_words: challenge.target_words,
            mode: challenge.mode.clone(),
            duration_seconds: challenge.duration_seconds,
            words_deleted: challenge.words_deleted,
            pauses_consumed: challenge.pauses_consumed,
            destination_scene: challenge.destination_scene,
            started_at: challenge.started_at,
            updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        };
        let bytes = serde_json::to_vec_pretty(&draft)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
        use std::io::Write;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        fs::File::open(directory)?.sync_all()?;
        Ok(())
    }
    fn switch(&mut self, w: Workspace) {
        if self.workspace != w {
            if self.history_index + 1 < self.history.len() {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(self.workspace);
            self.history_index = self.history.len() - 1;
            if w == Workspace::Revisions {
                self.revision_index = 0;
                self.revision_diff = None;
                self.revision_scene = None;
                self.revision_hunk = 0;
            }
            if w == Workspace::Research {
                self.proposal_review_active = false;
                self.proposal_review_rows.clear();
                self.proposal_review_index = 0;
            }
            self.workspace = w;
        }
    }
    fn selected_scene(&self) -> String {
        self.scenes
            .get(self.selected)
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "No scene selected".into())
    }
    fn toggle_menu(&mut self) {
        if self.menu.take().is_some() {
            self.message = "Menu closed; editor focus restored".into();
        } else {
            self.menu = Some(MenuState {
                top: 0,
                selected: 0,
            });
            self.message = "Main menu · arrows navigate · Enter selects · Esc closes".into();
        }
    }
    fn menu_commands(&self) -> Vec<CommandSpec> {
        let Some(menu) = self.menu else {
            return Vec::new();
        };
        let top = MENU_CATEGORIES[menu.top.min(MENU_CATEGORIES.len() - 1)];
        command_registry()
            .into_iter()
            .filter(|spec| spec.id != "menu.main" && menu_category(spec.category) == top)
            .collect()
    }
    fn command_disabled_reason(&self, spec: CommandSpec) -> Option<&'static str> {
        match spec.action {
            CommandAction::Save
                if self.editing_scene.is_none() && self.assembly_draft.is_none() =>
            {
                Some("Open a scene or assembly draft first")
            }
            CommandAction::RestoreRevisionHunk if self.revision_diff.is_none() => {
                Some("Open a revision diff first")
            }
            CommandAction::RestoreRevisionHunk if self.dirty => {
                Some("Save the manuscript buffer first")
            }
            _ => None,
        }
    }
    fn handle_menu_key(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
        let mut menu = self.menu.ok_or("menu is closed")?;
        match key.code {
            KeyCode::Esc => {
                self.menu = None;
                self.message = "Menu closed; editor focus restored".into();
                return Ok(());
            }
            KeyCode::Left => {
                menu.top = (menu.top + MENU_CATEGORIES.len() - 1) % MENU_CATEGORIES.len();
                menu.selected = 0;
                self.menu = Some(menu);
                return Ok(());
            }
            KeyCode::Right => {
                menu.top = (menu.top + 1) % MENU_CATEGORIES.len();
                menu.selected = 0;
                self.menu = Some(menu);
                return Ok(());
            }
            KeyCode::Up => {
                let count = self.menu_commands().len();
                if count > 0 {
                    menu.selected = (menu.selected + count - 1) % count;
                }
            }
            KeyCode::Down => {
                let count = self.menu_commands().len();
                if count > 0 {
                    menu.selected = (menu.selected + 1) % count;
                }
            }
            KeyCode::Enter => {
                let commands = self.menu_commands();
                if let Some(spec) = commands.get(menu.selected).copied() {
                    if let Some(reason) = self.command_disabled_reason(spec) {
                        self.message = format!("{} unavailable: {reason}", spec.label);
                        return Ok(());
                    }
                    if spec.takes_arguments {
                        let prefix = spec
                            .palette_prefix
                            .ok_or("menu command has no command-palette route")?;
                        self.menu = None;
                        self.mode = Some(':');
                        self.prompt = format!("{prefix} ");
                        self.message = spec.description.into();
                        return Ok(());
                    }
                    self.menu = None;
                    self.dispatch_registered(spec.action)?;
                    return Ok(());
                }
                return Ok(());
            }
            KeyCode::Char(ch) => {
                if let Some(index) = MENU_CATEGORIES.iter().position(|category| {
                    category
                        .chars()
                        .next()
                        .is_some_and(|first| first.eq_ignore_ascii_case(&ch))
                }) {
                    menu.top = index;
                    menu.selected = 0;
                    self.menu = Some(menu);
                    return Ok(());
                } else if let Some((index, spec)) =
                    self.menu_commands().iter().enumerate().find(|(_, spec)| {
                        spec.mnemonic
                            .is_some_and(|mnemonic| mnemonic.eq_ignore_ascii_case(&ch))
                    })
                {
                    menu.selected = index;
                    if let Some(reason) = self.command_disabled_reason(*spec) {
                        self.message = format!("{} unavailable: {reason}", spec.label);
                        self.menu = Some(menu);
                        return Ok(());
                    }
                    if spec.takes_arguments {
                        let prefix = spec
                            .palette_prefix
                            .ok_or("menu command has no command-palette route")?;
                        self.menu = None;
                        self.mode = Some(':');
                        self.prompt = format!("{prefix} ");
                        self.message = spec.description.into();
                        return Ok(());
                    }
                    self.menu = None;
                    self.dispatch_registered(spec.action)?;
                    return Ok(());
                }
            }
            _ => {}
        }
        menu.selected = menu
            .selected
            .min(self.menu_commands().len().saturating_sub(1));
        self.menu = Some(menu);
        Ok(())
    }
    fn dispatch_registered(
        &mut self,
        action: CommandAction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.dispatch_registered_with_args(action, None)
    }
    fn dispatch_registered_with_args(
        &mut self,
        action: CommandAction,
        arguments: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match action {
            CommandAction::Workspace(workspace) => self.switch(workspace),
            CommandAction::OpenMainMenu => self.toggle_menu(),
            CommandAction::Save => self.save()?,
            CommandAction::Help => {
                self.message = command_registry()
                    .into_iter()
                    .map(|command| {
                        let mnemonic = command
                            .mnemonic
                            .map(|key| format!(" [{key}]"))
                            .unwrap_or_default();
                        format!(
                            "{} > {}{} — {}",
                            command.category, command.label, mnemonic, command.description
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
            }
            CommandAction::RestoreRevisionHunk => self.restore_selected_revision_hunk()?,
            CommandAction::Process {
                expand_transclusions,
            } => {
                let reports = if expand_transclusions {
                    self.app.run_processing_with_transclusions()?
                } else {
                    self.app.run_processing()?
                };
                self.message = reports
                    .into_iter()
                    .map(|report| format!("{:?}:{:?}", report.stage, report.status))
                    .collect::<Vec<_>>()
                    .join(" · ");
            }
            CommandAction::ProcessSelected {
                expand_transclusions,
            } => {
                let stages = parse_processing_stages(
                    arguments.ok_or("selected Compile command needs stage names")?,
                )?;
                let reports = self
                    .app
                    .run_processing_stages(&stages, expand_transclusions)?;
                self.message = reports
                    .into_iter()
                    .map(|report| format!("{:?}:{:?}", report.stage, report.status))
                    .collect::<Vec<_>>()
                    .join(" · ");
            }
            CommandAction::LegacyPalette => {
                self.execute_legacy_palette_command(
                    arguments.ok_or("legacy palette action requires its command text")?,
                )?;
            }
        }
        Ok(())
    }
    fn command(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = self.prompt.trim().to_owned();
        self.prompt.clear();
        let mode = self.mode.take();
        if mode == Some('n') {
            self.create_reading_selection("note", &cmd)?;
            return Ok(());
        }
        if mode == Some('/') {
            if self.workspace == Workspace::Reading {
                let matches = self.app.search_reading(&cmd)?;
                self.message = matches
                    .into_iter()
                    .map(|(_, title, snippet)| format!("{title}: {snippet}"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                return Ok(());
            }
            self.scenes = self
                .app
                .search_scenes(&cmd)?
                .into_iter()
                .filter_map(|id| self.app.get_scene(id).ok())
                .map(|r| (r.scene.id, r.scene.title, r.scene.status, r.actual_words))
                .collect();
            self.query_view = None;
            self.search_hits.clear();
            self.selected = 0;
            self.workspace = Workspace::Search;
            self.message = format!("{} search results", self.scenes.len());
            return Ok(());
        }
        if let Some((spec, arguments)) = command_palette_invocation(&cmd) {
            let dispatch_arguments = if spec.action == CommandAction::LegacyPalette {
                Some(cmd.as_str())
            } else {
                arguments
            };
            self.dispatch_registered_with_args(spec.action, dispatch_arguments)?;
            self.record_command_macro_step(spec, &cmd)?;
        } else {
            self.execute_legacy_palette_command(&cmd)?;
        }
        Ok(())
    }
    fn execute_legacy_palette_command(
        &mut self,
        cmd: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if cmd == "keybinding-list" {
            let config = read_keymap(self.app.project_root())?;
            self.message = if config.commands.is_empty() {
                "No project command shortcuts are configured".into()
            } else {
                config
                    .commands
                    .iter()
                    .map(|(chord, id)| format!("{chord} → {id}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            return Ok(());
        }
        if let Some(rest) = cmd.strip_prefix("keybinding-set ") {
            let (chord, command_id) = rest
                .split_once(' ')
                .ok_or("use keybinding-set CTRL+KEY COMMAND-ID")?;
            if chord.trim() != chord || command_id.trim() != command_id || command_id.contains(' ')
            {
                return Err("use keybinding-set CTRL+KEY COMMAND-ID".into());
            }
            let event = parse_shortcut(chord)?;
            if is_reserved_command_shortcut(event) {
                return Err(
                    format!("{chord} conflicts with a reserved application shortcut").into(),
                );
            }
            let command = command_by_id(command_id)
                .ok_or_else(|| format!("unknown command ID: {command_id}"))?;
            if command.takes_arguments {
                return Err(format!(
                    "command {command_id} requires arguments and cannot be bound directly"
                )
                .into());
            }
            let mut config = read_keymap(self.app.project_root())?;
            let duplicates = config
                .commands
                .keys()
                .filter_map(|existing| {
                    parse_shortcut(existing)
                        .ok()
                        .filter(|candidate| {
                            candidate.code == event.code && candidate.modifiers == event.modifiers
                        })
                        .map(|_| existing.clone())
                })
                .collect::<Vec<_>>();
            for duplicate in duplicates {
                config.commands.remove(&duplicate);
            }
            config
                .commands
                .insert(chord.to_owned(), command_id.to_owned());
            save_keymap(self.app.project_root(), &config)?;
            self.command_bindings = load_command_bindings(self.app.project_root())?;
            self.message = format!("Bound {chord} to {}", command.label);
            return Ok(());
        }
        if let Some(chord) = cmd.strip_prefix("keybinding-remove ") {
            let event = parse_shortcut(chord.trim())?;
            let mut config = read_keymap(self.app.project_root())?;
            let removals = config
                .commands
                .keys()
                .filter_map(|existing| {
                    parse_shortcut(existing)
                        .ok()
                        .filter(|candidate| {
                            candidate.code == event.code && candidate.modifiers == event.modifiers
                        })
                        .map(|_| existing.clone())
                })
                .collect::<Vec<_>>();
            if removals.is_empty() {
                self.message = format!("No command shortcut is bound to {chord}");
                return Ok(());
            }
            for removal in removals {
                config.commands.remove(&removal);
            }
            save_keymap(self.app.project_root(), &config)?;
            self.command_bindings = load_command_bindings(self.app.project_root())?;
            self.message = format!("Removed command shortcut {chord}");
            return Ok(());
        }
        if let Some(name) = cmd.strip_prefix("command-macro-record ") {
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Err("use command-macro-record NAME with a single-word name".into());
            }
            if self.recording_command_macro.is_some() {
                return Err("stop the current command macro recording first".into());
            }
            self.recording_command_macro = Some((name.to_owned(), Vec::new()));
            self.persist_editor_tools()?;
            self.message =
                format!("Recording command macro {name}; run commands, then command-macro-stop");
            return Ok(());
        }
        if cmd == "command-macro-stop" {
            let (name, steps) = self
                .recording_command_macro
                .take()
                .ok_or("no command macro is being recorded")?;
            if steps.is_empty() {
                self.persist_editor_tools()?;
                self.message = format!("Discarded empty command macro {name}");
            } else {
                self.command_macros.insert(name.clone(), steps.clone());
                self.persist_editor_tools()?;
                self.message = format!("Saved command macro {name} ({} steps)", steps.len());
            }
            return Ok(());
        }
        if cmd == "command-macro-list" {
            self.message = if self.command_macros.is_empty() {
                "No saved command macros".into()
            } else {
                self.command_macros
                    .iter()
                    .map(|(name, steps)| format!("{name} ({} steps)", steps.len()))
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            return Ok(());
        }
        if let Some(name) = cmd.strip_prefix("command-macro-run ") {
            return self.replay_command_macro(name.trim());
        }
        if let Some(start) = parse_challenge_command(cmd)? {
            let target = start.target_words;
            let seconds = start.duration_seconds;
            let mode = start.mode;
            let destination_scene = start.destination_scene;
            if self.active_challenge.is_some() {
                return Err("finish the current challenge before starting another".into());
            }
            if self.dirty {
                return Err(
                    "save existing manuscript edits before starting an isolated challenge".into(),
                );
            }
            let challenge_config = load_challenge_config(self.app.project_root())?;
            if let Some(scene) = destination_scene {
                self.app.load_scene_text(scene)?;
                if self.editing_scene != Some(scene) {
                    self.app
                        .create_snapshot(scene, &format!("Before {} word challenge", target))?;
                }
            }
            if let Some(scene) = self.editing_scene {
                self.app
                    .create_snapshot(scene, &format!("Before {} word challenge", target))?;
            }
            let id = self.app.start_writing_challenge(target, seconds, &mode)?;
            let previous_editor = std::mem::replace(&mut self.editor, vec![String::new()]);
            let previous_cursor = self.cursor;
            let previous_dirty = self.dirty;
            let previous_scene = self.editing_scene;
            let previous_hash = self.saved_hash.clone();
            let previous_selection = self.editor_selection;
            let previous_workspace = self.workspace;
            let initial_words = 0;
            let now = Instant::now();
            self.active_challenge = Some(ActiveWritingChallenge {
                id,
                initial_words,
                target_words: target,
                duration_seconds: seconds,
                deadline: now + Duration::from_secs(u64::from(seconds)),
                mode: mode.clone(),
                last_activity: now,
                kamikaze_idle_seconds: challenge_config.kamikaze_idle_seconds,
                previous_editor,
                previous_cursor,
                previous_dirty,
                previous_scene,
                previous_hash,
                previous_selection,
                previous_workspace,
                grace_pauses_remaining: challenge_config.grace_pauses,
                penalties_applied: 0,
                deletion_words_per_penalty: challenge_config.deletion_words_per_penalty,
                words_deleted: 0,
                pauses_consumed: 0,
                failure_taunt: challenge_config.failure_taunt.clone(),
                destination_scene,
                started_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            });
            self.cursor = (0, 0);
            self.editor_selection = None;
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.dirty = false;
            self.workspace = Workspace::Editor;
            self.message = if mode == "kamikaze" {
                format!(
                    "Isolated Kamikaze draft armed: {target} words; first {} idle intervals are grace pauses.",
                    challenge_config.kamikaze_idle_seconds
                )
            } else {
                format!(
                    "Isolated {} draft: {target} words in {seconds} seconds",
                    mode
                )
            };
            self.persist_challenge_draft()?;
        } else if cmd == "challenge-finish" {
            self.finish_active_challenge("quit")?;
        } else if cmd == "challenge-list-drafts" {
            self.message = list_challenge_drafts(self.app.project_root())?;
        } else if let Some(rest) = cmd.strip_prefix("challenge-save-new ") {
            let (draft_id, rest) = rest
                .split_once(' ')
                .ok_or("use challenge-save-new DRAFT-UUID CHAPTER-UUID TITLE")?;
            let (chapter_id, title) = rest
                .split_once(' ')
                .ok_or("provide a chapter UUID and scene title")?;
            let draft_id = uuid::Uuid::parse_str(draft_id)?.to_string();
            let draft = read_challenge_draft(self.app.project_root(), &draft_id)?;
            let chapter = story_core::NodeId(uuid::Uuid::parse_str(chapter_id)?);
            let scene = self.app.create_scene(chapter, title.trim(), 0)?;
            let (_, hash) = self.app.load_scene_text(scene)?;
            self.app.save_scene_text(scene, Some(&hash), &draft.text)?;
            remove_challenge_draft(self.app.project_root(), &draft_id)?;
            self.refresh()?;
            self.message = format!(
                "Recovered challenge draft saved as a new scene ({scene}); existing prose was untouched"
            );
        } else if let Some(draft_id) = cmd.strip_prefix("challenge-save-target ") {
            let draft_id = uuid::Uuid::parse_str(draft_id.trim())?.to_string();
            let draft = read_challenge_draft(self.app.project_root(), &draft_id)?;
            let scene = draft
                .destination_scene
                .ok_or("this challenge has no destination scene; use challenge-save-new")?;
            let (existing, expected_hash) = self.app.load_scene_text(scene)?;
            let combined = if existing.is_empty() {
                draft.text.clone()
            } else if draft.text.is_empty() {
                existing
            } else {
                format!("{existing}\n\n{}", draft.text)
            };
            self.app
                .save_scene_text(scene, Some(&expected_hash), &combined)?;
            remove_challenge_draft(self.app.project_root(), &draft_id)?;
            self.refresh()?;
            self.message = format!(
                "Recovered challenge draft appended to scene {scene} using optimistic concurrency"
            );
        } else if let Some(draft_id) = cmd.strip_prefix("challenge-discard ") {
            let draft_id = uuid::Uuid::parse_str(draft_id.trim())?.to_string();
            remove_challenge_draft(self.app.project_root(), &draft_id)?;
            self.message = format!("Challenge draft {draft_id} discarded");
        } else if let Some(value) = cmd.strip_prefix("challenge-idle ") {
            let seconds = value.trim().parse::<u64>()?;
            if !(1..=3600).contains(&seconds) {
                return Err("Kamikaze idle threshold must be between 1 and 3600 seconds".into());
            }
            let mut config = load_challenge_config(self.app.project_root())?;
            config.kamikaze_idle_seconds = seconds;
            save_challenge_config(self.app.project_root(), &config)?;
            self.message = format!("Saved Kamikaze idle threshold: {seconds} seconds");
        } else if let Some(value) = cmd.strip_prefix("challenge-pauses ") {
            let pauses = value.trim().parse::<u8>()?;
            if pauses > 20 {
                return Err("grace pause count must be between 0 and 20".into());
            }
            let mut config = load_challenge_config(self.app.project_root())?;
            config.grace_pauses = pauses;
            save_challenge_config(self.app.project_root(), &config)?;
            self.message = format!("Saved challenge grace pauses: {pauses}");
        } else if let Some(value) = cmd.strip_prefix("challenge-delete-rate ") {
            let rate = value.trim().parse::<u32>()?;
            if !(1..=100).contains(&rate) {
                return Err("deletion rate must be between 1 and 100 words per penalty".into());
            }
            let mut config = load_challenge_config(self.app.project_root())?;
            config.deletion_words_per_penalty = rate;
            save_challenge_config(self.app.project_root(), &config)?;
            self.message =
                format!("Saved progressive deletion rate: {rate} words per penalty step");
        } else if let Some(value) = cmd.strip_prefix("challenge-taunt ") {
            let taunt = value.trim();
            if taunt.is_empty()
                || taunt.len() > 200
                || taunt.chars().any(char::is_control)
                || !taunt.contains("{words}")
            {
                return Err("taunt must be 1–200 printable characters and include {words}".into());
            }
            let mut config = load_challenge_config(self.app.project_root())?;
            config.failure_taunt = taunt.to_owned();
            save_challenge_config(self.app.project_root(), &config)?;
            self.message = "Saved the challenge failure taunt".into();
        } else if let Some(rest) = cmd.strip_prefix("snippet ") {
            let (name, body) = rest.split_once(' ').ok_or("use snippet NAME TEXT")?;
            self.snippets.insert(name.to_owned(), body.to_owned());
            self.persist_editor_tools()?;
            self.message = format!("Snippet {name} saved in this project");
        } else if let Some(name) = cmd.strip_prefix("snippet-insert ") {
            let mut parts = name.split_whitespace();
            let snippet_name = parts
                .next()
                .ok_or("use snippet-insert NAME [key=value ...]")?;
            let values = parts
                .map(|item| {
                    let (key, value) = item
                        .split_once('=')
                        .ok_or("snippet values must use key=value")?;
                    Ok((key.to_owned(), value.to_owned()))
                })
                .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;
            let source = self
                .snippets
                .get(snippet_name)
                .ok_or("unknown session snippet")?
                .clone();
            let expansion = story_core::expand_snippet(&source, &values)?;
            self.insert_text_at_cursor(&expansion.text)?;
            let text = self.editor.join("\n");
            if let Some((row, column)) = editor_position_from_byte(
                &text,
                editor_position_byte(&self.editor, self.cursor.0, self.cursor.1)
                    .saturating_sub(expansion.text.len())
                    .saturating_add(expansion.cursor_byte),
            ) {
                self.cursor = (row, column);
            }
            self.message = format!("Inserted snippet {snippet_name}");
        } else if let Some(name) = cmd.strip_prefix("register-save ") {
            let value = if let Some(start) = self.editor_selection {
                let a = editor_position_byte(&self.editor, start.0, start.1);
                let b = editor_position_byte(&self.editor, self.cursor.0, self.cursor.1);
                self.editor
                    .join("\n")
                    .get(a.min(b)..a.max(b))
                    .unwrap_or("")
                    .to_owned()
            } else {
                self.editor[self.cursor.0].clone()
            };
            self.registers.insert(name.trim().to_owned(), value);
            self.persist_editor_tools()?;
            self.message = format!("Saved register {}", name.trim());
        } else if let Some(name) = cmd.strip_prefix("register-insert ") {
            let value = self
                .registers
                .get(name.trim())
                .ok_or("unknown session register")?
                .clone();
            self.insert_text_at_cursor(&value)?;
            self.message = format!("Inserted register {}", name.trim());
        } else if let Some(names) = cmd.strip_prefix("register-rotate ") {
            let names = names.split_whitespace().collect::<Vec<_>>();
            if names.len() < 2 {
                return Err("use register-rotate NAME NAME [NAME...]".into());
            }
            let unique = names.iter().copied().collect::<BTreeSet<_>>();
            if unique.len() != names.len() {
                return Err("register names must be unique".into());
            }
            let mut values = names
                .iter()
                .map(|name| self.registers.get(*name).cloned().ok_or("unknown register"))
                .collect::<Result<Vec<_>, _>>()?;
            values.rotate_right(1);
            for (name, value) in names.iter().zip(values) {
                self.registers.insert((*name).to_owned(), value);
            }
            self.persist_editor_tools()?;
            self.message = format!("Rotated {} registers (right)", names.len());
        } else if let Some(rest) = cmd.strip_prefix("cursor-add ") {
            let (row, column) = rest
                .split_once(' ')
                .ok_or("use cursor-add ROW COLUMN (zero based grapheme positions)")?;
            let row = row.parse::<usize>()?;
            let column = column.parse::<usize>()?;
            if row >= self.editor.len() || column > grapheme_count(&self.editor[row]) {
                return Err("multi-cursor position is outside the editor buffer".into());
            }
            let cursor = (row, column);
            if cursor != self.cursor && !self.extra_cursors.contains(&cursor) {
                self.extra_cursors.push(cursor);
            }
            self.message = format!("{} additional cursor(s) active", self.extra_cursors.len());
        } else if cmd == "cursor-clear" {
            self.extra_cursors.clear();
            self.message = "Additional cursors cleared".into();
        } else if let Some(rest) = cmd.strip_prefix("multi-insert ") {
            let (positions, text) = rest
                .split_once(' ')
                .ok_or("use multi-insert BYTE,BYTE,... TEXT")?;
            let anchors = positions
                .split(',')
                .map(str::parse::<usize>)
                .collect::<Result<Vec<_>, _>>()?;
            self.push_undo();
            let original = self.editor.join("\n");
            let updated = story_core::insert_at_anchors(&original, &anchors, text)?;
            self.editor = updated.lines().map(str::to_owned).collect();
            if self.editor.is_empty() {
                self.editor.push(String::new());
            }
            self.dirty = true;
            self.message = format!(
                "Inserted at {} distinct UTF-8 cursor anchors",
                anchors.iter().collect::<BTreeSet<_>>().len()
            );
        } else if let Some(rest) = cmd.strip_prefix("macro-define ") {
            let (name, recipe) = rest
                .split_once(' ')
                .ok_or("use macro-define NAME OP|OP...")?;
            let operations = parse_macro_recipe(recipe)?;
            self.macros.insert(name.to_owned(), operations);
            self.persist_editor_tools()?;
            self.message = format!("Saved macro {name}");
        } else if let Some(name) = cmd.strip_prefix("macro-record ") {
            let name = name.trim();
            if name.is_empty() {
                return Err("macro name cannot be empty".into());
            }
            if self.recording_macro.is_some() {
                return Err("stop the current macro recording first".into());
            }
            self.recording_macro = Some((name.to_owned(), Vec::new()));
            self.message = format!("Recording macro {name}; edit text, then run macro-stop");
        } else if cmd == "macro-stop" {
            let (name, operations) = self
                .recording_macro
                .take()
                .ok_or("no macro is being recorded")?;
            if operations.is_empty() {
                self.message = format!("Discarded empty macro {name}");
            } else {
                self.macros.insert(name.clone(), operations);
                self.persist_editor_tools()?;
                self.message = format!("Saved recorded macro {name}");
            }
        } else if let Some(arguments) = cmd.strip_prefix("macro-preview ") {
            let (name, repetitions) = parse_macro_run_arguments(arguments)?;
            let operations = self.macros.get(name).ok_or("unknown saved macro")?;
            let repeated = repeated_macro_operations(operations, repetitions)?;
            let source = self.editor.join("\n");
            let output = self.preview_edit_macro(&repeated)?;
            self.macro_preview = Some(MacroPreview {
                name: name.to_owned(),
                repetitions,
                source: source.clone(),
            });
            self.message = format!(
                "Macro preview · {name} × {repetitions} · {} → {} bytes · run macro-run {name} {repetitions} to apply",
                source.len(),
                output.len()
            );
        } else if let Some(arguments) = cmd.strip_prefix("macro-run ") {
            let (name, repetitions) = parse_macro_run_arguments(arguments)?;
            let operations = self.macros.get(name).ok_or("unknown saved macro")?.clone();
            let repeated = repeated_macro_operations(&operations, repetitions)?;
            let source = self.editor.join("\n");
            let needs_preview = repetitions > 1 && repeated.iter().any(is_destructive_macro_op);
            let preview_is_current = self.macro_preview.as_ref().is_some_and(|preview| {
                preview.name == name
                    && preview.repetitions == repetitions
                    && preview.source == source
            });
            if needs_preview && !preview_is_current {
                let output = self.preview_edit_macro(&repeated)?;
                self.macro_preview = Some(MacroPreview {
                    name: name.to_owned(),
                    repetitions,
                    source: source.clone(),
                });
                self.message = format!(
                    "Destructive macro preview · {} → {} bytes · repeat macro-run {name} {repetitions} to confirm",
                    source.len(),
                    output.len()
                );
            } else {
                self.macro_preview = None;
                let snapshot = if needs_preview {
                    if self.dirty {
                        self.save()?;
                        if self.dirty {
                            return Err("repeated destructive macro cancelled because the manuscript could not be saved safely".into());
                        }
                    }
                    self.app
                        .git_snapshot(&format!("Before repeated macro {name}"))?
                } else {
                    None
                };
                self.apply_edit_macro(&repeated)?;
                self.message = format!(
                    "Applied saved macro {name} × {repetitions} (undo available){}",
                    snapshot
                        .map(|_| " · pre-change project snapshot created")
                        .unwrap_or("")
                );
            }
        } else if let Some(value) = cmd.strip_prefix("text-expansion-set ") {
            let (abbreviation, replacement) = value
                .split_once("=>")
                .ok_or("use text-expansion-set ABBREVIATION => REPLACEMENT")?;
            let abbreviation = abbreviation.trim();
            if abbreviation.is_empty() || abbreviation.chars().any(char::is_whitespace) {
                return Err("abbreviation must be non-empty and contain no whitespace".into());
            }
            self.text_expansion
                .mappings
                .insert(abbreviation.to_owned(), replacement.trim().to_owned());
            save_text_expansion(self.app.project_root(), &self.text_expansion)?;
            self.message = format!("Saved text expansion {abbreviation}");
        } else if let Some(abbreviation) = cmd.strip_prefix("text-expansion-remove ") {
            if self
                .text_expansion
                .mappings
                .remove(abbreviation.trim())
                .is_none()
            {
                return Err("unknown text expansion".into());
            }
            save_text_expansion(self.app.project_root(), &self.text_expansion)?;
            self.message = format!("Removed text expansion {}", abbreviation.trim());
        } else if let Some(value) = cmd.strip_prefix("text-expansion ") {
            match value.trim() {
                "on" | "off" => {
                    self.text_expansion.enabled = value.trim() == "on";
                    save_text_expansion(self.app.project_root(), &self.text_expansion)?;
                    self.message = format!(
                        "Text expansion {}",
                        if self.text_expansion.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                "list" => {
                    self.message = self
                        .text_expansion
                        .mappings
                        .iter()
                        .map(|(abbreviation, replacement)| {
                            format!("{abbreviation} → {replacement}")
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                }
                _ => return Err("use text-expansion on|off|list".into()),
            }
        } else if cmd == "text-expand" {
            if self.expand_text_before_cursor("", "", true) {
                self.message = "Expanded abbreviation (undo available)".into();
            } else {
                self.message = "No matching abbreviation before the cursor".into();
            }
        } else if let Some(value) = cmd.strip_prefix("editor-preferences ") {
            let mut parts = value.split_whitespace();
            match parts
                .next()
                .ok_or("use editor-preferences show|line-ending|indentation|indent-width")?
            {
                "show" if parts.next().is_none() => {
                    self.message = format!(
                        "line-ending={} · indentation={} · indent-width={}",
                        self.editor_preferences.line_ending,
                        self.editor_preferences.indentation,
                        self.editor_preferences.indent_width
                    );
                }
                "line-ending" => {
                    self.editor_preferences.line_ending =
                        parts.next().ok_or("choose lf or crlf")?.to_owned();
                }
                "indentation" => {
                    self.editor_preferences.indentation =
                        parts.next().ok_or("choose tabs or spaces")?.to_owned();
                }
                "indent-width" => {
                    self.editor_preferences.indent_width =
                        parts.next().ok_or("choose a width from 1 to 8")?.parse()?;
                }
                _ => {
                    return Err(
                        "use editor-preferences show|line-ending|indentation|indent-width".into(),
                    );
                }
            }
            if parts.next().is_some() {
                return Err("editor-preferences accepts one value per setting".into());
            }
            save_editor_preferences(self.app.project_root(), &self.editor_preferences)?;
            if !value.starts_with("show") {
                self.message = "Saved project editor preferences; existing text changes on the next scene save".into();
            }
        } else if let Some(name) = cmd.strip_prefix("manuscript-bookmark-save ") {
            let name = name.trim();
            if name.is_empty() {
                return Err("bookmark name cannot be empty".into());
            }
            let scene_id = self.editing_scene.ok_or("open a manuscript scene first")?;
            let text = self.editor.join("\n");
            let (offset, quote) = if let Some((start, end)) = self.selection_bounds() {
                let start = editor_position_byte(&self.editor, start.0, start.1);
                let end = editor_position_byte(&self.editor, end.0, end.1);
                let quote = text.get(start..end).ok_or("selection is not valid UTF-8")?;
                (start, quote.to_owned())
            } else {
                let offset = editor_position_byte(&self.editor, self.cursor.0, 0);
                (offset, self.editor[self.cursor.0].clone())
            };
            if quote.is_empty() {
                return Err("bookmark a non-empty selection or current line".into());
            }
            self.bookmarks.insert(
                name.to_owned(),
                EditorBookmark {
                    scene_id,
                    offset,
                    quote,
                },
            );
            self.persist_editor_tools()?;
            self.message = format!("Saved manuscript bookmark {name}");
        } else if cmd == "manuscript-bookmark-list" {
            self.message = self
                .bookmarks
                .iter()
                .map(|(name, bookmark)| {
                    format!(
                        "{name} · {} · {}",
                        bookmark.scene_id,
                        bookmark.quote.replace('\n', " ↵ ")
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(name) = cmd.strip_prefix("manuscript-bookmark-jump ") {
            if self.dirty {
                return Err("save the current manuscript before jumping to a bookmark".into());
            }
            let bookmark = self
                .bookmarks
                .get(name.trim())
                .ok_or("unknown manuscript bookmark")?
                .clone();
            let (text, _) = self.app.load_scene_text(bookmark.scene_id)?;
            let offset = text
                .get(bookmark.offset..bookmark.offset.saturating_add(bookmark.quote.len()))
                .filter(|candidate| *candidate == bookmark.quote)
                .map(|_| bookmark.offset)
                .or_else(|| {
                    let mut matches = text.match_indices(&bookmark.quote);
                    let first = matches.next().map(|(offset, _)| offset)?;
                    matches.next().is_none().then_some(first)
                })
                .ok_or("bookmark passage is missing or ambiguous; source was left unchanged")?;
            self.open_scene(bookmark.scene_id)?;
            self.cursor = editor_position_from_byte(&text, offset)
                .ok_or("bookmark offset is not a valid UTF-8 position")?;
            self.message = format!("Jumped to manuscript bookmark {}", name.trim());
        } else if cmd == "kill-ring-list" {
            self.message = self
                .kill_ring
                .iter()
                .enumerate()
                .map(|(index, text)| format!("{}: {}", index + 1, text.replace('\n', " ↵ ")))
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(value) = cmd.strip_prefix("kill-ring-insert ") {
            let index = value
                .trim()
                .parse::<usize>()?
                .checked_sub(1)
                .ok_or("kill-ring entries are numbered from 1")?;
            self.insert_kill_ring_entry(index)?;
            self.message = format!("Inserted kill-ring entry {}", index + 1);
        } else if cmd == "yank-pop" {
            self.yank_pop()?;
            self.message =
                "Replaced the last yank with the next kill-ring entry (undo available)".into();
        } else if let Some(rest) = cmd.strip_prefix("macro ") {
            let operations = parse_macro_recipe(rest)?;
            self.apply_edit_macro(&operations)?;
            self.message = "Edit macro applied (undo available)".into();
        } else if cmd == "story-template-list" {
            self.message = self
                .app
                .list_story_bible_templates()?
                .into_iter()
                .map(|template| {
                    format!(
                        "{} · {} · {} fields · {}",
                        template.id,
                        template.entity_kind.as_str(),
                        template.fields.len(),
                        template.name
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(file) = cmd.strip_prefix("story-template-import ") {
            let template: story_core::StoryBibleTemplate =
                toml::from_str(&fs::read_to_string(file.trim())?)?;
            self.app.save_story_bible_template(&template)?;
            self.message = format!("Imported Story Bible template {}", template.id);
        } else if let Some(rest) = cmd.strip_prefix("story-profile-show ") {
            let mut parts = rest.split_whitespace();
            let kind = parse_story_bible_kind(parts.next().ok_or("missing entity kind")?)?;
            let id = parts.next().ok_or("missing entity UUID")?;
            let profile = self.app.load_story_bible_profile(kind, id)?;
            self.message = serde_json::to_string(&profile)?;
        } else if let Some(file) = cmd.strip_prefix("story-profile-import ") {
            let profile: story_core::StoryBibleProfile =
                serde_json::from_slice(&fs::read(file.trim())?)?;
            self.app.save_story_bible_profile(&profile)?;
            self.message = format!(
                "Stored profile for {} {}",
                profile.entity_kind.as_str(),
                profile.entity_id
            );
        } else if let Some(value) = cmd.strip_prefix("writing-stats ") {
            match value.trim() {
                "on" => {
                    if self.writing_session.is_none() {
                        let scene_id = self.editing_scene.or_else(|| self.active_scene());
                        let initial_words = self.current_writing_words(scene_id)?;
                        let id = self.app.start_writing_session(scene_id, initial_words)?;
                        self.writing_session = Some(ActiveWritingSession {
                            id,
                            scene_id,
                            initial_words,
                        });
                    }
                    self.writing_stats_enabled = true;
                    self.message =
                        "Local writing session started; manuscript text is never logged".into();
                }
                "off" => {
                    if let Some(session) = self.writing_session.take() {
                        let end_words = self
                            .current_writing_words(session.scene_id)
                            .unwrap_or(session.initial_words);
                        self.app
                            .finish_writing_session(&session.id, end_words, false)?;
                    }
                    self.writing_stats_enabled = false;
                    self.message = "Writing statistics disabled; session words recorded".into();
                }
                "report" => {
                    self.message = format!(
                        "{} · {} typed chars · {} pasted chars · {} saves",
                        if self.writing_stats_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        },
                        self.typed_characters,
                        self.pasted_characters,
                        self.successful_saves
                    );
                }
                "reset" => {
                    self.typed_characters = 0;
                    self.pasted_characters = 0;
                    self.successful_saves = 0;
                    self.message = "Local aggregate writing counts reset".into();
                }
                _ => return Err("use writing-stats on|off|report|reset".into()),
            }
        } else if let Some(rest) = cmd.strip_prefix("goal-create ") {
            let mut parts = rest
                .splitn(8, char::is_whitespace)
                .filter(|part| !part.is_empty());
            let start = parts.next().ok_or("goal-create requires start date")?;
            let end = parts.next().ok_or("goal-create requires end date")?;
            let target = parts
                .next()
                .ok_or("goal-create requires target words")?
                .parse::<i64>()?;
            let baseline = parts
                .next()
                .ok_or("goal-create requires baseline words")?
                .parse::<i64>()?;
            let time_zone = parts
                .next()
                .ok_or("goal-create requires UTC or a fixed offset")?;
            let success_metric = parts.next().ok_or("goal-create requires raw or real")?;
            let scope = parts
                .next()
                .ok_or("goal-create requires project or a scene UUID")?;
            let title = parts.next().ok_or("goal-create requires a title")?.trim();
            if title.is_empty() {
                return Err("goal title cannot be empty".into());
            }
            let scope_scene_id = if scope == "project" {
                None
            } else {
                Some(SceneId(scope.parse()?))
            };
            let id = self
                .app
                .create_writing_goal(&story_storage::NewWritingGoal {
                    title: title.into(),
                    scope_scene_id,
                    start_date: start.into(),
                    end_date: end.into(),
                    target_words: target,
                    baseline_words: baseline,
                    time_zone: time_zone.into(),
                    word_count_policy: "unicode-word-boundaries".into(),
                    success_metric: success_metric.into(),
                })?;
            self.message = format!("Created writing goal {id}");
        } else if cmd == "goal-list" {
            self.message = self
                .app
                .list_writing_goals(false)?
                .into_iter()
                .map(|goal| {
                    format!(
                        "{} · {} · {}–{} · target {} · baseline {} · {} metric",
                        goal.id,
                        goal.title,
                        goal.start_date,
                        goal.end_date,
                        goal.target_words,
                        goal.baseline_words,
                        goal.success_metric
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("goal-events ") {
            let goal_id = rest.trim();
            self.message = self
                .app
                .list_writing_goal_events(goal_id)?
                .into_iter()
                .map(|event| {
                    format!(
                        "{} · {} · source {} · fingerprint {} · raw {} · real {} · {} · {}",
                        event.id,
                        event.origin,
                        event.source_identifier,
                        event.source_fingerprint,
                        event.raw_delta,
                        event.real_delta,
                        event.reconciliation_state,
                        event.detail
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("goal-record ") {
            let mut parts = rest
                .splitn(9, char::is_whitespace)
                .filter(|part| !part.is_empty());
            let goal_id = parts.next().ok_or("goal-record requires a goal UUID")?;
            let occurred_at = parts
                .next()
                .ok_or("goal-record requires a Unix timestamp")?
                .parse::<i64>()?;
            let origin = parts.next().ok_or("goal-record requires an origin")?;
            let source_identifier = parts
                .next()
                .ok_or("goal-record requires a source identifier")?;
            let source_fingerprint = parts.next().ok_or("goal-record requires a fingerprint")?;
            let scene = parts
                .next()
                .ok_or("goal-record requires a scene UUID or none")?;
            let scene_id = if scene == "none" {
                None
            } else {
                Some(SceneId(scene.parse()?))
            };
            let raw_delta = parts
                .next()
                .ok_or("goal-record requires a raw word delta")?
                .parse::<i64>()?;
            let real_delta = parts
                .next()
                .ok_or("goal-record requires a real word delta")?
                .parse::<i64>()?;
            let detail = parts.next().unwrap_or("").trim();
            let id = self
                .app
                .record_writing_goal_event(&story_storage::NewWritingGoalEvent {
                    goal_id: goal_id.into(),
                    occurred_at,
                    origin: origin.into(),
                    source_identifier: source_identifier.into(),
                    source_fingerprint: source_fingerprint.into(),
                    scene_id,
                    raw_delta,
                    real_delta,
                    detail: detail.into(),
                })?;
            self.message = format!("Recorded writing event {id}");
        } else if let Some(rest) = cmd.strip_prefix("goal-progress ") {
            let goal_id = rest.trim();
            let as_of = time::OffsetDateTime::now_utc().unix_timestamp();
            self.message = self
                .app
                .writing_goal_progress(goal_id, as_of)?
                .into_iter()
                .map(|point| {
                    format!(
                        "{} target={} raw={} real={}{}",
                        point.date,
                        point.target_words,
                        point
                            .raw_words
                            .map_or_else(|| "future".into(), |value| value.to_string()),
                        point
                            .real_words
                            .map_or_else(|| "future".into(), |value| value.to_string()),
                        if point.projected { " projected" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("goal-weekly ") {
            let goal_id = rest.trim();
            self.message = writing_goal_weekly_rows(&self.app, goal_id)?.join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("goal-archive ") {
            self.app.archive_writing_goal(rest.trim())?;
            self.message = format!("Archived writing goal {}", rest.trim());
        } else if let Some(rest) = cmd.strip_prefix("goal-reconcile ") {
            let mut parts = rest
                .splitn(5, char::is_whitespace)
                .filter(|part| !part.is_empty());
            let event_id = parts
                .next()
                .ok_or("goal-reconcile requires an event UUID")?;
            let expected_hash = parts
                .next()
                .ok_or("goal-reconcile requires the inspected fingerprint")?;
            let raw = parts
                .next()
                .ok_or("goal-reconcile requires corrected raw words")?
                .parse::<i64>()?;
            let real = parts
                .next()
                .ok_or("goal-reconcile requires corrected real words")?
                .parse::<i64>()?;
            let reason = parts
                .next()
                .ok_or("goal-reconcile requires a reason")?
                .trim();
            let new_id = self.app.reconcile_writing_goal_event(
                event_id,
                expected_hash,
                raw,
                real,
                reason,
            )?;
            self.message = format!("Recorded correction as event {new_id}");
        } else if let Some(value) = cmd.strip_prefix("assembly-resume ") {
            let id = story_core::AssemblyDraftId(value.trim().parse()?);
            self.open_assembly_draft(id)?;
        } else if cmd == "assembly-drafts" {
            self.message = self
                .app
                .list_assembly_drafts()?
                .into_iter()
                .map(|draft| {
                    format!(
                        "{} · assembly {} · {} scenes",
                        draft.id,
                        draft.assembly_id,
                        draft.items.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(value) = cmd.strip_prefix("assembly-edit ") {
            let assembly = story_core::AssemblyId(value.trim().parse()?);
            let draft = self.app.create_assembly_draft(assembly)?;
            self.open_assembly_draft(draft.id)?;
        } else if cmd == "board-back" {
            self.planning_board_id = None;
            self.selected = 0;
            self.message = "Planning board list".into();
        } else if let Some(value) = cmd.strip_prefix("board-open ") {
            let id = story_core::PlanningBoardId(value.trim().parse()?);
            if !self
                .app
                .list_planning_boards()?
                .iter()
                .any(|board| board.id == id)
            {
                return Err("planning board no longer exists".into());
            }
            self.planning_board_id = Some(id);
            self.selected = 0;
            self.switch(Workspace::PlanningBoards);
            self.message = format!("Opened planning board {id} · Ctrl+arrows move cards");
        } else if let Some(title) = cmd.strip_prefix("board ") {
            let id = self.app.create_planning_board(title.trim())?;
            self.planning_board_id = Some(id);
            self.selected = 0;
            self.switch(Workspace::PlanningBoards);
            self.message = format!("Created planning board {id}");
        } else if let Some(rest) = cmd.strip_prefix("board-list ") {
            let board = story_core::PlanningBoardId(rest.trim().parse()?);
            self.message = self
                .app
                .list_planning_board_items(board)?
                .into_iter()
                .map(|item| {
                    format!(
                        "{} · {} {} at ({}, {})",
                        item.id, item.entity_kind, item.entity_id, item.x, item.y
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("board-export ") {
            let board = story_core::PlanningBoardId(rest.trim().parse()?);
            let title = self
                .app
                .list_planning_boards()?
                .into_iter()
                .find(|candidate| candidate.id == board)
                .ok_or("planning board not found")?
                .title;
            self.message =
                format_planning_board_markdown(&title, &self.app.list_planning_board_items(board)?);
        } else if let Some(rest) = cmd.strip_prefix("board-move ") {
            let mut parts = rest.split_whitespace();
            let item = story_core::PlanningBoardItemId(
                parts.next().ok_or("missing board item UUID")?.parse()?,
            );
            let x = parts.next().ok_or("missing x coordinate")?.parse()?;
            let y = parts.next().ok_or("missing y coordinate")?.parse()?;
            self.app.move_planning_board_item(item, x, y)?;
            self.message = format!("Moved planning card to ({x}, {y})");
        } else if let Some(rest) = cmd.strip_prefix("board-resize ") {
            let mut parts = rest.split_whitespace();
            let item = story_core::PlanningBoardItemId(
                parts.next().ok_or("missing board item UUID")?.parse()?,
            );
            let width = parts.next().ok_or("missing width")?.parse()?;
            let height = parts.next().ok_or("missing height")?.parse()?;
            self.app.resize_planning_board_item(item, width, height)?;
            self.message = format!("Resized planning card to {width:.1}×{height:.1}");
        } else if let Some(rest) = cmd.strip_prefix("board-connect ") {
            let mut parts = rest.split_whitespace();
            let board =
                story_core::PlanningBoardId(parts.next().ok_or("missing board UUID")?.parse()?);
            let source = story_core::PlanningBoardItemId(
                parts.next().ok_or("missing source item UUID")?.parse()?,
            );
            let target = story_core::PlanningBoardItemId(
                parts.next().ok_or("missing target item UUID")?.parse()?,
            );
            let label = parts.next().unwrap_or("");
            let id = self
                .app
                .connect_planning_board_items(board, source, target, label)?;
            self.message = format!("Added board connector {id}");
        } else if let Some(rest) = cmd.strip_prefix("board-add ") {
            let mut parts = rest.split_whitespace();
            let board =
                story_core::PlanningBoardId(parts.next().ok_or("missing board UUID")?.parse()?);
            let kind = parts.next().ok_or("missing entity kind")?;
            let entity = parts.next().ok_or("missing entity UUID")?;
            let x: f64 = parts.next().ok_or("missing x coordinate")?.parse()?;
            let y: f64 = parts.next().ok_or("missing y coordinate")?.parse()?;
            let note = parts.collect::<Vec<_>>().join(" ");
            let id = self
                .app
                .add_planning_board_item(board, kind, entity, x, y, &note)?;
            self.message = format!("Placed {kind} on planning board as {id}");
        } else if let Some(title) = cmd.strip_prefix("assembly ") {
            let id = self.app.create_assembly(title.trim())?;
            self.message = format!("Created manuscript assembly {id}");
        } else if cmd == "assembly-list" {
            self.message = self
                .app
                .list_assemblies()?
                .into_iter()
                .map(|assembly| format!("{} · {}", assembly.id, assembly.title))
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("assembly-add ") {
            let mut parts = rest.split_whitespace();
            let assembly =
                story_core::AssemblyId(parts.next().ok_or("missing assembly UUID")?.parse()?);
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let position = parts.next().ok_or("missing position")?.parse()?;
            let item = self.app.add_assembly_scene(assembly, scene, position)?;
            self.message = format!("Added scene to assembly as item {item}");
        } else if let Some(rest) = cmd.strip_prefix("assembly-move ") {
            let mut parts = rest.split_whitespace();
            let assembly =
                story_core::AssemblyId(parts.next().ok_or("missing assembly UUID")?.parse()?);
            let item =
                story_core::AssemblyItemId(parts.next().ok_or("missing item UUID")?.parse()?);
            let position = parts.next().ok_or("missing position")?.parse()?;
            self.app.move_assembly_item(assembly, item, position)?;
            self.message = "Moved assembly item; canonical manuscript order is unchanged".into();
        } else if let Some(rest) = cmd.strip_prefix("assembly-include ") {
            let mut parts = rest.split_whitespace();
            let item =
                story_core::AssemblyItemId(parts.next().ok_or("missing item UUID")?.parse()?);
            let included = match parts.next().ok_or("use on or off")? {
                "on" => true,
                "off" => false,
                _ => return Err("assembly inclusion must be on or off".into()),
            };
            self.app.set_assembly_item_included(item, included)?;
            self.message = format!(
                "Assembly item {}",
                if included { "included" } else { "excluded" }
            );
        } else if let Some(value) = cmd.strip_prefix("assembly-compile ") {
            let assembly = story_core::AssemblyId(value.trim().parse()?);
            let text = self.app.compile_assembly_markdown(assembly)?;
            let destination = self
                .app
                .project_root()
                .join("exports")
                .join(format!("assembly-{assembly}.md"));
            fs::write(&destination, text.as_bytes())?;
            self.message = format!("Compiled assembly to {}", destination.display());
        } else if let Some(arguments) = cmd.strip_prefix("compile-format ") {
            let mut parts = arguments.split_whitespace();
            let format_name = parts.next().ok_or("usage: compile-format <text|markdown|latex|html|epub|docx|pdf> [output-path] [--expand-transclusions]")?;
            let format = story_document::CompileFormat::parse(format_name)
                .ok_or("format must be text, markdown, latex, html, epub, docx, or pdf")?;
            let rest = parts.collect::<Vec<_>>();
            let expand = rest.contains(&"--expand-transclusions");
            let design = rest
                .iter()
                .find_map(|argument| argument.strip_prefix("--design="))
                .map(|name| {
                    story_document::BookDesign::parse(name)
                        .ok_or("design must be trade-paperback, hardback, manuscript, nonfiction, or technical-academic")
                })
                .transpose()?
                .unwrap_or_default();
            let output = rest.iter().find(|item| !item.starts_with("--"));
            let artifact = if let Some(profile_path) = rest
                .iter()
                .find_map(|argument| argument.strip_prefix("--design-file="))
            {
                let root = self.app.project_root().canonicalize()?;
                let path = root.join(profile_path).canonicalize()?;
                if !path.starts_with(&root) || !path.is_file() {
                    return Err("design profile must be a project-local regular file".into());
                }
                let profile =
                    story_document::BookDesignProfile::from_toml(&fs::read_to_string(path)?)?;
                self.app
                    .compile_artifact_with_profile(format, expand, profile)?
            } else {
                self.app
                    .compile_artifact_with_design(format, expand, design)?
            };
            let destination = output
                .map(Path::new)
                .map(Path::to_path_buf)
                .unwrap_or_else(|| {
                    self.app
                        .project_root()
                        .join("exports")
                        .join(format!("compiled.{}", format.extension()))
                });
            atomic_export(&destination, &artifact.bytes)?;
            for auxiliary in &artifact.auxiliary_files {
                let extension = Path::new(&auxiliary.name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("log");
                atomic_export(&destination.with_extension(extension), &auxiliary.bytes)?;
            }
            let sidecar =
                Path::new(&format!("{}.source-map.json", destination.display())).to_path_buf();
            let source_map = serde_json::json!({
                "schema_version": 1,
                "output_format": format,
                "offset_space": "compile-ir-markdown-utf8",
                "visual_layout_mapping": false,
                "entries": artifact.source_map,
            });
            atomic_export(&sidecar, &serde_json::to_vec_pretty(&source_map)?)?;
            self.message = format!(
                "Compiled to {} · source map {}",
                destination.display(),
                sidecar.display()
            );
        } else if let Some(arguments) = cmd.strip_prefix("review-merge ") {
            let mut parts = arguments.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let base_path = parts.next().ok_or("missing base manuscript file")?;
            let reviewed_path = parts.next().ok_or("missing reviewed manuscript file")?;
            let base = fs::read_to_string(base_path)?;
            let reviewed = fs::read_to_string(reviewed_path)?;
            let result = self.app.merge_review_text(scene, &base, &reviewed)?;
            self.message = if let Some(merged) = result.merged_text {
                if self.editing_scene == Some(scene) {
                    self.editor = merged.lines().map(str::to_owned).collect();
                    self.dirty = false;
                    self.cursor = (0, 0);
                }
                "Editorial merge applied through the manuscript write journal".into()
            } else {
                if self.dirty {
                    return Err(
                        "save the open scene buffer before reviewing an editorial merge".into(),
                    );
                }
                self.open_scene(scene)?;
                let content = format_conflict_review(&result.conflicts, self.width);
                self.open_reference_buffer(
                    format!("scene:{scene}"),
                    Some(scene),
                    "Editorial conflict review".into(),
                    content,
                );
                self.reference_focused = false;
                format!(
                    "{} conflict(s) shown beside the canonical scene; prose unchanged",
                    result.conflicts.len()
                )
            };
        } else if let Some(arguments) = cmd.strip_prefix("review-resolve ") {
            let mut parts = arguments.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let base_path = parts.next().ok_or("missing base manuscript file")?;
            let reviewed_path = parts.next().ok_or("missing reviewed manuscript file")?;
            let raw_choices = parts
                .next()
                .ok_or("missing comma-separated conflict choices")?;
            let choices = raw_choices
                .split(',')
                .map(|choice| match choice {
                    "base" => Ok(story_import::MergeConflictChoice::Base),
                    "local" => Ok(story_import::MergeConflictChoice::Local),
                    "external" => Ok(story_import::MergeConflictChoice::External),
                    _ => Err("choices must be base, local, or external"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let base = fs::read_to_string(base_path)?;
            let reviewed = fs::read_to_string(reviewed_path)?;
            let merged = self
                .app
                .resolve_review_text(scene, &base, &reviewed, &choices)?;
            if self.editing_scene == Some(scene) {
                self.editor = merged.lines().map(str::to_owned).collect();
                self.dirty = false;
                self.cursor = (0, 0);
            }
            self.message = format!(
                "Resolved {} conflict(s) and saved through the manuscript write journal",
                choices.len()
            );
        } else if let Some(path) = cmd.strip_prefix("review-inspect ") {
            self.message = self
                .app
                .inspect_docx_review(path.trim())?
                .into_iter()
                .map(|item| {
                    format!(
                        "{} p{} {}{}: {}",
                        item.kind,
                        item.paragraph_index
                            .map(|index| (index + 1).to_string())
                            .unwrap_or_else(|| "?".into()),
                        item.author.unwrap_or_else(|| "unknown author".into()),
                        item.date
                            .map(|date| format!(" · {date}"))
                            .unwrap_or_default(),
                        item.text
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(arguments) = cmd.strip_prefix("review-docx ") {
            let mut parts = arguments.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let base_path = parts.next().ok_or("missing base manuscript file")?;
            let review_path = parts.next().ok_or("missing reviewed DOCX file")?;
            let accept_changes = match parts.next().ok_or("choose accept or reject")? {
                "accept" => true,
                "reject" => false,
                _ => return Err("review disposition must be accept or reject".into()),
            };
            if parts.next().is_some() {
                return Err("usage: review-docx SCENE BASE_FILE REVIEW.docx accept|reject".into());
            }
            let base = fs::read_to_string(base_path)?;
            let result = self
                .app
                .merge_docx_review(scene, &base, review_path, accept_changes)?;
            self.message = if let Some(merged) = result.merged_text {
                if self.editing_scene == Some(scene) {
                    self.editor = merged.lines().map(str::to_owned).collect();
                    if self.editor.is_empty() {
                        self.editor.push(String::new());
                    }
                    self.dirty = false;
                    self.cursor = (0, 0);
                    self.saved_hash = self.app.load_scene_text(scene)?.1;
                }
                format!(
                    "DOCX review changes {} and merged through the durable save journal",
                    if accept_changes {
                        "accepted"
                    } else {
                        "rejected"
                    }
                )
            } else {
                format!(
                    "{} overlapping review conflict(s); manuscript left unchanged",
                    result.conflicts.len()
                )
            };
        } else if let Some(rest) = cmd.strip_prefix("scratchpad ") {
            let (title, body) = rest.split_once('|').unwrap_or((rest, ""));
            let id = self.app.create_scratchpad_note(title.trim(), body.trim())?;
            self.message = format!("Captured scratchpad note {id}");
        } else if cmd == "scratchpad-list" {
            self.message = self
                .app
                .list_scratchpad_notes(true)?
                .into_iter()
                .map(|note| {
                    format!(
                        "{}{} · {} · {}",
                        if note.pinned { "📌 " } else { "" },
                        note.title,
                        note.state,
                        note.body
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("scratchpad-pin ") {
            let mut parts = rest.split_whitespace();
            let id = story_core::ScratchpadId(parts.next().ok_or("missing note UUID")?.parse()?);
            let pinned = match parts.next().ok_or("use on or off")? {
                "on" => true,
                "off" => false,
                _ => return Err("pin value must be on or off".into()),
            };
            let note = self
                .app
                .list_scratchpad_notes(true)?
                .into_iter()
                .find(|note| note.id == id)
                .ok_or("scratchpad note not found")?;
            self.app
                .update_scratchpad_note(id, &note.title, &note.body, pinned)?;
            self.message = if pinned {
                "Scratchpad note pinned"
            } else {
                "Scratchpad note unpinned"
            }
            .into();
        } else if let Some(rest) = cmd.strip_prefix("scratchpad-promote ") {
            let mut parts = rest.split_whitespace();
            let id = story_core::ScratchpadId(parts.next().ok_or("missing note UUID")?.parse()?);
            let kind = parts.next().ok_or("missing destination kind")?;
            let destination = parts.next().ok_or("missing destination UUID")?;
            self.app.promote_scratchpad_note(id, kind, destination)?;
            self.message =
                format!("Promoted note {id} to {kind} {destination}; source text retained");
        } else if cmd == "mentions" {
            let scene = self
                .editing_scene
                .or_else(|| self.active_scene())
                .ok_or("open or select a scene first")?;
            self.message = self
                .app
                .list_scene_mentions(scene)?
                .into_iter()
                .map(|mention| {
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
                    format!(
                        "{} · {} · bytes {}..{} · {}",
                        mention.state,
                        mention.quote,
                        mention.start_byte,
                        mention.end_byte,
                        candidates
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(value) = cmd.strip_prefix("mention-jump ") {
            let mention_id = story_core::MentionId(value.trim().parse()?);
            let scene_id = self
                .editing_scene
                .or_else(|| self.active_scene())
                .ok_or("open or select a scene first")?;
            let mention = self
                .app
                .list_scene_mentions(scene_id)?
                .into_iter()
                .find(|mention| mention.id == mention_id)
                .ok_or("mention is not in the current scene")?;
            self.open_scene(scene_id)?;
            let text = self.app.load_scene_text(scene_id)?.0;
            let offset = usize::try_from(mention.start_byte)?;
            let prefix = text
                .get(..offset)
                .ok_or("mention offset is no longer valid")?;
            self.cursor.0 = prefix
                .chars()
                .filter(|character| *character == '\n')
                .count();
            self.cursor.1 = prefix
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count();
            self.message = format!("Jumped to {} · {}", mention.state, mention.quote);
        } else if let Some(rest) = cmd.strip_prefix("mention-appearances ") {
            let mut parts = rest.split_whitespace();
            let kind = parts.next().ok_or("missing entity kind")?;
            let entity = parts.next().ok_or("missing entity UUID")?;
            let mut appearances = Vec::new();
            for record in self.app.list_scenes(false)? {
                for mention in self.app.list_scene_mentions(record.scene.id)? {
                    if mention.state == "confirmed"
                        && mention.entity_kind.as_deref() == Some(kind)
                        && mention.entity_id.as_deref() == Some(entity)
                    {
                        appearances.push(format!("{} · {}", record.scene.title, mention.quote));
                    }
                }
            }
            self.message = appearances.join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("mention-confirm ") {
            let mut parts = rest.split_whitespace();
            let mention =
                story_core::MentionId(parts.next().ok_or("missing mention UUID")?.parse()?);
            let kind = match parts.next().ok_or("missing candidate kind")? {
                "character" => story_core::MentionEntityKind::Character,
                "location" => story_core::MentionEntityKind::Location,
                "object" => story_core::MentionEntityKind::Object,
                _ => return Err("candidate kind must be character, location or object".into()),
            };
            let entity = parts.next().ok_or("missing entity UUID")?;
            self.app.confirm_scene_mention(mention, kind, entity)?;
            self.message = "Mention confirmed; narrative participation was not changed".into();
        } else if let Some(value) = cmd.strip_prefix("mention-dismiss ") {
            let id = story_core::MentionId(value.trim().parse()?);
            self.app.dismiss_scene_mention(id)?;
            self.message = "Mention dismissed".into();
        } else if let Some(body) = cmd.strip_prefix("comment ") {
            if self.dirty {
                return Err("save the current scene before creating a passage comment".into());
            }
            let scene = self.editing_scene.ok_or("open a scene first")?;
            let ((start_row, start_col), (end_row, end_col)) = self
                .selection_bounds()
                .ok_or("select manuscript text first")?;
            let quote = self.selected_text().ok_or("select manuscript text first")?;
            let start = editor_position_byte(&self.editor, start_row, start_col);
            let end = editor_position_byte(&self.editor, end_row, end_col);
            let id = self.app.create_personal_comment(
                scene,
                start as u64,
                end as u64,
                &quote,
                body.trim(),
            )?;
            self.message = format!("Added personal comment {id} to selected passage");
            let _ = (start_row, start_col, end_row, end_col);
        } else if cmd == "comments" {
            let scene = self
                .editing_scene
                .or_else(|| self.active_scene())
                .ok_or("open or select a scene first")?;
            self.message = self
                .app
                .list_scene_comments(scene)?
                .into_iter()
                .map(|comment| {
                    format!(
                        "{} · {} · {} · {}",
                        comment.state,
                        comment.anchor_state.as_deref().unwrap_or("orphaned"),
                        comment.quote.as_deref().unwrap_or("passage unavailable"),
                        comment.body
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("comment-state ") {
            let mut parts = rest.split_whitespace();
            let id =
                story_core::PersonalCommentId(parts.next().ok_or("missing comment UUID")?.parse()?);
            let state = parts.next().ok_or("use open or resolved")?;
            self.app.set_personal_comment_state(id, state)?;
            self.message = format!("Comment {id} marked {state}");
        } else if let Some(rest) = cmd.strip_prefix("object ") {
            let (name, description) = rest.split_once('|').unwrap_or((rest, ""));
            let id = self
                .app
                .create_story_object(name.trim(), description.trim())?;
            self.message = format!("Created story object {id}");
        } else if let Some(title) = cmd.strip_prefix("section ") {
            let position = self
                .app
                .list_structural_nodes()?
                .iter()
                .filter(|node| node.parent_id.is_none())
                .count() as u32;
            let id = self.app.create_section(title.trim(), position)?;
            self.message = format!("Created section {id}");
        } else if let Some(title) = cmd.strip_prefix("chapter ") {
            let position = self
                .app
                .list_structural_nodes()?
                .iter()
                .filter(|node| node.parent_id.is_none())
                .count() as u32;
            let id = self.app.create_chapter(None, title.trim(), position)?;
            self.message = format!("Created chapter {id}");
        } else if let Some(name) = cmd.strip_prefix("character ") {
            let id = self.app.create_character(name.trim())?;
            self.selected = self
                .app
                .list_characters()?
                .iter()
                .position(|character| character.id == id)
                .ok_or("created character was not listed")?;
            self.switch(Workspace::StoryBible);
            self.message = format!("Created character {id}");
        } else if let Some(rest) = cmd.strip_prefix("character-profile ") {
            let id = story_core::CharacterId(rest.trim().parse()?);
            let character = self.app.get_character(id)?;
            self.message = format!(
                "{} ({}) · {} · goals: {} · motivations: {} · aliases: {}",
                character.full_name,
                character.preferred_name,
                character.status,
                character.goals,
                character.motivations,
                character.aliases.join(", ")
            );
            self.switch(Workspace::StoryBible);
        } else if let Some(rest) = cmd.strip_prefix("character-set ") {
            let (identity, value) = rest
                .split_once('|')
                .ok_or("use character-set <character-uuid> <field> | <value>")?;
            let mut parts = identity.split_whitespace();
            let id =
                story_core::CharacterId(parts.next().ok_or("missing character UUID")?.parse()?);
            let field = parts.next().ok_or("missing character field")?;
            if parts.next().is_some() {
                return Err("character-set accepts one field name".into());
            }
            self.app.set_character_field(id, field, value.trim())?;
            self.message = format!("Updated character {id} field {field}");
        } else if let Some(rest) = cmd.strip_prefix("character-alias ") {
            let (identity, alias) = rest
                .split_once('|')
                .ok_or("use character-alias <character-uuid> | <alias>")?;
            let id = story_core::CharacterId(identity.trim().parse()?);
            self.app.add_character_alias(id, alias.trim())?;
            self.message = format!("Added alias for character {id}");
        } else if let Some(rest) = cmd.strip_prefix("character-un-alias ") {
            let (identity, alias) = rest
                .split_once('|')
                .ok_or("use character-un-alias <character-uuid> | <alias>")?;
            let id = story_core::CharacterId(identity.trim().parse()?);
            self.app.remove_character_alias(id, alias.trim())?;
            self.message = format!("Removed alias from character {id}");
        } else if let Some(name) = cmd.strip_prefix("location ") {
            let id = self.app.create_location(name.trim())?;
            self.selected = self.app.list_characters()?.len()
                + self
                    .app
                    .list_locations()?
                    .iter()
                    .position(|location| location.id == id)
                    .ok_or("created location was not listed")?;
            self.switch(Workspace::StoryBible);
            self.message = format!("Created location {id}");
        } else if let Some(rest) = cmd.strip_prefix("character-link ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let character =
                story_core::CharacterId(parts.next().ok_or("missing character UUID")?.parse()?);
            let role = parse_participation_role(parts.next().ok_or("missing participation role")?)?;
            self.app.link_character(scene, character, role)?;
            self.message = format!("Linked character {character} to scene {scene} as {role:?}");
        } else if let Some(rest) = cmd.strip_prefix("plotline-link ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let plotline =
                story_core::PlotlineId(parts.next().ok_or("missing plotline UUID")?.parse()?);
            let policy = match parts.next().ok_or("specify overlap or equal attribution")? {
                "overlap" => story_core::WordAttributionPolicy::Overlap,
                "equal" => story_core::WordAttributionPolicy::EqualShare,
                value => return Err(format!("unknown word attribution policy: {value}").into()),
            };
            let role = parts.next().unwrap_or("advances").to_owned();
            let mut links = self.app.scene_plotline_links(scene)?;
            if links.iter().any(|link| link.plotline_id == plotline) {
                return Err("scene already belongs to that plotline".into());
            }
            links.push(story_core::ScenePlotline {
                plotline_id: plotline,
                role,
                share_basis_points: None,
            });
            self.app.set_scene_plotlines(scene, &links, policy)?;
            self.message = format!("Linked plotline {plotline} to scene {scene}");
        } else if let Some(rest) = cmd.strip_prefix("plotline-shares ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let links = parts
                .map(|allocation| {
                    let (id, share) = allocation
                        .split_once('=')
                        .ok_or("allocations use PLOTLINE-UUID=BASIS-POINTS")?;
                    Ok(story_core::ScenePlotline {
                        plotline_id: story_core::PlotlineId(id.parse()?),
                        role: "advances".into(),
                        share_basis_points: Some(share.parse()?),
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
            self.app.set_scene_plotlines(
                scene,
                &links,
                story_core::WordAttributionPolicy::AuthorShares,
            )?;
            self.message = format!("Saved author-defined plotline shares for scene {scene}");
        } else if let Some(rest) = cmd.strip_prefix("scene-field ") {
            let (identity, value) = rest
                .split_once('|')
                .ok_or("use scene-field <scene-uuid> <synopsis|purpose|goal|outcome> | <text>")?;
            let mut parts = identity.split_whitespace();
            let id = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let field = parts.next().ok_or("missing scene field")?;
            let mut scene = self.app.get_scene(id)?.scene;
            let target = match field {
                "synopsis" => &mut scene.synopsis,
                "purpose" => &mut scene.purpose,
                "goal" => &mut scene.goal,
                "outcome" => &mut scene.outcome,
                _ => return Err(format!("unsupported core scene field: {field}").into()),
            };
            *target = value.trim().to_owned();
            self.app.update_scene_metadata(&scene)?;
            self.refresh()?;
            self.message = format!("Updated {field} for scene {id}");
        } else if let Some(name) = cmd.strip_prefix("plotline ") {
            let id = self.app.create_plotline(name.trim())?;
            self.selected = self.app.list_characters()?.len()
                + self.app.list_locations()?.len()
                + self
                    .app
                    .list_plotlines()?
                    .iter()
                    .position(|plotline| plotline.id == id)
                    .ok_or("created plotline was not listed")?;
            self.switch(Workspace::StoryBible);
            self.message = format!("Created plotline {id}");
        } else if let Some(title) = cmd.strip_prefix("issue ") {
            let id = self.app.create_issue(title.trim())?;
            self.selected = self
                .app
                .list_issues()?
                .iter()
                .position(|issue| issue.id == id)
                .ok_or("created issue was not listed")?;
            self.switch(Workspace::Issues);
            self.message = format!("Created issue {id}");
        } else if let Some(rest) = cmd.strip_prefix("issue-link ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let issue = story_core::IssueId(parts.next().ok_or("missing issue UUID")?.parse()?);
            self.app.link_issue(scene, issue)?;
            self.message = format!("Linked issue {issue} to scene {scene}");
        } else if let Some(rest) = cmd.strip_prefix("issue-status ") {
            let mut parts = rest.split_whitespace();
            let issue = story_core::IssueId(parts.next().ok_or("missing issue UUID")?.parse()?);
            let status = parts.next().ok_or("missing issue status")?;
            self.app.set_issue_status(issue, status)?;
            self.message = format!("Updated issue {issue} to {status}");
        } else if let Some(rest) = cmd.strip_prefix("event-time ") {
            let fields = rest.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() != 4 || fields[0].is_empty() {
                return Err("use event-time TITLE | START | END | SOURCE-TIME".into());
            }
            let event = story_core::StoryEvent {
                id: story_core::EventId::new(),
                title: fields[0].into(),
                description: String::new(),
                start: parse_story_time_input(fields[1])?,
                end: parse_story_time_input(fields[2])?,
                source_time_text: fields[3].into(),
            };
            self.app.add_story_event(&event)?;
            self.selected = self.app.list_characters()?.len()
                + self.app.list_locations()?.len()
                + self.app.list_plotlines()?.len()
                + self
                    .app
                    .list_events()?
                    .iter()
                    .position(|candidate| candidate.id == event.id)
                    .ok_or("created event was not listed")?;
            self.switch(Workspace::StoryBible);
            self.message = format!("Created timed story event {}", event.id);
        } else if let Some(rest) = cmd.strip_prefix("event-time-set ") {
            let fields = rest.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() != 4 || fields[0].is_empty() {
                return Err("use event-time-set EVENT-UUID | START | END | SOURCE-TIME".into());
            }
            let id = story_core::EventId(fields[0].parse()?);
            self.app.update_story_event_time(
                id,
                parse_story_time_input(fields[1])?,
                parse_story_time_input(fields[2])?,
                fields[3],
            )?;
            self.message = format!("Updated story-time bounds for event {id}");
        } else if let Some(rest) = cmd.strip_prefix("event ") {
            let (title, source_time) = rest.split_once('|').unwrap_or((rest, ""));
            let id = self.app.add_event(title.trim(), source_time.trim())?;
            self.selected = self.app.list_characters()?.len()
                + self.app.list_locations()?.len()
                + self.app.list_plotlines()?.len()
                + self
                    .app
                    .list_events()?
                    .iter()
                    .position(|event| event.id == id)
                    .ok_or("created event was not listed")?;
            self.switch(Workspace::StoryBible);
            self.message = format!("Created story event {id}");
        } else if let Some(rest) = cmd.strip_prefix("event-link ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let event = story_core::EventId(parts.next().ok_or("missing event UUID")?.parse()?);
            let kind = parts.next().unwrap_or("depicts");
            self.app.link_event(scene, event, kind)?;
            self.message = format!("Linked event {event} to scene {scene}");
        } else if let Some(rest) = cmd.strip_prefix("research ") {
            let (title, body) = rest.split_once('|').unwrap_or((rest, ""));
            let id =
                self.app
                    .create_research_item(title.trim(), body.trim(), "note", None, None)?;
            self.selected = self
                .app
                .list_research_items()?
                .iter()
                .position(|item| item.id == id)
                .ok_or("created research note was not listed")?;
            self.switch(Workspace::Research);
            self.message = format!("Created research note {id}");
        } else if let Some(rest) = cmd.strip_prefix("subchapter ") {
            let (parent, title) = rest
                .split_once(' ')
                .ok_or("use subchapter <section-uuid> <title>")?;
            let parent = story_core::NodeId(parent.parse()?);
            let position = self
                .app
                .list_structural_nodes()?
                .iter()
                .filter(|node| node.parent_id == Some(parent))
                .count() as u32;
            let id = self
                .app
                .create_chapter(Some(parent), title.trim(), position)?;
            self.message = format!("Created chapter {id}");
        } else if let Some(rest) = cmd.strip_prefix("scene ") {
            let (chapter, title) = rest
                .split_once(' ')
                .ok_or("use scene <chapter-uuid> <title>")?;
            let chapter = story_core::NodeId(chapter.parse()?);
            let position = self
                .app
                .list_scenes(false)?
                .iter()
                .filter(|scene| scene.chapter_id == Some(chapter))
                .count() as u32;
            let id = self.app.create_scene(chapter, title.trim(), position)?;
            self.refresh()?;
            self.message = format!("Created scene {id}");
        } else if let Some(rest) = cmd.strip_prefix("move ") {
            let mut parts = rest.split_whitespace();
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let chapter = story_core::NodeId(parts.next().ok_or("missing chapter UUID")?.parse()?);
            let position = parts
                .next()
                .ok_or("missing destination position")?
                .parse()?;
            self.app.move_scene(scene, chapter, position)?;
            self.refresh()?;
            self.message = format!("Moved scene {scene}");
        } else if let Some(rest) = cmd.strip_prefix("rename ") {
            let (id, title) = rest
                .split_once(' ')
                .ok_or("use rename <scene-uuid> <title>")?;
            let mut scene = self.app.get_scene(SceneId(id.parse()?))?.scene;
            scene.title = title.trim().into();
            self.app.update_scene_metadata(&scene)?;
            self.refresh()?;
            self.message = format!("Renamed scene {}", scene.id);
        } else if let Some(rest) = cmd.strip_prefix("scene-status ") {
            let (id, status) = rest
                .split_once(' ')
                .ok_or("use scene-status <scene-uuid> <status>")?;
            let mut scene = self.app.get_scene(SceneId(id.parse()?))?.scene;
            scene.status = parse_terminal_status(status.trim())?;
            self.app.update_scene_metadata(&scene)?;
            self.refresh()?;
            self.message = format!("Updated status for {}", scene.id);
        } else if let Some(rest) = cmd.strip_prefix("extract ") {
            let mut args = rest.split_whitespace();
            let source_kind = args
                .next()
                .ok_or("use extract <scene|reading> <UUID> <model> [endpoint]")?;
            let source_id = args.next().ok_or("missing source UUID")?;
            let model = args.next().ok_or("missing Ollama model")?;
            let endpoint = args.next().unwrap_or("http://localhost:11434");
            let proposal = self
                .app
                .extract_with_ollama(source_kind, source_id, model, endpoint)?;
            let filename = format!(
                "{source_kind}-{source_id}-{}-{}.json",
                &proposal.source_fingerprint[..16],
                story_core::QueryId::new()
            );
            let target = self
                .app
                .save_extraction_proposal(&filename, &serde_json::to_vec_pretty(&proposal)?)?;
            self.message = format!("Saved unapproved extraction: {}", target.display());
        } else if let Some(rest) = cmd.strip_prefix("approve ") {
            let (file, index) = rest
                .rsplit_once(' ')
                .ok_or("use approve <proposal-json-path> <claim-index>")?;
            let index: usize = index.parse()?;
            let file = std::path::PathBuf::from(file);
            let (original, fingerprint) = self.app.load_extraction_proposal_file(&file)?;
            let mut proposal: story_ollama::ProvisionalExtraction =
                serde_json::from_slice(&original)?;
            proposal
                .claims
                .get(index)
                .ok_or("claim index is out of range")?;
            let id = self.app.approve_extraction_claim(&proposal, index)?;
            proposal.claims[index].approved = true;
            self.app.update_extraction_proposal_file(
                &file,
                &fingerprint,
                &serde_json::to_vec_pretty(&proposal)?,
            )?;
            if self.proposal_review_active {
                self.proposal_review_rows = load_proposal_review_rows(&self.app)?;
            }
            self.workspace = Workspace::StoryBible;
            self.selected = self.app.list_characters()?.len()
                + self.app.list_locations()?.len()
                + self.app.list_plotlines()?.len()
                + self.app.list_events()?.len()
                + self
                    .app
                    .list_approved_knowledge_claims()?
                    .iter()
                    .position(|claim| claim.id == id)
                    .unwrap_or(0);
            self.message = format!("Approved claim {index} as evidence-linked knowledge {id}");
        } else if let Some(rest) = cmd.strip_prefix("dismiss ") {
            let (file, index) = rest
                .rsplit_once(' ')
                .ok_or("use dismiss <proposal-json-path> <claim-index>")?;
            let index: usize = index.parse()?;
            let file = std::path::PathBuf::from(file);
            let (original, fingerprint) = self.app.load_extraction_proposal_file(&file)?;
            let mut proposal: story_ollama::ProvisionalExtraction =
                serde_json::from_slice(&original)?;
            self.app.dismiss_extraction_claim(&mut proposal, index)?;
            self.app.update_extraction_proposal_file(
                &file,
                &fingerprint,
                &serde_json::to_vec_pretty(&proposal)?,
            )?;
            if self.proposal_review_active {
                self.proposal_review_rows = load_proposal_review_rows(&self.app)?;
            }
            self.message = format!("Dismissed provisional claim {index}; evidence retained");
        } else if cmd == "proposals" {
            self.switch(Workspace::Research);
            self.proposal_review_rows = load_proposal_review_rows(&self.app)?;
            self.proposal_review_active = true;
            self.proposal_review_index = 0;
            self.message = if self.proposal_review_rows.is_empty() {
                "No extraction proposals".into()
            } else {
                "Provisional extraction review; evidence and status are shown per claim".into()
            };
        } else if cmd == "knowledge" {
            self.workspace = Workspace::StoryBible;
            self.selected = self.app.list_characters()?.len()
                + self.app.list_locations()?.len()
                + self.app.list_plotlines()?.len()
                + self.app.list_events()?.len();
            self.message = format!(
                "{} approved evidence-linked knowledge claims",
                self.app.list_approved_knowledge_claims()?.len()
            );
        } else if let Some(rest) = cmd.strip_prefix("read-import ") {
            let path = std::path::Path::new(rest);
            let source = story_reading::import_reference(path)?;
            let id = self.app.import_reading_document(&source, rest)?;
            self.workspace = Workspace::Reading;
            let docs = self.app.list_reading_documents()?;
            self.selected = docs.iter().position(|d| d.id == id).unwrap_or(0);
            self.open_reading_selected()?;
            self.message = format!("Imported reference {}", source.title);
        } else if let Some(rest) = cmd.strip_prefix("read ") {
            let id = story_core::ReadingId(rest.parse()?);
            let doc = self.app.get_reading_document(id)?;
            self.reading_document = Some(id);
            self.reading_text = doc.extracted_text;
            self.reading_offset = 0;
            self.workspace = Workspace::Reading;
        } else if let Some(rest) = cmd.strip_prefix("highlight ") {
            let (range, quote) = rest
                .split_once(' ')
                .ok_or("use highlight <start>:<end> <quoted text>")?;
            let (start, end) = range.split_once(':').ok_or("range must be start:end")?;
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app.create_reading_annotation(
                id,
                &story_core::ReadingAnnotationDraft {
                    kind: "highlight".into(),
                    start_byte: start.parse()?,
                    end_byte: end.parse()?,
                    quote: quote.into(),
                    body: String::new(),
                    prefix_context: String::new(),
                    suffix_context: String::new(),
                },
            )?;
            self.message = "Highlight saved".into();
        } else if let Some(rest) = cmd.strip_prefix("note ") {
            let (selection, body) = rest
                .split_once('|')
                .ok_or("use note <start>:<end> <quote> | <annotation>")?;
            let (range, quote) = selection
                .trim()
                .split_once(' ')
                .ok_or("missing quoted passage")?;
            let (start, end) = range.split_once(':').ok_or("range must be start:end")?;
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app.create_reading_annotation(
                id,
                &story_core::ReadingAnnotationDraft {
                    kind: "note".into(),
                    start_byte: start.parse()?,
                    end_byte: end.parse()?,
                    quote: quote.into(),
                    body: body.trim().into(),
                    prefix_context: String::new(),
                    suffix_context: String::new(),
                },
            )?;
            self.message = "Annotation saved".into();
        } else if let Some(title) = cmd.strip_prefix("bookmark ") {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app
                .create_reading_bookmark(id, title, self.reading_offset)?;
            self.message = "Bookmark saved".into();
        } else if cmd == "checkpoint" {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app.set_reading_checkpoint(id, self.reading_offset)?;
            self.message = "Reading checkpoint saved".into();
        } else if let Some(rest) = cmd.strip_prefix("term ") {
            let (kind, name) = rest
                .split_once(' ')
                .ok_or("use term <character|species|term> <name>")?;
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app.create_reading_term(id, kind, name, "", "")?;
            self.message = "Index entry saved".into();
        } else if let Some(rest) = cmd.strip_prefix("entity-link ") {
            let (kind, id) = rest
                .split_once(' ')
                .ok_or("use entity-link <scene|character|plotline|research> <UUID>")?;
            let doc = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.app
                .link_reading_entity(doc, None, parse_entity(kind, id)?, "references")?;
            self.message = "Reference entity linked".into();
        } else if cmd == "reading-links" {
            let doc = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.message = self
                .app
                .reading_entity_links(doc)?
                .into_iter()
                .map(|link| {
                    format!(
                        "{} {} → {}:{}",
                        link.relation, link.id, link.target_kind, link.target_id
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(index) = cmd.strip_prefix("reading-follow ") {
            let doc = self
                .reading_document
                .ok_or("open a reading document first")?;
            let link = self
                .app
                .reading_entity_links(doc)?
                .get(index.parse::<usize>()?)
                .cloned()
                .ok_or("reading link index is out of range")?;
            let entity = parse_entity(&link.target_kind, &link.target_id)?;
            match entity {
                story_core::EntityRef::Scene(id) => {
                    self.selected = self
                        .scenes
                        .iter()
                        .position(|scene| scene.0 == id)
                        .ok_or("linked scene is not in the current scene list")?;
                    self.open_selected()?;
                }
                story_core::EntityRef::Character(id) => {
                    self.selected = self
                        .app
                        .list_characters()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("linked character no longer exists")?;
                    self.switch(Workspace::StoryBible);
                    self.message = format!("Opened linked character {}", id);
                }
                story_core::EntityRef::Plotline(id) => {
                    let plotline_index = self
                        .app
                        .list_plotlines()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("linked plotline no longer exists")?;
                    self.selected = self.app.list_characters()?.len()
                        + self.app.list_locations()?.len()
                        + plotline_index;
                    self.switch(Workspace::StoryBible);
                    self.message = format!("Opened linked plotline {}", id);
                }
                story_core::EntityRef::Research(id) => {
                    self.selected = self
                        .app
                        .list_research_items()?
                        .iter()
                        .position(|x| x.id == id)
                        .ok_or("linked research item no longer exists")?;
                    self.switch(Workspace::Research);
                    self.message = format!("Opened linked research item {}", id);
                }
                story_core::EntityRef::Reading(id) => {
                    self.selected = self
                        .app
                        .list_reading_documents()?
                        .iter()
                        .position(|document| document.id == id)
                        .ok_or("linked reading document no longer exists")?;
                    self.switch(Workspace::Reading);
                    self.open_reading_selected()?;
                }
            }
        } else if let Some(label) = cmd.strip_prefix("git-snapshot ") {
            if self.dirty {
                self.save()?;
            }
            self.message = match self.app.git_snapshot(label)? {
                Some(commit) => format!("Project snapshot created: {commit}"),
                None => "No project changes; snapshot not created".into(),
            };
        } else if cmd == "git-history" {
            self.message = self
                .app
                .git_history(20)?
                .into_iter()
                .map(|commit| {
                    format!(
                        "{} {} {}",
                        &commit.id[..commit.id.len().min(10)],
                        commit.date,
                        commit.subject
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(args) = cmd.strip_prefix("git-diff ") {
            let mut parts = args.splitn(3, ' ');
            let from = parts
                .next()
                .ok_or("use git-diff <commit-a> <commit-b> [path]")?;
            let to = parts
                .next()
                .ok_or("use git-diff <commit-a> <commit-b> [path]")?;
            let diff = self.app.git_diff(from, to, parts.next())?;
            self.message = diff.lines().take(12).collect::<Vec<_>>().join(" · ");
        } else if let Some(args) = cmd.strip_prefix("git-restore-new ") {
            let mut parts = args.splitn(2, ' ');
            let commit = parts
                .next()
                .ok_or("use git-restore-new COMMIT DESTINATION")?;
            let destination = parts
                .next()
                .ok_or("use git-restore-new COMMIT DESTINATION")?;
            let files =
                WriterApp::restore_git_snapshot(self.app.project_root(), commit, destination)?;
            self.message = format!(
                "Restored {commit} to {destination} ({} project files); current project unchanged",
                files.len()
            );
        } else if let Some(args) = cmd.strip_prefix("git-restore-scene ") {
            let mut parts = args.split_whitespace();
            let commit = parts
                .next()
                .ok_or("use git-restore-scene COMMIT SCENE-UUID")?;
            let scene = story_core::SceneId(
                parts
                    .next()
                    .ok_or("use git-restore-scene COMMIT SCENE-UUID")?
                    .parse()?,
            );
            self.save()?;
            self.app.restore_scene_from_git(commit, scene)?;
            if self.editing_scene == Some(scene) {
                let (text, hash) = self.app.load_scene_text(scene)?;
                self.editor = text.split('\n').map(str::to_owned).collect();
                self.saved_hash = hash;
                self.cursor.0 = self.cursor.0.min(self.editor.len().saturating_sub(1));
                self.cursor.1 = 0;
            }
            self.refresh()?;
            self.message = format!(
                "Scene prose restored from {commit}; the prior project state was snapshotted"
            );
        } else if let Some(label) = cmd.strip_prefix("snapshot ") {
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            let id = self.app.create_snapshot(scene, label)?;
            self.message = format!("Snapshot created: {id}");
        } else if cmd == "history" {
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            self.message = self
                .app
                .list_snapshots(scene)?
                .into_iter()
                .map(|s| format!("{} {} {}", s.id, s.created_at, s.label))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("diff ") {
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            let id = story_core::SnapshotId(rest.parse()?);
            self.message = self
                .app
                .diff_snapshot(scene, id)?
                .into_iter()
                .filter(|line| line.kind != story_storage::SnapshotLineKind::Equal)
                .map(|line| {
                    format!(
                        "{} {}",
                        match line.kind {
                            story_storage::SnapshotLineKind::Added => '+',
                            story_storage::SnapshotLineKind::Removed => '-',
                            story_storage::SnapshotLineKind::Equal => ' ',
                        },
                        line.text.trim_end()
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("restore ") {
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            let id = story_core::SnapshotId(rest.parse()?);
            let (_, hash) = self.app.load_scene_text(scene)?;
            let new = self.app.restore_snapshot(scene, id, &hash)?;
            self.saved_hash = new;
            self.dirty = false;
            self.message = "Snapshot restored; prior text was snapshotted".into();
        } else if let Some(rest) = cmd.strip_prefix("restore-lines ") {
            let (id,ranges)=rest.split_once(' ').ok_or("use restore-lines <snapshot> <current-start>:<current-end> <snapshot-start>:<snapshot-end>")?;
            let mut parts = ranges.split_whitespace();
            let (cs, ce) = parts
                .next()
                .ok_or("missing current range")?
                .split_once(':')
                .ok_or("range must be start:end")?;
            let (ss, se) = parts
                .next()
                .ok_or("missing snapshot range")?
                .split_once(':')
                .ok_or("range must be start:end")?;
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            let id = story_core::SnapshotId(id.parse()?);
            let (_, hash) = self.app.load_scene_text(scene)?;
            self.saved_hash = self.app.restore_snapshot_lines(
                scene,
                id,
                &hash,
                story_core::SnapshotRestoreSelection {
                    current_start: cs.parse()?,
                    current_end: ce.parse()?,
                    snapshot_start: ss.parse()?,
                    snapshot_end: se.parse()?,
                },
            )?;
            self.dirty = false;
            self.message = "Selected lines restored; prior text was snapshotted".into();
        } else if cmd == "annotations" {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.message = self
                .app
                .list_reading_annotations(id)?
                .into_iter()
                .enumerate()
                .map(|(index, a)| format!("{index}: {} [{}] {}", a.kind, a.state, a.quote))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if cmd == "bookmarks" {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.message = self
                .app
                .list_reading_bookmarks(id)?
                .into_iter()
                .enumerate()
                .map(|(index, b)| format!("{index}: {} @{} [{}]", b.title, b.offset_byte, b.state))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("annotation-jump ") {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            let index: usize = rest.parse()?;
            let annotation = self
                .app
                .list_reading_annotations(id)?
                .get(index)
                .cloned()
                .ok_or("annotation index is out of range")?;
            self.jump_reading(annotation.start_byte)?;
            self.message = format!("Jumped to annotation {index}: {}", annotation.quote);
        } else if let Some(rest) = cmd.strip_prefix("bookmark-jump ") {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            let index: usize = rest.parse()?;
            let bookmark = self
                .app
                .list_reading_bookmarks(id)?
                .get(index)
                .cloned()
                .ok_or("bookmark index is out of range")?;
            self.jump_reading(bookmark.offset_byte)?;
            self.message = format!("Jumped to bookmark {index}: {}", bookmark.title);
        } else if cmd == "terms" {
            let id = self
                .reading_document
                .ok_or("open a reading document first")?;
            self.message = self
                .app
                .list_reading_terms(id)?
                .into_iter()
                .map(|t| format!("{}: {} {}", t.kind, t.name, t.aliases))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("jump ") {
            self.jump_reading(rest.parse()?)?;
        } else if let Some(rest) = cmd.strip_prefix("open ") {
            if let Some(i) = self.scenes.iter().position(|s| s.0.to_string() == rest) {
                self.selected = i;
                self.open_selected()?;
            } else {
                self.message = "Scene is not in the outline".into();
            }
        } else if let Some(rest) = cmd.strip_prefix("workspace ") {
            self.switch(
                Workspace::ALL
                    .into_iter()
                    .find(|w| w.title().eq_ignore_ascii_case(rest))
                    .ok_or("unknown workspace")?,
            );
        } else if let Some(code) = cmd.strip_prefix("project-language ") {
            self.app.set_project_language(code.trim())?;
            self.message = format!(
                "Project language set to {}",
                self.app.get_project()?.language_code
            );
        } else if let Some(id) = cmd.strip_prefix("reference-scene ") {
            let id = SceneId(id.parse()?);
            let row = self.app.get_scene(id)?;
            let (content, _) = self.app.load_scene_text(id)?;
            self.open_reference_buffer(format!("scene:{id}"), Some(id), row.scene.title, content);
            self.reference_focused = false;
            self.message = format!(
                "Reference buffer {} of {} opened · :reference-next/prev switches buffers",
                self.active_reference + 1,
                self.reference_panes.len()
            );
        } else if let Some(id) = cmd.strip_prefix("reference-research ") {
            let id = story_core::ResearchId(id.parse()?);
            let row = self
                .app
                .list_research_items()?
                .into_iter()
                .find(|row| row.id == id)
                .ok_or("research reference not found")?;
            let content = self.app.load_research_text(id)?;
            self.open_reference_buffer(format!("research:{id}"), None, row.title, content);
            self.reference_focused = false;
            self.message = format!(
                "Reference buffer {} of {} opened · :reference-next/prev switches buffers",
                self.active_reference + 1,
                self.reference_panes.len()
            );
        } else if let Some(id) = cmd.strip_prefix("reference-character ") {
            let id = story_core::CharacterId(id.parse()?);
            let character = self.app.get_character(id)?;
            self.open_reference_buffer(
                format!("character:{id}"),
                None,
                character.preferred_name.clone(),
                format_character_reference(&character),
            );
            self.reference_focused = false;
            self.message = format!(
                "Character reference opened in buffer {} of {}",
                self.active_reference + 1,
                self.reference_panes.len()
            );
        } else if let Some(id) = cmd.strip_prefix("reference-location ") {
            let id = story_core::LocationId(id.parse()?);
            let location = self.app.get_location(id)?;
            self.open_reference_buffer(
                format!("location:{id}"),
                None,
                location.name.clone(),
                format_location_reference(&location),
            );
            self.reference_focused = false;
            self.message = format!(
                "Location reference opened in buffer {} of {}",
                self.active_reference + 1,
                self.reference_panes.len()
            );
        } else if let Some(id) = cmd.strip_prefix("reference-plotline ") {
            let id = story_core::PlotlineId(id.parse()?);
            let row = self
                .app
                .list_plotlines()?
                .into_iter()
                .find(|row| row.id == id)
                .ok_or("plotline reference not found")?;
            self.open_reference_buffer(
                format!("plotline:{id}"),
                None,
                row.name.clone(),
                format!("Plotline: {}\nScenes: {}", row.name, row.scene_count),
            );
            self.reference_focused = false;
            self.message = format!(
                "Plotline reference opened in buffer {}",
                self.active_reference + 1
            );
        } else if let Some(id) = cmd.strip_prefix("reference-event ") {
            let id = story_core::EventId(id.parse()?);
            let row = self
                .app
                .list_events()?
                .into_iter()
                .find(|row| row.id == id)
                .ok_or("event reference not found")?;
            self.open_reference_buffer(
                format!("event:{id}"),
                None,
                row.title.clone(),
                format!(
                    "Event: {}\nSource time: {}\nStart: {:?}\nEnd: {:?}",
                    row.title, row.source_time, row.start, row.end
                ),
            );
            self.reference_focused = false;
            self.message = format!(
                "Event reference opened in buffer {}",
                self.active_reference + 1
            );
        } else if let Some(id) = cmd.strip_prefix("reference-object ") {
            let id = story_core::StoryObjectId(id.parse()?);
            let row = self
                .app
                .list_story_objects()?
                .into_iter()
                .find(|row| row.id == id)
                .ok_or("object reference not found")?;
            self.open_reference_buffer(
                format!("object:{id}"),
                None,
                row.name.clone(),
                format!("Object: {}\n{}", row.name, row.description),
            );
            self.reference_focused = false;
            self.message = format!(
                "Object reference opened in buffer {}",
                self.active_reference + 1
            );
        } else if cmd == "reference-next" {
            if !self.reference_panes.is_empty() {
                self.active_reference = (self.active_reference + 1) % self.reference_panes.len();
            }
            self.message = "Next reference buffer".into();
        } else if cmd == "reference-prev" {
            if !self.reference_panes.is_empty() {
                self.active_reference = (self.active_reference + self.reference_panes.len() - 1)
                    % self.reference_panes.len();
            }
            self.message = "Previous reference buffer".into();
        } else if cmd == "reference-close" {
            if !self.reference_panes.is_empty() {
                self.reference_panes.remove(self.active_reference);
            }
            self.active_reference = self
                .active_reference
                .min(self.reference_panes.len().saturating_sub(1));
            self.reference_focused = false;
            self.message = if self.reference_panes.is_empty() {
                "Reference pane closed"
            } else {
                "Reference buffer closed"
            }
            .into();
        } else if let Some(chapter) = cmd.strip_prefix("narrow ") {
            let chapter = story_core::NodeId(chapter.parse()?);
            self.scenes = self
                .app
                .list_scenes(false)?
                .into_iter()
                .filter(|record| record.chapter_id == Some(chapter))
                .map(|record| {
                    (
                        record.scene.id,
                        record.scene.title,
                        record.scene.status,
                        record.actual_words,
                    )
                })
                .collect();
            self.selected = self
                .editing_scene
                .and_then(|id| self.scenes.iter().position(|scene| scene.0 == id))
                .unwrap_or(0);
            self.message =
                format!("Outline narrowed to chapter {chapter} · :widen restores all scenes");
        } else if cmd == "widen" {
            self.scenes = self
                .app
                .list_scenes(false)?
                .into_iter()
                .map(|record| {
                    (
                        record.scene.id,
                        record.scene.title,
                        record.scene.status,
                        record.actual_words,
                    )
                })
                .collect();
            self.selected = self
                .editing_scene
                .and_then(|id| self.scenes.iter().position(|scene| scene.0 == id))
                .unwrap_or(0);
            self.message = "Full outline restored".into();
        } else if cmd == "proofread" || cmd.starts_with("proofread ") {
            let mut scope = "current";
            let mut severities = None;
            for argument in cmd.split_whitespace().skip(1) {
                if let Some(filter) = argument.strip_prefix("severity=") {
                    let values = filter.split(',').collect::<std::collections::BTreeSet<_>>();
                    if values.is_empty()
                        || values
                            .iter()
                            .any(|value| !matches!(*value, "error" | "warning" | "information"))
                    {
                        return Err(
                            "severity filter must contain error, warning, or information".into(),
                        );
                    }
                    severities = Some(values);
                } else {
                    scope = argument;
                }
            }
            let target = self.editing_scene.or(self.active_scene());
            let records = self.app.list_scenes(false)?;
            let chapter_id = scope
                .strip_prefix("chapter:")
                .map(|value| {
                    uuid::Uuid::parse_str(value)
                        .map(story_core::NodeId)
                        .map_err(|_| "invalid chapter scope UUID")
                })
                .transpose()?;
            let scene_id = if scope == "all" || chapter_id.is_some() {
                None
            } else if scope == "current" {
                Some(target.ok_or("select a scene first")?)
            } else {
                let value = scope.strip_prefix("scene:").unwrap_or(scope);
                Some(
                    uuid::Uuid::parse_str(value)
                        .map(story_core::SceneId)
                        .map_err(|_| "scope must be current, all, scene:UUID, or chapter:UUID")?,
                )
            };
            if let Some(chapter) = chapter_id
                && !self
                    .app
                    .list_structural_nodes()?
                    .iter()
                    .any(|node| node.id == chapter && node.kind == "chapter")
            {
                return Err("chapter scope does not identify a chapter".into());
            }
            let records = records
                .into_iter()
                .filter(|record| {
                    scope == "all"
                        || scene_id == Some(record.scene.id)
                        || chapter_id == record.chapter_id
                })
                .collect::<Vec<_>>();
            let mut rows = Vec::new();
            for record in records {
                let (text, _) = self.app.load_scene_text(record.scene.id)?;
                for finding in self.app.proofread_text(&text)? {
                    if severities
                        .as_ref()
                        .is_some_and(|values| !values.contains(finding.severity.as_str()))
                    {
                        continue;
                    }
                    rows.push(format!(
                        "{} {}..{} {} {} · {}",
                        record.scene.title,
                        finding.byte_range.start,
                        finding.byte_range.end,
                        finding.category,
                        finding.severity,
                        finding.message
                    ));
                }
            }
            self.message = if rows.is_empty() {
                "No mechanical proofreading suggestions".into()
            } else {
                rows.join(" | ")
            };
        } else if cmd == "spellcheck" || cmd.starts_with("spellcheck ") {
            let mut parts = cmd.split_whitespace().skip(1);
            let language = parts.next().unwrap_or("en");
            let dictionary = parts.next();
            let executable = parts.next().unwrap_or("aspell");
            let findings =
                spellcheck_terminal_project(&self.app, language, dictionary, executable)?;
            self.message = if findings.is_empty() {
                "No spelling candidates found; manuscript unchanged".into()
            } else {
                findings
                    .iter()
                    .take(8)
                    .map(|finding| {
                        format!(
                            "{} {}: {}",
                            finding.entity_kind,
                            finding.title,
                            finding
                                .candidates
                                .iter()
                                .map(|(word, count)| format!("{word}×{count}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
        } else if cmd == "query-list" {
            self.message = self
                .app
                .list_saved_queries()?
                .into_iter()
                .map(|query| format!("{} {}", query.id, query.name))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if cmd == "query-clear" {
            self.query_view = None;
            self.search_hits.clear();
            self.selected = 0;
            self.message = "Saved query view cleared".into();
        } else if let Some(rest) = cmd.strip_prefix("query-assert-save ") {
            let mut parts = rest.split_whitespace();
            let query = story_core::QueryId(
                parts
                    .next()
                    .ok_or("use query-assert-save QUERY-UUID RULE...")?
                    .parse()?,
            );
            let assertions = parts
                .map(parse_terminal_query_assertion)
                .collect::<Result<Vec<_>, _>>()?;
            self.app.set_query_assertions(query, &assertions)?;
            self.message = format!("Saved {} query assertion(s)", assertions.len());
        } else if let Some(value) = cmd.strip_prefix("query-assert-check ") {
            let query = story_core::QueryId(value.trim().parse()?);
            let assertions = self.app.get_query_assertions(query)?;
            if assertions.is_empty() {
                return Err("query has no saved assertions".into());
            }
            let actual = self.app.run_scene_query(query)?.len();
            let results = assertions
                .iter()
                .map(|assertion| story_analysis::evaluate_query_assertion(actual, assertion))
                .collect::<Vec<_>>();
            self.message = results
                .iter()
                .map(|result| {
                    format!(
                        "{} {} (actual {})",
                        if result.passed { "PASS" } else { "FAIL" },
                        result.expectation,
                        result.actual
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("query-evidence ") {
            let mut parts = rest.split_whitespace();
            let query = story_core::QueryId(
                parts
                    .next()
                    .ok_or("use query-evidence QUERY-UUID SCENE-UUID")?
                    .parse()?,
            );
            let scene = story_core::SceneId(
                parts
                    .next()
                    .ok_or("use query-evidence QUERY-UUID SCENE-UUID")?
                    .parse()?,
            );
            let evidence = self.app.explain_scene_query_match(query, scene)?;
            self.message = evidence
                .iter()
                .map(|fact| {
                    let range = fact
                        .passage_bytes
                        .map(|(start, end)| format!(" [{start}..{end} bytes]"))
                        .or_else(|| {
                            fact.field_bytes.as_ref().map(|(field, start, end)| {
                                format!(" [{field} {start}..{end} bytes]")
                            })
                        })
                        .unwrap_or_default();
                    format!(
                        "{} {}{} · {}",
                        fact.source_kind, fact.source_id, range, fact.detail
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("event-dependency-add ") {
            let mut parts = rest.splitn(4, ' ');
            let predecessor = story_core::EventId(parts.next().ok_or("use event-dependency-add BEFORE-EVENT AFTER-EVENT precedes|requires [NOTES]")?.parse()?);
            let successor = story_core::EventId(
                parts
                    .next()
                    .ok_or("missing successor event UUID")?
                    .parse()?,
            );
            let kind = parts.next().ok_or("missing precedes|requires")?;
            let notes = parts.next().unwrap_or_default().trim();
            self.app
                .add_event_dependency(&story_core::StoryEventDependency {
                    predecessor,
                    successor,
                    kind: kind.into(),
                    notes: notes.into(),
                })?;
            self.message = "Added event dependency".into();
        } else if cmd == "event-dependency-list" {
            self.message = self
                .app
                .list_event_dependencies()?
                .iter()
                .map(|dep| {
                    format!(
                        "{} {} {} · {}",
                        dep.predecessor, dep.kind, dep.successor, dep.notes
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            if self.message.is_empty() {
                self.message = "No event dependencies".into();
            }
        } else if let Some(rest) = cmd.strip_prefix("event-dependency-update ") {
            let mut parts = rest.splitn(4, ' ');
            let predecessor = story_core::EventId(
                parts
                    .next()
                    .ok_or("use event-dependency-update BEFORE-EVENT AFTER-EVENT KIND NOTES")?
                    .parse()?,
            );
            let successor = story_core::EventId(
                parts
                    .next()
                    .ok_or("missing successor event UUID")?
                    .parse()?,
            );
            let kind = parts.next().ok_or("missing dependency kind")?;
            let notes = parts.next().unwrap_or_default().trim();
            self.app
                .update_event_dependency(&story_core::StoryEventDependency {
                    predecessor,
                    successor,
                    kind: kind.into(),
                    notes: notes.into(),
                })?;
            self.message = "Updated event dependency".into();
        } else if let Some(rest) = cmd.strip_prefix("event-dependency-remove ") {
            let mut parts = rest.split_whitespace();
            let predecessor = story_core::EventId(
                parts
                    .next()
                    .ok_or("use event-dependency-remove BEFORE-EVENT AFTER-EVENT KIND")?
                    .parse()?,
            );
            let successor = story_core::EventId(
                parts
                    .next()
                    .ok_or("missing successor event UUID")?
                    .parse()?,
            );
            let kind = parts.next().ok_or("missing dependency kind")?;
            self.app
                .remove_event_dependency(predecessor, successor, kind)?;
            self.message = "Removed event dependency".into();
        } else if cmd == "event-dependency-check" {
            self.message = self
                .app
                .inspect_event_dependencies()?
                .iter()
                .map(|(dep, result)| {
                    format!(
                        "{:?} {} {} {} · {}",
                        result.status,
                        dep.predecessor,
                        dep.kind,
                        dep.successor,
                        result.evidence.join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            if self.message.is_empty() {
                self.message = "No event dependency assertions".into();
            }
        } else if let Some(rest) = cmd.strip_prefix("location-rule-add ") {
            let mut parts = rest.splitn(3, ' ');
            let first_location = story_core::LocationId(
                parts
                    .next()
                    .ok_or("use location-rule-add LOCATION-A LOCATION-B DESCRIPTION")?
                    .parse()?,
            );
            let second_location = story_core::LocationId(
                parts
                    .next()
                    .ok_or("use location-rule-add LOCATION-A LOCATION-B DESCRIPTION")?
                    .parse()?,
            );
            let description = parts.next().unwrap_or_default().trim();
            if description.is_empty() {
                return Err("location rule description cannot be empty".into());
            }
            let rule = story_core::LocationIncompatibilityRule {
                id: story_core::ContinuityRuleId::new(),
                first_location,
                second_location,
                description: description.into(),
                enabled: true,
            };
            self.app.add_location_incompatibility_rule(&rule)?;
            self.message = format!("Added location conflict rule {}", rule.id);
        } else if cmd == "location-rule-list" {
            self.message = self
                .app
                .list_location_incompatibility_rules()?
                .iter()
                .map(|rule| {
                    format!(
                        "{} {} ↔ {} · {}{}",
                        rule.id,
                        rule.first_location,
                        rule.second_location,
                        rule.description,
                        if rule.enabled { "" } else { " (disabled)" }
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
        } else if let Some(rest) = cmd.strip_prefix("location-rule-update ") {
            let mut parts = rest.splitn(3, ' ');
            let id = story_core::ContinuityRuleId(
                parts
                    .next()
                    .ok_or("use location-rule-update RULE-UUID enabled|disabled DESCRIPTION")?
                    .parse()?,
            );
            let enabled = match parts.next().ok_or("missing enabled|disabled")? {
                "enabled" => true,
                "disabled" => false,
                _ => return Err("rule state must be enabled or disabled".into()),
            };
            let description = parts.next().unwrap_or_default().trim();
            let mut rule = self
                .app
                .list_location_incompatibility_rules()?
                .into_iter()
                .find(|rule| rule.id == id)
                .ok_or("unknown location rule")?;
            rule.enabled = enabled;
            rule.description = description.into();
            self.app.update_location_incompatibility_rule(&rule)?;
            self.message = format!("Updated location rule {id}");
        } else if let Some(id) = cmd.strip_prefix("location-rule-remove ") {
            let id = story_core::ContinuityRuleId(id.trim().parse()?);
            self.app.remove_location_incompatibility_rule(id)?;
            self.message = format!("Removed location rule {id}");
        } else if cmd == "location-rule-check" {
            self.message = self
                .app
                .inspect_location_incompatibilities()?
                .iter()
                .map(|finding| {
                    let status = match finding.status {
                        story_analysis::LocationRuleFindingStatus::DefiniteConflict => "CONFLICT",
                        story_analysis::LocationRuleFindingStatus::PossibleConflict => "POSSIBLE",
                        story_analysis::LocationRuleFindingStatus::Unverified => "UNVERIFIED",
                    };
                    format!(
                        "{status} {} {} ↔ {} · {}",
                        finding.rule_id,
                        finding.first_scene_id,
                        finding.second_scene_id,
                        finding.description
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            if self.message.is_empty() {
                self.message = "No location-rule findings".into();
            }
        } else if let Some(rest) = cmd.strip_prefix("coverage-add ") {
            let mut parts = rest.splitn(5, ' ');
            let id = parts.next().unwrap_or_default().trim();
            let dimension = parts.next().unwrap_or_default().trim();
            let entity_id = parts.next().unwrap_or_default().trim();
            let minimum_scenes = parts
                .next()
                .ok_or("use coverage-add ID character|plotline|pov|location|event ENTITY-UUID MIN LABEL")?
                .parse::<usize>()?;
            let label = parts.next().unwrap_or_default().trim();
            if id.is_empty()
                || label.is_empty()
                || entity_id.is_empty()
                || !matches!(
                    dimension,
                    "character" | "plotline" | "pov" | "location" | "event"
                )
            {
                return Err("coverage requires a unique ID, supported dimension, entity UUID, minimum and label".into());
            }
            let root = self.app.project_root();
            let mut config = load_coverage_config(root)?;
            if config.requirements.iter().any(|entry| entry.id == id) {
                return Err(
                    format!("coverage requirement {id} already exists; remove it first").into(),
                );
            }
            config.requirements.push(CoverageConfigEntry {
                id: id.into(),
                label: label.into(),
                entity_id: entity_id.into(),
                dimension: dimension.into(),
                minimum_scenes,
            });
            save_coverage_config(root, &config)?;
            self.message = format!("Added coverage requirement {id}");
        } else if let Some(id) = cmd.strip_prefix("coverage-remove ") {
            let root = self.app.project_root();
            let mut config = load_coverage_config(root)?;
            let before = config.requirements.len();
            config.requirements.retain(|entry| entry.id != id.trim());
            if config.requirements.len() == before {
                return Err(format!("unknown coverage requirement {}", id.trim()).into());
            }
            save_coverage_config(root, &config)?;
            self.message = format!("Removed coverage requirement {}", id.trim());
        } else if cmd == "coverage-list" {
            let root = self.app.project_root();
            let config = load_coverage_config(root)?;
            self.message = if config.requirements.is_empty() {
                "No configured coverage requirements".into()
            } else {
                config
                    .requirements
                    .into_iter()
                    .map(|entry| {
                        format!(
                            "{} {} {} ≥{}",
                            entry.id, entry.dimension, entry.label, entry.minimum_scenes
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
        } else if cmd == "coverage-check" {
            let root = self.app.project_root();
            let data = self.app.analytics_dataset()?;
            self.message = coverage_report_rows(root, &data).join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("link ") {
            let mut parts = rest.splitn(6, ' ');
            let source_kind = parts.next().ok_or("missing source kind")?;
            let source_id = parts.next().ok_or("missing source UUID")?;
            let target_kind = parts.next().ok_or("missing target kind")?;
            let target_id = parts.next().ok_or("missing target UUID")?;
            let link_kind = parse_terminal_link_kind(parts.next().ok_or("missing link kind")?)?;
            let label = parts.next().unwrap_or("").trim();
            self.app.create_story_link(&story_core::StoryLink {
                id: story_core::LinkId::new(),
                source: parse_entity(source_kind, source_id)?,
                source_anchor: None,
                target: parse_entity(target_kind, target_id)?,
                target_anchor: None,
                kind: link_kind,
                label: label.into(),
            })?;
            self.message = "Story link saved".into();
        } else if let Some(rest) = cmd.strip_prefix("outgoing ") {
            let (kind, id) = rest
                .split_once(' ')
                .ok_or("use outgoing <entity-kind> <UUID>")?;
            self.message = self
                .app
                .outgoing_links(parse_entity(kind, id)?)?
                .into_iter()
                .map(|link| {
                    format!(
                        "{} {} → {}:{}",
                        link.kind.as_str(),
                        link.id,
                        link.target.kind(),
                        link.target.id()
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("backlinks ") {
            let (kind, id) = rest
                .split_once(' ')
                .ok_or("use backlinks <entity-kind> <UUID>")?;
            self.message = self
                .app
                .backlinks(parse_entity(kind, id)?)?
                .into_iter()
                .map(|link| {
                    format!(
                        "{} {} ← {}:{}",
                        link.kind.as_str(),
                        link.id,
                        link.source.kind(),
                        link.source.id()
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if cmd == "broken-links" {
            self.message = self
                .app
                .broken_links()?
                .into_iter()
                .map(|broken| format!("{} {}", broken.link_id, broken.reason))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if cmd == "anchors" {
            let scene = self
                .editing_scene
                .or(self.active_scene())
                .ok_or("select a scene first")?;
            self.message = self
                .app
                .list_passage_anchors(scene)?
                .into_iter()
                .map(|a| {
                    format!(
                        "{} {}..{} [{}] {}",
                        a.id, a.start_byte, a.end_byte, a.state, a.quote
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("anchor-move ") {
            let mut parts = rest.split_whitespace();
            let anchor = story_core::AnchorId(parts.next().ok_or("missing anchor UUID")?.parse()?);
            let start = parts.next().ok_or("missing start byte")?.parse()?;
            let end = parts.next().ok_or("missing end byte")?.parse()?;
            self.app.reanchor_passage(anchor, start, end)?;
            self.message = format!("Reattached passage anchor {anchor}");
        } else if let Some(rest) = cmd.strip_prefix("transclude ") {
            let scene = SceneId(rest.parse()?);
            self.message = self
                .app
                .expand_scene_transclusions(scene)?
                .into_iter()
                .map(|passage| format!("{}{}", "  ".repeat(passage.depth), passage.text))
                .collect::<Vec<_>>()
                .join("\n");
        } else if let Some(rest) = cmd.strip_prefix("query-save ") {
            let mut parts = rest.split_whitespace();
            let name = parts
                .next()
                .ok_or("use query-save <name> [status=...|character=UUID|present=UUID|mentioned=UUID|absent=UUID|location=UUID|event=UUID|chapter=UUID|text=TERM|plotline=UUID|meta-exists=FIELD|meta-text=FIELD:TEXT|meta-number=FIELD:OP:int|decimal:VALUE|before=TICK|after=TICK|issue=unresolved]")?;
            let mut filters = Vec::new();
            for filter in parts {
                let (key, value) = filter
                    .split_once('=')
                    .ok_or("query filters use key=value")?;
                filters.push(match key {
                    "status" => story_core::SceneQueryFilter::Status(parse_terminal_status(value)?),
                    "not-status" => {
                        story_core::SceneQueryFilter::ExcludeStatus(parse_terminal_status(value)?)
                    }
                    "character" => story_core::SceneQueryFilter::Character(
                        story_core::CharacterId(value.parse()?),
                    ),
                    "present" => story_core::SceneQueryFilter::CharacterPresent(
                        story_core::CharacterId(value.parse()?),
                    ),
                    "mentioned" => story_core::SceneQueryFilter::CharacterMentioned(
                        story_core::CharacterId(value.parse()?),
                    ),
                    "absent" => story_core::SceneQueryFilter::CharacterAbsent(
                        story_core::CharacterId(value.parse()?),
                    ),
                    "location" => story_core::SceneQueryFilter::Location(story_core::LocationId(
                        value.parse()?,
                    )),
                    "event" => {
                        story_core::SceneQueryFilter::Event(story_core::EventId(value.parse()?))
                    }
                    "assembly" => story_core::SceneQueryFilter::Assembly(story_core::AssemblyId(
                        value.parse()?,
                    )),
                    "issue-status" => story_core::SceneQueryFilter::IssueStatus(value.into()),
                    "before-definitely" => {
                        story_core::SceneQueryFilter::TimeBeforeDefinitely(value.parse()?)
                    }
                    "before-possibly" => {
                        story_core::SceneQueryFilter::TimeBeforePossibly(value.parse()?)
                    }
                    "after-definitely" => {
                        story_core::SceneQueryFilter::TimeAfterDefinitely(value.parse()?)
                    }
                    "after-possibly" => {
                        story_core::SceneQueryFilter::TimeAfterPossibly(value.parse()?)
                    }
                    "chapter" => story_core::SceneQueryFilter::Chapter(value.into()),
                    "text" => story_core::SceneQueryFilter::TextContains(value.into()),
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
                        story_core::SceneQueryFilter::CustomMetadataEquals { field_id, value }
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
                        story_core::SceneQueryFilter::CustomMetadataContainsEntity {
                            field_id,
                            target_kind,
                            target_id,
                        }
                    }
                    "meta-exists" => story_core::SceneQueryFilter::CustomMetadataExists {
                        field_id: story_core::FieldId(value.parse()?),
                    },
                    "meta-text" => {
                        let (field_id, text) = value
                            .split_once(':')
                            .ok_or("meta-text uses FIELD-UUID:TEXT")?;
                        story_core::SceneQueryFilter::CustomMetadataTextContains {
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
                            _ => {
                                return Err(
                                    "numeric comparison must be lt, lte, eq, ne, gte, or gt".into(),
                                );
                            }
                        };
                        let raw = parts.next().ok_or("missing numeric metadata type")?;
                        let value = parts.next().ok_or("missing numeric metadata value")?;
                        let value = match raw {
                            "int" => story_core::FieldValue::Integer(value.parse()?),
                            "decimal" => story_core::FieldValue::Decimal(value.to_owned()),
                            _ => return Err("numeric metadata type must be int or decimal".into()),
                        };
                        story_core::SceneQueryFilter::CustomMetadataNumberCompare {
                            field_id,
                            comparison,
                            value,
                        }
                    }
                    "character-any" => story_core::SceneQueryFilter::AnyCharacter(
                        value
                            .split(',')
                            .map(|id| id.parse().map(story_core::CharacterId))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    "plotline" => story_core::SceneQueryFilter::Plotline(story_core::PlotlineId(
                        value.parse()?,
                    )),
                    "plotline-any" => story_core::SceneQueryFilter::AnyPlotline(
                        value
                            .split(',')
                            .map(|id| id.parse().map(story_core::PlotlineId))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    "before" => story_core::SceneQueryFilter::TimeBefore(value.parse()?),
                    "after" => story_core::SceneQueryFilter::TimeAfter(value.parse()?),
                    "issue" if value == "unresolved" => {
                        story_core::SceneQueryFilter::UnresolvedIssue
                    }
                    _ => return Err(format!("unsupported query filter: {filter}").into()),
                });
            }
            let query = story_core::SavedSceneQuery {
                id: story_core::QueryId::new(),
                name: name.into(),
                description: String::new(),
                filters,
            };
            self.app.save_scene_query(&query)?;
            self.message = format!("Saved query {} ({})", query.name, query.id);
        } else if let Some(rest) = cmd.strip_prefix("query ") {
            let id = story_core::QueryId(rest.parse()?);
            let saved_queries = self.app.list_saved_queries()?;
            let query = saved_queries
                .iter()
                .find(|query| query.id == id)
                .ok_or("saved query does not exist")?;
            let query_name = query.name.clone();
            self.scenes = self
                .app
                .list_scenes(false)?
                .into_iter()
                .map(|row| {
                    (
                        row.scene.id,
                        row.scene.title,
                        row.scene.status,
                        row.actual_words,
                    )
                })
                .collect();
            self.query_view = Some(id);
            self.search_hits.clear();
            self.selected = 0;
            self.workspace = Workspace::Search;
            self.message = format!("{query_name} · dynamic collection");
        } else if let Some(rest) = cmd.strip_prefix("character-absence ") {
            let mut parts = rest.splitn(3, ' ');
            let scene = SceneId(parts.next().ok_or("missing scene UUID")?.parse()?);
            let character =
                story_core::CharacterId(parts.next().ok_or("missing character UUID")?.parse()?);
            let notes = parts.next().unwrap_or("");
            self.app.record_character_absence(scene, character, notes)?;
            self.message = "Explicit character absence recorded".into();
        } else if let Some(rest) = cmd.strip_prefix("query-expression-set ") {
            let (id, source) = rest
                .split_once(' ')
                .ok_or("use query-expression-set QUERY-UUID JSON|clear")?;
            let id = story_core::QueryId(id.parse()?);
            if source == "clear" {
                self.app.set_scene_query_expression(id, None)?;
            } else {
                let expression: story_core::QueryExpression = serde_json::from_str(source)?;
                self.app.set_scene_query_expression(id, Some(&expression))?;
            }
            self.message = format!("Updated query expression {id}");
        } else if let Some(terms) = cmd.strip_prefix("search-all ") {
            self.search_hits = self.app.search_project(terms, 200)?;
            self.query_view = None;
            self.selected = 0;
            self.switch(Workspace::Search);
            self.message = format!("{} project-wide matches", self.search_hits.len());
        } else if let Some(rest) = cmd.strip_prefix("find ") {
            let mut parts = rest.splitn(3, ' ');
            let mode = parse_find_mode(parts.next().ok_or("use find MODE current|all PATTERN")?)?;
            let scope = parts.next().ok_or("missing find scope")?;
            let pattern = parts.next().ok_or("missing search pattern")?;
            let scenes = self.scenes_for_scope(scope)?;
            let mut found = Vec::new();
            self.search_locations.clear();
            for record in scenes {
                let (text, _) = self.app.load_scene_text(record.scene.id)?;
                let matches = story_core::find_matches(&text, pattern, mode)?;
                if !matches.is_empty() {
                    for matched in &matches {
                        self.search_locations
                            .push((record.scene.id, matched.start, matched.end));
                    }
                    found.push((record.scene.id, record.scene.title, matches.len()));
                }
            }
            self.search_location_index = self.search_locations.len().saturating_sub(1);
            if let Some((scene, title, _)) = found.first() {
                if self.dirty && self.editing_scene != Some(*scene) {
                    self.message = format!(
                        "{} matches found; save the current scene before navigating to {title}",
                        self.search_locations.len()
                    );
                } else {
                    self.navigate_search_match(true)?;
                    self.message = format!(
                        "{} match(es) in {} scene(s); F3/Shift-F3 navigates · first in {title}",
                        self.search_locations.len(),
                        found.len()
                    );
                }
            } else {
                self.message = "No matches".into();
            }
        } else if let Some(rest) = cmd.strip_prefix("replace-preview ") {
            let (head, replacement) = rest
                .split_once(" => ")
                .ok_or("use replace-preview MODE current|all PATTERN => REPLACEMENT")?;
            let mut parts = head.splitn(3, ' ');
            let mode = parse_find_mode(parts.next().ok_or("missing replace mode")?)?;
            let scope = parts.next().ok_or("missing replace scope")?;
            let pattern = parts.next().ok_or("missing replace pattern")?;
            let scenes = self.scenes_for_scope(scope)?;
            let mut output = Vec::new();
            let mut total = 0usize;
            for record in scenes {
                let source = if self.editing_scene == Some(record.scene.id) {
                    self.editor.join("\n")
                } else {
                    self.app.load_scene_text(record.scene.id)?.0
                };
                let (updated, count) =
                    story_core::replace_matches(&source, pattern, replacement, mode)?;
                if count > 0 {
                    total += count;
                    output.push(format!(
                        "## {} — {count} replacement(s)\n--- current ---\n{}\n--- proposed ---\n{}",
                        record.scene.title, source, updated
                    ));
                }
            }
            if output.is_empty() {
                self.message = "Preview: no replacements found; manuscript unchanged".into();
            } else {
                let preview = output.join("\n\n");
                let mut clipped = String::new();
                for ch in preview.chars().take(12_000) {
                    clipped.push(ch);
                }
                let suffix = if preview.chars().count() > 12_000 {
                    "\n\n[Preview truncated at 12,000 characters; use CLI replace-preview for complete JSON.]"
                } else {
                    ""
                };
                self.message = format!(
                    "Read-only preview: {total} replacement(s) in {} scene(s). Use replace to apply.\n\n{clipped}{suffix}",
                    output.len()
                );
            }
        } else if let Some(rest) = cmd.strip_prefix("replace ") {
            self.search_locations.clear();
            let (head, replacement) = rest
                .split_once(" => ")
                .ok_or("use replace MODE current|all PATTERN => REPLACEMENT")?;
            let mut parts = head.splitn(3, ' ');
            let mode = parse_find_mode(parts.next().ok_or("missing replace mode")?)?;
            let scope = parts.next().ok_or("missing replace scope")?;
            let pattern = parts.next().ok_or("missing replace pattern")?;
            let scenes = self.scenes_for_scope(scope)?;
            let mut changed = 0usize;
            let mut replacements = 0usize;
            for record in scenes {
                let (text, hash) = if self.editing_scene == Some(record.scene.id) {
                    (self.editor.join("\n"), self.saved_hash.clone())
                } else {
                    self.app.load_scene_text(record.scene.id)?
                };
                let (updated, count) =
                    story_core::replace_matches(&text, pattern, replacement, mode)?;
                if count != 0 {
                    let saved_hash =
                        self.app
                            .save_scene_text(record.scene.id, Some(&hash), &updated)?;
                    changed += 1;
                    replacements += count;
                    if self.editing_scene == Some(record.scene.id) {
                        self.editor = updated.split('\n').map(str::to_owned).collect();
                        if self.editor.is_empty() {
                            self.editor.push(String::new());
                        }
                        self.dirty = false;
                        self.saved_hash = saved_hash;
                    }
                }
            }
            self.message = format!("Replaced {replacements} occurrence(s) in {changed} scene(s)");
        } else if cmd == "help" {
            self.message="F1-F9, F11-F14 workspaces; Ctrl+/ or F10 main menu (menu UI pending); arrows select/scroll; Enter opens; : commands; / searches; Ctrl-S saves; q quits".into();
        } else {
            self.message = "Commands: section TITLE | chapter TITLE | character NAME | location NAME | character-link SCENE UUID ROLE | scene-field SCENE FIELD | plotline NAME | plotline-link SCENE PLOTLINE overlap|equal [ROLE] | issue TITLE | event TITLE | research TITLE | scene CHAPTER-ID TITLE | move SCENE-ID CHAPTER-ID POSITION | rename SCENE-ID TITLE | scene-status SCENE-ID STATUS | open UUID | workspace NAME | search-all TERMS | find MODE current|all PATTERN | replace MODE current|all PATTERN => REPLACEMENT | query UUID | query-list | query-clear | query-evidence QUERY-UUID SCENE-UUID | query-save NAME filters... (before-definitely=TICK, before-possibly=TICK, after-definitely=TICK, after-possibly=TICK, meta-contains=FIELD:KIND:ENTITY, meta-exists=FIELD, meta-text=FIELD:TEXT, meta-number=FIELD:OP:int|decimal:VALUE) | query-assert-save QUERY-UUID RULE... | query-assert-check QUERY-UUID | event-dependency-add BEFORE-EVENT AFTER-EVENT precedes|requires [NOTES] | event-dependency-list | event-dependency-check | event-dependency-update BEFORE AFTER KIND NOTES | event-dependency-remove BEFORE AFTER KIND | location-rule-add LOCATION-A LOCATION-B DESCRIPTION | location-rule-list | location-rule-update RULE-UUID enabled|disabled DESCRIPTION | location-rule-check | location-rule-remove RULE-UUID | coverage-add ID DIMENSION ENTITY-UUID MIN LABEL | coverage-remove ID | coverage-list | coverage-check | spellcheck LANGUAGE [PROJECT-WORDLIST] [ASPELL] | link SRC-KIND SRC-ID DST-KIND DST-ID TYPE [LABEL] | outgoing KIND UUID | backlinks KIND UUID | broken-links | anchors | transclude SCENE-ID | process | compile-format <text|markdown|latex|html|epub|docx|pdf> [OUTPUT] | compile-watch via CLI | snippet NAME TEMPLATE | snippet-insert NAME [key=value...] | register-save NAME | register-insert NAME | register-rotate NAME NAME [NAME...] | multi-insert BYTE,BYTE,... TEXT | macro-record NAME | macro-stop | macro-run NAME [REPETITIONS] | macro-preview NAME [REPETITIONS] | command-macro-record NAME | command-macro-stop | command-macro-run NAME | command-macro-list | keybinding-set CTRL+KEY COMMAND-ID | keybinding-remove CTRL+KEY | keybinding-list | macro insert TEXT|upper|lower|replace FROM=>TO | text-expansion-set ABBREVIATION => REPLACEMENT | text-expansion on|off|list | text-expansion-remove ABBREVIATION | text-expand | editor-preferences show|line-ending|indentation|indent-width | kill-ring-list | kill-ring-insert INDEX | yank-pop | manuscript-bookmark-save NAME | manuscript-bookmark-list | manuscript-bookmark-jump NAME | challenge-start TARGET_WORDS SECONDS [pressure|kamikaze] [scene=UUID] | challenge-preset short|standard|long [pressure|kamikaze] [scene=UUID] | challenge-finish | challenge-list-drafts | challenge-save-new DRAFT-UUID CHAPTER-UUID TITLE | challenge-save-target DRAFT-UUID | challenge-discard DRAFT-UUID | challenge-idle SECONDS | challenge-pauses COUNT | challenge-delete-rate WORDS | challenge-taunt TEXT (use {words}/{deleted}) | writing-stats on|off|report|reset | goal-create START END TARGET BASELINE TIMEZONE raw|real project|SCENE_UUID TITLE | goal-list | goal-record GOAL TIME ORIGIN SOURCE FINGERPRINT SCENE|none RAW REAL DETAIL | goal-events UUID | goal-progress UUID | goal-weekly UUID | goal-archive UUID | goal-reconcile EVENT_UUID FINGERPRINT RAW REAL REASON | read-import PATH | read UUID | highlight start:end quote | note start:end quote | bookmarks | annotations | help".into();
        }
        Ok(())
    }
    fn finish_active_challenge(
        &mut self,
        requested_outcome: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let challenge = self
            .active_challenge
            .clone()
            .ok_or("no active writing challenge")?;
        let current = story_core::count_words(&self.editor.join("\n"));
        let written = current.saturating_sub(challenge.initial_words);
        self.persist_challenge_draft()?;
        let outcome = if written >= challenge.target_words {
            "success"
        } else {
            requested_outcome
        };
        self.app.finish_writing_challenge_with_activity(
            &challenge.id,
            written,
            challenge.words_deleted,
            challenge.pauses_consumed,
            outcome,
            challenge.destination_scene,
        )?;
        self.active_challenge = None;
        self.editor = challenge.previous_editor;
        self.cursor = challenge.previous_cursor;
        self.dirty = challenge.previous_dirty;
        self.editing_scene = challenge.previous_scene;
        self.saved_hash = challenge.previous_hash;
        self.editor_selection = challenge.previous_selection;
        self.workspace = challenge.previous_workspace;
        self.undo_stack.clear();
        self.redo_stack.clear();
        let success = outcome == "success";
        let seconds_remaining = challenge
            .deadline
            .saturating_duration_since(Instant::now())
            .as_secs();
        let elapsed_seconds =
            u64::from(challenge.duration_seconds).saturating_sub(seconds_remaining);
        self.message = if success {
            format!(
                "Success: {written}/{} words in {elapsed_seconds}s; {} lost and {} pauses used. Draft kept separately; use challenge-save-new to save it.",
                challenge.target_words, challenge.words_deleted, challenge.pauses_consumed
            )
        } else if outcome == "quit" {
            format!(
                "YOU HAVE FAILED. {} {} words lost in {elapsed_seconds}s. Your pitiful effort is recoverable; use challenge-save-new to save it or challenge-discard to remove it.",
                render_challenge_taunt(&challenge.failure_taunt, written, challenge.words_deleted,),
                challenge.words_deleted,
            )
        } else {
            format!(
                "YOU HAVE FAILED: {} {written}/{} words in {elapsed_seconds}s; {} lost and {} pauses used. Draft preserved separately; use challenge-save-new to save it.",
                render_challenge_taunt(&challenge.failure_taunt, written, challenge.words_deleted,),
                challenge.target_words,
                challenge.words_deleted,
                challenge.pauses_consumed,
            )
        };
        Ok(())
    }
    fn apply_edit_macro(
        &mut self,
        operations: &[story_core::EditMacroOp],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let selection = self.macro_selection();
        self.push_undo();
        let (updated, selection) =
            story_core::run_edit_macro(&self.editor.join("\n"), selection, operations)?;
        self.editor = updated.split('\n').map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        if let Some((row, column)) = editor_position_from_byte(&updated, selection.end) {
            self.cursor = (row, column);
        }
        self.dirty = true;
        Ok(())
    }
    fn macro_selection(&self) -> std::ops::Range<usize> {
        self.editor_selection
            .map(|anchor| {
                let a = editor_position_byte(&self.editor, anchor.0, anchor.1);
                let b = editor_position_byte(&self.editor, self.cursor.0, self.cursor.1);
                a.min(b)..a.max(b)
            })
            .unwrap_or_else(|| {
                let start = editor_position_byte(&self.editor, self.cursor.0, 0);
                start
                    ..editor_position_byte(
                        &self.editor,
                        self.cursor.0,
                        grapheme_count(&self.editor[self.cursor.0]),
                    )
            })
    }
    fn preview_edit_macro(
        &self,
        operations: &[story_core::EditMacroOp],
    ) -> Result<String, Box<dyn std::error::Error>> {
        let (updated, _) = story_core::run_edit_macro(
            &self.editor.join("\n"),
            self.macro_selection(),
            operations,
        )?;
        Ok(updated)
    }
    fn record_macro_op(&mut self, operation: story_core::EditMacroOp) {
        if let Some((_, operations)) = &mut self.recording_macro {
            match operation {
                story_core::EditMacroOp::Insert(text) => {
                    if let Some(story_core::EditMacroOp::Insert(previous)) = operations.last_mut() {
                        previous.push_str(&text);
                    } else {
                        operations.push(story_core::EditMacroOp::Insert(text));
                    }
                }
                other => operations.push(other),
            }
        }
    }
    fn jump_reading(&mut self, offset: u64) -> Result<(), Box<dyn std::error::Error>> {
        let byte = usize::try_from(offset)?;
        let prefix = self
            .reading_text
            .get(..byte)
            .ok_or("reading position is outside the text or splits a UTF-8 character")?;
        self.reading_offset = offset;
        self.scroll = prefix.matches('\n').count();
        self.reading_focus = true;
        self.switch(Workspace::Reading);
        Ok(())
    }
    fn insert(&mut self, ch: char) {
        self.last_yank = None;
        let insertion = if ch == '\t' && self.editor_preferences.indentation == "spaces" {
            " ".repeat(self.editor_preferences.indent_width)
        } else {
            ch.to_string()
        };
        self.record_macro_op(story_core::EditMacroOp::Insert(insertion.clone()));
        if self.writing_stats_enabled {
            self.typed_characters = self
                .typed_characters
                .saturating_add(insertion.chars().count() as u64);
        }
        for character in insertion.chars() {
            self.insert_text_char(character);
        }
        if ch == ' ' {
            self.expand_text_before_cursor("space", &insertion, false);
        } else if ch == '\t' {
            self.expand_text_before_cursor("tab", &insertion, false);
        }
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
    }
    fn tick_challenge(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(challenge) = self.active_challenge.as_ref() else {
            return Ok(false);
        };
        let now = Instant::now();
        if now >= challenge.deadline {
            self.finish_active_challenge("timeout")?;
            return Ok(false);
        }
        if self.workspace != Workspace::Editor
            || self.mode.is_some()
            || self.menu.is_some()
            || self.reference_focused
        {
            self.active_challenge
                .as_mut()
                .expect("challenge remains active")
                .last_activity = now;
            return Ok(false);
        }
        if challenge.mode != "kamikaze"
            || now.duration_since(challenge.last_activity)
                < Duration::from_secs(challenge.kamikaze_idle_seconds)
        {
            return Ok(false);
        }
        let (deleted, idle_seconds, previous_deleted) = {
            let challenge = self
                .active_challenge
                .as_mut()
                .expect("challenge remains active");
            challenge.last_activity = now;
            if challenge.grace_pauses_remaining > 0 {
                challenge.grace_pauses_remaining -= 1;
                challenge.pauses_consumed = challenge.pauses_consumed.saturating_add(1);
                self.message = format!(
                    "Kamikaze grace pause used; {} automatic pause(s) remain",
                    challenge.grace_pauses_remaining
                );
                return Ok(false);
            }
            challenge.penalties_applied = challenge.penalties_applied.saturating_add(1);
            (
                challenge
                    .penalties_applied
                    .saturating_mul(challenge.deletion_words_per_penalty),
                challenge.kamikaze_idle_seconds,
                challenge.words_deleted,
            )
        };
        if deleted == 0 {
            self.message = "Kamikaze is armed; the isolated draft is empty".into();
            return Ok(false);
        }
        let before = self.editor.join("\n");
        let after = remove_last_words(&before, deleted);
        if after != before {
            let before_count = story_core::count_words(&before);
            let after_count = story_core::count_words(&after);
            let removed = before_count.saturating_sub(after_count);
            self.active_challenge
                .as_mut()
                .expect("challenge remains active")
                .words_deleted = previous_deleted.saturating_add(removed);
            self.editor = after.lines().map(str::to_owned).collect();
            if self.editor.is_empty() {
                self.editor.push(String::new());
            }
            self.cursor =
                editor_position_from_byte(&self.editor.join("\n"), self.editor.join("\n").len())
                    .unwrap_or((0, 0));
            self.dirty = true;
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.message = format!(
                "Kamikaze removed {deleted} word(s) after {idle_seconds}s idle. Penalties cannot be undone during the challenge."
            );
        } else {
            self.message = "Kamikaze is armed; the isolated draft is empty".into();
        }
        Ok(self.dirty)
    }
    fn pause_challenge_inactivity(&mut self) {
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
    }
    fn expand_text_before_cursor(
        &mut self,
        trigger: &str,
        delimiter: &str,
        record_undo: bool,
    ) -> bool {
        if !self.text_expansion.enabled || !self.extra_cursors.is_empty() {
            return false;
        }
        let original = self.editor.join("\n");
        let cursor = editor_position_byte(&self.editor, self.cursor.0, self.cursor.1);
        let before_cursor = &original[..cursor];
        let trigger_bytes = delimiter.len();
        if trigger_bytes > before_cursor.len()
            || !before_cursor.ends_with(delimiter)
            || (!trigger.is_empty()
                && !self
                    .text_expansion
                    .triggers
                    .iter()
                    .any(|configured| configured == trigger))
        {
            return false;
        }
        let token_end = cursor - trigger_bytes;
        let candidates = self
            .text_expansion
            .mappings
            .iter()
            .filter(|(abbreviation, _)| original[..token_end].ends_with(abbreviation.as_str()))
            .collect::<Vec<_>>();
        let Some((abbreviation, replacement)) = candidates
            .into_iter()
            .max_by_key(|(abbreviation, _)| abbreviation.len())
            .map(|(abbreviation, replacement)| (abbreviation.clone(), replacement.clone()))
        else {
            return false;
        };
        let start = token_end - abbreviation.len();
        if start > 0 {
            let previous = original[..start].chars().next_back();
            if previous.is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
                return false;
            }
        }
        if record_undo {
            self.push_undo();
        }
        let mut updated = original;
        updated.replace_range(start..token_end, &replacement);
        self.editor = updated.split('\n').map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        if let Some((row, column)) =
            editor_position_from_byte(&updated, cursor - abbreviation.len() + replacement.len())
        {
            self.cursor = (row, column);
        }
        self.dirty = true;
        true
    }
    fn insert_text_at_cursor(&mut self, insertion: &str) -> Result<(), Box<dyn std::error::Error>> {
        let original = self.editor.join("\n");
        let at = editor_position_byte(&self.editor, self.cursor.0, self.cursor.1);
        let mut updated = original;
        if !updated.is_char_boundary(at) {
            return Err("cursor is not at a UTF-8 boundary".into());
        }
        self.push_undo();
        updated.insert_str(at, insertion);
        self.editor = updated.lines().map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        if let Some((row, column)) = editor_position_from_byte(&updated, at + insertion.len()) {
            self.cursor = (row, column);
        }
        self.dirty = true;
        Ok(())
    }
    fn insert_text_char(&mut self, ch: char) {
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
        if self.editor_selection.is_none() && !self.extra_cursors.is_empty() {
            let original = self.editor.join("\n");
            let mut cursors = vec![self.cursor];
            cursors.extend(self.extra_cursors.iter().copied());
            let anchors = cursors
                .iter()
                .map(|(row, column)| editor_position_byte(&self.editor, *row, *column))
                .collect::<Vec<_>>();
            let insertion = ch.to_string();
            let unique = anchors.iter().copied().collect::<BTreeSet<_>>();
            let Ok(updated) = story_core::insert_at_anchors(&original, &anchors, &insertion) else {
                return;
            };
            let mapped = cursors
                .iter()
                .zip(anchors.iter())
                .map(|(_, anchor)| {
                    let shift = unique
                        .range(..*anchor)
                        .count()
                        .saturating_mul(insertion.len());
                    editor_position_from_byte(&updated, anchor + shift + insertion.len())
                        .unwrap_or((0, 0))
                })
                .collect::<Vec<_>>();
            self.editor = updated.lines().map(str::to_owned).collect();
            if self.editor.is_empty() {
                self.editor.push(String::new());
            }
            self.cursor = mapped[0];
            self.extra_cursors = mapped.into_iter().skip(1).collect();
            self.dirty = true;
            return;
        }
        self.delete_selection();
        let (row, col) = self.cursor;
        let line = &mut self.editor[row];
        let byte = grapheme_byte(line, col);
        line.insert(byte, ch);
        self.cursor.1 = grapheme_count(&line[..byte + ch.len_utf8()]);
        self.dirty = true;
    }
    fn backspace(&mut self) {
        self.last_yank = None;
        self.record_macro_op(story_core::EditMacroOp::Backspace);
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
        if self.delete_at_all_cursors(true) {
            return;
        }
        if self.delete_selection() {
            return;
        }
        let (row, col) = self.cursor;
        if col > 0 {
            let line = &mut self.editor[row];
            let end = grapheme_byte(line, col);
            let start = grapheme_byte(line, col - 1);
            line.replace_range(start..end, "");
            self.cursor.1 -= 1;
            self.dirty = true;
        } else if row > 0 {
            let tail = self.editor.remove(row);
            self.cursor.0 -= 1;
            self.cursor.1 = grapheme_count(&self.editor[self.cursor.0]);
            self.editor[self.cursor.0].push_str(&tail);
            self.dirty = true;
        }
    }
    fn delete_forward(&mut self) {
        self.last_yank = None;
        self.record_macro_op(story_core::EditMacroOp::DeleteForward);
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
        if self.delete_at_all_cursors(false) {
            return;
        }
        if self.delete_selection() {
            return;
        }
        let (row, col) = self.cursor;
        let count = grapheme_count(&self.editor[row]);
        if col < count {
            let line = &mut self.editor[row];
            let start = grapheme_byte(line, col);
            let end = grapheme_byte(line, col + 1);
            line.replace_range(start..end, "");
            self.dirty = true;
        } else if row + 1 < self.editor.len() {
            let tail = self.editor.remove(row + 1);
            self.editor[row].push_str(&tail);
            self.dirty = true;
        }
    }
    fn paste_text(&mut self, text: &str) {
        let range_start = self
            .selection_bounds()
            .map(|((row, column), _)| editor_position_byte(&self.editor, row, column))
            .unwrap_or_else(|| editor_position_byte(&self.editor, self.cursor.0, self.cursor.1));
        self.delete_selection();
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        self.record_macro_op(story_core::EditMacroOp::Insert(normalized.clone()));
        if self.writing_stats_enabled {
            self.pasted_characters = self
                .pasted_characters
                .saturating_add(normalized.chars().count() as u64);
        }
        for ch in normalized.chars() {
            if ch == '\n' {
                self.split_line()
            } else {
                self.insert_text_char(ch)
            }
        }
        let range_end = editor_position_byte(&self.editor, self.cursor.0, self.cursor.1);
        self.last_yank = Some(LastYank {
            scene_id: self.editing_scene,
            range: range_start..range_end,
            text: normalized,
        });
        self.kill_ring_index = self
            .kill_ring
            .iter()
            .position(|entry| {
                self.last_yank
                    .as_ref()
                    .is_some_and(|last| entry == &last.text)
            })
            .unwrap_or(0);
    }
    fn enter(&mut self) {
        self.last_yank = None;
        self.record_macro_op(story_core::EditMacroOp::Insert("\n".into()));
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
        self.split_line();
        self.expand_text_before_cursor("enter", "\n", false);
        if let Some(challenge) = &mut self.active_challenge {
            challenge.last_activity = Instant::now();
        }
    }
    fn split_line(&mut self) {
        self.delete_selection();
        let (row, col) = self.cursor;
        let line = &mut self.editor[row];
        let byte = grapheme_byte(line, col);
        let tail = line.split_off(byte);
        self.editor.insert(row + 1, tail);
        self.cursor = (row + 1, 0);
        self.dirty = true;
    }
    fn selection_bounds(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.editor_selection?;
        if anchor == self.cursor {
            return None;
        }
        Some(if anchor < self.cursor {
            (anchor, self.cursor)
        } else {
            (self.cursor, anchor)
        })
    }
    fn selected_text(&self) -> Option<String> {
        let ((start_row, start_col), (end_row, end_col)) = self.selection_bounds()?;
        if start_row == end_row {
            return Some(
                self.editor[start_row]
                    .graphemes(true)
                    .skip(start_col)
                    .take(end_col - start_col)
                    .collect(),
            );
        }
        let mut lines = Vec::new();
        lines.push(
            self.editor[start_row]
                .graphemes(true)
                .skip(start_col)
                .collect::<String>(),
        );
        lines.extend(self.editor[start_row + 1..end_row].iter().cloned());
        lines.push(self.editor[end_row].graphemes(true).take(end_col).collect());
        Some(lines.join("\n"))
    }
    fn delete_selection(&mut self) -> bool {
        let Some(((start_row, start_col), (end_row, end_col))) = self.selection_bounds() else {
            return false;
        };
        let prefix: String = self.editor[start_row]
            .graphemes(true)
            .take(start_col)
            .collect();
        let suffix: String = self.editor[end_row].graphemes(true).skip(end_col).collect();
        self.editor[start_row] = format!("{prefix}{suffix}");
        if end_row > start_row {
            self.editor.drain(start_row + 1..=end_row);
        }
        self.cursor = (start_row, start_col);
        self.editor_selection = None;
        self.dirty = true;
        true
    }
    fn move_cursor(&mut self, row: usize, col: usize, selecting: bool) {
        if selecting {
            self.editor_selection.get_or_insert(self.cursor);
        } else {
            self.editor_selection = None;
        }
        let row = row.min(self.editor.len().saturating_sub(1));
        self.cursor = (row, col.min(grapheme_count(&self.editor[row])));
    }
    fn move_editor_cursors(&mut self, motion: CursorMotion, selecting: bool) {
        self.last_yank = None;
        let move_position = |(row, column): (usize, usize)| -> (usize, usize) {
            match motion {
                CursorMotion::Left if column > 0 => (row, column - 1),
                CursorMotion::Left if row > 0 => (row - 1, grapheme_count(&self.editor[row - 1])),
                CursorMotion::Right if column < grapheme_count(&self.editor[row]) => {
                    (row, column + 1)
                }
                CursorMotion::Right if row + 1 < self.editor.len() => (row + 1, 0),
                CursorMotion::Up => (row.saturating_sub(1), column),
                CursorMotion::Down => ((row + 1).min(self.editor.len() - 1), column),
                CursorMotion::Home => (row, 0),
                CursorMotion::End => (row, grapheme_count(&self.editor[row])),
                _ => (row, column),
            }
        };
        let new_primary = move_position(self.cursor);
        let new_extras = self
            .extra_cursors
            .iter()
            .copied()
            .map(move_position)
            .collect();
        self.move_cursor(new_primary.0, new_primary.1, selecting);
        self.extra_cursors = new_extras;
        self.extra_cursors
            .retain(|position| *position != self.cursor);
        self.extra_cursors.sort_unstable();
        self.extra_cursors.dedup();
    }

    fn delete_at_all_cursors(&mut self, backward: bool) -> bool {
        if self.extra_cursors.is_empty() || self.editor_selection.is_some() {
            return false;
        }
        let cursors = std::iter::once(self.cursor)
            .chain(self.extra_cursors.iter().copied())
            .collect::<Vec<_>>();
        let mut ranges = Vec::new();
        for (row, column) in &cursors {
            let base = editor_position_byte(&self.editor, *row, 0);
            let count = grapheme_count(&self.editor[*row]);
            if backward && *column > 0 {
                let line = &self.editor[*row];
                ranges.push((
                    base + grapheme_byte(line, column - 1),
                    base + grapheme_byte(line, *column),
                ));
            } else if backward && *row > 0 {
                ranges.push((base - 1, base));
            } else if !backward && *column < count {
                let line = &self.editor[*row];
                ranges.push((
                    base + grapheme_byte(line, *column),
                    base + grapheme_byte(line, column + 1),
                ));
            } else if !backward && *row + 1 < self.editor.len() {
                let end = base + self.editor[*row].len();
                ranges.push((end, end + 1));
            }
        }
        ranges.sort_unstable();
        ranges.dedup();
        if ranges.is_empty() {
            return false;
        }
        self.push_undo();
        let source = self.editor.join("\n");
        let original_anchors = cursors
            .iter()
            .map(|(row, column)| editor_position_byte(&self.editor, *row, *column))
            .collect::<Vec<_>>();
        let mut updated = source;
        for (start, end) in ranges.iter().rev() {
            updated.replace_range(*start..*end, "");
        }
        let mapped = original_anchors
            .into_iter()
            .map(|anchor| {
                let removed = ranges
                    .iter()
                    .filter(|(_, end)| *end <= anchor)
                    .map(|(start, end)| end - start)
                    .sum::<usize>();
                editor_position_from_byte(&updated, anchor.saturating_sub(removed))
                    .unwrap_or((0, 0))
            })
            .collect::<Vec<_>>();
        self.editor = updated.lines().map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        self.cursor = mapped[0];
        self.extra_cursors = mapped
            .into_iter()
            .skip(1)
            .filter(|position| *position != self.cursor)
            .collect();
        self.extra_cursors.sort_unstable();
        self.extra_cursors.dedup();
        self.dirty = true;
        true
    }
    fn push_undo(&mut self) {
        const MAX_UNDO_STEPS: usize = 100;
        self.undo_stack.push(EditorSnapshot {
            lines: self.editor.clone(),
            cursor: self.cursor,
            selection: self.editor_selection,
        });
        if self.undo_stack.len() > MAX_UNDO_STEPS {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
    fn undo_edit(&mut self) {
        if let Some(previous) = self.undo_stack.pop() {
            self.redo_stack.push(EditorSnapshot {
                lines: self.editor.clone(),
                cursor: self.cursor,
                selection: self.editor_selection,
            });
            self.editor = previous.lines;
            self.cursor = previous.cursor;
            self.editor_selection = previous.selection;
            self.dirty = true;
            self.message = "Edit undone".into();
        } else {
            self.message = "No editor changes to undo".into();
        }
    }
    fn redo_edit(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(EditorSnapshot {
                lines: self.editor.clone(),
                cursor: self.cursor,
                selection: self.editor_selection,
            });
            self.editor = next.lines;
            self.cursor = next.cursor;
            self.editor_selection = next.selection;
            self.dirty = true;
            self.message = "Edit redone".into();
        } else {
            self.message = "No editor changes to redo".into();
        }
    }
    fn create_reading_selection(
        &mut self,
        kind: &str,
        body: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let id = self
            .reading_document
            .ok_or("open a reading document first")?;
        let start = self
            .reading_selection_start
            .ok_or("press v at the passage start first")?;
        let a = start.min(self.reading_offset) as usize;
        let b = start.max(self.reading_offset) as usize;
        if a == b {
            return Err("move the reading cursor to select a non-empty passage".into());
        }
        let quote = self
            .reading_text
            .get(a..b)
            .ok_or("selection does not align with UTF-8 text boundaries")?
            .to_owned();
        self.app.create_reading_annotation(
            id,
            &story_core::ReadingAnnotationDraft {
                kind: kind.into(),
                start_byte: a as u64,
                end_byte: b as u64,
                quote,
                body: body.into(),
                prefix_context: String::new(),
                suffix_context: String::new(),
            },
        )?;
        self.reading_selection_start = None;
        self.message = format!("{kind} saved for selected passage");
        Ok(())
    }

    fn restore_selected_revision_hunk(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.dirty {
            return Err("save the current manuscript buffer before restoring a revision".into());
        }
        let scene = self.revision_scene.ok_or("open a snapshot diff first")?;
        let snapshot = self.revision_diff.ok_or("open a snapshot diff first")?;
        let diff = self.app.diff_snapshot(scene, snapshot)?;
        let hunk = revision_hunks(&diff)
            .get(self.revision_hunk)
            .cloned()
            .ok_or("the selected snapshot has no changed hunk")?;
        let (_, expected_hash) = self.app.load_scene_text(scene)?;
        self.app
            .restore_snapshot_lines(scene, snapshot, &expected_hash, hunk.selection)?;
        if self.editing_scene == Some(scene) {
            let (text, hash) = self.app.load_scene_text(scene)?;
            self.editor = text.split('\n').map(str::to_owned).collect();
            self.saved_hash = hash;
            self.cursor = (0, 0);
            self.editor_selection = None;
        }
        self.revision_diff = None;
        self.revision_hunk = 0;
        self.refresh()?;
        self.message = "Selected revision hunk restored; previous text was snapshotted".into();
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: writers-ide-tui <project-path>")?;
    let project_path = std::path::PathBuf::from(path);
    let function_bindings = load_bindings(&project_path)?;
    let command_bindings = load_command_bindings(&project_path)?;
    let mut app = WriterApp::open_project(&project_path)?;
    app.recover_interrupted_writing_challenges()?;
    let mut state = AppState::new(app, function_bindings);
    state.command_bindings = command_bindings;
    state.restore_session()?;
    state.refresh()?;
    let mut terminal = setup()?;
    let result = run(&mut terminal, &mut state);
    restore(&mut terminal)?;
    result
}
fn setup() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(
        out,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    )?;
    Terminal::new(CrosstermBackend::new(out))
}
fn restore(t: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(
        t.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
    )?;
    t.show_cursor()
}
fn run(
    t: &mut Terminal<CrosstermBackend<io::Stdout>>,
    s: &mut AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut autosave_deadline: Option<Instant> = None;
    let mut auto_snapshot_check = Instant::now();
    loop {
        if auto_snapshot_check.elapsed() >= Duration::from_secs(10) {
            auto_snapshot_check = Instant::now();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            match s.app.git_auto_snapshot_if_due(now) {
                Ok(Some(commit)) => {
                    s.message = format!("Automatic project snapshot created: {commit}")
                }
                Ok(None) => {}
                Err(error) => s.message = format!("Automatic project snapshot failed: {error}"),
            }
        }
        t.draw(|f| draw(f, s))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(k) => {
                    let should_quit = handle(s, k)?;
                    if s.active_challenge.as_ref().is_some_and(|challenge| {
                        story_core::count_words(&s.editor.join("\n")) >= challenge.target_words
                    }) {
                        s.finish_active_challenge("success")?;
                    }
                    if s.active_challenge.is_some() {
                        s.persist_challenge_draft()?;
                    }
                    autosave_deadline = if s.dirty {
                        Some(Instant::now() + Duration::from_secs(2))
                    } else {
                        None
                    };
                    if let Err(error) = s.save_session() {
                        s.message = format!("Session state could not be saved: {error}");
                    }
                    if should_quit {
                        break;
                    }
                }
                Event::Paste(text) if s.workspace == Workspace::Editor => {
                    s.push_undo();
                    s.paste_text(&text);
                    if s.active_challenge.as_ref().is_some_and(|challenge| {
                        story_core::count_words(&s.editor.join("\n")) >= challenge.target_words
                    }) {
                        s.finish_active_challenge("success")?;
                    }
                    if s.active_challenge.is_some() {
                        s.persist_challenge_draft()?;
                    }
                    autosave_deadline = Some(Instant::now() + Duration::from_secs(2));
                    if let Err(error) = s.save_session() {
                        s.message = format!("Session state could not be saved: {error}");
                    }
                }
                Event::Resize(_, _) => s.pause_challenge_inactivity(),
                _ => {}
            }
        }
        if s.tick_challenge()? && s.dirty {
            autosave_deadline = Some(Instant::now() + Duration::from_secs(2));
        }
        if autosave_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            autosave_deadline = None;
            s.save()?;
            if s.dirty {
                s.message = "Autosave found a conflict; your text remains open and unsaved. Resolve it before saving again.".into();
            } else {
                s.message = "Autosaved".into();
                if let Err(error) = s.save_session() {
                    s.message =
                        format!("Saved manuscript; session state could not be saved: {error}");
                }
            }
        }
    }
    if s.active_challenge.is_some() {
        s.finish_active_challenge("quit")?;
    }
    Ok(())
}
fn editor_grapheme_is_mentioned(byte_offset: u64, byte_len: u64, ranges: &[(u64, u64)]) -> bool {
    let Some(end) = byte_offset.checked_add(byte_len) else {
        return false;
    };
    ranges
        .iter()
        .any(|(start, finish)| *start < end && *finish > byte_offset)
}

fn draw(f: &mut ratatui::Frame<'_>, s: &mut AppState) {
    let area = f.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(area);
    s.width = area.width as usize;
    let project = s
        .app
        .get_project()
        .map(|p| p.title)
        .unwrap_or_else(|_| "Project".into());
    let dirty = if s.dirty { "● unsaved" } else { "saved" };
    let head = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {project}  |  {}  |  {dirty} ", s.workspace.title()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("F1-F9, F11-F14 workspaces  Ctrl+/ or F10 menu  Ctrl-G focus  Ctrl+arrows split  ? help  : commands  / search  Ctrl-S save  q quit"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(head, rows[0]);
    let body = if pane_direction(s.width) == Direction::Vertical {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(s.vertical_split_ratio),
                Constraint::Percentage(100 - s.vertical_split_ratio),
            ])
            .split(rows[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(s.horizontal_split_ratio),
                Constraint::Percentage(100 - s.horizontal_split_ratio),
            ])
            .split(rows[1])
    };
    if s.workspace == Workspace::Editor {
        if !s.editor_focus_mode {
            let list = List::new(
                s.scenes
                    .iter()
                    .map(|(_, title, status, words)| {
                        ListItem::new(format!("{title}  [{}] {words}", status.as_str()))
                    })
                    .collect::<Vec<_>>(),
            )
            .block(Block::default().title("Scenes").borders(Borders::ALL));
            let mut state = ListState::default().with_selected(Some(s.selected));
            f.render_stateful_widget(list, body[0], &mut state);
        }
        let selection = s.selection_bounds();
        let mention_ranges = (!s.dirty)
            .then_some(s.editing_scene)
            .flatten()
            .and_then(|scene| s.app.list_scene_mentions(scene).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|mention| matches!(mention.state.as_str(), "detected" | "confirmed"))
            .map(|mention| (mention.start_byte, mention.end_byte))
            .collect::<Vec<_>>();
        let mut line_start_byte = s
            .editor
            .iter()
            .take(s.scroll)
            .map(|line| line.len() as u64 + 1)
            .sum::<u64>();
        let content = s
            .editor
            .iter()
            .enumerate()
            .skip(s.scroll)
            .map(|(row, line)| {
                let prefix = if row == s.cursor.0 { "> " } else { "  " };
                let mut spans = vec![Span::raw(prefix)];
                let mut byte_offset = line_start_byte;
                for (col, ch) in line.graphemes(true).enumerate() {
                    let selected = selection.is_some_and(|((sr, sc), (er, ec))| {
                        (row > sr || (row == sr && col >= sc))
                            && (row < er || (row == er && col < ec))
                    });
                    let decorated =
                        editor_grapheme_is_mentioned(byte_offset, ch.len() as u64, &mention_ranges);
                    if selected {
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(Color::White).bg(Color::Blue),
                        ));
                    } else if decorated {
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                    } else {
                        spans.push(Span::raw(ch.to_string()));
                    }
                    byte_offset += ch.len() as u64;
                }
                line_start_byte = byte_offset + 1;
                Line::from(spans)
            })
            .collect::<Vec<_>>();
        let editor_title = s
            .assembly_draft
            .map(|id| format!("Assembly Draft {id} · stable scene markers required"))
            .unwrap_or_else(|| format!("Editor — {}", s.selected_scene()));
        let editor = Paragraph::new(content).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(if s.reference_focused {
                    format!("{editor_title} · reference focused")
                } else {
                    editor_title
                })
                .borders(Borders::ALL),
        );
        let editor_and_reference = if s.editor_focus_mode {
            rows[1]
        } else {
            body[1]
        };
        if !s.reference_panes.is_empty() {
            let panes = Layout::default()
                .direction(pane_direction(s.width))
                .constraints([
                    Constraint::Percentage(s.horizontal_split_ratio),
                    Constraint::Percentage(100 - s.horizontal_split_ratio),
                ])
                .split(editor_and_reference);
            f.render_widget(editor, panes[0]);
            let reference_areas = reference_pane_layout(panes[1], s.reference_panes.len(), s.width);
            for (index, (reference, area)) in
                s.reference_panes.iter().zip(reference_areas).enumerate()
            {
                let reference = Paragraph::new(
                    reference
                        .content
                        .lines()
                        .skip(reference.scroll)
                        .map(Line::from)
                        .collect::<Vec<_>>(),
                )
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(format!(
                            "{} · {}{}",
                            reference.title,
                            index + 1,
                            if index == s.active_reference {
                                if s.reference_focused {
                                    " · active · Ctrl-Tab focus"
                                } else {
                                    " · selected · Ctrl-Tab focus"
                                }
                            } else {
                                ""
                            }
                        ))
                        .borders(Borders::ALL),
                );
                f.render_widget(reference, area);
            }
        } else {
            f.render_widget(editor, editor_and_reference);
        }
    } else if s.workspace == Workspace::Reading {
        let items = workspace_items(s)
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>();
        let mut list_state =
            ListState::default().with_selected(Some(s.selected.min(items.len().saturating_sub(1))));
        f.render_stateful_widget(
            List::new(items).highlight_symbol("▸ ").block(
                Block::default()
                    .title("Reference Library · h/l changes pane")
                    .borders(Borders::ALL),
            ),
            body[0],
            &mut list_state,
        );
        let text = if s.reading_text.is_empty() {
            "Choose a reference and press Enter. :read-import PATH imports a read-only copy."
                .to_owned()
        } else {
            s.reading_text.clone()
        };
        let notes = s
            .reading_document
            .and_then(|id| s.app.list_reading_annotations(id).ok())
            .unwrap_or_default();
        let content_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(body[1]);
        f.render_widget(
            Paragraph::new(reading_text_display(&text, &notes))
                .scroll((s.scroll as u16, 0))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title("Reading material · source remains unchanged")
                        .borders(Borders::ALL),
                ),
            content_area[0],
        );
        let annotation_text = notes
            .iter()
            .map(|a| format!("{}: {} [{}]", a.kind, a.quote, a.state))
            .collect::<Vec<_>>()
            .join(" · ");
        f.render_widget(
            Paragraph::new(annotation_text)
                .block(Block::default().title("Annotations").borders(Borders::ALL)),
            content_area[1],
        );
    } else {
        let items = workspace_items(s)
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>();
        let list_selection = if s.workspace == Workspace::Revisions {
            s.revision_index
        } else if s.workspace == Workspace::Research && s.proposal_review_active {
            s.proposal_review_index
        } else {
            s.selected
        };
        let mut state = ListState::default()
            .with_selected(Some(list_selection.min(items.len().saturating_sub(1))));
        let list = List::new(items)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .title(s.workspace.title())
                    .borders(Borders::ALL),
            );
        f.render_stateful_widget(list, body[0], &mut state);
        if s.workspace == Workspace::Revisions
            && let Some(snapshot_id) = s.revision_diff
        {
            let diff = s
                .revision_scene
                .and_then(|scene| s.app.diff_snapshot(scene, snapshot_id).ok());
            if let Some(diff) = diff {
                let hunks = revision_hunks(&diff);
                let selected_hunk = s.revision_hunk.min(hunks.len().saturating_sub(1));
                let lines = diff
                    .iter()
                    .enumerate()
                    .map(|(index, line)| {
                        let marker = match line.kind {
                            story_storage::SnapshotLineKind::Equal => ' ',
                            story_storage::SnapshotLineKind::Added => '+',
                            story_storage::SnapshotLineKind::Removed => '-',
                        };
                        let in_hunk = hunks
                            .get(selected_hunk)
                            .is_some_and(|hunk| hunk.diff_lines.contains(&index));
                        let style = if in_hunk {
                            Style::default().fg(Color::Black).bg(Color::Yellow)
                        } else {
                            match line.kind {
                                story_storage::SnapshotLineKind::Equal => Style::default(),
                                story_storage::SnapshotLineKind::Added => {
                                    Style::default().fg(Color::Green)
                                }
                                story_storage::SnapshotLineKind::Removed => {
                                    Style::default().fg(Color::Red)
                                }
                            }
                        };
                        Line::from(Span::styled(
                            format!("{marker}{}", line.text.trim_end()),
                            style,
                        ))
                    })
                    .collect::<Vec<_>>();
                let snapshot_label = s
                    .revision_scene
                    .and_then(|scene| {
                        s.app
                            .list_snapshots(scene)
                            .ok()?
                            .into_iter()
                            .find(|snapshot| snapshot.id == snapshot_id)
                            .map(|snapshot| snapshot.label)
                    })
                    .unwrap_or_else(|| snapshot_id.to_string());
                let diff_title = if hunks.is_empty() {
                    format!("{snapshot_label} · no changes · Esc close")
                } else {
                    format!(
                        "{snapshot_label} · hunk {}/{} · [ ] select · r restore · Esc close",
                        selected_hunk + 1,
                        hunks.len()
                    )
                };
                f.render_widget(
                    Paragraph::new(lines)
                        .scroll((s.scroll as u16, 0))
                        .wrap(Wrap { trim: false })
                        .block(Block::default().title(diff_title).borders(Borders::ALL)),
                    body[1],
                );
            } else {
                f.render_widget(
                    Paragraph::new("Unable to load snapshot diff").block(
                        Block::default()
                            .title("Revision diff")
                            .borders(Borders::ALL),
                    ),
                    body[1],
                );
            }
        } else if s.workspace == Workspace::PlanningBoards
            && let Some(board) = s.planning_board_id
        {
            let items = s.app.list_planning_board_items(board).unwrap_or_default();
            let (x_bounds, y_bounds) = planning_board_bounds(&items);
            let selected = s.selected;
            let canvas = Canvas::default()
                .x_bounds(x_bounds)
                .y_bounds(y_bounds)
                .paint(|context| {
                    for (index, item) in items.iter().enumerate() {
                        let short_id = item.entity_id.chars().take(6).collect::<String>();
                        let label = if item.note.trim().is_empty() {
                            format!("{}:{short_id}", item.entity_kind)
                        } else {
                            format!(
                                "{}:{}",
                                item.entity_kind,
                                item.note.chars().take(12).collect::<String>()
                            )
                        };
                        let color = if index == selected {
                            Color::Yellow
                        } else {
                            Color::Cyan
                        };
                        context.draw(&Rectangle {
                            x: item.x,
                            y: item.y,
                            width: item.width,
                            height: item.height,
                            color,
                        });
                        context.print(
                            item.x,
                            item.y,
                            Span::styled(label, Style::default().fg(color)),
                        );
                    }
                })
                .block(
                    Block::default()
                        .title("Planning Board · Ctrl+arrows move selected card")
                        .borders(Borders::ALL),
                );
            f.render_widget(canvas, body[1]);
        } else {
            let detail = if s.workspace == Workspace::Research && s.proposal_review_active {
                format!(
                    "{}\n\n{}\n\n{}",
                    workspace_items(s)
                        .get(s.proposal_review_index)
                        .map(String::as_str)
                        .unwrap_or("No proposal claim selected"),
                    "Use :approve PROPOSAL-PATH CLAIM-INDEX or :dismiss PROPOSAL-PATH CLAIM-INDEX to record a decision.",
                    s.message
                )
            } else if matches!(
                s.workspace,
                Workspace::StoryBible | Workspace::Issues | Workspace::Research
            ) {
                let selected = workspace_items(s).get(s.selected).cloned();
                format!(
                    "{}\n\n{}\n\n{}",
                    selected.as_deref().unwrap_or("No record selected"),
                    "Enter inspects this record; relationships use stable entity IDs.",
                    s.message
                )
            } else if let Some((id, title, status, words)) = s.scenes.get(s.selected) {
                format!(
                    "Scene: {title}\nID: {id}\nStatus: {}\nWords: {words}\n\nEnter opens the manuscript editor.\n\n{}",
                    status.as_str(),
                    s.message
                )
            } else {
                format!("{}\n\n{}", s.workspace.title(), s.message)
            };
            f.render_widget(
                Paragraph::new(detail).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .title("Details / split pane")
                        .borders(Borders::ALL),
                ),
                body[1],
            );
        }
    }
    let footer = if let Some(mode) = s.mode {
        format!(" {mode} {}_", s.prompt)
    } else {
        let challenge = s
            .active_challenge
            .as_ref()
            .map(|challenge| challenge_footer(challenge, &s.editor))
            .unwrap_or_default();
        format!(
            " {}   {}   Enter open | j/k or ↑/↓ select | Tab next | Backspace back | {}",
            s.message,
            challenge,
            if s.dirty { "modified" } else { "clean" }
        )
    };
    f.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::TOP)),
        rows[2],
    );
    if s.menu.is_some() {
        draw_menu_overlay(f, s);
    }
}

struct TerminalSpellingFinding {
    entity_kind: String,
    title: String,
    candidates: BTreeMap<String, u32>,
}

fn spellcheck_terminal_project(
    app: &WriterApp,
    language: &str,
    dictionary: Option<&str>,
    executable: &str,
) -> Result<Vec<TerminalSpellingFinding>, Box<dyn std::error::Error>> {
    if language.trim().is_empty() {
        return Err("spellcheck language tag cannot be empty".into());
    }
    let root = app.project_root().canonicalize()?;
    let allowed = if let Some(relative) = dictionary {
        let path = root.join(relative).canonicalize()?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err("personal dictionary must be a regular project-local file".into());
        }
        story_document::parse_personal_dictionary(&fs::read_to_string(path)?)?
    } else {
        std::collections::BTreeSet::new()
    };
    let input = app.processing_input()?;
    let prose = input
        .scenes
        .into_iter()
        .map(|scene| ("scene".to_owned(), scene.title, scene.text))
        .chain(
            input
                .content_units
                .into_iter()
                .map(|unit| ("content-unit".to_owned(), unit.title, unit.text)),
        );
    let mut findings = Vec::new();
    for (kind, title, text) in prose {
        let mut child = Command::new(executable)
            .arg(format!("--lang={language}"))
            .arg("list")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        use std::io::Write;
        child
            .stdin
            .take()
            .ok_or("Aspell stdin unavailable")?
            .write_all(text.as_bytes())?;
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(format!(
                "Aspell failed for {kind} {title}: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let mut candidates = BTreeMap::new();
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
            findings.push(TerminalSpellingFinding {
                entity_kind: kind,
                title,
                candidates,
            });
        }
    }
    Ok(findings)
}

fn planning_board_bounds(items: &[story_storage::PlanningBoardItemRecord]) -> ([f64; 2], [f64; 2]) {
    fn bounds(values: impl Iterator<Item = f64>) -> [f64; 2] {
        let values = values.collect::<Vec<_>>();
        let Some(minimum) = values.iter().copied().reduce(f64::min) else {
            return [0.0, 10.0];
        };
        let maximum = values.iter().copied().reduce(f64::max).unwrap_or(minimum);
        let padding = ((maximum - minimum) * 0.1).max(1.0);
        [minimum - padding, maximum + padding]
    }
    (
        bounds(items.iter().flat_map(|item| [item.x, item.x + item.width])),
        bounds(items.iter().map(|item| item.y + item.height)),
    )
}

fn format_conflict_review(
    conflicts: &[story_import::MergeConflict],
    terminal_width: usize,
) -> String {
    let column_width = (terminal_width / 6).clamp(10, 28);
    let format_cell = |value: &str| {
        let text = value
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(column_width.saturating_sub(1))
            .collect::<String>();
        format!("{text:<width$}", width = column_width)
    };
    let mut output = String::new();
    for (index, conflict) in conflicts.iter().enumerate() {
        output.push_str(&format!(
            "Conflict {} · base lines {}–{}\n",
            index + 1,
            conflict.base_start_line + 1,
            conflict.base_end_line.max(conflict.base_start_line + 1)
        ));
        output.push_str(&format!(
            "{} │ {} │ {}\n",
            format_cell("BASE"),
            format_cell("LOCAL"),
            format_cell("REVIEWED")
        ));
        let columns = [
            conflict.base.lines().collect::<Vec<_>>(),
            conflict.local.lines().collect::<Vec<_>>(),
            conflict.external.lines().collect::<Vec<_>>(),
        ];
        let row_count = columns.iter().map(Vec::len).max().unwrap_or_default();
        for row in 0..row_count {
            let cells = columns
                .iter()
                .map(|column| format_cell(column.get(row).copied().unwrap_or("")))
                .collect::<Vec<_>>();
            output.push_str(&format!("{} │ {} │ {}\n", cells[0], cells[1], cells[2]));
        }
        output.push('\n');
    }
    output
}

fn format_character_reference(character: &story_core::Character) -> String {
    let mut lines = vec![format!("# {}", character.full_name)];
    if character.preferred_name != character.full_name {
        lines.push(format!("Preferred name: {}", character.preferred_name));
    }
    if !character.aliases.is_empty() {
        lines.push(format!("Aliases: {}", character.aliases.join(", ")));
    }
    for (label, value) in [
        ("Status", character.status.as_str()),
        ("Description", character.description.as_str()),
        ("Background", character.background.as_str()),
        ("Goals", character.goals.as_str()),
        ("Motivations", character.motivations.as_str()),
        ("Secrets", character.secrets.as_str()),
        ("Internal conflict", character.internal_conflict.as_str()),
        ("External conflict", character.external_conflict.as_str()),
    ] {
        if !value.trim().is_empty() {
            lines.push(format!("\n## {label}\n{value}"));
        }
    }
    lines.join("\n")
}

fn format_location_reference(location: &story_core::Location) -> String {
    let mut lines = vec![format!("# {}", location.name)];
    if let Some(parent) = location.parent {
        lines.push(format!("Parent location: {parent}"));
    }
    if !location.description.trim().is_empty() {
        lines.push(format!("\n## Description\n{}", location.description));
    }
    lines.join("\n")
}

fn format_planning_board_markdown(
    title: &str,
    items: &[story_storage::PlanningBoardItemRecord],
) -> String {
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
            escape(&item.note),
        ));
    }
    output
}

fn challenge_footer(challenge: &ActiveWritingChallenge, editor: &[String]) -> String {
    let current_words = story_core::count_words(&editor.join("\n"));
    let written = current_words.saturating_sub(challenge.initial_words);
    let remaining = challenge
        .deadline
        .saturating_duration_since(Instant::now())
        .as_secs();
    if challenge.mode == "kamikaze" {
        let idle_remaining = challenge
            .kamikaze_idle_seconds
            .saturating_sub(challenge.last_activity.elapsed().as_secs());
        format!(
            "Challenge {remaining}s · {written}/{} words · {} grace pause(s) · kamikaze in {idle_remaining}s",
            challenge.target_words, challenge.grace_pauses_remaining
        )
    } else {
        format!(
            "Challenge {remaining}s · {written}/{} words · {} mode",
            challenge.target_words, challenge.mode
        )
    }
}

fn remove_last_words(source: &str, count: u32) -> String {
    let graphemes = source.graphemes(true).collect::<Vec<_>>();
    let mut start = graphemes.len();
    for _ in 0..count {
        while start > 0 && graphemes[start - 1].chars().all(char::is_whitespace) {
            start -= 1;
        }
        let word_end = start;
        while start > 0 && !graphemes[start - 1].chars().all(char::is_whitespace) {
            start -= 1;
        }
        if start == word_end {
            break;
        }
    }
    let byte_start = graphemes.iter().take(start).map(|g| g.len()).sum::<usize>();
    source[..byte_start].to_owned()
}

fn render_challenge_taunt(template: &str, words: u32, deleted: u32) -> String {
    template
        .replace("{words}", &words.to_string())
        .replace("{deleted}", &deleted.to_string())
}

fn parse_challenge_command(
    command: &str,
) -> Result<Option<ChallengeStart>, Box<dyn std::error::Error>> {
    let parsed = if let Some(rest) = command.strip_prefix("challenge-start ") {
        let mut parts = rest.split_whitespace();
        let target = parts
            .next()
            .ok_or("use challenge-start TARGET_WORDS DURATION_SECONDS [pressure|kamikaze]")?
            .parse::<u32>()?;
        let seconds = parts
            .next()
            .ok_or("missing challenge duration")?
            .parse::<u32>()?;
        let tail = parts.collect::<Vec<_>>();
        let (mode, destination) = parse_challenge_tail(&tail)?;
        (target, seconds, mode, destination)
    } else if let Some(rest) = command.strip_prefix("challenge-preset ") {
        let mut parts = rest.split_whitespace();
        let preset = parts
            .next()
            .ok_or("use challenge-preset short|standard|long [pressure|kamikaze]")?;
        let tail = parts.collect::<Vec<_>>();
        let (mode, destination) = parse_challenge_tail(&tail)?;
        let (target, seconds) = match preset {
            "short" => (250, 5 * 60),
            "standard" => (500, 10 * 60),
            "long" => (1_000, 30 * 60),
            _ => return Err("unknown challenge preset".into()),
        };
        (target, seconds, mode, destination)
    } else {
        return Ok(None);
    };
    if parsed.0 == 0 || parsed.1 == 0 {
        return Err(
            "challenge requires positive target/duration and mode pressure|kamikaze".into(),
        );
    }
    Ok(Some(ChallengeStart {
        target_words: parsed.0,
        duration_seconds: parsed.1,
        mode: parsed.2,
        destination_scene: parsed.3,
    }))
}

fn parse_challenge_tail(
    tail: &[&str],
) -> Result<(String, Option<SceneId>), Box<dyn std::error::Error>> {
    let mut index = 0;
    let mode = if tail
        .first()
        .is_some_and(|mode| matches!(*mode, "pressure" | "steady" | "kamikaze"))
    {
        index = 1;
        tail[0]
    } else {
        "pressure"
    };
    let destination_argument = tail.get(index);
    if tail.len() > index + usize::from(destination_argument.is_some()) {
        return Err("too many challenge options".into());
    }
    let destination = destination_argument
        .map(|item| {
            let id = item
                .strip_prefix("scene=")
                .ok_or("challenge option must use scene=UUID")?;
            Ok::<SceneId, Box<dyn std::error::Error>>(SceneId(uuid::Uuid::parse_str(id)?))
        })
        .transpose()?;
    Ok((mode.to_owned(), destination))
}

fn draw_menu_overlay(f: &mut ratatui::Frame<'_>, s: &AppState) {
    let area = f.area();
    let width = area.width.clamp(40, 86);
    let height = area.height.clamp(8, 24);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Main Menu · Ctrl+/ or F10 · Esc close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(10)])
        .split(inner);
    let menu = s.menu.unwrap_or(MenuState {
        top: 0,
        selected: 0,
    });
    let roots = MENU_CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            ListItem::new(format!(
                "{}{}",
                if i == menu.top { "▸ " } else { "  " },
                name
            ))
        })
        .collect::<Vec<_>>();
    f.render_stateful_widget(
        List::new(roots)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .block(Block::default().title("Menu").borders(Borders::RIGHT)),
        panes[0],
        &mut ListState::default().with_selected(Some(menu.top)),
    );
    let commands = s.menu_commands();
    let items = if commands.is_empty() {
        vec![ListItem::new("No commands available yet").style(Style::default().fg(Color::DarkGray))]
    } else {
        commands
            .iter()
            .map(|spec| {
                let disabled = s.command_disabled_reason(*spec);
                let mark = spec
                    .mnemonic
                    .map(|key| format!(" [{key}]"))
                    .unwrap_or_default();
                let args = if spec.takes_arguments { " …" } else { "" };
                let label = if let Some(reason) = disabled {
                    format!("{}{}{} — unavailable: {}", spec.label, mark, args, reason)
                } else {
                    format!("{}{}{} — {}", spec.label, mark, args, spec.description)
                };
                ListItem::new(label).style(if disabled.is_some() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                })
            })
            .collect()
    };
    let selected = if commands.is_empty() {
        None
    } else {
        Some(menu.selected.min(commands.len() - 1))
    };
    f.render_stateful_widget(
        List::new(items)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
            .block(
                Block::default()
                    .title(MENU_CATEGORIES[menu.top])
                    .borders(Borders::LEFT),
            ),
        panes[1],
        &mut ListState::default().with_selected(selected),
    );
}
fn pane_direction(width: usize) -> Direction {
    if width < 100 {
        Direction::Vertical
    } else {
        Direction::Horizontal
    }
}

fn reference_pane_layout(area: Rect, count: usize, terminal_width: usize) -> Vec<Rect> {
    let count = count.max(1);
    Layout::default()
        .direction(if terminal_width >= 160 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints(vec![Constraint::Ratio(1, count as u32); count])
        .split(area)
        .to_vec()
}

fn default_split_ratio() -> u16 {
    40
}

fn atomic_export(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    io::Write::write_all(&mut temporary, bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod assembly_buffer_tests {
    use super::*;
    #[test]
    fn continuous_assembly_buffer_round_trips_scene_text_and_rejects_marker_damage() {
        let draft_id = story_core::AssemblyDraftId::new();
        let first = SceneId::new();
        let second = SceneId::new();
        let draft = story_core::AssemblyDraft {
            id: draft_id,
            assembly_id: story_core::AssemblyId::new(),
            project_id: story_core::ProjectId::new(),
            created_at: String::new(),
            items: vec![
                story_core::AssemblyDraftItem {
                    scene_id: first,
                    expected_hash: "a".into(),
                    text: "Opening line\nSecond line".into(),
                },
                story_core::AssemblyDraftItem {
                    scene_id: second,
                    expected_hash: "b".into(),
                    text: String::new(),
                },
            ],
        };
        let rendered = format_assembly_draft(&draft);
        assert_eq!(
            parse_assembly_draft(&draft, &rendered).unwrap(),
            vec![
                (first, "Opening line\nSecond line".into()),
                (second, String::new())
            ]
        );
        let edited = rendered.replace("Opening line", "Revised opening");
        assert_eq!(
            parse_assembly_draft(&draft, &edited).unwrap()[0].1,
            "Revised opening\nSecond line"
        );
        assert!(
            parse_assembly_draft(
                &draft,
                &rendered.replace(&assembly_scene_marker(draft_id, first), "removed marker")
            )
            .is_err()
        );
    }
}

fn assembly_scene_marker(draft: story_core::AssemblyDraftId, scene: SceneId) -> String {
    format!("<!-- WI-ASSEMBLY-DRAFT:{draft}:SCENE:{scene} -->")
}

fn format_assembly_draft(draft: &story_core::AssemblyDraft) -> String {
    draft
        .items
        .iter()
        .map(|item| {
            format!(
                "{}\n{}",
                assembly_scene_marker(draft.id, item.scene_id),
                item.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn parse_assembly_draft(
    draft: &story_core::AssemblyDraft,
    text: &str,
) -> Result<Vec<(SceneId, String)>, Box<dyn std::error::Error>> {
    let lines = text.split('\n').collect::<Vec<_>>();
    let markers = draft
        .items
        .iter()
        .map(|item| assembly_scene_marker(draft.id, item.scene_id))
        .collect::<Vec<_>>();
    let mut result = Vec::with_capacity(draft.items.len());
    let mut cursor = 0;
    for (index, item) in draft.items.iter().enumerate() {
        if lines.get(cursor).copied() != Some(markers[index].as_str()) {
            return Err(format!(
                "expected stable scene marker for {} at buffer line {}",
                item.scene_id,
                cursor + 1
            )
            .into());
        }
        cursor += 1;
        let end = if index + 1 < markers.len() {
            (cursor..lines.len())
                .find(|line| lines[*line] == markers[index + 1])
                .ok_or("next assembly scene marker is missing or out of order")?
        } else {
            if lines[cursor..]
                .iter()
                .any(|line| markers.iter().any(|marker| line == marker))
            {
                return Err("assembly scene markers are duplicated or out of order".into());
            }
            lines.len()
        };
        let mut section = lines[cursor..end].to_vec();
        if index + 1 < markers.len() && section.last() == Some(&"") {
            section.pop();
        }
        result.push((item.scene_id, section.join("\n")));
        cursor = end;
    }
    if result.len() != draft.items.len() {
        return Err("assembly buffer is incomplete".into());
    }
    Ok(result)
}
fn reading_text_display(
    text: &str,
    annotations: &[story_storage::ReadingAnnotationRecord],
) -> Text<'static> {
    let mut byte_styles = vec![0u8; text.len()];
    for annotation in annotations
        .iter()
        .filter(|annotation| annotation.state == "current")
    {
        let Ok(start) = usize::try_from(annotation.start_byte) else {
            continue;
        };
        let Ok(end) = usize::try_from(annotation.end_byte) else {
            continue;
        };
        if start >= end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            continue;
        }
        let style = if annotation.kind == "highlight" { 1 } else { 2 };
        for byte in &mut byte_styles[start..end] {
            if *byte == 0 || style == 1 {
                *byte = style;
            }
        }
    }
    let style_for = |kind| match kind {
        1 => Style::default().fg(Color::Black).bg(Color::Yellow),
        2 => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
        _ => Style::default(),
    };
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    for chunk in text.split_inclusive('\n') {
        let line = chunk.strip_suffix('\n').unwrap_or(chunk);
        let mut spans = Vec::new();
        let mut segment_start = 0usize;
        let mut current_style = line
            .chars()
            .next()
            .map(|_| byte_styles[line_start])
            .unwrap_or(0);
        for (offset, _) in line.char_indices() {
            let kind = byte_styles[line_start + offset];
            if kind != current_style {
                spans.push(Span::styled(
                    line[segment_start..offset].to_owned(),
                    style_for(current_style),
                ));
                segment_start = offset;
                current_style = kind;
            }
        }
        if !line.is_empty() {
            spans.push(Span::styled(
                line[segment_start..].to_owned(),
                style_for(current_style),
            ));
        }
        lines.push(Line::from(spans));
        line_start += chunk.len();
    }
    if text.is_empty() {
        lines.push(Line::from(""));
    }
    Text::from(lines)
}

fn analytics_report_rows(
    data: &[story_analysis::SceneAnalysisInput],
    target_words: Option<u64>,
    actual_words: u32,
    drafted_scenes: usize,
) -> Vec<String> {
    let heatmap = story_analysis::character_heatmap(data);
    let interactions = story_analysis::character_interactions(data);
    let gaps = story_analysis::subplot_gaps(data, 3);
    let chronology = story_analysis::chronology_ambiguities(data);
    let scenes_by_id = data
        .iter()
        .map(|scene| (scene.scene_id.as_str(), scene.title.as_str()))
        .collect::<BTreeMap<_, _>>();
    let character_names = data
        .iter()
        .flat_map(|scene| {
            scene
                .characters
                .iter()
                .map(|character| (character.id.as_str(), character.name.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let plotline_names = data
        .iter()
        .flat_map(|scene| {
            scene
                .plotlines
                .iter()
                .map(|plotline| (plotline.id.as_str(), plotline.name.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut rows = vec![
        format!("{} scenes", data.len()),
        format!("{actual_words} actual words"),
        format!("{drafted_scenes} drafted or complete scenes"),
        format!("{} characters", character_names.len()),
        format!("{} plotlines", plotline_names.len()),
        format!("{} co-appearance pairs", interactions.len()),
        format!("{} subplot gaps of 3+ scenes", gaps.len()),
        format!("{} uncertain chronology pairs", chronology.len()),
        "Character appearance heatmap · P=POV · M=major · m=minor · o=off-camera · ?=mentioned"
            .into(),
    ];
    if let Some(target) = target_words {
        let percent = (u64::from(actual_words)
            .saturating_mul(100)
            .checked_div(target)
            .unwrap_or(0))
        .min(100);
        let filled = usize::try_from(percent / 5).unwrap_or(20).min(20);
        rows.push(format!(
            "Manuscript progress · {actual_words}/{target} words · {percent}% · {}{}",
            "█".repeat(filled),
            "░".repeat(20 - filled)
        ));
    }
    let mut chapter_progress = BTreeMap::<String, (usize, u64, u64)>::new();
    for scene in data {
        if let Some(chapter) = &scene.chapter_id {
            let progress = chapter_progress.entry(chapter.clone()).or_default();
            progress.0 += 1;
            progress.1 = progress.1.saturating_add(u64::from(scene.actual_words));
            progress.2 = progress.2.saturating_add(scene.target_words.unwrap_or(0));
        }
    }
    for (chapter, (scenes, words, target)) in chapter_progress {
        let percent = (words.saturating_mul(100).checked_div(target).unwrap_or(0)).min(100);
        let filled = usize::try_from(percent / 5).unwrap_or(20).min(20);
        rows.push(format!("Chapter progress · {chapter} · {scenes} scenes · {words}/{target} words · {percent}% · {}{}", "█".repeat(filled), "░".repeat(20-filled)));
    }
    rows.extend(heatmap.into_iter().map(|row| {
        let participants = row
            .participants
            .iter()
            .map(|(_, name, role)| {
                let marker = match role {
                    story_core::ParticipationRole::Pov => "P",
                    story_core::ParticipationRole::Major => "M",
                    story_core::ParticipationRole::Minor => "m",
                    story_core::ParticipationRole::OffCamera => "o",
                    story_core::ParticipationRole::Mentioned => "?",
                };
                format!("{name}:{marker}")
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("Heatmap · {} │ {participants}", row.scene_title)
    }));
    rows.push("Character interactions · shared scene count".into());
    rows.extend(interactions.into_iter().map(|pair| {
        format!(
            "Interaction · {} ↔ {} · {} scenes",
            character_names
                .get(pair.first_character.as_str())
                .copied()
                .unwrap_or(&pair.first_character),
            character_names
                .get(pair.second_character.as_str())
                .copied()
                .unwrap_or(&pair.second_character),
            pair.shared_scenes
        )
    }));
    rows.push("Subplot gaps · manuscript order".into());
    rows.extend(gaps.into_iter().map(|gap| {
        format!(
            "Gap · {} · {} → {} · {} intervening scenes",
            gap.plotline_name,
            scenes_by_id
                .get(gap.from_scene_id.as_str())
                .copied()
                .unwrap_or(&gap.from_scene_id),
            scenes_by_id
                .get(gap.to_scene_id.as_str())
                .copied()
                .unwrap_or(&gap.to_scene_id),
            gap.intervening_scenes
        )
    }));
    rows.push("Chronology · possible overlaps or indeterminate intervals".into());
    rows.extend(chronology.into_iter().map(|finding| {
        format!(
            "Chronology · {} ↔ {} · uncertain",
            scenes_by_id
                .get(finding.first_scene_id.as_str())
                .copied()
                .unwrap_or(&finding.first_scene_id),
            scenes_by_id
                .get(finding.second_scene_id.as_str())
                .copied()
                .unwrap_or(&finding.second_scene_id)
        )
    }));
    if let Some(target_words) = target_words {
        let budget = story_analysis::preview_budget_redistribution(target_words, data);
        rows.push(format!(
            "Budget preview · target {} · fixed {} · remaining {} · preview only",
            budget.target_words, budget.fixed_words, budget.unallocated_words
        ));
        rows.extend(
            budget
                .suggested_scene_targets
                .into_iter()
                .map(|(id, words)| {
                    format!(
                        "Budget suggestion · {} · {words} words",
                        scenes_by_id.get(id.as_str()).copied().unwrap_or(&id)
                    )
                }),
        );
    }
    rows
}

fn format_story_tick(tick: i64) -> String {
    let day = tick.div_euclid(story_core::STORY_TICKS_PER_DAY);
    let day_seconds = tick.rem_euclid(story_core::STORY_TICKS_PER_DAY);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!("D{day} {hour:02}:{minute:02}:{second:02}")
}

fn parse_story_time_input(
    value: &str,
) -> Result<story_core::StoryTime, Box<dyn std::error::Error>> {
    use story_core::{StoryTime, TimePrecision};
    if value == "?" {
        return Ok(StoryTime::unknown());
    }
    if let Some(value) = value.strip_prefix('~') {
        let parts = value.split(',').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err("approximate time syntax is ~NOMINAL,LOWER,UPPER".into());
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

fn format_story_interval(time: story_core::StoryTime) -> String {
    use story_core::TimePrecision;
    match time.precision {
        TimePrecision::Unknown => "unknown".into(),
        TimePrecision::Exact => time
            .nominal
            .map(format_story_tick)
            .unwrap_or_else(|| "unknown".into()),
        TimePrecision::Approximate => {
            let nominal = time.nominal.map(format_story_tick).unwrap_or_default();
            match (time.lower, time.upper) {
                (Some(lower), Some(upper)) => format!(
                    "~{nominal} [{}–{}]",
                    format_story_tick(lower),
                    format_story_tick(upper)
                ),
                _ => format!("~{nominal}"),
            }
        }
        TimePrecision::Bounded => match (time.lower, time.upper) {
            (Some(lower), Some(upper)) => format!(
                "[{}–{}]",
                format_story_tick(lower),
                format_story_tick(upper)
            ),
            (Some(lower), None) => format!("≥{}", format_story_tick(lower)),
            (None, Some(upper)) => format!("≤{}", format_story_tick(upper)),
            (None, None) => "bounded (no limits)".into(),
        },
    }
}

fn timeline_scene_order(app: &WriterApp) -> Vec<story_storage::SceneRecord> {
    let mut scenes = app.list_scenes(false).unwrap_or_default();
    scenes.sort_by(|first, second| {
        let first_tick = first
            .scene
            .start_time
            .nominal
            .or(first.scene.start_time.lower)
            .or(first.scene.start_time.upper);
        let second_tick = second
            .scene
            .start_time
            .nominal
            .or(second.scene.start_time.lower)
            .or(second.scene.start_time.upper);
        (first_tick.is_none(), first_tick.unwrap_or(i64::MAX))
            .cmp(&(second_tick.is_none(), second_tick.unwrap_or(i64::MAX)))
            .then_with(|| first.scene.title.cmp(&second.scene.title))
            .then_with(|| first.scene.id.cmp(&second.scene.id))
    });
    scenes
}

fn timeline_items(app: &WriterApp) -> Vec<TimelineItem> {
    let mut items = timeline_scene_order(app)
        .into_iter()
        .map(TimelineItem::Scene)
        .chain(
            app.list_events()
                .unwrap_or_default()
                .into_iter()
                .map(TimelineItem::Event),
        )
        .collect::<Vec<_>>();
    items.sort_by(|first, second| {
        let key = |item: &TimelineItem| match item {
            TimelineItem::Scene(scene) => (
                scene
                    .scene
                    .start_time
                    .nominal
                    .or(scene.scene.start_time.lower)
                    .or(scene.scene.start_time.upper),
                scene.scene.title.clone(),
                scene.scene.id.to_string(),
            ),
            TimelineItem::Event(event) => (
                event
                    .start
                    .nominal
                    .or(event.start.lower)
                    .or(event.start.upper),
                event.title.clone(),
                event.id.to_string(),
            ),
        };
        let first_key = key(first);
        let second_key = key(second);
        (
            first_key.0.is_none(),
            first_key.0.unwrap_or(i64::MAX),
            first_key.1,
            first_key.2,
        )
            .cmp(&(
                second_key.0.is_none(),
                second_key.0.unwrap_or(i64::MAX),
                second_key.1,
                second_key.2,
            ))
    });
    items
}

fn next_list_selection(selected: usize, item_count: usize) -> usize {
    if item_count == 0 {
        0
    } else {
        selected.saturating_add(1).min(item_count - 1)
    }
}

fn revision_hunks(diff: &[story_storage::SnapshotDiffLine]) -> Vec<RevisionHunk> {
    let mut hunks = Vec::new();
    let (mut snapshot_line, mut current_line, mut index) = (0usize, 0usize, 0usize);
    while index < diff.len() {
        if diff[index].kind == story_storage::SnapshotLineKind::Equal {
            snapshot_line += 1;
            current_line += 1;
            index += 1;
            continue;
        }
        let start = index;
        let snapshot_start = snapshot_line;
        let current_start = current_line;
        while index < diff.len() && diff[index].kind != story_storage::SnapshotLineKind::Equal {
            match diff[index].kind {
                story_storage::SnapshotLineKind::Equal => unreachable!(),
                story_storage::SnapshotLineKind::Removed => snapshot_line += 1,
                story_storage::SnapshotLineKind::Added => current_line += 1,
            }
            index += 1;
        }
        hunks.push(RevisionHunk {
            diff_lines: start..index,
            selection: story_core::SnapshotRestoreSelection {
                current_start,
                current_end: current_line,
                snapshot_start,
                snapshot_end: snapshot_line,
            },
        });
    }
    hunks
}

fn revision_items(s: &AppState) -> Vec<(SceneId, story_storage::SnapshotRecord)> {
    s.scenes
        .iter()
        .flat_map(|(scene, _, _, _)| {
            s.app
                .list_snapshots(*scene)
                .unwrap_or_default()
                .into_iter()
                .map(|snapshot| (*scene, snapshot))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn load_proposal_review_rows(app: &WriterApp) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for file in app.list_extraction_proposal_files()? {
        let (bytes, _) = app.load_extraction_proposal_file(&file)?;
        let proposal: story_ollama::ProvisionalExtraction = serde_json::from_slice(&bytes)?;
        rows.push(format!(
            "{} · {} {}",
            file.display(),
            proposal.source_kind,
            proposal.source_id
        ));
        for (index, claim) in proposal.claims.iter().enumerate() {
            let status = if claim.approved {
                "approved"
            } else if claim.dismissed {
                "dismissed"
            } else {
                "provisional"
            };
            rows.push(format!(
                "  [{index}] {status}: {} · {} — {}; evidence: {}",
                claim.claim.kind, claim.claim.name, claim.claim.claim, claim.claim.evidence_quote
            ));
        }
    }
    Ok(rows)
}

fn workspace_items(s: &AppState) -> Vec<String> {
    match s.workspace {
        Workspace::Dashboard => vec![
            format!("{} scenes", s.scenes.len()),
            format!(
                "{} drafted",
                s.scenes
                    .iter()
                    .filter(|x| matches!(x.2, SceneStatus::DraftComplete | SceneStatus::Complete))
                    .count()
            ),
            format!("{} words", s.scenes.iter().map(|x| x.3).sum::<u32>()),
        ],
        Workspace::Outline => {
            let chapters = s
                .app
                .list_structural_nodes()
                .unwrap_or_default()
                .into_iter()
                .map(|node| (node.id, node.title))
                .collect::<BTreeMap<_, _>>();
            s.app
                .list_scenes(false)
                .unwrap_or_default()
                .into_iter()
                .map(|scene| {
                    let chapter = scene
                        .chapter_id
                        .and_then(|id| chapters.get(&id).cloned())
                        .unwrap_or_else(|| "Unplaced".into());
                    format!(
                        "{chapter} / {}   {}   {} words",
                        scene.scene.title,
                        scene.scene.status.as_str(),
                        scene.actual_words
                    )
                })
                .collect()
        }
        Workspace::Cards => s
            .scenes
            .iter()
            .map(|(_, title, status, words)| {
                format!("{title}   {}   {words} words", status.as_str())
            })
            .collect(),
        Workspace::Search if !s.search_hits.is_empty() => s
            .search_hits
            .iter()
            .map(|hit| format!("{} · {} · {}", hit.entity_kind, hit.title, hit.excerpt))
            .collect(),
        Workspace::Search if let Some(query) = s.query_view => {
            let matches = s
                .app
                .run_scene_query(query)
                .unwrap_or_default()
                .into_iter()
                .collect::<BTreeSet<_>>();
            s.scenes
                .iter()
                .filter(|scene| matches.contains(&scene.0))
                .map(|(_, title, status, words)| {
                    format!("{title}   {}   {words} words", status.as_str())
                })
                .collect()
        }
        Workspace::Search => s
            .scenes
            .iter()
            .map(|(_, title, status, words)| {
                format!("{title}   {}   {words} words", status.as_str())
            })
            .collect(),
        Workspace::Editor => Vec::new(),
        Workspace::Characters => character_matrix_rows(s),
        Workspace::PlotGrid => plot_grid_rows(s),
        Workspace::StoryBible => {
            let mut rows = Vec::new();
            rows.extend(
                s.app
                    .list_characters()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| format!("Character · {}", c.name)),
            );
            rows.extend(
                s.app
                    .list_locations()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|location| format!("Location · {}", location.name)),
            );
            rows.extend(
                s.app
                    .list_plotlines()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| format!("Plotline · {}", p.name)),
            );
            rows.extend(
                s.app
                    .list_events()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| format!("Event · {} · {}", e.title, e.source_time)),
            );
            rows.extend(
                s.app
                    .list_approved_knowledge_claims()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|claim| {
                        format!(
                            "Approved knowledge · {} · {}: {} · evidence [{}:{} · {}]",
                            claim.kind,
                            claim.name,
                            claim.claim,
                            claim.source_kind,
                            claim.source_id,
                            claim.evidence_quote
                        )
                    }),
            );
            rows
        }
        Workspace::Timeline => timeline_items(&s.app)
            .into_iter()
            .map(|item| match item {
                TimelineItem::Scene(scene) => format!(
                    "Scene · {} → {} · {}",
                    format_story_interval(scene.scene.start_time),
                    format_story_interval(scene.scene.end_time),
                    scene.scene.title
                ),
                TimelineItem::Event(event) => format!(
                    "Event · {} → {} · {} · {}",
                    format_story_interval(event.start),
                    format_story_interval(event.end),
                    event.title,
                    event.source_time
                ),
            })
            .collect(),
        Workspace::Analytics => {
            let total = s.scenes.iter().map(|x| x.3).sum::<u32>();
            let draft = s
                .scenes
                .iter()
                .filter(|x| matches!(x.2, SceneStatus::DraftComplete | SceneStatus::Complete))
                .count();
            let data = s.app.analytics_dataset().unwrap_or_default();
            let target = s.app.get_project().ok().and_then(|p| p.target_words);
            let mut rows = analytics_report_rows(&data, target, total, draft);
            rows.extend(writing_goal_graph_rows(&s.app));
            rows.extend(challenge_progress_rows(
                &s.app.list_writing_challenges().unwrap_or_default(),
            ));
            rows.extend(challenge_hall_rows(
                &s.app.list_writing_challenges().unwrap_or_default(),
            ));
            rows.extend(daily_writing_rows(
                &s.app.list_writing_challenges().unwrap_or_default(),
            ));
            rows.extend(challenge_streak_rows(
                &s.app.list_writing_challenges().unwrap_or_default(),
                time::OffsetDateTime::now_utc()
                    .unix_timestamp()
                    .div_euclid(86_400),
            ));
            let sessions = s.app.list_writing_sessions().unwrap_or_default();
            rows.extend(daily_session_rows(&sessions));
            rows.extend(writing_streak_rows(
                &sessions,
                time::OffsetDateTime::now_utc()
                    .unix_timestamp()
                    .div_euclid(86_400),
            ));
            rows.extend(scene_writing_rows(&sessions, &data));
            rows.extend(coverage_report_rows(s.app.project_root(), &data));
            rows
        }
        Workspace::Research if s.proposal_review_active => s.proposal_review_rows.clone(),
        Workspace::Research => s
            .app
            .list_research_items()
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                format!(
                    "{}   {}   {}",
                    r.title,
                    r.media_type,
                    r.source_path.unwrap_or_default()
                )
            })
            .collect(),
        Workspace::Issues => s
            .app
            .list_issues()
            .unwrap_or_default()
            .into_iter()
            .map(|i| format!("[{}] {}   ({} scenes)", i.status, i.title, i.scene_count))
            .collect(),
        Workspace::Reading => s
            .app
            .list_reading_documents()
            .unwrap_or_default()
            .into_iter()
            .map(|r| format!("{}   {}", r.title, r.media_type))
            .collect(),
        Workspace::Revisions => revision_items(s)
            .into_iter()
            .map(|(scene, snapshot)| {
                let title = s
                    .scenes
                    .iter()
                    .find(|row| row.0 == scene)
                    .map(|row| row.1.as_str())
                    .unwrap_or("Unknown scene");
                format!(
                    "{}  {}  {}  ·  {}",
                    title, snapshot.id, snapshot.created_at, snapshot.label
                )
            })
            .collect(),
        Workspace::PlanningBoards => {
            if let Some(board) = s.planning_board_id {
                s.app
                    .list_planning_board_items(board)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|item| {
                        format!(
                            "{} · {} {} · ({:.1}, {:.1}) · {}",
                            item.id, item.entity_kind, item.entity_id, item.x, item.y, item.note
                        )
                    })
                    .collect()
            } else {
                s.app
                    .list_planning_boards()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|board| format!("{} · {}", board.id, board.title))
                    .collect()
            }
        }
    }
}

fn writing_goal_graph_rows(app: &WriterApp) -> Vec<String> {
    let as_of = time::OffsetDateTime::now_utc().unix_timestamp();
    let goals = match app.list_writing_goals(false) {
        Ok(goals) => goals,
        Err(error) => return vec![format!("Writing goal ledger unavailable: {error}")],
    };
    if goals.is_empty() {
        return vec!["No custom writing goals recorded".into()];
    }
    let mut rows = Vec::new();
    for goal in goals {
        rows.push(format!(
            "Writing goal · {} · target {} · baseline {} · success metric {}",
            goal.title, goal.target_words, goal.baseline_words, goal.success_metric
        ));
        match app.writing_goal_progress(&goal.id, as_of) {
            Ok(points) => {
                let split = points
                    .iter()
                    .position(|point| point.projected)
                    .unwrap_or(points.len());
                let start = split.saturating_sub(7);
                let end = (split + 7).min(points.len());
                rows.extend(
                    points
                        .into_iter()
                        .skip(start)
                        .take(end - start)
                        .map(|point| {
                            let actual = if goal.success_metric == "raw" {
                                point.raw_words
                            } else {
                                point.real_words
                            };
                            let scale = goal.target_words.max(1) as f64;
                            let target_bar = ((point.target_words.max(0) as f64 / scale) * 12.0)
                                .round()
                                .clamp(0.0, 12.0)
                                as usize;
                            let actual_bar = actual.map(|value| {
                                ((value.max(0) as f64 / scale) * 12.0)
                                    .round()
                                    .clamp(0.0, 12.0) as usize
                            });
                            format!(
                                "  {} target {:>7} {} actual {:>7} {}{}",
                                point.date,
                                point.target_words,
                                "─".repeat(target_bar),
                                actual.map_or_else(|| "future".into(), |value| value.to_string()),
                                actual_bar.map_or_else(String::new, |count| "█".repeat(count)),
                                if point.projected { " projected" } else { "" }
                            )
                        }),
                );
            }
            Err(error) => rows.push(format!("  Progress unavailable: {error}")),
        }
    }
    rows
}

fn writing_goal_weekly_rows(
    app: &WriterApp,
    goal_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let goal = app
        .list_writing_goals(true)?
        .into_iter()
        .find(|goal| goal.id == goal_id)
        .ok_or("writing goal not found")?;
    let points =
        app.writing_goal_progress(goal_id, time::OffsetDateTime::now_utc().unix_timestamp())?;
    let mut prior_target = goal.baseline_words;
    let mut prior_raw = goal.baseline_words;
    let mut prior_real = goal.baseline_words;
    Ok(points
        .chunks(7)
        .map(|week| {
            let last = week.last().expect("week chunks are non-empty");
            let target_delta = last.target_words.saturating_sub(prior_target);
            prior_target = last.target_words;
            let raw_delta = last.raw_words.map(|raw| {
                let delta = raw.saturating_sub(prior_raw);
                prior_raw = raw;
                delta
            });
            let real_delta = last.real_words.map(|real| {
                let delta = real.saturating_sub(prior_real);
                prior_real = real;
                delta
            });
            format!(
                "{}–{} target pace +{} words · raw {} · real {}{}",
                week.first().map_or("?", |point| point.date.as_str()),
                last.date,
                target_delta,
                raw_delta.map_or_else(|| "future".into(), |value| format!("+{value}")),
                real_delta.map_or_else(|| "future".into(), |value| format!("+{value}")),
                if last.projected { " (projected)" } else { "" }
            )
        })
        .collect())
}

fn challenge_progress_rows(challenges: &[story_storage::WritingChallengeRecord]) -> Vec<String> {
    if challenges.is_empty() {
        return vec!["No writing challenge sessions recorded".into()];
    }
    challenges
        .iter()
        .map(|challenge| {
            let written = challenge.words_written.unwrap_or(0);
            let filled = if challenge.target_words == 0 {
                0
            } else {
                ((u64::from(written).saturating_mul(20) / u64::from(challenge.target_words))
                    .min(20)) as usize
            };
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(20 - filled));
            let elapsed = challenge
                .ended_at
                .unwrap_or(challenge.deadline_at)
                .saturating_sub(challenge.started_at)
                .max(0);
            format!(
                "Challenge · {} · {written}/{} words · {bar} · {} · {} · {elapsed}s · lost {} · pauses {}",
                challenge.mode,
                challenge.target_words,
                challenge.outcome,
                challenge.started_at,
                challenge.words_deleted,
                challenge.pauses_consumed
            )
        })
        .collect()
}

fn challenge_hall_rows(challenges: &[story_storage::WritingChallengeRecord]) -> Vec<String> {
    let failures = challenges
        .iter()
        .filter(|challenge| challenge.outcome == "timeout" || challenge.outcome == "quit")
        .collect::<Vec<_>>();
    let lost = failures
        .iter()
        .map(|challenge| u64::from(challenge.words_deleted))
        .sum::<u64>();
    let best = challenges
        .iter()
        .filter(|challenge| challenge.outcome == "success")
        .map(|challenge| challenge.words_written.unwrap_or(0))
        .max()
        .unwrap_or(0);
    let mut rows = vec![format!(
        "Hall of Shame · {} failed/quit · {lost} words lost · personal best {best} words",
        failures.len()
    )];
    rows.extend(failures.into_iter().map(|challenge| {
        format!(
            "Failed challenge · {} retained · {} deleted · {} pauses · {}",
            challenge.words_written.unwrap_or(0),
            challenge.words_deleted,
            challenge.pauses_consumed,
            if challenge.outcome == "quit" {
                "YOU QUIT BEFORE THE WORDS DID"
            } else {
                "TIME WON THIS ROUND"
            }
        )
    }));
    rows
}

fn daily_writing_rows(challenges: &[story_storage::WritingChallengeRecord]) -> Vec<String> {
    let mut days = BTreeMap::<String, (u64, usize)>::new();
    for challenge in challenges {
        let Some(words) = challenge.words_written else {
            continue;
        };
        let Ok(timestamp) = time::OffsetDateTime::from_unix_timestamp(challenge.started_at) else {
            continue;
        };
        let date = format!(
            "{:04}-{:02}-{:02}",
            timestamp.year(),
            u8::from(timestamp.month()),
            timestamp.day()
        );
        let entry = days.entry(date).or_default();
        entry.0 = entry.0.saturating_add(u64::from(words));
        entry.1 += 1;
    }
    let maximum = days.values().map(|entry| entry.0).max().unwrap_or(0);
    let mut cumulative = 0u64;
    days.into_iter()
        .map(|(date, (words, sessions))| {
            cumulative = cumulative.saturating_add(words);
            let filled = words
                .saturating_mul(20)
                .checked_div(maximum)
                .unwrap_or(0)
                .min(20) as usize;
            let bar = "█".repeat(filled);
            let session_label = if sessions == 1 { "session" } else { "sessions" };
            format!("{date}  {bar}  {words} words · {sessions} {session_label} · {cumulative} cumulative")
        })
        .collect()
}

fn challenge_streak_rows(
    challenges: &[story_storage::WritingChallengeRecord],
    today_utc_day: i64,
) -> Vec<String> {
    let success_days = challenges
        .iter()
        .filter(|challenge| challenge.outcome == "success")
        .filter(|challenge| challenge.words_written.is_some_and(|words| words > 0))
        .map(|challenge| challenge.started_at.div_euclid(86_400))
        .collect::<BTreeSet<_>>();
    if success_days.is_empty() {
        return vec!["Challenge streak · no successful challenge days yet".into()];
    }
    let mut best = 0usize;
    let mut run = 0usize;
    let mut previous: Option<i64> = None;
    for day in &success_days {
        run = if previous.is_some_and(|prior| prior.saturating_add(1) == *day) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        previous = Some(*day);
    }
    let latest = *success_days.last().expect("non-empty success days");
    let mut current = 0usize;
    if latest >= today_utc_day.saturating_sub(1) {
        let mut day = latest;
        while success_days.contains(&day) {
            current += 1;
            let Some(previous_day) = day.checked_sub(1) else {
                break;
            };
            day = previous_day;
        }
    }
    vec![format!(
        "Challenge streak · current {current} days · best {best} days · completed positive-word challenges"
    )]
}

fn daily_session_rows(sessions: &[story_storage::WritingSessionRecord]) -> Vec<String> {
    let mut days = BTreeMap::<String, (i64, usize)>::new();
    for session in sessions {
        let Some(delta) = session.words_delta else {
            continue;
        };
        let Ok(timestamp) = time::OffsetDateTime::from_unix_timestamp(session.started_at) else {
            continue;
        };
        let date = format!(
            "{:04}-{:02}-{:02}",
            timestamp.year(),
            u8::from(timestamp.month()),
            timestamp.day()
        );
        let entry = days.entry(date).or_default();
        entry.0 = entry.0.saturating_add(delta);
        entry.1 += 1;
    }
    let maximum = days
        .values()
        .map(|entry| entry.0.unsigned_abs())
        .max()
        .unwrap_or(0);
    let mut cumulative = 0i64;
    days.into_iter()
        .map(|(date, (delta, count))| {
            cumulative = cumulative.saturating_add(delta);
            let filled = delta
                .unsigned_abs()
                .saturating_mul(20)
                .checked_div(maximum)
                .unwrap_or(0)
                .min(20) as usize;
            let sign = if delta < 0 { "−" } else { "" };
            format!(
                "Drafting · {date}  {sign}{}  {delta:+} words · {count} sessions · {cumulative:+} cumulative",
                "█".repeat(filled)
            )
        })
        .collect()
}

fn writing_streak_rows(
    sessions: &[story_storage::WritingSessionRecord],
    today_utc_day: i64,
) -> Vec<String> {
    let active_days = sessions
        .iter()
        .filter(|session| session.status == "completed")
        .filter(|session| session.words_delta.is_some_and(|delta| delta > 0))
        .map(|session| session.started_at.div_euclid(86_400))
        .collect::<BTreeSet<_>>();
    if active_days.is_empty() {
        return vec!["Writing streak · no completed positive-word days yet".into()];
    }

    let mut best = 0usize;
    let mut run = 0usize;
    let mut previous = None;
    for day in &active_days {
        run = if previous.is_some_and(|prior| prior + 1 == *day) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        previous = Some(*day);
    }
    let latest = *active_days.last().expect("non-empty active days");
    let mut current = 0usize;
    if latest >= today_utc_day.saturating_sub(1) {
        let mut day = latest;
        while active_days.contains(&day) {
            current += 1;
            let Some(previous_day) = day.checked_sub(1) else {
                break;
            };
            day = previous_day;
        }
    }
    vec![format!(
        "Writing streak · current {current} days · best {best} days · based on completed sessions with positive word deltas"
    )]
}

fn scene_writing_rows(
    sessions: &[story_storage::WritingSessionRecord],
    scenes: &[story_analysis::SceneAnalysisInput],
) -> Vec<String> {
    let titles = scenes
        .iter()
        .map(|scene| (scene.scene_id.as_str(), scene.title.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut totals = BTreeMap::<String, (i64, usize, i64, i64)>::new();
    let mut series = BTreeMap::<(String, String), (i64, usize, i64)>::new();
    for session in sessions {
        let (Some(scene_id), Some(delta)) = (session.scene_id, session.words_delta) else {
            continue;
        };
        let id = scene_id.to_string();
        let entry = totals.entry(id.clone()).or_default();
        entry.0 = entry.0.saturating_add(delta);
        entry.1 += 1;
        let elapsed = session
            .ended_at
            .map(|ended| ended.saturating_sub(session.started_at).max(0))
            .unwrap_or(0);
        entry.2 = entry.2.saturating_add(elapsed);
        if session.status == "completed" {
            entry.3 = entry.3.saturating_add(delta);
        }
        let Ok(timestamp) = time::OffsetDateTime::from_unix_timestamp(session.started_at) else {
            continue;
        };
        let date = format!(
            "{:04}-{:02}-{:02}",
            timestamp.year(),
            u8::from(timestamp.month()),
            timestamp.day()
        );
        let entry = series.entry((id, date)).or_default();
        entry.0 = entry.0.saturating_add(delta);
        entry.1 += 1;
        entry.2 = entry.2.saturating_add(elapsed);
    }
    let mut rows = totals
        .into_iter()
        .map(|(id, (delta, count, seconds, completed_delta))| {
            let productivity = if seconds > 0 {
                completed_delta.saturating_mul(3_600) / seconds
            } else {
                0
            };
            format!(
                "Scene writing · {} · {delta:+} net words · {count} sessions · {} focused time · {productivity} completed words/hour",
                titles.get(id.as_str()).copied().unwrap_or("Deleted scene"),
                display_duration(seconds)
            )
        })
        .collect::<Vec<_>>();
    let mut maxima = BTreeMap::<String, u64>::new();
    for ((id, _), (delta, _, _)) in &series {
        let maximum = maxima.entry(id.clone()).or_default();
        *maximum = (*maximum).max(delta.unsigned_abs());
    }
    rows.extend(series.into_iter().map(|((id, date), (delta, count, seconds))| {
        let maximum = maxima.get(&id).copied().unwrap_or(0);
        let filled = delta
            .unsigned_abs()
            .saturating_mul(12)
            .checked_div(maximum)
            .unwrap_or(0)
            .min(12) as usize;
        let sign = if delta < 0 { "−" } else { "" };
        format!(
            "Scene history · {} · {date} · {sign}{} {delta:+} words · {count} sessions · {} focused",
            titles.get(id.as_str()).copied().unwrap_or("Deleted scene"),
            "█".repeat(filled),
            display_duration(seconds)
        )
    }));
    rows
}

fn display_duration(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {}m", minutes / 60, minutes % 60)
    }
}

fn coverage_config_path(root: &Path) -> std::path::PathBuf {
    root.join(".writers-ide/coverage.toml")
}

fn load_coverage_config(root: &Path) -> Result<CoverageConfig, Box<dyn std::error::Error>> {
    let path = coverage_config_path(root);
    match fs::read_to_string(path) {
        Ok(source) => {
            let config: CoverageConfig = toml::from_str(&source)?;
            if config.version != 1 {
                return Err(
                    format!("unsupported coverage config version {}", config.version).into(),
                );
            }
            let mut ids = BTreeSet::new();
            for entry in &config.requirements {
                if entry.id.trim().is_empty()
                    || entry.label.trim().is_empty()
                    || entry.entity_id.trim().is_empty()
                    || !matches!(entry.dimension.as_str(), "character" | "plotline")
                    || !ids.insert(&entry.id)
                {
                    return Err(
                        format!("invalid or duplicate coverage requirement {}", entry.id).into(),
                    );
                }
            }
            Ok(config)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(CoverageConfig {
            version: 1,
            requirements: Vec::new(),
        }),
        Err(error) => Err(error.into()),
    }
}

fn save_coverage_config(
    root: &Path,
    config: &CoverageConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join(".writers-ide");
    fs::create_dir_all(&directory)?;
    let bytes = toml::to_string_pretty(config)?;
    let path = coverage_config_path(root);
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temporary.write_all(bytes.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn challenge_config_path(root: &Path) -> std::path::PathBuf {
    root.join(".writers-ide/challenge.toml")
}

fn challenge_draft_path(directory: &Path, id: &str) -> std::path::PathBuf {
    directory.join(format!("challenge-{id}.json"))
}

fn remove_challenge_draft(root: &Path, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let directory = root.join(".writers-ide/recovery");
    let path = challenge_draft_path(&directory, id);
    fs::remove_file(path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn read_challenge_draft(
    root: &Path,
    id: &str,
) -> Result<ChallengeDraftFile, Box<dyn std::error::Error>> {
    let id = uuid::Uuid::parse_str(id)?.to_string();
    let path = challenge_draft_path(&root.join(".writers-ide/recovery"), &id);
    let draft: ChallengeDraftFile = serde_json::from_slice(&fs::read(path)?)?;
    if draft.version != 1
        || draft.challenge_id != id
        || draft.target_words == 0
        || draft.duration_seconds == 0
        || !matches!(draft.mode.as_str(), "steady" | "pressure" | "kamikaze")
    {
        return Err("challenge recovery record identity or version mismatch".into());
    }
    Ok(draft)
}

fn list_challenge_drafts(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let directory = root.join(".writers-ide/recovery");
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok("No recoverable challenge drafts".into());
        }
        Err(error) => return Err(error.into()),
    };
    let mut drafts = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = name
            .strip_prefix("challenge-")
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        match read_challenge_draft(root, id) {
            Ok(draft) => drafts.push(format!(
                "{} · {} words · {} · {} lost · {} pauses · use challenge-save-new {} CHAPTER-UUID TITLE or challenge-discard {}",
                draft.challenge_id,
                story_core::count_words(&draft.text),
                draft.mode,
                draft.words_deleted,
                draft.pauses_consumed,
                draft.challenge_id,
                draft.challenge_id
            )),
            Err(error) => drafts.push(format!("{id} · recovery record needs inspection: {error}")),
        }
    }
    drafts.sort();
    if drafts.is_empty() {
        Ok("No recoverable challenge drafts".into())
    } else {
        Ok(drafts.join("\n"))
    }
}

fn load_challenge_config(root: &Path) -> Result<ChallengeConfig, Box<dyn std::error::Error>> {
    match fs::read_to_string(challenge_config_path(root)) {
        Ok(source) => {
            let config: ChallengeConfig = toml::from_str(&source)?;
            if config.version != 1
                || !(1..=3600).contains(&config.kamikaze_idle_seconds)
                || config.grace_pauses > 20
                || !(1..=100).contains(&config.deletion_words_per_penalty)
                || !valid_challenge_taunt(&config.failure_taunt)
            {
                return Err("invalid challenge configuration version or idle threshold".into());
            }
            Ok(config)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ChallengeConfig::default()),
        Err(error) => Err(error.into()),
    }
}

fn save_challenge_config(
    root: &Path,
    config: &ChallengeConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if config.version != 1
        || !(1..=3600).contains(&config.kamikaze_idle_seconds)
        || config.grace_pauses > 20
        || !(1..=100).contains(&config.deletion_words_per_penalty)
        || !valid_challenge_taunt(&config.failure_taunt)
    {
        return Err("invalid challenge configuration version or idle threshold".into());
    }
    let directory = root.join(".writers-ide");
    fs::create_dir_all(&directory)?;
    let bytes = toml::to_string_pretty(config)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temporary.write_all(bytes.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(challenge_config_path(root))?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

fn valid_challenge_taunt(taunt: &str) -> bool {
    !taunt.trim().is_empty()
        && taunt.len() <= 200
        && !taunt.chars().any(char::is_control)
        && taunt.contains("{words}")
}

fn coverage_report_rows(root: &Path, scenes: &[story_analysis::SceneAnalysisInput]) -> Vec<String> {
    let config = match load_coverage_config(root) {
        Ok(config) => config,
        Err(error) => return vec![format!("Coverage config error: {error}")],
    };
    if config.requirements.is_empty() {
        return vec!["No configured coverage requirements".into()];
    }
    let requirements = config
        .requirements
        .iter()
        .map(|entry| {
            Ok(story_analysis::CoverageRequirement {
                id: entry.id.clone(),
                label: entry.label.clone(),
                entity_id: entry.entity_id.clone(),
                dimension: match entry.dimension.as_str() {
                    "character" => story_analysis::CoverageDimension::Character,
                    "plotline" => story_analysis::CoverageDimension::Plotline,
                    "pov" => story_analysis::CoverageDimension::Pov,
                    "location" => story_analysis::CoverageDimension::Location,
                    "event" => story_analysis::CoverageDimension::Event,
                    _ => return Err(format!("unsupported dimension {}", entry.dimension)),
                },
                minimum_scenes: entry.minimum_scenes,
            })
        })
        .collect::<Result<Vec<_>, String>>();
    match requirements {
        Ok(requirements) => story_analysis::evaluate_coverage(scenes, &requirements)
            .into_iter()
            .map(|result| {
                format!(
                    "Coverage {} · {} · {}/{} scenes · {}",
                    if result.coverage_met { "PASS" } else { "GAP" },
                    result.label,
                    result.matching_scene_ids.len(),
                    result.minimum_scenes,
                    result.requirement_id
                )
            })
            .collect(),
        Err(error) => vec![format!("Coverage config error: {error}")],
    }
}

fn plot_grid_rows(s: &AppState) -> Vec<String> {
    let data = match s.app.analytics_dataset() {
        Ok(data) => data,
        Err(_) => return vec!["Unable to load plot data".into()],
    };
    let mut plotlines = BTreeMap::<String, String>::new();
    for scene in &data {
        for plot in &scene.plotlines {
            plotlines.insert(plot.id.to_string(), plot.name.clone());
        }
    }
    let plotlines = plotlines.into_iter().collect::<Vec<_>>();
    let nodes = s
        .app
        .list_structural_nodes()
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.id.to_string(), n.title))
        .collect::<BTreeMap<_, _>>();
    let mut rows = vec![format!(
        "Scene / Plotline | {}",
        plotlines
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    )];
    for scene in data {
        let chapter = scene
            .chapter_id
            .as_ref()
            .and_then(|id| nodes.get(id))
            .map(String::as_str)
            .unwrap_or("Unplaced");
        let active = scene
            .plotlines
            .iter()
            .map(|p| p.id.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let cells = plotlines
            .iter()
            .map(|(id, _)| if active.contains(id) { "●" } else { "·" })
            .collect::<Vec<_>>()
            .join("   ");
        rows.push(format!("{chapter} / {} | {cells}", scene.title));
    }
    rows
}

fn character_matrix_rows(s: &AppState) -> Vec<String> {
    let data = match s.app.analytics_dataset() {
        Ok(data) => data,
        Err(_) => return vec!["Unable to load character data".into()],
    };
    let mut characters = BTreeMap::<String, String>::new();
    for scene in &data {
        for person in &scene.characters {
            characters.insert(person.id.clone(), person.name.clone());
        }
    }
    if characters.is_empty() {
        return vec!["No character participation recorded".into()];
    }
    let characters = characters.into_iter().collect::<Vec<_>>();
    let mut rows = vec![format!(
        "Scene / Character | {}",
        characters
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    )];
    for scene in data {
        let by_id = scene
            .characters
            .iter()
            .map(|p| (p.id.as_str(), p.role))
            .collect::<BTreeMap<_, _>>();
        let cells = characters
            .iter()
            .map(|(id, _)| match by_id.get(id.as_str()) {
                Some(story_core::ParticipationRole::Pov) => "P",
                Some(story_core::ParticipationRole::Major) => "●",
                Some(story_core::ParticipationRole::Minor) => "○",
                Some(story_core::ParticipationRole::OffCamera) => "o",
                Some(story_core::ParticipationRole::Mentioned) => "m",
                None => "·",
            })
            .collect::<Vec<_>>()
            .join("   ");
        rows.push(format!("{} | {cells}", scene.title));
    }
    rows
}
fn parse_story_bible_kind(
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

fn parse_entity(kind: &str, id: &str) -> Result<story_core::EntityRef, Box<dyn std::error::Error>> {
    Ok(match kind {
        "scene" => story_core::EntityRef::Scene(SceneId(id.parse()?)),
        "character" => story_core::EntityRef::Character(story_core::CharacterId(id.parse()?)),
        "plotline" => story_core::EntityRef::Plotline(story_core::PlotlineId(id.parse()?)),
        "research" => story_core::EntityRef::Research(story_core::ResearchId(id.parse()?)),
        "reading" => story_core::EntityRef::Reading(story_core::ReadingId(id.parse()?)),
        _ => return Err(format!("unknown entity kind: {kind}").into()),
    })
}
fn parse_terminal_status(value: &str) -> Result<SceneStatus, Box<dyn std::error::Error>> {
    Ok(match value {
        "idea" => SceneStatus::Idea,
        "outlined" => SceneStatus::Outlined,
        "ready_to_draft" => SceneStatus::ReadyToDraft,
        "drafting" => SceneStatus::Drafting,
        "draft_complete" => SceneStatus::DraftComplete,
        "revising" => SceneStatus::Revising,
        "complete" => SceneStatus::Complete,
        "archived" => SceneStatus::Archived,
        _ => return Err(format!("unknown scene status: {value}").into()),
    })
}
fn parse_participation_role(
    value: &str,
) -> Result<story_core::ParticipationRole, Box<dyn std::error::Error>> {
    Ok(match value {
        "pov" => story_core::ParticipationRole::Pov,
        "major" => story_core::ParticipationRole::Major,
        "minor" => story_core::ParticipationRole::Minor,
        "off_camera" => story_core::ParticipationRole::OffCamera,
        "mentioned" => story_core::ParticipationRole::Mentioned,
        _ => return Err(format!("unknown participation role: {value}").into()),
    })
}
fn parse_terminal_link_kind(
    value: &str,
) -> Result<story_core::LinkKind, Box<dyn std::error::Error>> {
    Ok(match value {
        "related" => story_core::LinkKind::Related,
        "cites" => story_core::LinkKind::Cites,
        "transcludes" => story_core::LinkKind::Transcludes,
        "includes" => story_core::LinkKind::Includes,
        "navigates" => story_core::LinkKind::Navigates,
        _ => return Err(format!("unknown link kind: {value}").into()),
    })
}
fn handle(s: &mut AppState, k: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    if s.menu.is_some() {
        if is_menu_shortcut(k) || k.code == KeyCode::F(10) {
            s.toggle_menu();
        } else {
            s.handle_menu_key(k)?;
        }
        return Ok(false);
    }
    if is_menu_shortcut(k) {
        if let Some(spec) = command_by_id("menu.main") {
            s.dispatch_registered(spec.action)?;
        }
        return Ok(false);
    }
    if k.code == KeyCode::F(10) {
        if let Some(spec) = command_by_id("menu.main") {
            s.dispatch_registered(spec.action)?;
        }
        return Ok(false);
    }
    if s.mode.is_some() {
        if s.mode == Some('c') {
            match k.code {
                KeyCode::Esc => {
                    s.mode = None;
                    s.prompt.clear();
                    s.message = "Challenge continues".into();
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    s.persist_challenge_draft()?;
                    s.finish_active_challenge("quit")?;
                    return Ok(true);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    let challenge_id = s
                        .active_challenge
                        .as_ref()
                        .ok_or("no active challenge")?
                        .id
                        .clone();
                    s.finish_active_challenge("quit")?;
                    let path = challenge_draft_path(
                        &s.app.project_root().join(".writers-ide/recovery"),
                        &challenge_id,
                    );
                    if path.exists() {
                        fs::remove_file(path)?;
                    }
                    return Ok(true);
                }
                _ => {}
            }
            return Ok(false);
        }
        match k.code {
            KeyCode::Esc => {
                s.mode = None;
                s.prompt.clear();
            }
            KeyCode::Enter => s.command()?,
            KeyCode::Backspace => {
                s.prompt.pop();
            }
            KeyCode::Char(c) if k.modifiers.is_empty() => s.prompt.push(c),
            _ => {}
        }
        return Ok(false);
    }
    if let Some((_, id)) = s
        .command_bindings
        .iter()
        .find(|(event, _)| same_keycode(event.code, k.code) && event.modifiers == k.modifiers)
    {
        if let Some(command) = command_by_id(id) {
            if let Some(reason) = s.command_disabled_reason(command) {
                s.message = reason.into();
            } else {
                s.dispatch_registered(command.action)?;
            }
        }
        return Ok(false);
    }
    if s.workspace == Workspace::Editor && k.code == KeyCode::F(3) {
        s.navigate_search_match(!k.modifiers.contains(KeyModifiers::SHIFT))?;
        return Ok(false);
    }
    if s.workspace == Workspace::Reading {
        match k.code {
            KeyCode::Char('h') => s.reading_focus = false,
            KeyCode::Char('l') => s.reading_focus = true,
            KeyCode::Up | KeyCode::Char('k') if s.reading_focus => {
                s.scroll = s.scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') if s.reading_focus => {
                s.scroll = s.scroll.saturating_add(1)
            }
            KeyCode::PageUp if s.reading_focus => s.scroll = s.scroll.saturating_sub(20),
            KeyCode::PageDown if s.reading_focus => s.scroll = s.scroll.saturating_add(20),
            KeyCode::Up | KeyCode::Char('k') => s.selected = s.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                let n = s.app.list_reading_documents()?.len();
                if n > 0 {
                    s.selected = (s.selected + 1).min(n - 1);
                }
            }
            KeyCode::Enter if !s.reading_focus => s.open_reading_selected()?,
            KeyCode::Enter if s.reading_focus => {
                if let Some(id) = s.reading_document {
                    s.app.set_reading_checkpoint(id, s.reading_offset)?;
                    s.message = "Checkpoint saved".into();
                }
            }
            KeyCode::Char('v') if s.reading_focus => {
                s.reading_selection_start = Some(s.reading_offset);
                s.message = format!("Selection start marked at byte {}", s.reading_offset);
            }
            KeyCode::Char('a') if s.reading_focus => {
                if let Err(error) = s.create_reading_selection("highlight", "") {
                    s.message = error.to_string();
                }
            }
            KeyCode::Char('n') if s.reading_focus => {
                s.mode = Some('n');
                s.prompt.clear();
            }
            KeyCode::Char('b') if s.reading_focus => {
                if let Some(id) = s.reading_document {
                    s.app.create_reading_bookmark(
                        id,
                        &format!("At byte {}", s.reading_offset),
                        s.reading_offset,
                    )?;
                    s.message = "Bookmark saved".into();
                }
            }
            KeyCode::Char('c') if s.reading_focus => {
                if let Some(id) = s.reading_document {
                    s.app.set_reading_checkpoint(id, s.reading_offset)?;
                    s.message = "Reading checkpoint saved".into();
                }
            }
            _ => {}
        }
        if s.reading_focus {
            s.reading_offset = s
                .reading_text
                .split_inclusive('\n')
                .take(s.scroll)
                .map(str::len)
                .sum::<usize>() as u64;
        }
        return Ok(false);
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('s') {
        if let Some(command) = command_by_id("file.save") {
            s.dispatch_registered(command.action)?;
        }
        return Ok(false);
    }
    if s.workspace == Workspace::Editor
        && k.modifiers.contains(KeyModifiers::CONTROL)
        && k.code == KeyCode::Tab
        && !s.reference_panes.is_empty()
    {
        s.reference_focused = !s.reference_focused;
        s.message = if s.reference_focused {
            "Reference pane focused · arrows scroll"
        } else {
            "Manuscript editor focused"
        }
        .into();
        return Ok(false);
    }
    if s.workspace == Workspace::Editor
        && k.modifiers.contains(KeyModifiers::CONTROL)
        && k.code == KeyCode::Char('g')
    {
        s.editor_focus_mode = !s.editor_focus_mode;
        s.message = if s.editor_focus_mode {
            "Focus mode enabled".into()
        } else {
            "Focus mode disabled".into()
        };
        let _ = s.save_session();
        return Ok(false);
    }
    if s.workspace == Workspace::Editor && k.modifiers.contains(KeyModifiers::CONTROL) {
        let ratio = match k.code {
            KeyCode::Left => Some((&mut s.horizontal_split_ratio, -5i16)),
            KeyCode::Right => Some((&mut s.horizontal_split_ratio, 5)),
            KeyCode::Up => Some((&mut s.vertical_split_ratio, -5)),
            KeyCode::Down => Some((&mut s.vertical_split_ratio, 5)),
            _ => None,
        };
        if let Some((value, delta)) = ratio {
            *value = (*value as i16 + delta).clamp(20, 80) as u16;
            s.message = format!(
                "Split ratio updated: horizontal {}%, vertical {}%",
                s.horizontal_split_ratio, s.vertical_split_ratio
            );
            let _ = s.save_session();
            return Ok(false);
        }
    }
    if s.workspace == Workspace::PlanningBoards
        && let Some(board) = s.planning_board_id
        && k.modifiers.contains(KeyModifiers::CONTROL)
        && !k.modifiers.contains(KeyModifiers::SHIFT)
    {
        let delta = match k.code {
            KeyCode::Left => Some((-1.0, 0.0)),
            KeyCode::Right => Some((1.0, 0.0)),
            KeyCode::Up => Some((0.0, -1.0)),
            KeyCode::Down => Some((0.0, 1.0)),
            _ => None,
        };
        if let Some((dx, dy)) = delta {
            let items = s.app.list_planning_board_items(board)?;
            if let Some(item) = items.get(s.selected) {
                let item_id = item.id;
                s.app
                    .move_planning_board_item(item_id, item.x + dx, item.y + dy)?;
                if let Some(index) = s
                    .app
                    .list_planning_board_items(board)?
                    .iter()
                    .position(|card| card.id == item_id)
                {
                    s.selected = index;
                }
                s.message = format!(
                    "Moved {} card to ({:.1}, {:.1})",
                    item.entity_kind,
                    item.x + dx,
                    item.y + dy
                );
            }
            return Ok(false);
        }
    }
    if s.workspace == Workspace::PlanningBoards
        && let Some(board) = s.planning_board_id
        && k.modifiers
            .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    {
        let resize = match k.code {
            KeyCode::Left => Some((-1.0, 0.0)),
            KeyCode::Right => Some((1.0, 0.0)),
            KeyCode::Up => Some((0.0, 1.0)),
            KeyCode::Down => Some((0.0, -1.0)),
            _ => None,
        };
        if let Some((dw, dh)) = resize
            && let Some(item) = s.app.list_planning_board_items(board)?.get(s.selected)
        {
            let item_id = item.id;
            let width = (item.width + dw).max(1.0);
            let height = (item.height + dh).max(1.0);
            s.app.resize_planning_board_item(item_id, width, height)?;
            s.message = format!("Resized planning card to {width:.1}×{height:.1}");
            return Ok(false);
        }
    }
    if s.workspace == Workspace::PlanningBoards && k.code == KeyCode::Esc {
        s.planning_board_id = None;
        s.selected = 0;
        s.message = "Planning board list".into();
        return Ok(false);
    }
    if k.modifiers.contains(KeyModifiers::ALT) && k.code == KeyCode::Left {
        if s.history_index > 0 {
            s.history_index -= 1;
            s.workspace = s.history[s.history_index];
        }
        return Ok(false);
    }
    if k.modifiers.contains(KeyModifiers::ALT) && k.code == KeyCode::Right {
        if s.history_index + 1 < s.history.len() {
            s.history_index += 1;
            s.workspace = s.history[s.history_index];
        }
        return Ok(false);
    }
    if k.code == KeyCode::Char('q') && k.modifiers.is_empty() {
        if s.active_challenge.is_some() {
            s.mode = Some('c');
            s.prompt = "Save and quit (s) · Discard and quit (d) · Esc continues".into();
            s.message = "Choose whether to keep the isolated challenge draft".into();
            return Ok(false);
        }
        if s.dirty {
            s.message = "Unsaved text: Ctrl-S to save before quitting".into();
            return Ok(false);
        }
        return Ok(true);
    }
    if s.workspace == Workspace::Editor {
        if s.reference_focused {
            if let Some(reference) = s.active_reference_pane_mut() {
                match k.code {
                    KeyCode::Up => reference.scroll = reference.scroll.saturating_sub(1),
                    KeyCode::Down => {
                        reference.scroll = reference
                            .scroll
                            .saturating_add(1)
                            .min(reference.content.lines().count().saturating_sub(1))
                    }
                    KeyCode::PageUp => reference.scroll = reference.scroll.saturating_sub(10),
                    KeyCode::PageDown => {
                        reference.scroll = reference
                            .scroll
                            .saturating_add(10)
                            .min(reference.content.lines().count().saturating_sub(1))
                    }
                    _ => {}
                }
            }
            return Ok(false);
        }
        if k.modifiers.contains(KeyModifiers::CONTROL)
            && !k.modifiers.contains(KeyModifiers::SHIFT)
            && k.code == KeyCode::Char('z')
        {
            s.undo_edit();
            return Ok(false);
        }
        if k.modifiers.contains(KeyModifiers::CONTROL)
            && (k.code == KeyCode::Char('y')
                || (k.code == KeyCode::Char('z') && k.modifiers.contains(KeyModifiers::SHIFT)))
        {
            s.redo_edit();
            return Ok(false);
        }
        let mutating = matches!(
            k.code,
            KeyCode::Enter | KeyCode::Backspace | KeyCode::Delete | KeyCode::Tab
        ) || (matches!(k.code, KeyCode::Char(_))
            && (!k.modifiers.contains(KeyModifiers::CONTROL)
                && !k.modifiers.contains(KeyModifiers::ALT)
                || (k.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(k.code, KeyCode::Char('x' | 'v')))));
        if mutating {
            s.search_locations.clear();
            s.push_undo();
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('c') => {
                    if let Some(text) = s.selected_text() {
                        s.clipboard = text;
                        let copied = s.clipboard.clone();
                        s.remember_kill(copied)?;
                    }
                    return Ok(false);
                }
                KeyCode::Char('x') => {
                    if let Some(text) = s.selected_text() {
                        s.clipboard = text;
                        let cut = s.clipboard.clone();
                        s.remember_kill(cut)?;
                        s.delete_selection();
                    }
                    return Ok(false);
                }
                KeyCode::Char('v') => {
                    let text = s.clipboard.clone();
                    s.paste_text(&text);
                    return Ok(false);
                }
                _ => {}
            }
        }
        let selecting = k.modifiers.contains(KeyModifiers::SHIFT);
        match k.code {
            KeyCode::Tab => s.insert('\t'),
            KeyCode::Enter => s.enter(),
            KeyCode::Backspace => s.backspace(),
            KeyCode::Delete => s.delete_forward(),
            KeyCode::Home => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectLineStart
                } else {
                    story_core::EditMacroOp::MoveLineStart
                });
                s.move_editor_cursors(CursorMotion::Home, selecting)
            }
            KeyCode::End => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectLineEnd
                } else {
                    story_core::EditMacroOp::MoveLineEnd
                });
                s.move_editor_cursors(CursorMotion::End, selecting)
            }
            KeyCode::Char('w') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                let row = s.cursor.0;
                while s.cursor.1 > 0
                    && s.editor[row]
                        .graphemes(true)
                        .nth(s.cursor.1 - 1)
                        .is_some_and(|g| g.chars().all(char::is_whitespace))
                {
                    s.backspace();
                }
                while s.cursor.1 > 0
                    && s.editor[row]
                        .graphemes(true)
                        .nth(s.cursor.1 - 1)
                        .is_some_and(|g| !g.chars().all(char::is_whitespace))
                {
                    s.backspace();
                }
            }
            KeyCode::Left => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectLeft
                } else {
                    story_core::EditMacroOp::MoveLeft
                });
                s.move_editor_cursors(CursorMotion::Left, selecting)
            }
            KeyCode::Right => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectRight
                } else {
                    story_core::EditMacroOp::MoveRight
                });
                s.move_editor_cursors(CursorMotion::Right, selecting)
            }
            KeyCode::Up => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectUp
                } else {
                    story_core::EditMacroOp::MoveUp
                });
                s.move_editor_cursors(CursorMotion::Up, selecting)
            }
            KeyCode::Down => {
                s.record_macro_op(if selecting {
                    story_core::EditMacroOp::SelectDown
                } else {
                    story_core::EditMacroOp::MoveDown
                });
                s.move_editor_cursors(CursorMotion::Down, selecting)
            }
            KeyCode::Char(c)
                if !k.modifiers.contains(KeyModifiers::CONTROL)
                    && !k.modifiers.contains(KeyModifiers::ALT) =>
            {
                s.insert(c)
            }
            KeyCode::Esc => s.switch(Workspace::Outline),
            _ => {}
        }
        return Ok(false);
    }
    match k.code {
        KeyCode::F(n) if (1..=14).contains(&n) => {
            if let Some((_, workspace)) =
                s.function_bindings.iter().find(|(number, _)| *number == n)
                && let Some(spec) = command_for_workspace(*workspace)
            {
                s.dispatch_registered(spec.action)?;
            }
        }
        KeyCode::Char(':') => {
            s.mode = Some(':');
            s.prompt.clear();
        }
        KeyCode::Char('/') => {
            s.mode = Some('/');
            s.prompt.clear();
        }
        KeyCode::Char('?') => {
            if let Some(command) = command_by_id("help.context") {
                s.dispatch_registered(command.action)?;
            }
        }
        KeyCode::Tab => {
            let i = Workspace::ALL
                .iter()
                .position(|x| *x == s.workspace)
                .unwrap_or(0);
            s.switch(Workspace::ALL[(i + 1) % Workspace::ALL.len()]);
        }
        KeyCode::BackTab => {
            let i = Workspace::ALL
                .iter()
                .position(|x| *x == s.workspace)
                .unwrap_or(0);
            s.switch(Workspace::ALL[(i + Workspace::ALL.len() - 1) % Workspace::ALL.len()]);
        }
        KeyCode::Up | KeyCode::Char('k')
            if s.workspace == Workspace::Research && s.proposal_review_active =>
        {
            s.proposal_review_index = s.proposal_review_index.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if s.workspace == Workspace::Research && s.proposal_review_active =>
        {
            s.proposal_review_index =
                next_list_selection(s.proposal_review_index, workspace_items(s).len());
        }
        KeyCode::Enter if s.workspace == Workspace::Research && s.proposal_review_active => {}
        KeyCode::Esc if s.workspace == Workspace::Revisions && s.revision_diff.is_some() => {
            s.revision_diff = None;
            s.revision_scene = None;
            s.revision_hunk = 0;
            s.message.clear();
        }
        KeyCode::Char('[') if s.workspace == Workspace::Revisions => {
            s.revision_hunk = s.revision_hunk.saturating_sub(1);
        }
        KeyCode::Char(']') if s.workspace == Workspace::Revisions => {
            if let (Some(scene), Some(snapshot)) = (s.revision_scene, s.revision_diff) {
                let count = s
                    .app
                    .diff_snapshot(scene, snapshot)
                    .map(|diff| revision_hunks(&diff).len())
                    .unwrap_or_default();
                if count > 0 {
                    s.revision_hunk = (s.revision_hunk + 1).min(count - 1);
                }
            }
        }
        KeyCode::Char('r') if s.workspace == Workspace::Revisions && s.revision_diff.is_some() => {
            if let Some(command) = command_by_id("revision.restore-hunk")
                && let Err(error) = s.dispatch_registered(command.action)
            {
                s.message = error.to_string();
            }
        }
        KeyCode::Up | KeyCode::Char('k') if s.workspace == Workspace::Revisions => {
            s.revision_index = s.revision_index.saturating_sub(1);
            s.revision_diff = None;
            s.revision_scene = None;
            s.revision_hunk = 0;
        }
        KeyCode::Down | KeyCode::Char('j') if s.workspace == Workspace::Revisions => {
            s.revision_index = next_list_selection(s.revision_index, workspace_items(s).len());
            s.revision_diff = None;
            s.revision_scene = None;
            s.revision_hunk = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => s.selected = s.selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => {
            s.selected = next_list_selection(s.selected, workspace_items(s).len());
        }
        KeyCode::Enter if s.workspace == Workspace::Revisions => {
            if let Some((scene, snapshot)) = revision_items(s).get(s.revision_index) {
                s.revision_scene = Some(*scene);
                s.revision_diff = Some(snapshot.id);
                s.revision_hunk = 0;
                s.scroll = 0;
                s.message = format!("Diff for {}", snapshot.label);
            }
        }
        KeyCode::Enter if s.workspace == Workspace::Timeline => {
            let timeline = timeline_items(&s.app);
            let item = timeline
                .get(s.selected)
                .cloned()
                .ok_or("timeline selection is out of range")?;
            match item {
                TimelineItem::Scene(scene) => {
                    s.selected = s
                        .scenes
                        .iter()
                        .position(|row| row.0 == scene.scene.id)
                        .ok_or("timeline scene no longer exists")?;
                    s.open_selected()?;
                }
                TimelineItem::Event(event) => {
                    let event_index = s
                        .app
                        .list_events()?
                        .iter()
                        .position(|row| row.id == event.id)
                        .ok_or("timeline event no longer exists")?;
                    s.selected = s.app.list_characters()?.len()
                        + s.app.list_plotlines()?.len()
                        + event_index;
                    s.switch(Workspace::StoryBible);
                    s.message = format!("Selected story event {}", event.title);
                }
            }
        }
        KeyCode::Enter => s.open_selected()?,
        _ => {}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mention_highlighting_uses_utf8_byte_ranges_and_does_not_mark_adjacent_text() {
        let text = "Éowyn met Éomer";
        let start = text.find("Éomer").unwrap() as u64;
        let end = start + "Éomer".len() as u64;
        assert!(editor_grapheme_is_mentioned(
            start,
            "É".len() as u64,
            &[(start, end)]
        ));
        assert!(!editor_grapheme_is_mentioned(start - 1, 1, &[(start, end)]));
        assert!(!editor_grapheme_is_mentioned(end, 1, &[(start, end)]));
    }

    #[test]
    fn command_shortcuts_load_from_registry_and_dispatch_without_replacing_slash_input() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Keys"), "Keys").unwrap();
        let config = project.project_root().join(".writers-ide/keybindings.toml");
        fs::write(&config, "[commands]\n\"Ctrl+Alt+A\" = \"view.analytics\"\n").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.command_bindings = load_command_bindings(state.app.project_root()).unwrap();
        handle(
            &mut state,
            KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
        )
        .unwrap();
        assert_eq!(state.workspace, Workspace::Analytics);
        state.workspace = Workspace::Editor;
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.editor.join("\n").contains('/'));
    }

    #[test]
    fn project_keybinding_commands_validate_persist_and_reload_immediately() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Keys"), "Keys").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state
            .execute_legacy_palette_command("keybinding-set Ctrl+Alt+K view.analytics")
            .unwrap();
        assert_eq!(state.command_bindings.len(), 1);
        handle(
            &mut state,
            KeyEvent::new(
                KeyCode::Char('k'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
        )
        .unwrap();
        assert_eq!(state.workspace, Workspace::Analytics);
        assert!(
            state
                .execute_legacy_palette_command("keybinding-set Ctrl+/ view.editor")
                .is_err()
        );
        assert!(
            state
                .execute_legacy_palette_command("keybinding-set Ctrl+Alt+J compile.selected")
                .is_err()
        );
        state
            .execute_legacy_palette_command("keybinding-remove Ctrl+Alt+K")
            .unwrap();
        assert!(state.command_bindings.is_empty());
        let config: KeymapFile = toml::from_str(
            &fs::read_to_string(
                state
                    .app
                    .project_root()
                    .join(".writers-ide/keybindings.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(config.commands.is_empty());
    }

    #[test]
    fn command_shortcuts_reject_duplicates_reserved_keys_unknown_and_argument_commands() {
        assert!(parse_shortcut("A").is_err());
        assert!(parse_shortcut("Ctrl+Alt+F10").is_err());
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Keys"), "Keys").unwrap();
        let config = project.project_root().join(".writers-ide/keybindings.toml");
        fs::write(&config, "[commands]\n\"Ctrl+/\" = \"view.editor\"\n").unwrap();
        assert!(load_command_bindings(project.project_root()).is_err());
        fs::write(
            &config,
            "[commands]\n\"Ctrl+Alt+A\" = \"unknown.command\"\n",
        )
        .unwrap();
        assert!(load_command_bindings(project.project_root()).is_err());
        fs::write(
            &config,
            "[commands]\n\"Ctrl+Alt+A\" = \"compile.selected\"\n",
        )
        .unwrap();
        assert!(load_command_bindings(project.project_root()).is_err());
        fs::write(
            &config,
            "[commands]\n\"Ctrl+Alt+A\" = \"view.editor\"\n\"Ctrl+Alt+a\" = \"view.analytics\"\n",
        )
        .unwrap();
        assert!(load_command_bindings(project.project_root()).is_err());
    }

    #[test]
    fn command_registry_has_stable_unique_ids_and_compile_taxonomy() {
        let registry = command_registry();
        assert_eq!(
            command_by_id("extract.dismiss").unwrap().palette_prefix,
            Some("dismiss")
        );
        assert_eq!(
            command_by_id("extract.review").unwrap().palette_prefix,
            Some("proposals")
        );
        let ids = registry
            .iter()
            .map(|command| command.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), registry.len());
        let mut aliases = BTreeSet::new();
        assert!(registry.iter().all(|command| {
            command
                .palette_aliases
                .iter()
                .copied()
                .chain(command.palette_prefix)
                .all(|alias| aliases.insert(alias))
        }));
        let compile = command_by_id("compile.run").unwrap();
        assert_eq!(compile.label, "Compile");
        assert_eq!(compile.category, "Compile");
        assert_eq!(command_palette_invocation("process").unwrap().0, compile);
        assert_eq!(
            command_palette_invocation("compile-format").unwrap().0.id,
            "compile.format"
        );
        assert_eq!(
            command_palette_invocation("review-merge").unwrap().0.id,
            "review.merge"
        );
        assert_eq!(
            command_by_id("compile.selected").unwrap().category,
            "Compile"
        );
        assert_eq!(
            command_palette_invocation("git-history").unwrap().0.id,
            "git.history"
        );
        assert_eq!(
            command_palette_invocation("git-snapshot Before edits")
                .unwrap()
                .0
                .id,
            "git.snapshot"
        );
        for (invocation, expected) in [
            ("snippet greet ${cursor}", "edit.snippet-save"),
            ("snippet-insert greet", "edit.snippet-insert"),
            ("register-save a", "edit.register-save"),
            ("multi-insert 0,4 x", "edit.multi-insert"),
            ("macro upper", "edit.macro"),
        ] {
            assert_eq!(
                command_palette_invocation(invocation).unwrap().0.id,
                expected
            );
        }
        assert_eq!(
            command_palette_invocation("process-only").unwrap().0.action,
            CommandAction::ProcessSelected {
                expand_transclusions: false
            }
        );
        let (selected, arguments) =
            command_palette_invocation("process-only export,continuity").unwrap();
        assert_eq!(
            selected.action,
            command_by_id("compile.selected").unwrap().action
        );
        assert_eq!(arguments, Some("export,continuity"));
        assert!(command_palette_invocation("compile extra-arguments").is_none());
        let (move_scene, move_args) = command_palette_invocation(
            "move 00000000-0000-0000-0000-000000000001 00000000-0000-0000-0000-000000000002 0",
        )
        .unwrap();
        assert_eq!(move_scene.id, "scene.move");
        assert_eq!(move_scene.action, CommandAction::LegacyPalette);
        assert_eq!(
            move_args,
            Some("00000000-0000-0000-0000-000000000001 00000000-0000-0000-0000-000000000002 0")
        );
        assert_eq!(
            command_palette_invocation("proposals").unwrap().0.id,
            "extract.review"
        );
        let (dismiss, dismiss_args) = command_palette_invocation("dismiss review.json 2").unwrap();
        assert_eq!(dismiss.id, "extract.dismiss");
        assert_eq!(dismiss_args, Some("review.json 2"));
    }

    #[test]
    fn terminal_timed_challenge_records_session_and_never_deletes_steady_prose() {
        let root = tempfile::tempdir().unwrap();
        let mut project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        let scene = project.create_scene(chapter, "Scene", 0).unwrap();
        let (_, original_hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&original_hash), "existing manuscript prose")
            .unwrap();
        let today = time::OffsetDateTime::now_utc().date();
        project
            .create_writing_goal(&story_storage::NewWritingGoal {
                title: "Challenge ledger".into(),
                scope_scene_id: Some(scene),
                start_date: format!(
                    "{:04}-{:02}-{:02}",
                    today.year(),
                    u8::from(today.month()),
                    today.day()
                ),
                end_date: format!(
                    "{:04}-{:02}-{:02}",
                    today.year(),
                    u8::from(today.month()),
                    today.day()
                ),
                target_words: 100,
                baseline_words: 0,
                time_zone: "UTC".into(),
                word_count_policy: "unicode-word-boundaries".into(),
                success_metric: "raw".into(),
            })
            .unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.open_scene(scene).unwrap();
        state.prompt = format!("challenge-start 10 60 steady scene={scene}");
        state.command().unwrap();
        assert_eq!(state.active_challenge.as_ref().unwrap().mode, "steady");
        state.editor[0] = "three four".into();
        state.prompt = "challenge-finish".into();
        state.command().unwrap();
        assert_eq!(state.editor[0], "existing manuscript prose");
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "existing manuscript prose"
        );
        let challenges = state.app.list_writing_challenges().unwrap();
        assert_eq!(challenges[0].words_written, Some(2));
        assert_eq!(challenges[0].status, "abandoned");
        assert_eq!(challenges[0].outcome, "quit");
        let goal = state.app.list_writing_goals(false).unwrap().remove(0);
        let ledger = state.app.list_writing_goal_events(&goal.id).unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].origin, "write-or-die");
        assert_eq!(ledger[0].raw_delta, 2);
        assert_eq!(ledger[0].real_delta, 0);
        let draft_id = &challenges[0].id;
        assert!(
            list_challenge_drafts(state.app.project_root())
                .unwrap()
                .contains(draft_id)
        );
        state.prompt = format!("challenge-save-new {draft_id} {chapter} Challenge draft");
        state.command().unwrap();
        let saved_id = state
            .scenes
            .iter()
            .find(|(_, title, _, _)| title == "Challenge draft")
            .unwrap()
            .0;
        assert_eq!(state.app.load_scene_text(saved_id).unwrap().0, "three four");
    }

    #[test]
    fn recovered_challenge_can_be_explicitly_appended_to_associated_scene() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "existing prose")
            .unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.prompt = format!("challenge-start 10 60 pressure scene={scene}");
        state.command().unwrap();
        state.editor[0] = "challenge addition".into();
        state.prompt = "challenge-finish".into();
        state.command().unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "existing prose"
        );

        let id = state.app.list_writing_challenges().unwrap()[0].id.clone();
        state.prompt = format!("challenge-save-target {id}");
        state.command().unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "existing prose\n\nchallenge addition"
        );
        assert!(
            !challenge_draft_path(&state.app.project_root().join(".writers-ide/recovery"), &id)
                .exists()
        );
    }

    #[test]
    fn kamikaze_uses_two_grace_pauses_and_penalties_are_not_undoable() {
        let root = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.prompt = "challenge-idle 2".into();
        state.command().unwrap();
        assert_eq!(
            load_challenge_config(state.app.project_root())
                .unwrap()
                .kamikaze_idle_seconds,
            2
        );
        state.prompt = "challenge-start 10 60 kamikaze".into();
        state.command().unwrap();
        state.editor = vec!["one two three".into()];
        state.cursor = (0, 13);
        for remaining in [1, 0] {
            state.active_challenge.as_mut().unwrap().last_activity =
                Instant::now() - Duration::from_secs(3);
            assert!(!state.tick_challenge().unwrap());
            assert_eq!(
                state
                    .active_challenge
                    .as_ref()
                    .unwrap()
                    .grace_pauses_remaining,
                remaining
            );
            assert_eq!(state.editor[0], "one two three");
        }
        state.active_challenge.as_mut().unwrap().last_activity =
            Instant::now() - Duration::from_secs(3);
        assert!(state.tick_challenge().unwrap());
        assert_eq!(state.editor[0], "one two ");
        assert!(state.message.contains("after 2s"));
        state.undo_edit();
        assert_eq!(state.editor[0], "one two ");

        state.active_challenge.as_mut().unwrap().last_activity =
            Instant::now() - Duration::from_secs(3);
        state.tick_challenge().unwrap();
        assert_eq!(state.editor[0], "");
        assert_eq!(state.active_challenge.as_ref().unwrap().words_deleted, 3);

        let footer = challenge_footer(
            state.active_challenge.as_ref().unwrap(),
            &["one two three".into()],
        );
        assert!(footer.contains("3/10 words"));
        assert!(footer.contains("kamikaze in 2s"));
    }

    #[test]
    fn challenge_penalty_deletes_progressively_and_respects_unicode_graphemes() {
        assert_eq!(remove_last_words("Élodie met 🌙", 1), "Élodie met ");
        assert_eq!(remove_last_words("one two three", 2), "one ");
        assert_eq!(remove_last_words("", 3), "");
        let short = parse_challenge_command("challenge-preset short kamikaze")
            .unwrap()
            .unwrap();
        assert_eq!((short.target_words, short.duration_seconds), (250, 300));
        assert_eq!(short.mode, "kamikaze");
        let custom = parse_challenge_command("challenge-start 100 120 pressure")
            .unwrap()
            .unwrap();
        assert_eq!((custom.target_words, custom.duration_seconds), (100, 120));
        assert_eq!(custom.mode, "pressure");
        assert_eq!(custom.destination_scene, None);
        let scene = SceneId(uuid::Uuid::nil());
        let associated =
            parse_challenge_command(&format!("challenge-start 100 120 kamikaze scene={scene}"))
                .unwrap()
                .unwrap();
        assert_eq!(associated.destination_scene, Some(scene));
    }

    #[test]
    fn interface_navigation_and_resize_pause_kamikaze_inactivity_clock() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.prompt = "challenge-start 10 600 kamikaze".into();
        state.command().unwrap();
        state.editor[0] = "keep this draft".into();
        state.workspace = Workspace::Dashboard;
        state.active_challenge.as_mut().unwrap().last_activity =
            Instant::now() - Duration::from_secs(30);
        assert!(!state.tick_challenge().unwrap());
        assert_eq!(state.editor[0], "keep this draft");
        assert_eq!(
            state
                .active_challenge
                .as_ref()
                .unwrap()
                .grace_pauses_remaining,
            2
        );

        state.pause_challenge_inactivity();
        assert!(
            state
                .active_challenge
                .as_ref()
                .unwrap()
                .last_activity
                .elapsed()
                < Duration::from_secs(1)
        );
    }

    #[test]
    fn challenge_grace_and_progressive_deletion_settings_are_captured_per_session() {
        let root = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.prompt = "challenge-pauses 0".into();
        state.command().unwrap();
        state.prompt = "challenge-delete-rate 2".into();
        state.command().unwrap();
        state.prompt = "challenge-taunt {words} retained; {deleted} lost".into();
        state.command().unwrap();
        state.prompt = "challenge-start 20 600 kamikaze".into();
        state.command().unwrap();
        assert_eq!(
            state
                .active_challenge
                .as_ref()
                .unwrap()
                .grace_pauses_remaining,
            0
        );
        assert_eq!(
            state
                .active_challenge
                .as_ref()
                .unwrap()
                .deletion_words_per_penalty,
            2
        );
        assert_eq!(
            state.active_challenge.as_ref().unwrap().failure_taunt,
            "{words} retained; {deleted} lost"
        );
        assert_eq!(render_challenge_taunt("{words}/{deleted}", 9, 4), "9/4");
        state.editor = vec!["one two three".into()];
        state.active_challenge.as_mut().unwrap().last_activity =
            Instant::now() - Duration::from_secs(11);
        state.tick_challenge().unwrap();
        assert_eq!(state.editor[0], "one ");
    }

    #[test]
    fn isolated_challenge_draft_is_recoverable_after_project_reopen() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Book");
        let project = WriterApp::create_project(&project_path, "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.prompt = "challenge-start 100 600 pressure".into();
        state.command().unwrap();
        state.editor = vec!["surviving challenge text".into()];
        state.persist_challenge_draft().unwrap();
        let challenge_id = state.active_challenge.as_ref().unwrap().id.clone();
        drop(state);

        let mut reopened = WriterApp::open_project(&project_path).unwrap();
        reopened.recover_interrupted_writing_challenges().unwrap();
        let recovered = read_challenge_draft(reopened.project_root(), &challenge_id).unwrap();
        assert_eq!(recovered.text, "surviving challenge text");
        let interrupted = reopened
            .list_writing_challenges()
            .unwrap()
            .into_iter()
            .find(|challenge| challenge.id == challenge_id)
            .unwrap();
        assert_eq!(interrupted.status, "abandoned");
        assert_eq!(interrupted.outcome, "quit");
        assert!(
            list_challenge_drafts(reopened.project_root())
                .unwrap()
                .contains(&challenge_id)
        );
    }

    #[test]
    fn challenge_success_is_recorded_and_early_quit_offers_save_or_discard() {
        let root = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.prompt = "challenge-start 2 600 pressure".into();
        state.command().unwrap();
        state.editor[0] = "two words".into();
        state.prompt = "challenge-finish".into();
        state.command().unwrap();
        let completed = state.app.list_writing_challenges().unwrap();
        assert_eq!(completed[0].outcome, "success");
        assert!(state.message.contains("Success: 2/2 words"));
        state.prompt = format!("challenge-discard {}", completed[0].id);
        state.command().unwrap();

        state.prompt = "challenge-start 100 600 kamikaze".into();
        state.command().unwrap();
        let quit_id = state.active_challenge.as_ref().unwrap().id.clone();
        let quit = handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(!quit);
        assert_eq!(state.mode, Some('c'));
        assert!(state.prompt.contains("Discard and quit"));
        let quit = handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        )
        .unwrap();
        assert!(quit);
        let quit_record = state
            .app
            .list_writing_challenges()
            .unwrap()
            .into_iter()
            .find(|challenge| challenge.id == quit_id)
            .unwrap();
        assert_eq!(quit_record.outcome, "quit");
        assert_eq!(
            list_challenge_drafts(state.app.project_root()).unwrap(),
            "No recoverable challenge drafts"
        );

        state.prompt = "challenge-start 100 600 pressure".into();
        state.command().unwrap();
        state.editor[0] = "save this draft".into();
        let quit_id = state.active_challenge.as_ref().unwrap().id.clone();
        assert!(
            !handle(
                &mut state,
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            )
            .unwrap()
        );
        assert!(
            handle(
                &mut state,
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            )
            .unwrap()
        );
        assert_eq!(
            read_challenge_draft(state.app.project_root(), &quit_id)
                .unwrap()
                .text,
            "save this draft"
        );
    }

    #[test]
    fn editor_tools_persist_and_native_extra_cursors_edit_together() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.editor = vec!["ab".into(), "cd".into()];
        state.cursor = (0, 1);
        state.prompt = "cursor-add 1 1".into();
        state.command().unwrap();
        state.insert('X');
        assert_eq!(state.editor, ["aXb", "cXd"]);
        state.prompt = "snippet greeting Hello ${name}.${cursor}".into();
        state.command().unwrap();
        state.prompt = "macro-define shout upper|insert:!".into();
        state.command().unwrap();
        state.registers.insert("a".into(), "Saved text".into());
        state.persist_editor_tools().unwrap();
        let reopened = WriterApp::open_project(root.path().join("Book")).unwrap();
        let loaded = AppState::new(reopened, default_bindings());
        assert!(loaded.snippets.contains_key("greeting"));
        assert_eq!(
            loaded.macros["shout"],
            [
                story_core::EditMacroOp::UppercaseSelection,
                story_core::EditMacroOp::Insert("!".into())
            ]
        );
        assert_eq!(loaded.registers["a"], "Saved text");
    }

    #[test]
    fn recorded_edit_macro_persists_insertions_and_cursor_edits() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.prompt = "macro-record punctuate".into();
        state.command().unwrap();
        state.insert('!');
        state.record_macro_op(story_core::EditMacroOp::MoveLeft);
        state.backspace();
        state.prompt = "macro-stop".into();
        state.command().unwrap();
        assert_eq!(
            state.macros["punctuate"],
            [
                story_core::EditMacroOp::Insert("!".into()),
                story_core::EditMacroOp::MoveLeft,
                story_core::EditMacroOp::Backspace,
            ]
        );
        let reopened = WriterApp::open_project(root.path().join("Book")).unwrap();
        let state = AppState::new(reopened, default_bindings());
        assert_eq!(state.macros["punctuate"].len(), 3);
    }

    #[test]
    fn command_macros_record_registry_ids_persist_and_replay_via_dispatcher() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Book");
        let project = WriterApp::create_project(&project_path, "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.prompt = "command-macro-record add_character".into();
        state.command().unwrap();
        state.prompt = "character Mara".into();
        state.command().unwrap();
        assert_eq!(state.app.list_characters().unwrap().len(), 1);
        assert_eq!(state.recording_command_macro.as_ref().unwrap().1.len(), 1);
        assert_eq!(
            state.recording_command_macro.as_ref().unwrap().1[0].command_id,
            "character.create"
        );
        drop(state);

        let reopened = WriterApp::open_project(&project_path).unwrap();
        let mut state = AppState::new(reopened, default_bindings());
        assert_eq!(
            state.recording_command_macro.as_ref().unwrap().1.len(),
            1,
            "in-progress command recording is recovered after reopening"
        );
        state.prompt = "command-macro-stop".into();
        state.command().unwrap();
        drop(state);

        let reopened = WriterApp::open_project(&project_path).unwrap();
        let mut state = AppState::new(reopened, default_bindings());
        assert_eq!(state.command_macros["add_character"].len(), 1);
        state.prompt = "command-macro-run add_character".into();
        state.command().unwrap();
        assert_eq!(state.app.list_characters().unwrap().len(), 2);
        assert!(
            state
                .message
                .contains("Replayed command macro add_character")
        );
    }

    #[test]
    fn command_macro_rejects_a_stale_registry_invocation_before_running_steps() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.command_macros.insert(
            "stale".into(),
            vec![RecordedCommand {
                command_id: "character.create".into(),
                invocation: "location Hidden Place".into(),
            }],
        );
        state.prompt = "command-macro-run stale".into();
        assert!(state.command().is_err());
        assert!(state.app.list_characters().unwrap().is_empty());
        assert!(state.app.list_locations().unwrap().is_empty());
    }

    #[test]
    fn destructive_repeated_macros_preview_then_apply_as_one_undoable_edit() {
        let root = tempfile::tempdir().unwrap();
        let mut project = WriterApp::create_project(root.path().join("Macro"), "Macro").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        let scene = project.create_scene(chapter, "Scene", 0).unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.open_scene(scene).unwrap();
        state.editor = vec!["word".into()];
        state.dirty = true;
        state
            .macros
            .insert("trim".into(), vec![story_core::EditMacroOp::Backspace]);
        state.prompt = "macro-run trim 2".into();
        state.command().unwrap();
        assert_eq!(state.editor, ["word"]);
        assert!(state.message.contains("repeat macro-run trim 2 to confirm"));
        state.prompt = "macro-run trim 2".into();
        state.command().unwrap();
        assert_eq!(state.editor, ["wo"]);
        assert!(
            state
                .app
                .git_history(5)
                .unwrap()
                .iter()
                .any(|commit| { commit.subject == "Before repeated macro trim" })
        );
        state.undo_edit();
        assert_eq!(state.editor, ["word"]);
        assert_eq!(
            parse_macro_run_arguments("trim 101"),
            Err("use macro-run NAME [REPETITIONS], with repetitions from 1 to 100".into())
        );
    }

    #[test]
    fn text_expansion_is_configurable_undoable_contextual_and_not_applied_to_paste() {
        let root = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(root.path().join("Text Expansion"), "Text Expansion")
            .unwrap();
        let mut config = TextExpansionConfig::default();
        config.triggers.push("tab".into());
        config
            .mappings
            .insert("%DWAP%".into(), "Department of Work and Pensions".into());
        config.mappings.insert("%D%".into(), "short".into());
        save_text_expansion(app.project_root(), &config).unwrap();
        let mut state = AppState::new(app, default_bindings());

        state.editor = vec!["%DWAP%".into()];
        state.cursor = (0, 6);
        state.push_undo();
        state.insert(' ');
        assert_eq!(state.editor, ["Department of Work and Pensions "]);
        state.undo_edit();
        assert_eq!(state.editor, ["%DWAP%"]);

        state.editor = vec!["word%D%".into()];
        state.cursor = (0, 7);
        state.push_undo();
        state.insert(' ');
        assert_eq!(state.editor, ["word%D% "]);

        state.editor = vec!["%D%".into()];
        state.cursor = (0, 3);
        state.paste_text(" ");
        assert_eq!(state.editor, ["%D% "]);

        state.editor = vec!["%D%".into()];
        state.cursor = (0, 3);
        state.push_undo();
        state.insert('\t');
        assert_eq!(state.editor, ["short\t"]);

        state.editor = vec!["%D%".into()];
        state.cursor = (0, 3);
        assert!(state.expand_text_before_cursor("", "", true));
        assert_eq!(state.editor, ["short"]);

        state.editor = vec!["%DWAP%".into()];
        state.cursor = (0, 6);
        state.enter();
        assert_eq!(state.editor, ["Department of Work and Pensions", ""]);
        let reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        assert_eq!(
            reopened.text_expansion.mappings["%DWAP%"],
            "Department of Work and Pensions"
        );
    }

    #[test]
    fn named_manuscript_bookmark_survives_restart_and_scene_move() {
        let root = tempfile::tempdir().unwrap();
        let mut app =
            WriterApp::create_project(root.path().join("Bookmarks"), "Bookmarks").unwrap();
        let first_chapter = app.create_chapter(None, "First", 0).unwrap();
        let second_chapter = app.create_chapter(None, "Second", 1).unwrap();
        let scene = app.create_scene(first_chapter, "Road", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "The old road bends.\nA gate stands.")
            .unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.open_scene(scene).unwrap();
        state.editor_selection = Some((0, 4));
        state.cursor = (0, 12);
        state
            .execute_legacy_palette_command("manuscript-bookmark-save old-road")
            .unwrap();
        state.app.move_scene(scene, second_chapter, 0).unwrap();

        let mut reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        reopened
            .execute_legacy_palette_command("manuscript-bookmark-jump old-road")
            .unwrap();
        assert_eq!(reopened.editing_scene, Some(scene));
        assert_eq!(reopened.cursor, (0, 4));
        assert_eq!(reopened.editor[0], "The old road bends.");
        reopened
            .execute_legacy_palette_command("manuscript-bookmark-list")
            .unwrap();
        assert!(reopened.message.contains("old-road"));
    }

    #[test]
    fn kill_ring_persists_and_yank_pop_is_undoable() {
        let root = tempfile::tempdir().unwrap();
        let project =
            WriterApp::create_project(root.path().join("Kill Ring"), "Kill Ring").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.remember_kill("first passage".into()).unwrap();
        state.remember_kill("second passage".into()).unwrap();
        state.editor = vec!["Before: ".into()];
        state.cursor = (0, 8);
        state.insert_kill_ring_entry(0).unwrap();
        assert_eq!(state.editor, ["Before: second passage"]);
        state.yank_pop().unwrap();
        assert_eq!(state.editor, ["Before: first passage"]);
        state.undo_edit();
        assert_eq!(state.editor, ["Before: second passage"]);

        let reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        assert_eq!(reopened.kill_ring, ["second passage", "first passage"]);
    }

    #[test]
    fn project_editor_preferences_apply_line_endings_and_tab_indentation() {
        let root = tempfile::tempdir().unwrap();
        let mut app =
            WriterApp::create_project(root.path().join("Preferences"), "Preferences").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state
            .execute_legacy_palette_command("editor-preferences line-ending crlf")
            .unwrap();
        state
            .execute_legacy_palette_command("editor-preferences indentation spaces")
            .unwrap();
        state
            .execute_legacy_palette_command("editor-preferences indent-width 2")
            .unwrap();
        state.open_scene(scene).unwrap();
        state.editor = vec!["first".into(), "second".into()];
        state.dirty = true;
        state.save().unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "first\r\nsecond"
        );
        state.editor = vec![String::new()];
        state.cursor = (0, 0);
        state.insert('\t');
        assert_eq!(state.editor, ["  "]);
        let reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        assert_eq!(reopened.editor_preferences.line_ending, "crlf");
        assert_eq!(reopened.editor_preferences.indentation, "spaces");
        assert_eq!(reopened.editor_preferences.indent_width, 2);
    }

    #[test]
    fn recorded_shift_selection_replays_as_a_text_replacement() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.switch(Workspace::Editor);
        state.editor = vec!["abc".into()];
        state.cursor = (0, 3);
        state.prompt = "macro-record replace-last".into();
        state.command().unwrap();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
        )
        .unwrap();
        state.insert('X');
        state.prompt = "macro-stop".into();
        state.command().unwrap();
        assert_eq!(
            state.macros["replace-last"],
            [
                story_core::EditMacroOp::SelectLeft,
                story_core::EditMacroOp::Insert("X".into())
            ]
        );

        state.editor = vec!["def".into()];
        state.cursor = (0, 3);
        state.editor_selection = None;
        state
            .apply_edit_macro(&state.macros["replace-last"].clone())
            .unwrap();
        assert_eq!(state.editor, ["deX"]);
    }

    #[test]
    fn named_registers_rotate_transactionally_and_persist() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.registers.extend([
            ("a".into(), "one".into()),
            ("b".into(), "two".into()),
            ("c".into(), "three".into()),
        ]);
        state.prompt = "register-rotate a b c".into();
        state.command().unwrap();
        assert_eq!(state.registers["a"], "three");
        assert_eq!(state.registers["b"], "one");
        assert_eq!(state.registers["c"], "two");
        state.prompt = "register-rotate a a".into();
        assert!(state.command().is_err());
        let reopened = WriterApp::open_project(root.path().join("Book")).unwrap();
        let state = AppState::new(reopened, default_bindings());
        assert_eq!(state.registers["a"], "three");
        assert_eq!(state.registers["b"], "one");
        assert_eq!(state.registers["c"], "two");
    }

    #[test]
    fn multi_cursors_move_and_delete_graphemes_across_lines_together() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.editor = vec!["ab".into(), "cd".into()];
        state.cursor = (0, 2);
        state.extra_cursors = vec![(1, 2)];
        state.move_editor_cursors(CursorMotion::Left, false);
        assert_eq!(state.cursor, (0, 1));
        assert_eq!(state.extra_cursors, [(1, 1)]);
        state.backspace();
        assert_eq!(state.editor, ["b", "d"]);
        assert_eq!(state.cursor, (0, 0));
        assert_eq!(state.extra_cursors, [(1, 0)]);
        state.undo_edit();
        assert_eq!(state.editor, ["ab", "cd"]);
    }

    #[test]
    fn writing_challenge_graph_is_bounded_and_exposes_target_progress() {
        let rows = challenge_progress_rows(&[
            story_storage::WritingChallengeRecord {
                id: "a".into(),
                started_at: 1,
                deadline_at: 60,
                target_words: 10,
                mode: "steady".into(),
                ended_at: Some(30),
                words_written: Some(5),
                status: "completed".into(),
                words_deleted: 2,
                pauses_consumed: 2,
                outcome: "timeout".into(),
            },
            story_storage::WritingChallengeRecord {
                id: "b".into(),
                started_at: 2,
                deadline_at: 60,
                target_words: 10,
                mode: "steady".into(),
                ended_at: Some(30),
                words_written: Some(100),
                status: "completed".into(),
                words_deleted: 0,
                pauses_consumed: 0,
                outcome: "success".into(),
            },
        ]);
        assert!(rows[0].contains("5/10 words"));
        assert!(rows[0].contains("██████████░░░░░░░░░░"));
        assert!(rows[1].contains("████████████████████"));
    }

    #[test]
    fn daily_writing_graph_groups_sessions_and_accumulates_words_by_utc_date() {
        let record =
            |id: &str, started_at: i64, words_written| story_storage::WritingChallengeRecord {
                id: id.into(),
                started_at,
                deadline_at: started_at + 60,
                target_words: 100,
                mode: "steady".into(),
                ended_at: Some(started_at + 30),
                words_written,
                status: "completed".into(),
                words_deleted: 0,
                pauses_consumed: 0,
                outcome: "success".into(),
            };
        let rows = daily_writing_rows(&[
            record("first", 1_700_000_000, Some(120)),
            record("second", 1_700_000_500, Some(80)),
            record("third", 1_700_086_400, Some(100)),
            record("active", 1_700_086_500, None),
        ]);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("200 words · 2 sessions · 200 cumulative"));
        assert!(rows[1].contains("100 words · 1 session · 300 cumulative"));
    }

    #[test]
    fn challenge_streaks_track_success_days_and_ignore_failures() {
        let today = 20_000i64;
        let record =
            |id: &str, day: i64, outcome: &str, words| story_storage::WritingChallengeRecord {
                id: id.into(),
                started_at: day * 86_400,
                deadline_at: day * 86_400 + 600,
                target_words: 100,
                mode: "steady".into(),
                ended_at: Some(day * 86_400 + 300),
                words_written: words,
                status: "completed".into(),
                words_deleted: 0,
                pauses_consumed: 0,
                outcome: outcome.into(),
            };
        let challenges = [
            record("a", today - 5, "success", Some(100)),
            record("b", today - 4, "success", Some(100)),
            record("failure", today - 3, "timeout", Some(50)),
            record("c", today - 1, "success", Some(100)),
            record("d", today, "success", Some(100)),
            record("zero", today, "success", Some(0)),
        ];
        let rows = challenge_streak_rows(&challenges, today);
        assert!(rows[0].contains("current 2 days"));
        assert!(rows[0].contains("best 2 days"));
        assert!(challenge_streak_rows(&challenges, today + 3)[0].contains("current 0 days"));
        assert!(challenge_streak_rows(&[], today)[0].contains("no successful"));
    }

    #[test]
    fn general_writing_session_records_scene_net_words_and_analytics_rows() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut app = project;
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.scenes = vec![(scene, "Scene".into(), SceneStatus::Idea, 0)];
        state.editor = vec!["one two".into()];
        state.editing_scene = Some(scene);
        state
            .execute_legacy_palette_command("writing-stats on")
            .unwrap();
        state.editor = vec!["one two three four".into()];
        state
            .execute_legacy_palette_command("writing-stats off")
            .unwrap();
        state
            .execute_legacy_palette_command("writing-stats on")
            .unwrap();
        state.editor = vec!["one two three four five".into()];
        state
            .execute_legacy_palette_command("writing-stats off")
            .unwrap();
        let sessions = state.app.list_writing_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(
            sessions
                .iter()
                .all(|session| session.scene_id == Some(scene))
        );
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.words_delta.unwrap())
                .sum::<i64>(),
            3
        );
        assert!(scene_writing_rows(&sessions, &[])[0].contains("+3 net words"));
        assert!(
            scene_writing_rows(&sessions, &[])
                .iter()
                .any(|row| row.contains("Scene history"))
        );
        assert!(
            daily_session_rows(&sessions)
                .iter()
                .any(|row| row.contains("cumulative"))
        );
    }

    #[test]
    fn custom_goal_commands_create_events_and_show_daily_progress() {
        let root = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(app, default_bindings());
        let today = time::OffsetDateTime::now_utc().date();
        let date = format!(
            "{:04}-{:02}-{:02}",
            today.year(),
            u8::from(today.month()),
            today.day()
        );
        state
            .execute_legacy_palette_command(&format!(
                "goal-create {date} {date} 500 100 UTC real project Drafting sprint"
            ))
            .unwrap();
        let goal = state.app.list_writing_goals(false).unwrap().remove(0);
        assert_eq!(goal.title, "Drafting sprint");
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        state
            .execute_legacy_palette_command(&format!(
                "goal-record {} {now} manual fixture-source abc123 none 90 75 Hand-entered count",
                goal.id
            ))
            .unwrap();
        assert_eq!(
            state.app.list_writing_goal_events(&goal.id).unwrap().len(),
            1
        );
        state
            .execute_legacy_palette_command(&format!("goal-progress {}", goal.id))
            .unwrap();
        assert!(state.message.contains("target=500"));
        assert!(state.message.contains("real=175"));
    }

    #[test]
    fn scene_writing_analytics_report_focused_time_and_completed_productivity() {
        let scene = story_core::SceneId::new();
        let session = story_storage::WritingSessionRecord {
            id: "timed-scene-session".into(),
            started_at: 1_000,
            ended_at: Some(4_600),
            scene_id: Some(scene),
            start_words: 100,
            end_words: Some(160),
            words_delta: Some(60),
            status: "completed".into(),
        };
        let rows = scene_writing_rows(&[session], &[]);
        assert!(rows.iter().any(|row| {
            row.contains("60 completed words/hour") && row.contains("1h 0m focused time")
        }));
        assert!(rows.iter().any(|row| row.contains("1h 0m focused")));
    }

    #[test]
    fn writing_streaks_count_distinct_positive_completed_days() {
        let today = 20_000i64;
        let sessions = [today - 6, today - 5, today - 4, today - 1, today]
            .into_iter()
            .enumerate()
            .map(|(index, day)| story_storage::WritingSessionRecord {
                id: format!("session-{index}"),
                started_at: day * 86_400,
                ended_at: Some(day * 86_400 + 60),
                scene_id: None,
                start_words: 0,
                end_words: Some(10),
                words_delta: Some(10),
                status: "completed".into(),
            })
            .chain(std::iter::once(story_storage::WritingSessionRecord {
                id: "abandoned".into(),
                started_at: (today - 2) * 86_400,
                ended_at: Some((today - 2) * 86_400 + 60),
                scene_id: None,
                start_words: 0,
                end_words: Some(50),
                words_delta: Some(50),
                status: "abandoned".into(),
            }))
            .collect::<Vec<_>>();

        let rows = writing_streak_rows(&sessions, today);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("current 2 days"));
        assert!(rows[0].contains("best 3 days"));
        assert!(writing_streak_rows(&[], today)[0].contains("no completed"));
        assert!(writing_streak_rows(&sessions, today + 3)[0].contains("current 0 days"));
    }

    #[test]
    fn coverage_rules_are_project_local_persistent_and_read_only() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        let character_id = story_core::CharacterId::new().to_string();
        state.prompt =
            format!("coverage-add protag character {character_id} 2 Protagonist coverage");
        state.command().unwrap();
        assert!(state.message.contains("Added coverage requirement"));
        let rows = coverage_report_rows(state.app.project_root(), &[]);
        assert!(rows[0].contains("GAP"));
        state.prompt = "coverage-list".into();
        state.command().unwrap();
        assert!(
            state
                .message
                .contains("protag character Protagonist coverage ≥2")
        );
        state.prompt = "coverage-remove protag".into();
        state.command().unwrap();
        assert_eq!(
            load_coverage_config(state.app.project_root())
                .unwrap()
                .requirements
                .len(),
            0
        );
    }

    #[test]
    fn terminal_manages_and_checks_persistent_location_rules() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        let first = state.app.create_location("Bridge").unwrap();
        let second = state.app.create_location("Vault").unwrap();
        state
            .execute_legacy_palette_command(&format!(
                "location-rule-add {first} {second} impossible together"
            ))
            .unwrap();
        assert!(state.message.contains("Added location conflict rule"));
        let rule = state
            .app
            .list_location_incompatibility_rules()
            .unwrap()
            .remove(0);
        state
            .execute_legacy_palette_command("location-rule-list")
            .unwrap();
        assert!(state.message.contains("impossible together"));
        state
            .execute_legacy_palette_command(&format!(
                "location-rule-update {} disabled inactive rule",
                rule.id
            ))
            .unwrap();
        assert!(state.message.contains("Updated location rule"));
        assert!(!state.app.list_location_incompatibility_rules().unwrap()[0].enabled);
        state
            .execute_legacy_palette_command(&format!(
                "location-rule-update {} enabled impossible together",
                rule.id
            ))
            .unwrap();
        state
            .execute_legacy_palette_command("location-rule-check")
            .unwrap();
        assert!(state.message.contains("No location-rule findings"));
        state
            .execute_legacy_palette_command(&format!("location-rule-remove {}", rule.id))
            .unwrap();
        assert!(
            state
                .app
                .list_location_incompatibility_rules()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn terminal_manages_and_checks_event_dependency_assertions() {
        let root = tempfile::tempdir().unwrap();
        let project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let mut state = AppState::new(project, default_bindings());
        let first = state.app.add_event("Message", "").unwrap();
        let second = state.app.add_event("Response", "").unwrap();
        state
            .execute_legacy_palette_command(&format!(
                "event-dependency-add {first} {second} precedes delivery comes first"
            ))
            .unwrap();
        assert!(state.message.contains("Added event dependency"));
        state
            .execute_legacy_palette_command("event-dependency-list")
            .unwrap();
        assert!(state.message.contains("delivery comes first"));
        state
            .execute_legacy_palette_command("event-dependency-check")
            .unwrap();
        assert!(state.message.contains("Unknown"));
        state
            .execute_legacy_palette_command(&format!(
                "event-dependency-update {first} {second} precedes order revised"
            ))
            .unwrap();
        assert_eq!(
            state.app.list_event_dependencies().unwrap()[0].notes,
            "order revised"
        );
        state
            .execute_legacy_palette_command(&format!(
                "event-dependency-remove {first} {second} precedes"
            ))
            .unwrap();
        assert!(state.app.list_event_dependencies().unwrap().is_empty());
    }

    #[test]
    fn terminal_updates_event_story_time_without_changing_event_identity_or_title() {
        let root = tempfile::tempdir().unwrap();
        let mut project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let id = story_core::EventId::new();
        project
            .add_story_event(&story_core::StoryEvent {
                id,
                title: "Crossing the bridge".into(),
                description: "They leave before dawn".into(),
                start: story_core::StoryTime::exact(1),
                end: story_core::StoryTime::exact(2),
                source_time_text: "first day".into(),
            })
            .unwrap();
        let mut state = AppState::new(project, default_bindings());
        state
            .execute_legacy_palette_command(&format!(
                "event-time-set {id} | ~6,4,8 | 9..12 | during the storm"
            ))
            .unwrap();
        let event = state
            .app
            .list_events()
            .unwrap()
            .into_iter()
            .find(|event| event.id == id)
            .unwrap();
        assert_eq!(event.title, "Crossing the bridge");
        assert_eq!(event.start.nominal, Some(6));
        assert_eq!(event.start.lower, Some(4));
        assert_eq!(event.start.upper, Some(8));
        assert_eq!(event.end.lower, Some(9));
        assert_eq!(event.end.upper, Some(12));
        assert_eq!(event.source_time, "during the storm");
    }

    #[test]
    fn terminal_restores_one_scene_from_project_history_safely() {
        let root = tempfile::tempdir().unwrap();
        let mut project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        let scene = project.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "First draft.")
            .unwrap();
        let commit = project.git_snapshot("First draft").unwrap().unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "Revised draft.")
            .unwrap();
        let mut state = AppState::new(project, default_bindings());

        state
            .execute_legacy_palette_command(&format!("git-restore-scene {commit} {scene}"))
            .unwrap();

        assert_eq!(state.app.load_scene_text(scene).unwrap().0, "First draft.");
        assert!(
            state
                .message
                .contains("prior project state was snapshotted")
        );
        assert!(
            state.app.git_history(10).unwrap()[0]
                .subject
                .contains("Before restoring scene Opening")
        );
    }

    #[test]
    fn terminal_spellcheck_uses_local_wordlist_without_changing_prose() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Book");
        let mut project = WriterApp::create_project(&project_path, "Book").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        let scene = project.create_scene(chapter, "Opening", 0).unwrap();
        let (_, hash) = project.load_scene_text(scene).unwrap();
        project
            .save_scene_text(scene, Some(&hash), "Baad word.")
            .unwrap();
        fs::create_dir_all(project_path.join("dictionaries")).unwrap();
        fs::write(project_path.join("dictionaries/en.txt"), "known\n").unwrap();
        let fake_aspell = root.path().join("aspell-mock");
        fs::write(
            &fake_aspell,
            "#!/bin/sh\ncat >/dev/null\nprintf 'Baad\\nKnown\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&fake_aspell, fs::Permissions::from_mode(0o755)).unwrap();
        let mut state = AppState::new(project, default_bindings());

        state
            .execute_legacy_palette_command(&format!(
                "spellcheck en dictionaries/en.txt {}",
                fake_aspell.display()
            ))
            .unwrap();

        assert!(state.message.contains("baad×1"));
        assert!(!state.message.contains("known"));
        assert_eq!(state.app.load_scene_text(scene).unwrap().0, "Baad word.");
        assert!(
            spellcheck_terminal_project(
                &state.app,
                "en",
                Some("../outside.txt"),
                fake_aspell.to_str().unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_persists_and_checks_saved_query_assertions() {
        let root = tempfile::tempdir().unwrap();
        let mut project = WriterApp::create_project(root.path().join("Book"), "Book").unwrap();
        let chapter = project.create_chapter(None, "Chapter", 0).unwrap();
        project.create_scene(chapter, "One", 0).unwrap();
        let query = story_core::SavedSceneQuery {
            id: story_core::QueryId::new(),
            name: "All scenes".into(),
            description: String::new(),
            filters: vec![],
        };
        project.save_scene_query(&query).unwrap();
        let mut state = AppState::new(project, default_bindings());
        state.prompt = format!("query-assert-save {} exactly:1 non-empty", query.id);
        state.command().unwrap();
        state.prompt = format!("query-assert-check {}", query.id);
        state.command().unwrap();
        assert!(state.message.contains("PASS exactly 1"));
        assert_eq!(
            state.app.get_query_assertions(query.id).unwrap(),
            [
                story_core::QueryAssertion::Exactly(1),
                story_core::QueryAssertion::NonEmpty
            ]
        );
    }

    #[test]
    fn terminal_palette_compiles_markdown_and_source_map_without_external_tools() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "Plain / manuscript prose.")
            .unwrap();
        let output = project.join("exports/test.md");
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.mode = Some(':');
        state.prompt = format!("compile-format markdown {}", output.display());
        state.command().unwrap();
        assert!(
            fs::read_to_string(&output)
                .unwrap()
                .contains("Plain / manuscript prose.")
        );
        let map = fs::read_to_string(format!("{}.source-map.json", output.display())).unwrap();
        assert!(map.contains(&scene.to_string()));
        assert!(state.message.contains("Compiled to"));
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "Plain / manuscript prose."
        );
    }

    #[test]
    fn terminal_review_merge_applies_non_overlapping_changes_and_leaves_conflicts_unmodified() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "One.\nTwo.\nThree.\n";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "One local.\nTwo.\nThree.\n")
            .unwrap();
        let base_file = directory.path().join("base.txt");
        let review_file = directory.path().join("review.txt");
        fs::write(&base_file, base).unwrap();
        fs::write(&review_file, "One.\nTwo.\nThree reviewed.\n").unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.mode = Some(':');
        state.prompt = format!(
            "review-merge {scene} {} {}",
            base_file.display(),
            review_file.display()
        );
        state.command().unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "One local.\nTwo.\nThree reviewed.\n"
        );
        fs::write(&review_file, "One external.\nTwo.\nThree.\n").unwrap();
        state.mode = Some(':');
        state.prompt = format!(
            "review-merge {scene} {} {}",
            base_file.display(),
            review_file.display()
        );
        state.command().unwrap();
        assert!(state.message.contains("conflict"));
        let review = state.active_reference_pane().unwrap();
        assert_eq!(review.title, "Editorial conflict review");
        assert!(review.content.contains("BASE"));
        assert!(review.content.contains("LOCAL"));
        assert!(review.content.contains("REVIEWED"));
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "One local.\nTwo.\nThree reviewed.\n"
        );
        state.mode = Some(':');
        state.prompt = format!(
            "review-resolve {scene} {} {} external",
            base_file.display(),
            review_file.display()
        );
        state.command().unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "One external.\nTwo.\nThree reviewed.\n"
        );
    }

    #[test]
    fn terminal_accepts_docx_tracked_changes_through_review_merge() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let base = "The old door.";
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), base).unwrap();
        let docx = directory.path().join("review.docx");
        let file = fs::File::create(&docx).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("word/document.xml", zip::write::FileOptions::default())
            .unwrap();
        archive
            .write_all(b"<w:document><w:body><w:p><w:r><w:t>The </w:t></w:r><w:del><w:r><w:delText>old</w:delText></w:r></w:del><w:ins><w:r><w:t>new</w:t></w:r></w:ins><w:r><w:t> door.</w:t></w:r></w:p></w:body></w:document>")
            .unwrap();
        archive.finish().unwrap();
        let source = fs::read(&docx).unwrap();
        let base_path = directory.path().join("base.md");
        fs::write(&base_path, base).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.open_scene(scene).unwrap();
        state.prompt = format!(
            "review-docx {scene} {} {} accept",
            base_path.display(),
            docx.display()
        );
        state.command().unwrap();
        assert_eq!(state.app.load_scene_text(scene).unwrap().0, "The new door.");
        assert_eq!(state.editor.join("\n"), "The new door.");
        assert!(state.message.contains("accepted"));
        assert_eq!(fs::read(docx).unwrap(), source);
    }

    #[test]
    fn menu_shortcuts_are_reserved_without_consuming_plain_slash() {
        let bindings = default_bindings();
        assert!(!bindings.iter().any(|(key, _)| *key == 10));
        assert!(
            bindings
                .iter()
                .any(|(key, workspace)| *key == 9 && *workspace == Workspace::Analytics)
        );
        assert!(
            bindings
                .iter()
                .any(|(key, workspace)| *key == 11 && *workspace == Workspace::Issues)
        );
        assert!(is_menu_shortcut(KeyEvent::new(
            KeyCode::Char('/'),
            KeyModifiers::CONTROL
        )));
        assert!(is_menu_shortcut(KeyEvent::new(
            KeyCode::Char('_'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_menu_shortcut(KeyEvent::new(
            KeyCode::Char('/'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn editor_slash_is_text_and_menu_invocation_preserves_edit_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = AppState {
            app: WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap(),
            workspace: Workspace::Editor,
            scenes: Vec::new(),
            selected: 0,
            editor: vec!["text".into()],
            editing_scene: None,
            cursor: (0, 4),
            saved_hash: String::new(),
            dirty: true,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Editor],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor, vec!["text/".to_owned()]);
        assert_eq!(state.cursor, (0, 5));

        state.editor_selection = Some((0, 1));
        let selection = state.editor_selection;
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor, vec!["text/".to_owned()]);
        assert_eq!(state.cursor, (0, 5));
        assert_eq!(state.editor_selection, selection);
        assert!(state.dirty);
        assert!(state.menu.is_some());
        assert_eq!(
            state.command_disabled_reason(command_by_id("file.save").unwrap()),
            Some("Open a scene or assembly draft first")
        );
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.menu.unwrap().top, 1);
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.menu.unwrap().top, 2);
        handle(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert!(state.menu.is_none());

        handle(
            &mut state,
            KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor, vec!["text/".to_owned()]);
        assert_eq!(state.cursor, (0, 5));
        assert_eq!(state.editor_selection, selection);
        assert!(state.menu.is_some());
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.menu.unwrap().top, 5);
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.menu.is_none());
        assert_eq!(state.workspace, Workspace::Dashboard);
        assert_eq!(state.editor, vec!["text/".to_owned()]);
        assert_eq!(state.cursor, (0, 5));

        state.toggle_menu();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(MENU_CATEGORIES[state.menu.unwrap().top], "Insert");
        let ids = state
            .menu_commands()
            .into_iter()
            .map(|command| command.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "scene.create",
                "scene.subchapter",
                "structure.section",
                "structure.chapter",
                "edit.snippet-save",
                "edit.snippet-insert",
                "edit.text-expansion-set",
                "edit.text-expansion",
                "edit.text-expand"
            ]
        );
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.menu.is_none());
        assert_eq!(state.mode, Some(':'));
        assert_eq!(state.prompt, "scene ");
        state
            .execute_legacy_palette_command("writing-stats on")
            .unwrap();
        state.insert('é');
        state.paste_text("α\nβ");
        assert_eq!(state.typed_characters, 1);
        assert_eq!(state.pasted_characters, 3);
        state
            .execute_legacy_palette_command("writing-stats off")
            .unwrap();
        state.insert('x');
        assert_eq!(state.typed_characters, 1);
        state.workspace = Workspace::Editor;
        state.editor_focus_mode = true;
        state.horizontal_split_ratio = 55;
        state.vertical_split_ratio = 65;
        state.scroll = 13;
        state.save_session().unwrap();
        state.workspace = Workspace::Dashboard;
        state.scroll = 0;
        state.horizontal_split_ratio = default_split_ratio();
        state.vertical_split_ratio = default_split_ratio();
        state.typed_characters = 0;
        state.pasted_characters = 0;
        state.restore_session().unwrap();
        assert_eq!(state.workspace, Workspace::Editor);
        assert!(state.editor_focus_mode);
        assert_eq!(state.scroll, 13);
        assert_eq!(state.horizontal_split_ratio, 55);
        assert_eq!(state.vertical_split_ratio, 65);
        state.mode = None;
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
        )
        .unwrap();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.horizontal_split_ratio, 60);
        assert_eq!(state.vertical_split_ratio, 70);
        assert!(!state.writing_stats_enabled);
        assert_eq!(state.typed_characters, 1);
        assert_eq!(state.pasted_characters, 3);
        let mut stale =
            serde_json::from_slice::<SessionSnapshot>(&fs::read(state.session_path()).unwrap())
                .unwrap();
        stale.editing_scene = Some(uuid::Uuid::new_v4().to_string());
        fs::write(state.session_path(), serde_json::to_vec(&stale).unwrap()).unwrap();
        state.restore_session().unwrap();
        assert!(
            state
                .message
                .contains("removed scene or reading reference was skipped")
        );
        assert!(state.editing_scene.is_none());

        let chapter = state.app.create_chapter(None, "Board chapter", 0).unwrap();
        let scene = state.app.create_scene(chapter, "Board scene", 0).unwrap();
        let board = state.app.create_planning_board("Canvas").unwrap();
        let card = state
            .app
            .add_planning_board_item(board, "scene", &scene.to_string(), 3.0, 4.0, "beat")
            .unwrap();
        state.refresh().unwrap();
        state.planning_board_id = Some(board);
        state.workspace = Workspace::PlanningBoards;
        state.selected = 0;
        state.mode = None;
        state.dirty = false;
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
        )
        .unwrap();
        let moved = state
            .app
            .list_planning_board_items(board)
            .unwrap()
            .into_iter()
            .find(|item| item.id == card)
            .unwrap();
        assert_eq!((moved.x, moved.y), (4.0, 4.0));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        )
        .unwrap();
        let resized = state
            .app
            .list_planning_board_items(board)
            .unwrap()
            .into_iter()
            .find(|item| item.id == card)
            .unwrap();
        assert_eq!(resized.width, 25.0);
        let export = format_planning_board_markdown(
            "Canvas",
            &state.app.list_planning_board_items(board).unwrap(),
        );
        assert!(export.contains("# Canvas"));
        assert!(export.contains("| scene |"));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.workspace, Workspace::Editor);
        assert_eq!(state.editing_scene, Some(scene));
    }

    #[test]
    fn narrowed_outline_and_split_layout_restore_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Session"), "Session").unwrap();
        let first = app.create_chapter(None, "First", 0).unwrap();
        let second = app.create_chapter(None, "Second", 1).unwrap();
        let first_scene = app.create_scene(first, "First scene", 0).unwrap();
        let second_scene = app.create_scene(second, "Second scene", 0).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state
            .execute_legacy_palette_command(&format!("narrow {second}"))
            .unwrap();
        state.selected = 0;
        state.horizontal_split_ratio = 60;
        state.vertical_split_ratio = 40;
        state.save_session().unwrap();

        let mut reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        reopened.restore_session().unwrap();
        assert_eq!(
            reopened
                .scenes
                .iter()
                .map(|scene| scene.0)
                .collect::<Vec<_>>(),
            [second_scene]
        );
        assert!(!reopened.scenes.iter().any(|scene| scene.0 == first_scene));
        assert_eq!(reopened.horizontal_split_ratio, 60);
        assert_eq!(reopened.vertical_split_ratio, 40);
    }

    #[test]
    fn plotline_event_and_object_reference_buffers_restore_by_stable_identity() {
        let root = tempfile::tempdir().unwrap();
        let mut app =
            WriterApp::create_project(root.path().join("References"), "References").unwrap();
        let plotline = app.create_plotline("The hidden thread").unwrap();
        let event = app.add_event("The eclipse", "day 4").unwrap();
        let object = app
            .create_story_object("The brass key", "Opens the western archive.")
            .unwrap();
        let mut state = AppState::new(app, default_bindings());
        state
            .execute_legacy_palette_command(&format!("reference-plotline {plotline}"))
            .unwrap();
        state
            .execute_legacy_palette_command(&format!("reference-event {event}"))
            .unwrap();
        state
            .execute_legacy_palette_command(&format!("reference-object {object}"))
            .unwrap();
        assert_eq!(state.reference_panes.len(), 3);
        assert!(
            state.reference_panes[0]
                .content
                .contains("The hidden thread")
        );
        assert!(state.reference_panes[1].content.contains("day 4"));
        assert!(state.reference_panes[2].content.contains("western archive"));
        state.save_session().unwrap();
        let mut reopened = AppState::new(
            WriterApp::open_project(state.app.project_root()).unwrap(),
            default_bindings(),
        );
        reopened.restore_session().unwrap();
        assert_eq!(reopened.reference_panes.len(), 3);
        assert_eq!(
            reopened.reference_panes[0].source_key,
            format!("plotline:{plotline}")
        );
        assert_eq!(
            reopened.reference_panes[1].source_key,
            format!("event:{event}")
        );
        assert_eq!(
            reopened.reference_panes[2].source_key,
            format!("object:{object}")
        );
    }

    #[test]
    fn replace_preview_shows_proposed_text_without_mutating_manuscript() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Replace"), "Replace").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Cats", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "A cat waits.\nThe cat leaves.")
            .unwrap();
        let mut state = AppState::new(app, default_bindings());
        state
            .execute_legacy_palette_command("replace-preview normal all cat => fox")
            .unwrap();
        assert!(state.message.contains("2 replacement(s)"));
        assert!(state.message.contains("A cat waits."));
        assert!(state.message.contains("A fox waits."));
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "A cat waits.\nThe cat leaves."
        );
    }

    #[test]
    fn replace_uses_dirty_active_editor_buffer_and_saves_with_its_expected_hash() {
        let root = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(root.path().join("Replace"), "Replace").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Cats", 0).unwrap();
        let (saved, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "A cat waits.")
            .unwrap();
        let saved_hash = app.load_scene_text(scene).unwrap().1;
        let mut state = AppState::new(app, default_bindings());
        state.editing_scene = Some(scene);
        state.editor = vec!["A cat waits, and the new sentence remains.".into()];
        state.saved_hash = saved_hash;
        state.dirty = true;
        state
            .execute_legacy_palette_command("replace normal all cat => fox")
            .unwrap();
        assert!(state.message.contains("1 occurrence(s)"));
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "A fox waits, and the new sentence remains."
        );
        assert_eq!(
            state.editor.join("\n"),
            "A fox waits, and the new sentence remains."
        );
        assert!(!state.dirty);
        assert_ne!(saved, state.editor.join("\n"));
    }

    #[test]
    fn project_keymap_overrides_function_key_workspace_targets() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_bindings(dir.path()).unwrap()[0].1,
            Workspace::Dashboard
        );
        fs::create_dir_all(dir.path().join(".writers-ide")).unwrap();
        fs::write(
            dir.path().join(".writers-ide/keybindings.toml"),
            "[bindings]\nF1 = \"Analytics\"\nF4 = \"Reading\"\n",
        )
        .unwrap();
        let bindings = load_bindings(dir.path()).unwrap();
        assert_eq!(bindings[0], (1, Workspace::Analytics));
        assert_eq!(bindings[3], (4, Workspace::Reading));
        assert_eq!(bindings[1].1, Workspace::Outline);
    }

    #[test]
    fn narrow_terminal_switches_split_panes_to_stacked_layout() {
        assert_eq!(pane_direction(80), Direction::Vertical);
        assert_eq!(pane_direction(99), Direction::Vertical);
        assert_eq!(pane_direction(100), Direction::Horizontal);
    }

    #[test]
    fn multiple_reference_buffers_receive_simultaneous_adaptive_panes() {
        let wide = reference_pane_layout(Rect::new(0, 0, 120, 30), 3, 200);
        assert_eq!(wide.len(), 3);
        assert!(wide.iter().all(|area| area.width > 0 && area.height == 30));
        let narrow = reference_pane_layout(Rect::new(0, 0, 70, 30), 3, 80);
        assert_eq!(narrow.len(), 3);
        assert!(
            narrow
                .iter()
                .all(|area| area.height > 0 && area.width == 70)
        );
    }
    use crossterm::event::KeyModifiers;

    #[test]
    fn editor_positions_follow_extended_grapheme_boundaries() {
        let line = "e\u{301}👩‍🚀字";
        assert_eq!(grapheme_count(line), 3);
        assert_eq!(&line[..grapheme_byte(line, 1)], "e\u{301}");
        assert_eq!(&line[..grapheme_byte(line, 2)], "e\u{301}👩‍🚀");
        assert_eq!(grapheme_byte(line, 99), line.len());
    }

    #[test]
    fn sustained_terminal_key_editing_survives_autosaves_and_reopens() {
        let directory = tempfile::tempdir().unwrap();
        let project_path = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project_path, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Sustained session", 0).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.open_scene(scene).unwrap();
        let mut expected = String::new();
        for total_paragraphs in 0..120 {
            let line = format!(
                "Session paragraph {total_paragraphs}: café — Mira follows the harbour lights."
            );
            for character in line.chars() {
                handle(
                    &mut state,
                    KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                )
                .unwrap();
            }
            handle(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            )
            .unwrap();
            expected.push_str(&line);
            expected.push('\n');

            if total_paragraphs % 30 == 29 {
                assert!(state.dirty);
                state.save().unwrap();
                assert!(!state.dirty);
                state.save_session().unwrap();
                drop(state);
                state = AppState::new(
                    WriterApp::open_project(&project_path).unwrap(),
                    default_bindings(),
                );
                state.restore_session().unwrap();
                assert_eq!(state.editing_scene, Some(scene));
                assert_eq!(state.editor.join("\n"), expected);
            }
        }

        let (saved, hash) = state.app.load_scene_text(scene).unwrap();
        assert_eq!(saved, expected);
        assert_eq!(hash, state.saved_hash);
        assert_eq!(state.app.get_scene(scene).unwrap().actual_words, 1080);
    }

    #[test]
    fn reference_buffers_switch_close_and_restore_by_stable_scene_id() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "Main", 0).unwrap();
        let second = app.create_scene(chapter, "Reference A", 1).unwrap();
        let third = app.create_scene(chapter, "Reference B", 2).unwrap();
        let research = app
            .create_research_item(
                "World Notes",
                "The city follows a lunar calendar.",
                "text",
                None,
                None,
            )
            .unwrap();
        let character = app.create_character("Mira Vale").unwrap();
        let location = app.create_location("The harbour").unwrap();
        let mut location_profile = app.get_location(location).unwrap();
        location_profile.description = "A narrow harbour beneath the cliffs.".into();
        app.update_location(&location_profile).unwrap();
        for (scene, prose) in [
            (first, "Main prose"),
            (second, "Reference A text"),
            (third, "Reference B text"),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), prose).unwrap();
        }
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.open_scene(first).unwrap();
        let execute = |state: &mut AppState, command: String| {
            state.mode = Some(':');
            state.prompt = command;
            state.command().unwrap();
        };
        execute(&mut state, format!("reference-scene {second}"));
        execute(&mut state, format!("reference-scene {third}"));
        execute(&mut state, "reference-prev".into());
        assert_eq!(
            state.active_reference_pane().unwrap().scene_id,
            Some(second)
        );
        assert_eq!(state.reference_panes.len(), 2);
        execute(&mut state, format!("reference-research {research}"));
        assert_eq!(
            state.active_reference_pane().unwrap().content,
            "The city follows a lunar calendar."
        );
        execute(&mut state, "reference-next".into());
        assert_eq!(
            state.active_reference_pane().unwrap().scene_id,
            Some(second)
        );
        execute(&mut state, format!("reference-character {character}"));
        assert!(
            state
                .active_reference_pane()
                .unwrap()
                .content
                .contains("Mira Vale")
        );
        execute(&mut state, format!("reference-location {location}"));
        assert!(
            state
                .active_reference_pane()
                .unwrap()
                .content
                .contains("narrow harbour")
        );
        execute(&mut state, "reference-next".into());
        assert_eq!(
            state.active_reference_pane().unwrap().scene_id,
            Some(second)
        );
        assert_eq!(state.reference_panes.len(), 5);
        state.save_session().unwrap();
        let saved: SessionSnapshot =
            serde_json::from_slice(&fs::read(state.session_path()).unwrap()).unwrap();
        assert_eq!(saved.reference_sources.len(), 5);
        let mut restored = AppState::new(
            WriterApp::open_project(&project).unwrap(),
            default_bindings(),
        );
        restored.restore_session().unwrap();
        assert_eq!(restored.reference_panes.len(), 5);
        assert_eq!(
            restored.active_reference_pane().unwrap().scene_id,
            Some(second)
        );
        execute(&mut restored, "reference-close".into());
        assert_eq!(restored.reference_panes.len(), 4);
        assert_eq!(
            restored.active_reference_pane().unwrap().scene_id,
            Some(third)
        );
        assert!(restored.reference_panes.iter().any(|pane| {
            pane.source_key == format!("research:{research}")
                && pane.content == "The city follows a lunar calendar."
        }));
        assert!(restored.reference_panes.iter().any(|pane| {
            pane.source_key == format!("character:{character}")
                && pane.content.contains("Mira Vale")
        }));
        assert!(restored.reference_panes.iter().any(|pane| {
            pane.source_key == format!("location:{location}")
                && pane.content.contains("narrow harbour")
        }));
    }

    #[test]
    fn editor_deletes_and_inserts_whole_combining_and_emoji_graphemes() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Unicode", 0).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "e\u{301}👩‍🚀字")
            .unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.open_scene(scene).unwrap();
        state.cursor = (0, 1);
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "👩‍🚀字");
        assert_eq!(state.cursor, (0, 0));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "字");
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        )
        .unwrap();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('\u{301}'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "e\u{301}字");
        assert_eq!(state.cursor, (0, 1));
    }

    #[test]
    fn editor_undo_redo_restores_text_cursor_and_selection() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Undo", 0).unwrap();
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.open_scene(scene).unwrap();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        )
        .unwrap();
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "ab");
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "a");
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "ab");
        handle(
            &mut state,
            KeyEvent::new(
                KeyCode::Char('z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "ab");
        assert_eq!(state.cursor, (0, 2));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor.join("\n"), "a");
        assert!(state.dirty);
    }

    #[test]
    fn find_navigates_all_matches_and_supports_chapter_scope() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let first_chapter = app.create_chapter(None, "One", 0).unwrap();
        let second_chapter = app.create_chapter(None, "Two", 1).unwrap();
        let first = app.create_scene(first_chapter, "First", 0).unwrap();
        let second = app.create_scene(first_chapter, "Second", 1).unwrap();
        let third = app.create_scene(second_chapter, "Third", 0).unwrap();
        for (scene, text) in [
            (first, "Mara met Mara."),
            (second, "Mara again."),
            (third, "Mara elsewhere."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), text).unwrap();
        }
        let mut state = AppState::new(app, default_bindings());
        state.refresh().unwrap();
        state.open_scene(first).unwrap();
        state.mode = Some(':');
        state.prompt = format!("find normal chapter:{first_chapter} Mara");
        state.command().unwrap();
        assert_eq!(state.search_locations.len(), 3);
        assert_eq!(state.search_locations[0].0, first);
        assert_eq!(state.search_locations[1].0, first);
        handle(&mut state, KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)).unwrap();
        assert_eq!(state.selected_text().as_deref(), Some("Mara"));
        assert_eq!(
            state.cursor,
            editor_position_from_byte("Mara met Mara.", 13).unwrap()
        );
        handle(
            &mut state,
            KeyEvent::new(KeyCode::F(3), KeyModifiers::SHIFT),
        )
        .unwrap();
        assert_eq!(state.search_location_index, 0);
    }

    #[test]
    fn proofread_scopes_to_a_chapter_and_rejects_non_chapters() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(directory.path().join("Novel"), "Novel").unwrap();
        let first_chapter = app.create_chapter(None, "One", 0).unwrap();
        let second_chapter = app.create_chapter(None, "Two", 1).unwrap();
        let first = app.create_scene(first_chapter, "Finding", 0).unwrap();
        let second = app.create_scene(second_chapter, "Elsewhere", 0).unwrap();
        for (scene, text) in [
            (first, "Mara said said hello."),
            (second, "Other said said."),
        ] {
            let (_, hash) = app.load_scene_text(scene).unwrap();
            app.save_scene_text(scene, Some(&hash), text).unwrap();
        }
        let mut state = AppState::new(app, default_bindings());
        state.mode = Some(':');
        state.prompt = format!("proofread chapter:{first_chapter}");
        state.command().unwrap();
        assert!(state.message.contains("Finding"));
        assert!(!state.message.contains("Elsewhere"));
        state.mode = Some(':');
        state.prompt = format!("proofread chapter:{}", first);
        assert!(state.command().is_err());
    }

    #[test]
    fn editor_selection_copy_cut_and_paste_preserve_multiline_text() {
        let mut state = AppState {
            app: WriterApp::create_project(tempfile::tempdir().unwrap().path().join("N"), "N")
                .unwrap(),
            workspace: Workspace::Editor,
            scenes: Vec::new(),
            selected: 0,
            editor: vec!["alpha".into(), "beta".into()],
            editing_scene: None,
            cursor: (1, 2),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Editor],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: Some((0, 2)),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        assert_eq!(state.selected_text().as_deref(), Some("pha\nbe"));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.clipboard, "pha\nbe");
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor, vec!["alta"]);
        assert_eq!(state.cursor, (0, 2));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert_eq!(state.editor, vec!["alpha", "beta"]);
        assert!(state.dirty);
    }

    #[test]
    fn editor_shift_arrows_extend_and_plain_arrows_clear_selection() {
        let mut state = AppState {
            app: WriterApp::create_project(tempfile::tempdir().unwrap().path().join("N"), "N")
                .unwrap(),
            workspace: Workspace::Editor,
            scenes: Vec::new(),
            selected: 0,
            editor: vec!["hello".into()],
            editing_scene: None,
            cursor: (0, 1),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Editor],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
        )
        .unwrap();
        assert_eq!(state.selected_text().as_deref(), Some("e"));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.editor_selection, None);
        assert_eq!(state.cursor, (0, 3));
    }

    #[test]
    fn editor_accepts_shifted_characters_and_moves_between_lines() {
        let mut state = AppState {
            app: WriterApp::create_project(tempfile::tempdir().unwrap().path().join("N"), "N")
                .unwrap(),
            workspace: Workspace::Editor,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new(), "next".into()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Editor],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT),
        )
        .unwrap();
        assert_eq!(state.editor[0], "A");
        state.cursor = (0, 1);
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.cursor, (1, 0));
        handle(&mut state, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)).unwrap();
        assert_eq!(state.cursor, (0, 1));
    }

    #[test]
    fn editor_keeps_dirty_scene_identity_and_refuses_silent_switch() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let second = app.create_scene(chapter, "Second", 1).unwrap();
        let scenes = app
            .list_scenes(false)
            .unwrap()
            .into_iter()
            .map(|s| (s.scene.id, s.scene.title, s.scene.status, s.actual_words))
            .collect();
        let mut state = AppState {
            app,
            workspace: Workspace::Outline,
            scenes,
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Outline],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        state.open_selected().unwrap();
        state.insert('A');
        state.selected = 1;
        state.open_selected().unwrap();
        assert_eq!(state.editing_scene, Some(first));
        assert_eq!(state.editor[0], "A");
        assert!(state.message.contains("Save with Ctrl-S"));
        state.save().unwrap();
        assert_eq!(state.app.load_scene_text(first).unwrap().0, "A");
        assert_eq!(state.app.load_scene_text(second).unwrap().0, "");
        assert!(!state.dirty);
        state.mode = Some(':');
        state.prompt = "query-save ideas status=idea".into();
        state.command().unwrap();
        assert_eq!(state.app.list_saved_queries().unwrap()[0].name, "ideas");
        state.mode = Some(':');
        state.prompt = "query-list".into();
        state.command().unwrap();
        assert!(state.message.contains("ideas"));
        state.mode = Some(':');
        state.prompt = format!("link scene {first} scene {second} related note");
        state.command().unwrap();
        state.mode = Some(':');
        state.prompt = format!("outgoing scene {first}");
        state.command().unwrap();
        assert!(state.message.contains(&second.to_string()));
        state.mode = Some(':');
        state.prompt = format!("backlinks scene {second}");
        state.command().unwrap();
        assert!(state.message.contains(&first.to_string()));
        state.mode = Some(':');
        state.prompt = "character Mara".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::StoryBible);
        assert_eq!(state.app.list_characters().unwrap()[0].name, "Mara");
        let character_id = state.app.list_characters().unwrap()[0].id;
        state.mode = Some(':');
        state.prompt = format!("character-link {first} {character_id} pov");
        state.command().unwrap();
        let character_matrix = character_matrix_rows(&state);
        assert!(character_matrix[0].contains("Mara"));
        assert!(
            character_matrix
                .iter()
                .any(|row| row.contains("First") && row.contains('P'))
        );
        state.mode = Some(':');
        state.prompt = "location Observatory".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::StoryBible);
        assert!(workspace_items(&state)[state.selected].contains("Observatory"));
        state.mode = Some(':');
        state.prompt = "plotline Political intrigue".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::StoryBible);
        assert_eq!(
            state.app.list_plotlines().unwrap()[0].name,
            "Political intrigue"
        );
        let plot_id = state.app.list_plotlines().unwrap()[0].id;
        state.mode = Some(':');
        state.prompt = format!("plotline-link {first} {plot_id} overlap advances");
        state.command().unwrap();
        let other_character = story_core::CharacterId::new();
        let other_plotline = story_core::PlotlineId::new();
        state.mode = Some(':');
        state.prompt = format!(
            "query-save mixed character-any={character_id},{other_character} plotline-any={plot_id},{other_plotline} not-status=complete"
        );
        state.command().unwrap();
        let query = state
            .app
            .list_saved_queries()
            .unwrap()
            .into_iter()
            .find(|query| query.name == "mixed")
            .unwrap();
        assert_eq!(state.app.run_scene_query(query.id).unwrap(), vec![first]);
        let expression = story_core::QueryExpression::All(vec![
            story_core::QueryExpression::Any(vec![
                story_core::QueryExpression::Filter(story_core::SceneQueryFilter::Character(
                    character_id,
                )),
                story_core::QueryExpression::Filter(story_core::SceneQueryFilter::Character(
                    other_character,
                )),
            ]),
            story_core::QueryExpression::Not(Box::new(story_core::QueryExpression::Filter(
                story_core::SceneQueryFilter::Status(SceneStatus::Complete),
            ))),
        ]);
        state.mode = Some(':');
        state.prompt = format!(
            "query-expression-set {} {}",
            query.id,
            serde_json::to_string(&expression).unwrap()
        );
        state.command().unwrap();
        assert_eq!(state.app.run_scene_query(query.id).unwrap(), vec![first]);
        let grid = plot_grid_rows(&state);
        assert!(grid[0].contains("Political intrigue"));
        assert!(
            grid.iter()
                .any(|row| row.contains("First") && row.contains('●'))
        );
        state.mode = Some(':');
        state.prompt = format!("plotline-shares {first} {plot_id}=10000");
        state.command().unwrap();
        assert_eq!(
            state.app.scene_plotline_links(first).unwrap()[0].share_basis_points,
            Some(10000)
        );
        state.mode = Some(':');
        state.prompt = "issue Clarify the timeline".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::Issues);
        assert_eq!(
            state.app.list_issues().unwrap()[0].title,
            "Clarify the timeline"
        );
        let issue_id = state.app.list_issues().unwrap()[0].id;
        state.mode = Some(':');
        state.prompt = format!("issue-link {first} {issue_id}");
        state.command().unwrap();
        state.mode = Some(':');
        state.prompt = format!("issue-status {issue_id} in_progress");
        state.command().unwrap();
        assert_eq!(state.app.list_issues().unwrap()[0].status, "in_progress");
        state.mode = Some(':');
        state.prompt = format!("scene-field {first} purpose | Reveal the betrayal");
        state.command().unwrap();
        assert_eq!(
            state.app.get_scene(first).unwrap().scene.purpose,
            "Reveal the betrayal"
        );
        state.mode = Some(':');
        state.prompt = "event Bridge collapses | Day 2, 09:00".into();
        state.command().unwrap();
        let event_id = story_core::EventId(
            state
                .message
                .split_whitespace()
                .last()
                .unwrap()
                .parse()
                .unwrap(),
        );
        state.mode = Some(':');
        state.prompt = format!("event-link {first} {event_id} causes");
        state.command().unwrap();
        assert_eq!(
            state.app.scene_event_links(first).unwrap(),
            vec![(event_id, "causes".into())]
        );
        state.mode = Some(':');
        state.prompt = "research City history | Founding records".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::Research);
        assert_eq!(
            state.app.list_research_items().unwrap()[0].title,
            "City history"
        );
        state.mode = Some(':');
        state.prompt = "search-all founding records".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::Search);
        assert_eq!(state.search_hits.len(), 1);
        assert!(workspace_items(&state)[0].contains("City history"));
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.workspace, Workspace::Research);
        assert!(state.search_hits.is_empty());
    }

    #[test]
    fn keyboard_save_is_not_a_quit_or_workspace_command() {
        let mut state = AppState {
            app: WriterApp::create_project(tempfile::tempdir().unwrap().path().join("N"), "N")
                .unwrap(),
            workspace: Workspace::Dashboard,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Dashboard],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        assert!(
            !handle(
                &mut state,
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
            )
            .unwrap()
        );
        assert_eq!(state.workspace, Workspace::Dashboard);
    }

    #[test]
    fn stale_save_keeps_dirty_text_and_exposes_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let scenes = app
            .list_scenes(false)
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.scene.id,
                    row.scene.title,
                    row.scene.status,
                    row.actual_words,
                )
            })
            .collect();
        let mut state = AppState {
            app,
            workspace: Workspace::Outline,
            scenes,
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Outline],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        state.open_selected().unwrap();
        state.insert('A');
        let (_, external_hash) = state.app.load_scene_text(scene).unwrap();
        state
            .app
            .save_scene_text(scene, Some(&external_hash), "Another writer changed this.")
            .unwrap();
        state.save().unwrap();
        assert!(state.dirty);
        assert!(state.message.contains("Save conflict/error"));
        assert_eq!(state.editor[0], "A");
    }

    #[test]
    fn revisions_workspace_restores_only_the_selected_changed_hunk() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Scene", 0).unwrap();
        let other_scene = app.create_scene(chapter, "Other scene", 1).unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "first\nold\nlast\n")
            .unwrap();
        let snapshot = app.create_snapshot(scene, "Old middle").unwrap();
        let (_, hash) = app.load_scene_text(scene).unwrap();
        app.save_scene_text(scene, Some(&hash), "first\nnew\nlast\n")
            .unwrap();
        let (_, other_hash) = app.load_scene_text(other_scene).unwrap();
        app.save_scene_text(other_scene, Some(&other_hash), "other scene text")
            .unwrap();
        let other_snapshot = app
            .create_snapshot(other_scene, "Other scene snapshot")
            .unwrap();
        let scenes = app
            .list_scenes(false)
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.scene.id,
                    row.scene.title,
                    row.scene.status,
                    row.actual_words,
                )
            })
            .collect();
        let mut state = AppState {
            app,
            workspace: Workspace::Revisions,
            scenes,
            selected: 1,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Revisions],
            history_index: 0,
            width: 100,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.revision_index, 1);
        assert_eq!(
            state.selected, 1,
            "revision browsing must preserve scene selection"
        );
        state.revision_index = 0;
        handle(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(state.revision_diff, Some(snapshot));
        assert_eq!(state.revision_scene, Some(scene));
        let diff = state.app.diff_snapshot(scene, snapshot).unwrap();
        let hunks = revision_hunks(&diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].selection.current_start, 1);
        assert_eq!(hunks[0].selection.current_end, 2);
        assert_eq!(hunks[0].selection.snapshot_start, 1);
        assert_eq!(hunks[0].selection.snapshot_end, 2);

        handle(
            &mut state,
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "first\nold\nlast\n"
        );
        assert_eq!(
            state.app.load_scene_text(other_scene).unwrap().0,
            "other scene text"
        );
        assert!(state.message.contains("Selected revision hunk restored"));
        assert!(state.app.list_snapshots(scene).unwrap().len() >= 2);
        assert!(
            state
                .app
                .list_snapshots(other_scene)
                .unwrap()
                .iter()
                .any(|snapshot| snapshot.id == other_snapshot)
        );
    }

    #[test]
    fn revision_hunks_map_insertions_and_deletions_to_empty_ranges() {
        let diff = vec![
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Equal,
                text: "a\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Removed,
                text: "old\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Added,
                text: "new\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Equal,
                text: "b\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Removed,
                text: "deleted\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Equal,
                text: "c\n".into(),
            },
            story_storage::SnapshotDiffLine {
                kind: story_storage::SnapshotLineKind::Added,
                text: "inserted\n".into(),
            },
        ];
        let hunks = revision_hunks(&diff);
        assert_eq!(hunks.len(), 3);
        assert_eq!(hunks[1].selection.current_start, 3);
        assert_eq!(hunks[1].selection.current_end, 3);
        assert_eq!(hunks[1].selection.snapshot_start, 3);
        assert_eq!(hunks[1].selection.snapshot_end, 4);
        assert_eq!(hunks[2].selection.current_start, 4);
        assert_eq!(hunks[2].selection.current_end, 5);
        assert_eq!(hunks[2].selection.snapshot_start, 5);
        assert_eq!(hunks[2].selection.snapshot_end, 5);
    }

    #[test]
    fn proposal_review_rows_show_provisional_status_and_source_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let scene = app.create_scene(chapter, "Discovery", 0).unwrap();
        let source = "Mara found the silver key.";
        let proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture-model",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "object".into(),
                    name: "Silver key".into(),
                    claim: "Mara found the key.".into(),
                    evidence_quote: source.into(),
                    approximate_story_time: None,
                }],
            },
        );
        app.save_extraction_proposal("review.json", &serde_json::to_vec(&proposal).unwrap())
            .unwrap();
        let rows = load_proposal_review_rows(&app).unwrap();
        assert!(rows[0].contains("review.json"));
        assert!(rows.iter().any(|row| row.contains("provisional")));
        assert!(rows.iter().any(|row| row.contains(source)));
    }

    #[test]
    fn reading_entity_link_can_navigate_to_story_character() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Novel");
        let mut app = WriterApp::create_project(&project, "Novel").unwrap();
        let character = app.create_character("Mara").unwrap();
        let source = dir.path().join("reference.txt");
        fs::write(
            &source,
            "Mara is the protagonist.\nA second reference passage.",
        )
        .unwrap();
        let imported = story_reading::import_reference(&source).unwrap();
        let reading = app
            .import_reading_document(&imported, "reference.txt")
            .unwrap();
        let passage_offset = imported.extracted_text.find("second reference").unwrap() as u64;
        app.create_reading_annotation(
            reading,
            &story_core::ReadingAnnotationDraft {
                kind: "highlight".into(),
                start_byte: passage_offset,
                end_byte: passage_offset + "second reference".len() as u64,
                quote: "second reference".into(),
                body: String::new(),
                prefix_context: "A ".into(),
                suffix_context: " passage.".into(),
            },
        )
        .unwrap();
        app.create_reading_bookmark(reading, "Second passage", passage_offset)
            .unwrap();
        app.link_reading_entity(
            reading,
            None,
            story_core::EntityRef::Character(character),
            "mentions",
        )
        .unwrap();
        let mut state = AppState {
            app,
            workspace: Workspace::Reading,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Reading],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: Some(reading),
            reading_text: imported.extracted_text.clone(),
            reading_offset: 0,
            reading_focus: true,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        state.mode = Some(':');
        state.prompt = "reading-links".into();
        state.command().unwrap();
        assert!(state.message.contains(&character.to_string()));
        state.mode = Some(':');
        state.prompt = "reading-follow 0".into();
        state.command().unwrap();
        assert_eq!(state.workspace, Workspace::StoryBible);
        assert_eq!(state.selected, 0);
        state.reading_document = Some(reading);
        state.reading_text = imported.extracted_text;
        state.mode = Some(':');
        state.prompt = "annotations".into();
        state.command().unwrap();
        assert!(state.message.contains("0: highlight"));
        state.mode = Some(':');
        state.prompt = "annotation-jump 0".into();
        state.command().unwrap();
        assert_eq!(state.reading_offset, passage_offset);
        assert_eq!(state.scroll, 1);
        let rendered = reading_text_display(
            &state.reading_text,
            &state.app.list_reading_annotations(reading).unwrap(),
        );
        assert!(
            rendered
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.content == "second reference" && span.style.bg == Some(Color::Yellow)
                })
        );
        state.mode = Some(':');
        state.prompt = "bookmarks".into();
        state.command().unwrap();
        assert!(state.message.contains("0: Second passage"));
        state.mode = Some(':');
        state.prompt = "bookmark-jump 0".into();
        state.command().unwrap();
        assert_eq!(state.reading_offset, passage_offset);
        assert_eq!(state.workspace, Workspace::Reading);
    }

    #[test]
    fn reading_render_styles_notes_and_ignores_stale_annotations() {
        let text = "highlight this; annotate that";
        let make = |kind: &str, state: &str, start: u64, end: u64, quote: &str| {
            story_storage::ReadingAnnotationRecord {
                id: story_core::AnnotationId::new(),
                document_id: story_core::ReadingId::new(),
                kind: kind.into(),
                start_byte: start,
                end_byte: end,
                quote: quote.into(),
                body: String::new(),
                state: state.into(),
            }
        };
        let annotations = [
            make("highlight", "current", 0, 14, "highlight this"),
            make("note", "current", 16, 29, "annotate that"),
            make("highlight", "needs_review", 0, 14, "highlight this"),
        ];
        let rendered = reading_text_display(text, &annotations);
        assert!(
            rendered
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.content == "highlight this" && span.style.bg == Some(Color::Yellow)
                })
        );
        assert!(
            rendered
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.content == "annotate that"
                        && span.style.fg == Some(Color::Cyan)
                        && span.style.add_modifier.contains(Modifier::UNDERLINED)
                })
        );
        let rendered_text = rendered
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(rendered_text, text);
    }

    #[test]
    fn analytics_workspace_rows_include_full_read_only_analysis() {
        let scenes = (0..5)
            .map(|index| story_analysis::SceneAnalysisInput {
                scene_id: format!("scene-{index}"),
                title: format!("Scene {index}"),
                chapter_id: None,
                actual_words: 100,
                target_words: None,
                start_time: story_core::StoryTime::unknown(),
                end_time: story_core::StoryTime::unknown(),
                characters: if index == 0 {
                    vec![
                        story_analysis::CharacterOnScene {
                            id: "mara".into(),
                            name: "Mara".into(),
                            role: story_core::ParticipationRole::Pov,
                        },
                        story_analysis::CharacterOnScene {
                            id: "orin".into(),
                            name: "Orin".into(),
                            role: story_core::ParticipationRole::Major,
                        },
                    ]
                } else {
                    Vec::new()
                },
                plotlines: if index == 0 || index == 4 {
                    vec![story_analysis::PlotlineOnScene {
                        id: "rebellion".into(),
                        name: "Rebellion".into(),
                    }]
                } else {
                    Vec::new()
                },
                locations: Vec::new(),
                events: Vec::new(),
            })
            .collect::<Vec<_>>();
        let rows = analytics_report_rows(&scenes, Some(2_000), 500, 2);
        assert!(rows.iter().any(|row| row == "Heatmap · Scene 4 │ "));
        assert!(
            rows.iter()
                .any(|row| row.contains("Interaction · Mara ↔ Orin · 1 scenes"))
        );
        assert!(
            rows.iter()
                .any(|row| row
                    .contains("Gap · Rebellion · Scene 0 → Scene 4 · 3 intervening scenes"))
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("Chronology · Scene 0 ↔ Scene 1 · uncertain"))
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("Budget suggestion · Scene 4 · 400 words"))
        );
        assert!(scenes.iter().all(|scene| scene.target_words.is_none()));
    }

    #[test]
    fn project_and_chapter_progress_rows_show_targeted_word_graphs() {
        let scene = story_analysis::SceneAnalysisInput {
            scene_id: "s1".into(),
            title: "First".into(),
            chapter_id: Some("chapter-1".into()),
            actual_words: 50,
            target_words: Some(100),
            start_time: story_core::StoryTime::unknown(),
            end_time: story_core::StoryTime::unknown(),
            characters: vec![],
            plotlines: vec![],
            locations: vec![],
            events: vec![],
        };
        let rows = analytics_report_rows(&[scene], Some(200), 50, 0);
        assert!(
            rows.iter()
                .any(|row| row.contains("Manuscript progress · 50/200 words · 25%"))
        );
        assert!(rows.iter().any(|row| {
            row.contains("Chapter progress · chapter-1 · 1 scenes · 50/100 words · 50%")
        }));
    }

    #[test]
    fn timeline_sorts_by_story_time_and_labels_uncertainty() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let later = app.create_scene(chapter, "Later", 0).unwrap();
        let unknown = app.create_scene(chapter, "Unknown", 1).unwrap();
        let earlier = app.create_scene(chapter, "Earlier", 2).unwrap();
        let mut scene = app.get_scene(later).unwrap().scene;
        scene.start_time = story_core::StoryTime::exact(50);
        app.update_scene_metadata(&scene).unwrap();
        let mut scene = app.get_scene(earlier).unwrap().scene;
        scene.start_time = story_core::StoryTime::exact(-story_core::STORY_TICKS_PER_DAY);
        app.update_scene_metadata(&scene).unwrap();
        let event = story_core::StoryEvent {
            id: story_core::EventId::new(),
            title: "The signal".into(),
            description: String::new(),
            start: story_core::StoryTime::exact(0),
            end: story_core::StoryTime::exact(30),
            source_time_text: "first night".into(),
        };
        app.add_story_event(&event).unwrap();
        assert_eq!(
            app.get_scene(earlier).unwrap().scene.start_time.nominal,
            Some(-story_core::STORY_TICKS_PER_DAY)
        );
        let ordered = timeline_scene_order(&app);
        assert_eq!(ordered[0].scene.id, earlier);
        assert_eq!(ordered[1].scene.id, later);
        assert_eq!(ordered[2].scene.id, unknown);
        let items = timeline_items(&app);
        assert!(matches!(&items[0], TimelineItem::Scene(row) if row.scene.id == earlier));
        assert!(matches!(&items[1], TimelineItem::Event(row) if row.id == event.id));
        assert!(matches!(&items[2], TimelineItem::Scene(row) if row.scene.id == later));
        assert!(matches!(&items[3], TimelineItem::Scene(row) if row.scene.id == unknown));
        let saved_event = app.list_events().unwrap().remove(0);
        assert_eq!(saved_event.start, event.start);
        assert_eq!(saved_event.end, event.end);
        assert_eq!(saved_event.source_time, "first night");
        assert_eq!(format_story_tick(-1), "D-1 23:59:59");
        assert_eq!(
            format_story_interval(story_core::StoryTime::unknown()),
            "unknown"
        );
        let bounded = story_core::StoryTime::new(
            Some(10),
            Some(5),
            Some(15),
            story_core::TimePrecision::Bounded,
        )
        .unwrap();
        assert_eq!(format_story_interval(bounded), "[D0 00:00:05–D0 00:00:15]");
        let approximate = story_core::StoryTime::new(
            Some(10),
            Some(5),
            Some(15),
            story_core::TimePrecision::Approximate,
        )
        .unwrap();
        assert_eq!(
            format_story_interval(approximate),
            "~D0 00:00:10 [D0 00:00:05–D0 00:00:15]"
        );
        assert_eq!(
            parse_story_time_input("?").unwrap(),
            story_core::StoryTime::unknown()
        );
        assert_eq!(
            parse_story_time_input("-7").unwrap(),
            story_core::StoryTime::exact(-7)
        );
        assert_eq!(
            parse_story_time_input("2..9").unwrap(),
            story_core::StoryTime::new(None, Some(2), Some(9), story_core::TimePrecision::Bounded)
                .unwrap()
        );
        assert_eq!(parse_story_time_input("~6,2,8").unwrap().nominal, Some(6));
        assert!(parse_story_time_input("~6,8,2").is_err());
        assert!(parse_story_time_input("not-a-tick").is_err());
    }

    #[test]
    fn list_navigation_can_reach_rows_beyond_scene_count() {
        assert_eq!(next_list_selection(10, 25), 11);
        assert_eq!(next_list_selection(24, 25), 24);
        assert_eq!(next_list_selection(3, 0), 0);
    }

    #[test]
    fn saved_query_is_a_dynamic_virtual_collection_and_opens_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let chapter = app.create_chapter(None, "Chapter", 0).unwrap();
        let first = app.create_scene(chapter, "First", 0).unwrap();
        let matching = app.create_scene(chapter, "Matching", 1).unwrap();
        let query = story_core::SavedSceneQuery {
            id: story_core::QueryId::new(),
            name: "Drafting scenes".into(),
            description: String::new(),
            filters: vec![story_core::SceneQueryFilter::Status(SceneStatus::Drafting)],
        };
        app.save_scene_query(&query).unwrap();
        let scenes = app
            .list_scenes(false)
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.scene.id,
                    row.scene.title,
                    row.scene.status,
                    row.actual_words,
                )
            })
            .collect::<Vec<_>>();
        let mut state = AppState {
            app,
            workspace: Workspace::Search,
            scenes: scenes.into_iter().take(1).collect(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Search],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        state.mode = Some(':');
        state.prompt = format!("query {}", query.id);
        state.command().unwrap();
        assert_eq!(state.query_view, Some(query.id));
        assert!(workspace_items(&state).is_empty());
        let mut scene = state.app.get_scene(matching).unwrap().scene;
        scene.status = SceneStatus::Drafting;
        state.app.update_scene_metadata(&scene).unwrap();
        let rows = workspace_items(&state);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].starts_with("Matching"));
        assert_eq!(state.scenes.len(), 2);
        assert_eq!(state.scenes[0].0, first);
        assert_eq!(state.scenes[1].0, matching);
        state.selected = 10;
        state.open_selected().unwrap();
        assert_eq!(state.editing_scene, Some(matching));
        assert_eq!(state.workspace, Workspace::Editor);
        let mut scene = state.app.get_scene(matching).unwrap().scene;
        scene.status = SceneStatus::Idea;
        state.app.update_scene_metadata(&scene).unwrap();
        state.workspace = Workspace::Search;
        state.open_selected().unwrap();
        assert_eq!(state.message, "Saved query has no matching scenes");
        assert_eq!(state.scenes.len(), 2);
        state.mode = Some(':');
        state.prompt = "query-clear".into();
        state.command().unwrap();
        assert_eq!(state.query_view, None);
        assert_eq!(workspace_items(&state).len(), 2);
    }

    #[test]
    fn project_search_routes_each_story_entity_to_its_record_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let character = app.create_character("Mara").unwrap();
        let location = app.create_location("Observatory").unwrap();
        let plotline = app.create_plotline("Rebellion").unwrap();
        let event = app.add_event("The signal", "Day 2").unwrap();
        let issue = app.create_issue("Check the map").unwrap();
        let mut state = AppState {
            app,
            workspace: Workspace::Search,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Search],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        let hit = |kind: &str, id: String, title: &str| story_storage::ProjectSearchHit {
            entity_kind: kind.into(),
            entity_id: id,
            title: title.into(),
            excerpt: title.into(),
        };
        for (kind, id, title, workspace, expected_index) in [
            (
                "character",
                character.to_string(),
                "Mara",
                Workspace::StoryBible,
                0,
            ),
            (
                "location",
                location.to_string(),
                "Observatory",
                Workspace::StoryBible,
                1,
            ),
            (
                "plotline",
                plotline.to_string(),
                "Rebellion",
                Workspace::StoryBible,
                2,
            ),
            (
                "event",
                event.to_string(),
                "The signal",
                Workspace::StoryBible,
                3,
            ),
            (
                "issue",
                issue.to_string(),
                "Check the map",
                Workspace::Issues,
                0,
            ),
        ] {
            state.workspace = Workspace::Search;
            state.selected = 0;
            state.search_hits = vec![hit(kind, id, title)];
            state.open_selected().unwrap();
            assert_eq!(state.workspace, workspace);
            assert_eq!(state.selected, expected_index);
            let row = workspace_items(&state)[expected_index].clone();
            assert!(row.contains(title));
            state.open_selected().unwrap();
            assert_eq!(state.workspace, workspace);
            assert_eq!(state.message, row);
            assert_eq!(state.editing_scene, None);
        }
        let chapter = state.app.create_chapter(None, "One", 0).unwrap();
        let scene = state.app.create_scene(chapter, "Evidence", 0).unwrap();
        let source = "Mara carries the silver key.";
        let (_, hash) = state.app.load_scene_text(scene).unwrap();
        state
            .app
            .save_scene_text(scene, Some(&hash), source)
            .unwrap();
        let proposal = story_ollama::attach_provenance(
            "scene",
            &scene.to_string(),
            source,
            "fixture",
            story_ollama::ExtractionResult {
                claims: vec![story_ollama::CandidateClaim {
                    kind: "object_possession".into(),
                    name: "silver key".into(),
                    claim: "Mara carries the key.".into(),
                    evidence_quote: "Mara carries the silver key.".into(),
                    approximate_story_time: None,
                }],
            },
        );
        let knowledge = state.app.approve_extraction_claim(&proposal, 0).unwrap();
        state.workspace = Workspace::Search;
        state.selected = 0;
        state.search_hits = vec![hit("knowledge_claim", knowledge.to_string(), "silver key")];
        state.open_selected().unwrap();
        assert_eq!(state.workspace, Workspace::StoryBible);
        assert_eq!(state.selected, 4);
        assert!(workspace_items(&state)[4].contains("Mara carries the key"));
    }

    #[test]
    fn outline_commands_create_move_rename_and_preserve_scene_content() {
        let dir = tempfile::tempdir().unwrap();
        let app = WriterApp::create_project(dir.path().join("Novel"), "Novel").unwrap();
        let mut state = AppState {
            app,
            workspace: Workspace::Outline,
            scenes: Vec::new(),
            selected: 0,
            editor: vec![String::new()],
            editing_scene: None,
            cursor: (0, 0),
            saved_hash: String::new(),
            dirty: false,
            mode: None,
            prompt: String::new(),
            message: String::new(),
            history: vec![Workspace::Outline],
            history_index: 0,
            width: 80,
            editor_focus_mode: false,
            horizontal_split_ratio: default_split_ratio(),
            vertical_split_ratio: default_split_ratio(),
            assembly_draft: None,
            planning_board_id: None,
            writing_stats_enabled: false,
            typed_characters: 0,
            pasted_characters: 0,
            successful_saves: 0,
            writing_session: None,
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            command_bindings: Vec::new(),
            revision_diff: None,
            revision_scene: None,
            revision_index: 0,
            revision_hunk: 0,
            search_hits: Vec::new(),
            search_locations: Vec::new(),
            search_location_index: 0,
            query_view: None,
            proposal_review_active: false,
            proposal_review_rows: Vec::new(),
            proposal_review_index: 0,
            editor_selection: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: String::new(),
            snippets: BTreeMap::new(),
            registers: BTreeMap::new(),
            macros: BTreeMap::new(),
            recording_macro: None,
            command_macros: BTreeMap::new(),
            recording_command_macro: None,
            text_expansion: TextExpansionConfig::default(),
            editor_preferences: EditorPreferences::default(),
            bookmarks: BTreeMap::new(),
            kill_ring: Vec::new(),
            kill_ring_index: 0,
            last_yank: None,
            macro_preview: None,
            extra_cursors: Vec::new(),
            active_challenge: None,
            menu: None,
            reference_panes: Vec::new(),
            active_reference: 0,
            reference_focused: false,
        };
        let execute = |state: &mut AppState, command: String| {
            state.mode = Some(':');
            state.prompt = command;
            state.command().unwrap();
        };
        let character = state.app.create_character("Mara Venn").unwrap();
        execute(
            &mut state,
            format!("character-set {character} preferred_name | Mara"),
        );
        execute(
            &mut state,
            format!("character-set {character} goals | Find the missing atlas"),
        );
        execute(
            &mut state,
            format!("character-alias {character} | The Cartographer"),
        );
        execute(&mut state, format!("character-profile {character}"));
        assert!(state.message.contains("Find the missing atlas"));
        assert!(state.message.contains("The Cartographer"));
        assert!(state.app.get_character(character).unwrap().preferred_name == "Mara");

        execute(&mut state, "section Act I".into());
        let section = state
            .app
            .list_structural_nodes()
            .unwrap()
            .into_iter()
            .find(|node| node.kind == "section")
            .unwrap()
            .id;
        execute(&mut state, format!("subchapter {section} Chapter One"));
        let chapter_one = state
            .app
            .list_structural_nodes()
            .unwrap()
            .into_iter()
            .find(|node| node.title == "Chapter One")
            .unwrap()
            .id;
        execute(&mut state, format!("scene {chapter_one} Opening"));
        let scene = state.app.list_scenes(false).unwrap()[0].scene.id;
        let (_, hash) = state.app.load_scene_text(scene).unwrap();
        state
            .app
            .save_scene_text(scene, Some(&hash), "The story begins.")
            .unwrap();
        let character = state.app.create_character("Mara").unwrap();
        state
            .app
            .link_character(scene, character, story_core::ParticipationRole::Pov)
            .unwrap();
        execute(&mut state, format!("subchapter {section} Chapter Two"));
        let chapter_two = state
            .app
            .list_structural_nodes()
            .unwrap()
            .into_iter()
            .find(|node| node.title == "Chapter Two")
            .unwrap()
            .id;
        execute(&mut state, format!("move {scene} {chapter_two} 0"));
        execute(&mut state, format!("rename {scene} Arrival"));
        let result = state.app.get_scene(scene).unwrap();
        assert_eq!(result.scene.title, "Arrival");
        assert_eq!(result.chapter_id, Some(chapter_two));
        assert_eq!(
            state.app.load_scene_text(scene).unwrap().0,
            "The story begins."
        );
        assert_eq!(
            state.app.analytics_dataset().unwrap()[0].characters[0].id,
            character.to_string()
        );
        execute(&mut state, "process-only export".into());
        assert!(state.message.contains("Compile:Complete"));
        assert!(state.message.contains("Export:Complete"));
        assert!(state.message.contains("WordCount:NotSelected"));
        assert!(
            !dir.path()
                .join("Novel/.writers-ide/processing/word-count.txt")
                .exists()
        );
    }
}
