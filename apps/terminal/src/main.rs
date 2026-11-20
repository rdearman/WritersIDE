use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::Path,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
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
    search_hits: Vec<story_storage::ProjectSearchHit>,
    query_view: Option<story_core::QueryId>,
    editor_selection: Option<(usize, usize)>,
    clipboard: String,
}

#[derive(Clone, Debug)]
enum TimelineItem {
    Scene(story_storage::SceneRecord),
    Event(story_storage::EventSummary),
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
        self.editor = text.lines().map(str::to_owned).collect();
        if self.editor.is_empty() {
            self.editor.push(String::new());
        }
        self.cursor = (0, 0);
        self.saved_hash = hash;
        self.editing_scene = Some(id);
        self.dirty = false;
        self.workspace = Workspace::Editor;
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
            self.query_view = None;
            self.search_hits.clear();
            self.selected = 0;
            self.workspace = Workspace::Search;
            self.message = format!("{} search results", self.scenes.len());
            return Ok(());
        }
        if cmd == "process" || cmd == "process-expand" {
            let reports = if cmd == "process-expand" {
                self.app.run_processing_with_transclusions()?
            } else {
                self.app.run_processing()?
            };
            self.message = reports
                .into_iter()
                .map(|report| format!("{:?}:{:?}", report.stage, report.status))
                .collect::<Vec<_>>()
                .join(" · ");
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
            }
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
        } else if let Some(terms) = cmd.strip_prefix("search-all ") {
            self.search_hits = self.app.search_project(terms, 200)?;
            self.query_view = None;
            self.selected = 0;
            self.switch(Workspace::Search);
            self.message = format!("{} project-wide matches", self.search_hits.len());
        } else if cmd == "help" {
            self.message="F1-F14 workspaces (override in .writers-ide/keybindings.toml); arrows select/scroll; Enter opens; : commands; / searches; Ctrl-S saves; q quits".into();
        } else {
            self.message = "Commands: section TITLE | chapter TITLE | character NAME | location NAME | character-link SCENE UUID ROLE | scene-field SCENE FIELD | TEXT | plotline NAME | plotline-link SCENE PLOTLINE overlap|equal [ROLE] | plotline-shares SCENE PLOTLINE=BPS... | issue TITLE | issue-link SCENE ISSUE | issue-status ISSUE STATUS | event TITLE | TIME | event-time TITLE | START | END | SOURCE-TIME | event-link SCENE EVENT [KIND] | research TITLE | BODY | subchapter SECTION-ID TITLE | scene CHAPTER-ID TITLE | move SCENE-ID CHAPTER-ID POSITION | rename SCENE-ID TITLE | scene-status SCENE-ID STATUS | open UUID | workspace NAME | search-all TERMS | query UUID | query-list | query-clear | query-save NAME filters... | link SRC-KIND SRC-ID DST-KIND DST-ID TYPE [LABEL] | outgoing KIND UUID | backlinks KIND UUID | broken-links | anchors | anchor-move UUID START END | transclude SCENE-ID | process | process-expand | extract scene|reading UUID MODEL | read-import PATH | read UUID | highlight start:end quote | note start:end quote | body | bookmark TITLE | bookmarks | annotations | annotation-jump INDEX | bookmark-jump INDEX | terms | jump BYTE | checkpoint | term KIND NAME | entity-link KIND UUID | reading-links | reading-follow INDEX | help".into();
        }
        Ok(())
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
        self.delete_selection();
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
        if self.delete_selection() {
            return;
        }
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
        if self.delete_selection() {
            return;
        }
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
        self.delete_selection();
        for ch in text.replace("\r\n", "\n").replace('\r', "\n").chars() {
            if ch == '\n' {
                self.enter()
            } else {
                self.insert(ch)
            }
        }
    }
    fn enter(&mut self) {
        self.delete_selection();
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
                    .chars()
                    .skip(start_col)
                    .take(end_col - start_col)
                    .collect(),
            );
        }
        let mut lines = Vec::new();
        lines.push(
            self.editor[start_row]
                .chars()
                .skip(start_col)
                .collect::<String>(),
        );
        lines.extend(self.editor[start_row + 1..end_row].iter().cloned());
        lines.push(self.editor[end_row].chars().take(end_col).collect());
        Some(lines.join("\n"))
    }
    fn delete_selection(&mut self) -> bool {
        let Some(((start_row, start_col), (end_row, end_col))) = self.selection_bounds() else {
            return false;
        };
        let prefix: String = self.editor[start_row].chars().take(start_col).collect();
        let suffix: String = self.editor[end_row].chars().skip(end_col).collect();
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
        self.cursor = (row, col.min(self.editor[row].chars().count()));
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
        search_hits: Vec::new(),
        query_view: None,
        editor_selection: None,
        clipboard: String::new(),
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
        let selection = s.selection_bounds();
        let content = s
            .editor
            .iter()
            .enumerate()
            .skip(s.scroll)
            .map(|(row, line)| {
                let prefix = if row == s.cursor.0 { "> " } else { "  " };
                let mut spans = vec![Span::raw(prefix)];
                for (col, ch) in line.chars().enumerate() {
                    let selected = selection.is_some_and(|((sr, sc), (er, ec))| {
                        (row > sr || (row == sr && col >= sc))
                            && (row < er || (row == er && col < ec))
                    });
                    if selected {
                        spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(Color::White).bg(Color::Blue),
                        ));
                    } else {
                        spans.push(Span::raw(ch.to_string()));
                    }
                }
                Line::from(spans)
            })
            .collect::<Vec<_>>();
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
        let detail = if matches!(
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
            analytics_report_rows(&data, target, total, draft)
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
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('c') => {
                    if let Some(text) = s.selected_text() {
                        s.clipboard = text;
                    }
                    return Ok(false);
                }
                KeyCode::Char('x') => {
                    if let Some(text) = s.selected_text() {
                        s.clipboard = text;
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
            KeyCode::Enter => s.enter(),
            KeyCode::Backspace => s.backspace(),
            KeyCode::Delete => s.delete_forward(),
            KeyCode::Home => s.move_cursor(s.cursor.0, 0, selecting),
            KeyCode::End => {
                s.move_cursor(s.cursor.0, s.editor[s.cursor.0].chars().count(), selecting)
            }
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
            KeyCode::Left if s.cursor.1 > 0 => s.move_cursor(s.cursor.0, s.cursor.1 - 1, selecting),
            KeyCode::Left if s.cursor.0 > 0 => {
                let row = s.cursor.0 - 1;
                let col = s.editor[row].chars().count();
                s.move_cursor(row, col, selecting);
            }
            KeyCode::Right if s.cursor.1 < s.editor[s.cursor.0].chars().count() => {
                s.move_cursor(s.cursor.0, s.cursor.1 + 1, selecting)
            }
            KeyCode::Right if s.cursor.0 + 1 < s.editor.len() => {
                s.move_cursor(s.cursor.0 + 1, 0, selecting);
            }
            KeyCode::Up => {
                let row = s.cursor.0.saturating_sub(1);
                s.move_cursor(row, s.cursor.1, selecting);
            }
            KeyCode::Down => {
                let row = (s.cursor.0 + 1).min(s.editor.len() - 1);
                s.move_cursor(row, s.cursor.1, selecting);
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
        KeyCode::F(n) if (1..=14).contains(&n)=>{if let Some((_, workspace))=s.function_bindings.get((n-1)as usize){s.switch(*workspace);}},
        KeyCode::Char(':')=>{s.mode=Some(':');s.prompt.clear();},
        KeyCode::Char('/')=>{s.mode=Some('/');s.prompt.clear();},
        KeyCode::Char('?')=>s.message="F1 Dashboard · F2 Outline · F3 Cards · F4 Editor · F5 Plot Grid · F6 Characters · F7 Story Bible · F8 Timeline · F9 Analytics · F10 Research · F11 Issues · F12 Search · F13 Reading · F14 Revisions. : commands; / search; Ctrl-S save.".into(),
        KeyCode::Tab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+1)%Workspace::ALL.len()]);},
        KeyCode::BackTab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+Workspace::ALL.len()-1)%Workspace::ALL.len()]);},
        KeyCode::Up|KeyCode::Char('k')=>s.selected=s.selected.saturating_sub(1),
        KeyCode::Down|KeyCode::Char('j')=>{s.selected=next_list_selection(s.selected,workspace_items(s).len());},
        KeyCode::Enter if s.workspace==Workspace::Revisions=>{let scene=s.active_scene().ok_or("select a scene first")?;let snapshots=s.app.list_snapshots(scene)?;if let Some(snapshot)=snapshots.get(s.selected){let diff=s.app.diff_snapshot(scene,snapshot.id)?;s.message=diff.into_iter().map(|line|format!("{}{}",match line.kind{story_storage::SnapshotLineKind::Equal=>' ',story_storage::SnapshotLineKind::Added=>'+',story_storage::SnapshotLineKind::Removed=>'-'},line.text.trim_end())).collect::<Vec<_>>().join(" | ");}},
        KeyCode::Enter if s.workspace==Workspace::Timeline=>{
            let timeline = timeline_items(&s.app);
            let item = timeline
                .get(s.selected)
                .cloned()
                .ok_or("timeline selection is out of range")?;
            match item {
                TimelineItem::Scene(scene) => {
                    s.selected=s.scenes.iter().position(|row|row.0==scene.scene.id).ok_or("timeline scene no longer exists")?;
                    s.open_selected()?;
                }
                TimelineItem::Event(event) => {
                    let event_index = s.app.list_events()?.iter().position(|row|row.id==event.id).ok_or("timeline event no longer exists")?;
                    s.selected = s.app.list_characters()?.len() + s.app.list_plotlines()?.len() + event_index;
                    s.switch(Workspace::StoryBible);
                    s.message = format!("Selected story event {}", event.title);
                }
            }
        },
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: Some((0, 2)),
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: Some(reading),
            reading_text: imported.extracted_text.clone(),
            reading_offset: 0,
            reading_focus: true,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
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
            scroll: 0,
            reading_document: None,
            reading_text: String::new(),
            reading_offset: 0,
            reading_focus: false,
            reading_selection_start: None,
            function_bindings: default_bindings(),
            search_hits: Vec::new(),
            query_view: None,
            editor_selection: None,
            clipboard: String::new(),
        };
        let execute = |state: &mut AppState, command: String| {
            state.mode = Some(':');
            state.prompt = command;
            state.command().unwrap();
        };
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
    }
}
