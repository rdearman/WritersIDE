use serde::Deserialize;
use std::{collections::BTreeMap, env, fs, io, path::Path, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use story_app::WriterApp;
use story_core::{SceneId, SceneStatus};

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
}
impl Workspace {
    const ALL: [Self; 14] = [
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
    scroll: usize,
    reading_document: Option<story_core::ReadingId>,
    reading_text: String,
    reading_offset: u64,
    reading_focus: bool,
    reading_selection_start: Option<u64>,
    function_bindings: Vec<(u8, Workspace)>,
}

#[derive(Deserialize)]
struct KeymapFile {
    bindings: BTreeMap<String, String>,
}

fn default_bindings() -> Vec<(u8, Workspace)> {
    Workspace::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, workspace)| ((index + 1) as u8, workspace))
        .collect()
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
        let workspace = Workspace::ALL
            .iter()
            .copied()
            .find(|w| w.title().eq_ignore_ascii_case(&name))
            .ok_or_else(|| format!("unknown workspace in keymap: {name}"))?;
        result[(number - 1) as usize] = (number, workspace);
    }
    Ok(result)
}
impl AppState {
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
        if let Some(id) = self.active_scene() {
            if self.dirty && self.editing_scene != Some(id) {
                self.message =
                    "Unsaved text is open. Save with Ctrl-S before opening another scene.".into();
                return Ok(());
            }
            let (text, hash) = self.app.load_scene_text(id)?;
            self.editor = text.lines().map(str::to_owned).collect();
            if self.editor.is_empty() {
                self.editor.push(String::new());
            }
            self.cursor = (0, 0);
            self.saved_hash = hash;
            self.editing_scene = Some(id);
            self.dirty = false;
            self.workspace = Workspace::Editor;
        }
        Ok(())
    }
    fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(id) = self.editing_scene {
            let text = self.editor.join("\n");
            match self.app.save_scene_text(id, Some(&self.saved_hash), &text) {
                Ok(hash) => {
                    self.saved_hash = hash;
                    self.dirty = false;
                    self.message = "Saved".into();
                }
                Err(e) => self.message = format!("Save conflict/error: {e}"),
            }
        }
        Ok(())
    }
    fn switch(&mut self, w: Workspace) {
        if self.workspace != w {
            if self.history_index + 1 < self.history.len() {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(self.workspace);
            self.history_index = self.history.len() - 1;
            self.workspace = w;
        }
    }
    fn selected_scene(&self) -> String {
        self.scenes
            .get(self.selected)
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "No scene selected".into())
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
            self.selected = 0;
            self.workspace = Workspace::Search;
            self.message = format!("{} search results", self.scenes.len());
            return Ok(());
        }
        if cmd == "process" {
            let reports = self.app.run_processing()?;
            self.message = reports
                .into_iter()
                .map(|report| format!("{:?}:{:?}", report.stage, report.status))
                .collect::<Vec<_>>()
                .join(" · ");
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
                .map(|a| format!("{} [{}] {}", a.kind, a.state, a.quote))
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
                .map(|b| format!("{} @{} [{}]", b.title, b.offset_byte, b.state))
                .collect::<Vec<_>>()
                .join(" · ");
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
            self.reading_offset = rest.parse()?;
            let offset = usize::try_from(self.reading_offset)?;
            let prefix = self
                .reading_text
                .get(..offset)
                .ok_or("reading position is outside the text or splits a UTF-8 character")?;
            self.scroll = prefix.matches('\n').count();
            self.reading_focus = true;
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
        } else if cmd == "query-list" {
            self.message = self
                .app
                .list_saved_queries()?
                .into_iter()
                .map(|query| format!("{} {}", query.id, query.name))
                .collect::<Vec<_>>()
                .join(" · ");
        } else if let Some(rest) = cmd.strip_prefix("query-save ") {
            let mut parts = rest.split_whitespace();
            let name = parts
                .next()
                .ok_or("use query-save <name> [key=value ...]")?;
            let mut filters = Vec::new();
            for filter in parts {
                let (key, value) = filter
                    .split_once('=')
                    .ok_or("query filters use key=value")?;
                filters.push(match key {
                    "status" => story_core::SceneQueryFilter::Status(parse_terminal_status(value)?),
                    "character" => story_core::SceneQueryFilter::Character(
                        story_core::CharacterId(value.parse()?),
                    ),
                    "plotline" => story_core::SceneQueryFilter::Plotline(story_core::PlotlineId(
                        value.parse()?,
                    )),
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
            let found = self.app.run_scene_query(id)?;
            self.scenes = self
                .app
                .list_scenes(false)?
                .into_iter()
                .filter(|s| found.contains(&s.scene.id))
                .map(|r| (r.scene.id, r.scene.title, r.scene.status, r.actual_words))
                .collect();
            self.selected = 0;
            self.workspace = Workspace::Search;
            self.message = format!("{} matching scenes", self.scenes.len());
        } else if cmd == "help" {
            self.message="F1-F14 workspaces (override in .writers-ide/keybindings.toml); arrows select/scroll; Enter opens; : commands; / searches; Ctrl-S saves; q quits".into();
        } else {
            self.message = "Commands: open UUID | workspace NAME | query UUID | query-list | query-save NAME filters... | process | read-import PATH | read UUID | highlight start:end quote | note start:end quote | body | bookmark TITLE | bookmarks | annotations | terms | jump BYTE | checkpoint | term KIND NAME | entity-link KIND UUID | help".into();
        }
        Ok(())
    }
    fn insert(&mut self, ch: char) {
        let (row, col) = self.cursor;
        let line = &mut self.editor[row];
        let byte = line
            .char_indices()
            .nth(col)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        line.insert(byte, ch);
        self.cursor.1 += 1;
        self.dirty = true;
    }
    fn backspace(&mut self) {
        let (row, col) = self.cursor;
        if col > 0 {
            let line = &mut self.editor[row];
            let end = line
                .char_indices()
                .nth(col)
                .map(|(b, _)| b)
                .unwrap_or(line.len());
            let start = line
                .char_indices()
                .nth(col - 1)
                .map(|(b, _)| b)
                .unwrap_or(0);
            line.replace_range(start..end, "");
            self.cursor.1 -= 1;
            self.dirty = true;
        } else if row > 0 {
            let tail = self.editor.remove(row);
            self.cursor.0 -= 1;
            self.cursor.1 = self.editor[self.cursor.0].chars().count();
            self.editor[self.cursor.0].push_str(&tail);
            self.dirty = true;
        }
    }
    fn delete_forward(&mut self) {
        let (row, col) = self.cursor;
        let count = self.editor[row].chars().count();
        if col < count {
            let line = &mut self.editor[row];
            let start = line
                .char_indices()
                .nth(col)
                .map(|(b, _)| b)
                .unwrap_or(line.len());
            let end = line
                .char_indices()
                .nth(col + 1)
                .map(|(b, _)| b)
                .unwrap_or(line.len());
            line.replace_range(start..end, "");
            self.dirty = true;
        } else if row + 1 < self.editor.len() {
            let tail = self.editor.remove(row + 1);
            self.editor[row].push_str(&tail);
            self.dirty = true;
        }
    }
    fn paste_text(&mut self, text: &str) {
        for ch in text.replace("\r\n", "\n").replace('\r', "\n").chars() {
            if ch == '\n' {
                self.enter()
            } else {
                self.insert(ch)
            }
        }
    }
    fn enter(&mut self) {
        let (row, col) = self.cursor;
        let line = &mut self.editor[row];
        let byte = line
            .char_indices()
            .nth(col)
            .map(|(b, _)| b)
            .unwrap_or(line.len());
        let tail = line.split_off(byte);
        self.editor.insert(row + 1, tail);
        self.cursor = (row + 1, 0);
        self.dirty = true;
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: writers-ide-tui <project-path>")?;
    let project_path = std::path::PathBuf::from(path);
    let function_bindings = load_bindings(&project_path)?;
    let app = WriterApp::open_project(&project_path)?;
    let mut state = AppState {
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
        message: String::new(),
        history: vec![Workspace::Dashboard],
        history_index: 0,
        width: 80,
        scroll: 0,
        reading_document: None,
        reading_text: String::new(),
        reading_offset: 0,
        reading_focus: false,
        reading_selection_start: None,
        function_bindings,
    };
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
    loop {
        t.draw(|f| draw(f, s))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(k) if handle(s, k)? => break,
                Event::Paste(text) if s.workspace == Workspace::Editor => s.paste_text(&text),
                _ => {}
            }
        }
    }
    Ok(())
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
        Span::raw("F1-F14 workspaces  ? help  : command  / search  Ctrl-S save  q quit"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(head, rows[0]);
    let body = if pane_direction(s.width) == Direction::Vertical {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(rows[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(rows[1])
    };
    if s.workspace == Workspace::Editor {
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
        let content = s
            .editor
            .iter()
            .enumerate()
            .skip(s.scroll)
            .map(|(i, line)| {
                if i == s.cursor.0 {
                    format!("> {line}")
                } else {
                    format!("  {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let editor = Paragraph::new(content).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(format!("Editor — {}", s.selected_scene()))
                .borders(Borders::ALL),
        );
        f.render_widget(editor, body[1]);
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
            Paragraph::new(text)
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
        let mut state =
            ListState::default().with_selected(Some(s.selected.min(items.len().saturating_sub(1))));
        let list = List::new(items)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .title(s.workspace.title())
                    .borders(Borders::ALL),
            );
        f.render_stateful_widget(list, body[0], &mut state);
        let detail = if let Some((id, title, status, words)) = s.scenes.get(s.selected) {
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
    let footer = if let Some(mode) = s.mode {
        format!(" {mode} {}_", s.prompt)
    } else {
        format!(
            " {}   Enter open | j/k or ↑/↓ select | Tab next | Backspace back | {}",
            s.message,
            if s.dirty { "modified" } else { "clean" }
        )
    };
    f.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::TOP)),
        rows[2],
    );
}
fn pane_direction(width: usize) -> Direction {
    if width < 100 {
        Direction::Vertical
    } else {
        Direction::Horizontal
    }
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
        Workspace::Outline | Workspace::Cards | Workspace::Search => s
            .scenes
            .iter()
            .map(|(_, title, status, words)| {
                format!("{title}   {}   {words} words", status.as_str())
            })
            .collect(),
        Workspace::Editor => Vec::new(),
        Workspace::Characters => s
            .app
            .list_characters()
            .unwrap_or_default()
            .into_iter()
            .map(|c| format!("{}   {} scene appearances", c.name, c.scene_count))
            .collect(),
        Workspace::PlotGrid => s
            .app
            .list_plotlines()
            .unwrap_or_default()
            .into_iter()
            .map(|p| format!("{}   {} scene appearances", p.name, p.scene_count))
            .collect(),
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
            rows
        }
        Workspace::Timeline => s
            .app
            .list_scenes(false)
            .unwrap_or_default()
            .into_iter()
            .map(|scene| {
                format!(
                    "{:>14}  {}",
                    scene
                        .scene
                        .start_time
                        .nominal
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "unknown".into()),
                    scene.scene.title
                )
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
            let heatmap = story_analysis::character_heatmap(&data);
            let interactions = story_analysis::character_interactions(&data);
            let gaps = story_analysis::subplot_gaps(&data, 3);
            let budget = s
                .app
                .get_project()
                .ok()
                .and_then(|p| p.target_words)
                .map(|target| story_analysis::preview_budget_redistribution(target, &data));
            let mut report = vec![
                format!("{} scenes", s.scenes.len()),
                format!("{total} actual words"),
                format!("{draft} drafted or complete scenes"),
                format!(
                    "{} characters",
                    s.app.list_characters().map(|x| x.len()).unwrap_or(0)
                ),
                format!(
                    "{} plotlines",
                    s.app.list_plotlines().map(|x| x.len()).unwrap_or(0)
                ),
                format!("{} co-appearance pairs", interactions.len()),
                format!("{} subplot gaps of 3+ scenes", gaps.len()),
            ];
            if let Some(budget) = budget {
                report.push(format!(
                    "Budget preview: {} target words; {} allocated; {} remaining",
                    budget.target_words, budget.fixed_words, budget.unallocated_words
                ));
            }
            report.extend(heatmap.into_iter().take(20).map(|row| {
                format!(
                    "{} │ {}",
                    row.scene_title,
                    row.participants
                        .iter()
                        .map(|(_id, name, role)| format!(
                            "{}:{}",
                            name,
                            if *role == story_core::ParticipationRole::Pov {
                                "POV"
                            } else {
                                "in"
                            }
                        ))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }));
            report
        }
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
        Workspace::Revisions => s
            .active_scene()
            .and_then(|scene| s.app.list_snapshots(scene).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|snapshot| {
                format!(
                    "{}  {}  {}",
                    snapshot.id, snapshot.created_at, snapshot.label
                )
            })
            .collect(),
    }
}
fn parse_entity(kind: &str, id: &str) -> Result<story_core::EntityRef, Box<dyn std::error::Error>> {
    Ok(match kind {
        "scene" => story_core::EntityRef::Scene(SceneId(id.parse()?)),
        "character" => story_core::EntityRef::Character(story_core::CharacterId(id.parse()?)),
        "plotline" => story_core::EntityRef::Plotline(story_core::PlotlineId(id.parse()?)),
        "research" => story_core::EntityRef::Research(story_core::ResearchId(id.parse()?)),
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
fn handle(s: &mut AppState, k: KeyEvent) -> Result<bool, Box<dyn std::error::Error>> {
    if s.mode.is_some() {
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
        s.save()?;
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
        if s.dirty {
            s.message = "Unsaved text: Ctrl-S to save before quitting".into();
            return Ok(false);
        }
        return Ok(true);
    }
    if s.workspace == Workspace::Editor {
        match k.code {
            KeyCode::Enter => s.enter(),
            KeyCode::Backspace => s.backspace(),
            KeyCode::Delete => s.delete_forward(),
            KeyCode::Home => s.cursor.1 = 0,
            KeyCode::End => s.cursor.1 = s.editor[s.cursor.0].chars().count(),
            KeyCode::Char('w') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                let row = s.cursor.0;
                while s.cursor.1 > 0
                    && s.editor[row]
                        .chars()
                        .nth(s.cursor.1 - 1)
                        .is_some_and(char::is_whitespace)
                {
                    s.backspace();
                }
                while s.cursor.1 > 0
                    && s.editor[row]
                        .chars()
                        .nth(s.cursor.1 - 1)
                        .is_some_and(|c| !c.is_whitespace())
                {
                    s.backspace();
                }
            }
            KeyCode::Left => s.cursor.1 = s.cursor.1.saturating_sub(1),
            KeyCode::Right => {
                s.cursor.1 = (s.cursor.1 + 1).min(s.editor[s.cursor.0].chars().count())
            }
            KeyCode::Up => s.cursor.0 = s.cursor.0.saturating_sub(1),
            KeyCode::Down => s.cursor.0 = (s.cursor.0 + 1).min(s.editor.len() - 1),
            KeyCode::Char(c) if k.modifiers.is_empty() => s.insert(c),
            KeyCode::Esc => s.switch(Workspace::Outline),
            _ => {}
        }
        return Ok(false);
    }
    match k.code {
        KeyCode::F(n) if (1..=14).contains(&n)=>{if let Some((_, workspace))=s.function_bindings.get((n-1)as usize){s.switch(*workspace);}},
        KeyCode::Char(':')=>{s.mode=Some(':');s.prompt.clear();},
        KeyCode::Char('/')=>{s.mode=Some('/');s.prompt.clear();},
        KeyCode::Char('?')=>s.message="F1 Dashboard · F2 Outline · F3 Cards · F4 Editor · F5 Plot Grid · F6 Characters · F7 Story Bible · F8 Timeline · F9 Analytics · F10 Research · F11 Issues · F12 Search · F13 Reading · F14 Revisions. : commands; / search; Ctrl-S save.".into(),
        KeyCode::Tab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+1)%Workspace::ALL.len()]);},
        KeyCode::BackTab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+Workspace::ALL.len()-1)%Workspace::ALL.len()]);},
        KeyCode::Up|KeyCode::Char('k')=>s.selected=s.selected.saturating_sub(1),
        KeyCode::Down|KeyCode::Char('j')=>{if !s.scenes.is_empty(){s.selected=(s.selected+1).min(s.scenes.len()-1);}},
        KeyCode::Enter if s.workspace==Workspace::Revisions=>{let scene=s.active_scene().ok_or("select a scene first")?;let snapshots=s.app.list_snapshots(scene)?;if let Some(snapshot)=snapshots.get(s.selected){let diff=s.app.diff_snapshot(scene,snapshot.id)?;s.message=diff.into_iter().map(|line|format!("{}{}",match line.kind{story_storage::SnapshotLineKind::Equal=>' ',story_storage::SnapshotLineKind::Added=>'+',story_storage::SnapshotLineKind::Removed=>'-'},line.text.trim_end())).collect::<Vec<_>>().join(" | ");}},
        KeyCode::Enter=>s.open_selected()?,
        _=>{}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    use crossterm::event::KeyModifiers;

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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
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
}
