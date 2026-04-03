mod config;
mod provider;

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use provider::{ApiEvent, Provider};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::{TerminalOptions, Viewport};
use tokio::sync::mpsc;

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const MAX_HISTORY: usize = 500;

// ---------------------------------------------------------------------------
// Prompt history (persisted to ~/.commandok/history)
// ---------------------------------------------------------------------------

struct History {
    entries: Vec<String>,
    index: usize,
    draft: String,
    path: PathBuf,
}

impl History {
    fn load() -> Self {
        let path = history_path();
        let entries = fs::File::open(&path)
            .map(|f| {
                io::BufReader::new(f)
                    .lines()
                    .map_while(Result::ok)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let index = entries.len();
        Self {
            entries,
            index,
            draft: String::new(),
            path,
        }
    }

    fn push(&mut self, entry: &str) {
        let entry = entry.trim().to_string();
        if entry.is_empty() {
            return;
        }
        if self.entries.last().map(|s| s.as_str()) == Some(&entry) {
            self.reset_nav();
            return;
        }
        self.entries.push(entry);
        if self.entries.len() > MAX_HISTORY {
            self.entries.drain(..self.entries.len() - MAX_HISTORY);
        }
        self.reset_nav();
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = fs::File::create(&self.path) {
            for e in &self.entries {
                let _ = writeln!(f, "{e}");
            }
        }
    }

    fn prev(&mut self, current_input: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        if self.index == self.entries.len() {
            self.draft = current_input.to_string();
        }
        if self.index > 0 {
            self.index -= 1;
            Some(&self.entries[self.index])
        } else {
            Some(&self.entries[0])
        }
    }

    fn next(&mut self) -> Option<&str> {
        if self.index >= self.entries.len() {
            return None;
        }
        self.index += 1;
        if self.index == self.entries.len() {
            Some(&self.draft)
        } else {
            Some(&self.entries[self.index])
        }
    }

    fn reset_nav(&mut self) {
        self.index = self.entries.len();
        self.draft.clear();
    }
}

fn history_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".commandok").join("history")
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

enum Mode {
    Input,
    Loading,
    Streaming(String),
    Result(String),
    Error(String),
}

struct App {
    input: String,
    cursor: usize,
    mode: Mode,
    quit: bool,
    accepted: Option<String>,
    tick: usize,
    history: History,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            mode: Mode::Input,
            quit: false,
            accepted: None,
            tick: 0,
            history: History::load(),
        }
    }
}

// ---------------------------------------------------------------------------
// System prompt helpers
// ---------------------------------------------------------------------------

fn detect_shell() -> String {
    std::env::var("SHELL")
        .map(|s| s.rsplit('/').next().unwrap_or(&s).to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn detect_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "unknown"
    }
}

fn build_system_prompt(base: &str) -> String {
    format!("OS: {}\nShell: {}\n\n{}", detect_os(), detect_shell(), base)
}

// ---------------------------------------------------------------------------
// UI rendering
// ---------------------------------------------------------------------------

fn render(f: &mut Frame, app: &App, provider_label: &str) {
    let area = f.area();

    let (color, content, hint, show_cursor) = match &app.mode {
        Mode::Input => {
            let line = Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(&app.input),
            ]);
            (
                Color::Cyan,
                line,
                " ↵ submit · ⇧Tab switch · esc cancel ",
                true,
            )
        }
        Mode::Loading => {
            let frame = SPINNER[app.tick % SPINNER.len()];
            let line = Line::from(Span::styled(
                format!("{frame} Generating..."),
                Style::default().fg(Color::Yellow),
            ));
            (Color::Yellow, line, " esc cancel ", false)
        }
        Mode::Streaming(partial) => {
            let line = Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Green).bold()),
                Span::styled(partial.as_str(), Style::default().fg(Color::White)),
                Span::styled("▌", Style::default().fg(Color::Yellow)),
            ]);
            (Color::Green, line, " streaming... · esc cancel ", false)
        }
        Mode::Result(cmd) => {
            let line = Line::from(vec![
                Span::styled("$ ", Style::default().fg(Color::Green).bold()),
                Span::styled(cmd.as_str(), Style::default().fg(Color::White).bold()),
            ]);
            (Color::Green, line, " ↵ accept · esc cancel ", false)
        }
        Mode::Error(e) => {
            let max = (area.width as usize).saturating_sub(6);
            let msg = if e.len() > max {
                format!("✗ {}…", &e[..max])
            } else {
                format!("✗ {e}")
            };
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::Red)));
            (Color::Red, line, " ↵ retry · esc cancel ", false)
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Line::from(vec![
            Span::styled(" ⌘OK", Style::default().fg(color).bold()),
            Span::styled(" · ", Style::default().fg(Color::DarkGray)),
            Span::styled(provider_label, Style::default().fg(Color::DarkGray)),
        ]))
        .title_bottom(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        )));

    f.render_widget(Paragraph::new(content).block(block), area);

    if show_cursor {
        let x = area.x + 1 + 2 + app.cursor as u16;
        f.set_cursor_position((x, area.y + 1));
    }
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config::load().map_err(|e| e)?;

    let available = cfg.available_providers();
    if available.is_empty() {
        return Err("No provider sections found in config.".into());
    }

    let mut provider_idx: usize = available
        .iter()
        .position(|(n, _)| n == &cfg.commandok.provider)
        .unwrap_or(0);

    let mut provider = Provider::from_name(&available[provider_idx].0, &available[provider_idx].1);
    let mut provider_label = format!(
        "{}({}) ",
        available[provider_idx].0, available[provider_idx].1.model
    );
    let system_prompt = build_system_prompt(&cfg.commandok.system_prompt);

    enable_raw_mode()?;
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(io::stderr()),
        TerminalOptions {
            viewport: Viewport::Inline(3),
        },
    )?;

    let mut app = App::new();
    let (tx, mut rx) = mpsc::unbounded_channel::<ApiEvent>();

    loop {
        terminal.draw(|f| render(f, &app, &provider_label))?;

        if app.quit {
            break;
        }

        // Drain pending stream events.
        let mut got_event = false;
        while let Ok(ev) = rx.try_recv() {
            got_event = true;
            match ev {
                ApiEvent::Delta(text) => match &mut app.mode {
                    Mode::Loading => app.mode = Mode::Streaming(text),
                    Mode::Streaming(buf) => buf.push_str(&text),
                    _ => {}
                },
                ApiEvent::Done => {
                    if let Mode::Streaming(buf) = &app.mode {
                        app.mode = Mode::Result(buf.trim().to_string());
                    }
                }
                ApiEvent::Error(e) => app.mode = Mode::Error(e),
            }
        }
        if got_event {
            continue;
        }

        if event::poll(Duration::from_millis(80))? {
            let ev = event::read()?;

            if matches!(ev, Event::Resize(_, _)) {
                // Clear screen to flush reflow artifacts, then redraw.
                // User's content is preserved in scrollback.
                terminal.clear()?;
                continue;
            }

            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.quit = true;
                    continue;
                }

                match &app.mode {
                    Mode::Input => match key.code {
                        KeyCode::Esc => app.quit = true,
                        KeyCode::BackTab => {
                            // Shift+Tab cycles to next provider.
                            if available.len() > 1 {
                                provider_idx = (provider_idx + 1) % available.len();
                                let (ref name, ref cfg) = available[provider_idx];
                                provider = Provider::from_name(name, cfg);
                                provider_label = format!("{name}({}) ", cfg.model);
                                config::save_default_provider(name);
                            }
                        }
                        KeyCode::Enter if !app.input.trim().is_empty() => {
                            app.history.push(&app.input);
                            app.mode = Mode::Loading;
                            app.tick = 0;
                            let p = provider.clone();
                            let q = app.input.clone();
                            let sp = system_prompt.clone();
                            let t = tx.clone();
                            tokio::spawn(async move {
                                p.stream(&q, &sp, t).await;
                            });
                        }
                        KeyCode::Up => {
                            if let Some(prev) = app.history.prev(&app.input) {
                                app.input = prev.to_string();
                                app.cursor = app.input.len();
                            }
                        }
                        KeyCode::Down => {
                            if let Some(next) = app.history.next() {
                                app.input = next.to_string();
                                app.cursor = app.input.len();
                            }
                        }
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                if c == 'u' {
                                    app.input.clear();
                                    app.cursor = 0;
                                }
                            } else {
                                app.input.insert(app.cursor, c);
                                app.cursor += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if app.cursor > 0 {
                                app.cursor -= 1;
                                app.input.remove(app.cursor);
                            }
                        }
                        KeyCode::Delete if app.cursor < app.input.len() => {
                            app.input.remove(app.cursor);
                        }
                        KeyCode::Left => app.cursor = app.cursor.saturating_sub(1),
                        KeyCode::Right if app.cursor < app.input.len() => {
                            app.cursor += 1;
                        }
                        KeyCode::Home => app.cursor = 0,
                        KeyCode::End => app.cursor = app.input.len(),
                        _ => {}
                    },
                    Mode::Loading | Mode::Streaming(_) => {
                        if key.code == KeyCode::Esc {
                            app.quit = true;
                        }
                    }
                    Mode::Result(cmd) => match key.code {
                        KeyCode::Enter => {
                            app.accepted = Some(cmd.clone());
                            app.quit = true;
                        }
                        KeyCode::Esc => app.quit = true,
                        _ => {}
                    },
                    Mode::Error(_) => match key.code {
                        KeyCode::Esc => app.quit = true,
                        KeyCode::Enter => app.mode = Mode::Input,
                        _ => {}
                    },
                }
            }
        } else if matches!(app.mode, Mode::Loading | Mode::Streaming(_)) {
            app.tick += 1;
        }
    }

    app.history.save();

    disable_raw_mode()?;
    terminal.clear()?;

    if let Some(cmd) = &app.accepted {
        let cmd = cmd.trim_end();
        inject_to_terminal(cmd);
    }

    Ok(())
}

fn inject_to_terminal(cmd: &str) {
    use std::os::unix::io::AsRawFd;

    let fd = io::stdin().as_raw_fd();
    for byte in cmd.bytes() {
        let b = byte;
        unsafe {
            libc::ioctl(fd, libc::TIOCSTI, &b as *const u8);
        }
    }
}
