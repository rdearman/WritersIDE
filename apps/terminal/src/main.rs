use std::{env, io, time::Duration};

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
}
impl Workspace {
    const ALL: [Self; 12] = [
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
        if mode == Some('/') {
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
        if let Some(rest) = cmd.strip_prefix("open ") {
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
            self.message="F1-F12 switch workspaces; arrows select; Enter opens; : commands; / search; Ctrl-S saves; q quits".into();
        } else {
            self.message = "Commands: open UUID | workspace NAME | query UUID | help".into();
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: writers-ide-tui <project-path>")?;
    let app = WriterApp::open_project(path)?;
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
        crossterm::event::EnableMouseCapture
    )?;
    Terminal::new(CrosstermBackend::new(out))
}
fn restore(t: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(
        t.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    t.show_cursor()
}
fn run(
    t: &mut Terminal<CrosstermBackend<io::Stdout>>,
    s: &mut AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        t.draw(|f| draw(f, s))?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(k) = event::read()?
            && handle(s, k)?
        {
            break;
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
        Span::raw("F1-F12 workspaces  ? help  : command  / search  Ctrl-S save  q quit"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(head, rows[0]);
    let body = if s.width < 100 {
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
            vec![
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
            ]
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
    }
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
        KeyCode::F(n) if (1..=12).contains(&n)=>s.switch(Workspace::ALL[(n-1)as usize]),
        KeyCode::Char(':')=>{s.mode=Some(':');s.prompt.clear();},
        KeyCode::Char('/')=>{s.mode=Some('/');s.prompt.clear();},
        KeyCode::Char('?')=>s.message="F1 Dashboard · F2 Outline · F3 Cards · F4 Editor · F5 Plot Grid · F6 Characters · F7 Story Bible · F8 Timeline · F9 Analytics · F10 Research · F11 Issues · F12 Search. : commands; / search; Ctrl-S save.".into(),
        KeyCode::Tab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+1)%Workspace::ALL.len()]);},
        KeyCode::BackTab=>{let i=Workspace::ALL.iter().position(|x|*x==s.workspace).unwrap_or(0);s.switch(Workspace::ALL[(i+Workspace::ALL.len()-1)%Workspace::ALL.len()]);},
        KeyCode::Up|KeyCode::Char('k')=>s.selected=s.selected.saturating_sub(1),
        KeyCode::Down|KeyCode::Char('j')=>{if !s.scenes.is_empty(){s.selected=(s.selected+1).min(s.scenes.len()-1);}},
        KeyCode::Enter=>s.open_selected()?,
        _=>{}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
