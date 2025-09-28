//! ---
//! ems_section: "12-gui-setup-wizard"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "UI service launcher for the setup wizard."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::{ArgAction, Parser};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use r_ems_common::version::VersionInfo;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};

#[derive(Parser, Debug)]
#[command(
    author,
    disable_version_flag = true,
    about = "Explore R-EMS log files in a terminal UI",
    propagate_version = false
)]
struct Cli {
    /// Directory containing log files (defaults to installer output)
    #[arg(long, default_value = "target/docker-logs")]
    dir: PathBuf,
    /// Refresh interval in milliseconds for reloading content while following
    #[arg(long, default_value_t = 500)]
    refresh: u64,
    /// Disable automatic follow (tail) behaviour
    #[arg(long)]
    no_follow: bool,

    /// Print extended version information and exit
    #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
    version: bool,
}

struct FileEntry {
    path: PathBuf,
    name: String,
    modified: Option<SystemTime>,
    size: u64,
}

impl FileEntry {
    fn display_line(&self) -> String {
        let stamp = self
            .modified
            .map(|m| {
                DateTime::<Local>::from(m)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|| "--".to_owned());
        let size_kb = if self.size > 0 {
            format!("{:>5} KB", self.size / 1024)
        } else {
            "    - KB".to_owned()
        };
        format!("{}  {}  {}", stamp, size_kb, self.name)
    }
}

struct App {
    dir: PathBuf,
    files: Vec<FileEntry>,
    selected: usize,
    lines: Vec<String>,
    view_offset: usize,
    view_height: usize,
    follow: bool,
}

impl App {
    fn new(dir: PathBuf, follow: bool) -> Result<Self> {
        let mut app = Self {
            dir,
            files: Vec::new(),
            selected: 0,
            lines: Vec::new(),
            view_offset: 0,
            view_height: 1,
            follow,
        };
        app.refresh_files()?;
        Ok(app)
    }

    fn refresh_files(&mut self) -> Result<()> {
        let previous = self.current_path().cloned();
        self.files = collect_files(&self.dir)?;
        if self.files.is_empty() {
            self.selected = 0;
            self.lines = vec!["No log files found".to_owned()];
            self.view_offset = 0;
            return Ok(());
        }
        if let Some(prev) = previous {
            if let Some(idx) = self.files.iter().position(|entry| entry.path == prev) {
                self.selected = idx;
            } else {
                self.selected = 0;
            }
        } else {
            self.selected = 0;
        }
        self.load_selected_file();
        Ok(())
    }

    fn current_path(&self) -> Option<&PathBuf> {
        self.files.get(self.selected).map(|entry| &entry.path)
    }

    fn load_selected_file(&mut self) {
        let Some(path) = self.current_path() else {
            self.lines = vec!["No log files found".to_owned()];
            self.view_offset = 0;
            return;
        };
        match fs::read_to_string(path) {
            Ok(content) => {
                let mut lines: Vec<String> = content.lines().map(|line| line.to_owned()).collect();
                if lines.is_empty() {
                    lines.push("(empty file)".to_owned());
                }
                self.lines = lines;
                if self.follow {
                    self.view_offset = self.max_scroll();
                } else {
                    self.view_offset = self.view_offset.min(self.max_scroll());
                }
            }
            Err(err) => {
                self.lines = vec![format!("Error reading {}: {err}", path.display())];
                self.view_offset = 0;
            }
        }
    }

    fn max_scroll(&self) -> usize {
        let visible = self.view_height.max(1);
        self.lines.len().saturating_sub(visible)
    }

    fn update_view_height(&mut self, height: u16) {
        self.view_height = height.max(1) as usize;
        self.view_offset = self.view_offset.min(self.max_scroll());
    }

    fn select_next(&mut self) {
        if self.files.is_empty() {
            return;
        }
        if self.selected + 1 < self.files.len() {
            self.selected += 1;
            self.view_offset = if self.follow { self.max_scroll() } else { 0 };
            self.load_selected_file();
        }
    }

    fn select_previous(&mut self) {
        if self.files.is_empty() {
            return;
        }
        if self.selected > 0 {
            self.selected -= 1;
            self.view_offset = if self.follow { self.max_scroll() } else { 0 };
            self.load_selected_file();
        }
    }

    fn scroll_up(&mut self, amount: usize) {
        self.view_offset = self.view_offset.saturating_sub(amount);
        self.follow = false;
    }

    fn scroll_down(&mut self, amount: usize) {
        let max = self.max_scroll();
        self.view_offset = (self.view_offset + amount).min(max);
        if self.view_offset < max {
            self.follow = false;
        }
    }

    fn scroll_to_start(&mut self) {
        self.view_offset = 0;
        self.follow = false;
    }

    fn scroll_to_end(&mut self) {
        self.view_offset = self.max_scroll();
    }

    fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            self.view_offset = self.max_scroll();
        }
    }

    fn page_step(&self) -> usize {
        self.view_height.max(1)
    }

    fn reload_current(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let follow = self.follow;
        self.load_selected_file();
        if follow {
            self.follow = true;
            self.view_offset = self.max_scroll();
        }
    }
}

fn collect_files(dir: &Path) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    if !dir.exists() {
        fs::create_dir_all(dir)
            .with_context(|| format!("failed to create log directory {}", dir.display()))?;
    }
    for entry in
        fs::read_dir(dir).with_context(|| format!("reading directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            let modified = metadata.modified().ok();
            let size = metadata.len();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            entries.push(FileEntry {
                path,
                name,
                modified,
                size,
            });
        }
    }
    entries.sort_by(|a, b| match (a.modified, b.modified) {
        (Some(ma), Some(mb)) => mb.cmp(&ma),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    Ok(entries)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("{}", VersionInfo::current().extended());
        return Ok(());
    }
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_app(&mut terminal, cli);
    cleanup_terminal(&mut terminal)?;
    if let Err(err) = result {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
    Ok(())
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen, Show)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, cli: Cli) -> Result<()> {
    let mut app = App::new(cli.dir, !cli.no_follow)?;
    let tick_rate = Duration::from_millis(cli.refresh.max(50));
    loop {
        terminal.draw(|frame| draw_ui(frame, &mut app))?;
        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_input(&mut app, key)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    // redraw with new geometry
                }
                _ => {}
            }
        } else if app.follow {
            app.reload_current();
        }
    }
    Ok(())
}

fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Char('r') | KeyCode::Char('R') => app.refresh_files()?,
        KeyCode::Char('f') | KeyCode::Char('F') => app.toggle_follow(),
        KeyCode::Char('j') | KeyCode::Down => app.select_next(),
        KeyCode::Char('k') | KeyCode::Up => app.select_previous(),
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::SHIFT) => app.scroll_to_start(),
        KeyCode::Char('g') if key.modifiers.is_empty() => app.scroll_to_start(),
        KeyCode::Char('G') | KeyCode::End => {
            app.scroll_to_end();
        }
        KeyCode::PageDown => app.scroll_down(app.page_step()),
        KeyCode::PageUp => app.scroll_up(app.page_step()),
        KeyCode::Right | KeyCode::Char('l') => app.scroll_down(1),
        KeyCode::Left | KeyCode::Char('h') => app.scroll_up(1),
        KeyCode::Char(' ') => app.scroll_down(app.page_step()),
        _ => {}
    };
    Ok(false)
}

fn draw_ui(frame: &mut Frame, app: &mut App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(frame.size());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(layout[0]);

    app.update_view_height(main[1].height.max(1));
    let mut state = ListState::default();
    if !app.files.is_empty() {
        state.select(Some(app.selected));
    }
    let items: Vec<ListItem> = if app.files.is_empty() {
        vec![ListItem::new(Line::from("(no log files)"))]
    } else {
        app.files
            .iter()
            .map(|entry| ListItem::new(entry.display_line()))
            .collect()
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Log Files"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, main[0], &mut state);

    let title = app
        .current_path()
        .map(|p| format!("{}", p.display()))
        .unwrap_or_else(|| "No file selected".to_owned());
    let scroll = app.view_offset.min(u16::MAX as usize) as u16;
    let text: Vec<Line> = if app.lines.is_empty() {
        vec![Line::from("(no content)")]
    } else {
        app.lines
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect()
    };
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(Color::Cyan))),
        )
        .scroll((scroll, 0));
    frame.render_widget(paragraph, main[1]);

    let help = Paragraph::new(
        "↑/↓ or j/k navigate files  ←/→ scroll  PgUp/PgDn page  f follow  r refresh  q quit",
    )
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(help, layout[1]);
}
