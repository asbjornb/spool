//! Browse command - Interactive TUI session browser.

mod ui;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use spool_adapters::{claude_code, SessionInfo};
use spool_format::{Entry, SecretDetector, ToolOutput};
use std::io;
use std::time::{Duration, Instant};

/// Cached preview data for the selected session.
struct PreviewData {
    /// Index into App::sessions that this preview corresponds to.
    session_index: usize,
    /// Converted entries.
    entries: Vec<Entry>,
}

/// Input mode.
#[derive(PartialEq)]
enum Mode {
    Normal,
    Search,
}

/// Application state.
struct App {
    sessions: Vec<SessionInfo>,
    /// Indices into `sessions` that match the current filter.
    filtered_indices: Vec<usize>,
    /// Index into `filtered_indices` for the current selection.
    selected: usize,
    preview: Option<PreviewData>,
    preview_scroll: usize,
    mode: Mode,
    search_input: String,
    agent_filter: Option<String>,
    status_message: Option<(String, Instant)>,
    should_quit: bool,
}

impl App {
    fn new(sessions: Vec<SessionInfo>, agent_filter: Option<String>) -> Self {
        let filtered_indices: Vec<usize> = sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                agent_filter
                    .as_ref()
                    .map(|f| s.agent.as_str() == f.as_str())
                    .unwrap_or(true)
            })
            .map(|(i, _)| i)
            .collect();

        App {
            sessions,
            filtered_indices,
            selected: 0,
            preview: None,
            preview_scroll: 0,
            mode: Mode::Normal,
            search_input: String::new(),
            agent_filter,
            status_message: None,
            should_quit: false,
        }
    }

    fn selected_session(&self) -> Option<&SessionInfo> {
        self.filtered_indices
            .get(self.selected)
            .map(|&i| &self.sessions[i])
    }

    fn selected_session_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).copied()
    }

    fn move_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.selected < self.filtered_indices.len() - 1 {
            self.selected += 1;
            self.preview_scroll = 0;
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.preview_scroll = 0;
        }
    }

    fn jump_top(&mut self) {
        self.selected = 0;
        self.preview_scroll = 0;
    }

    fn jump_bottom(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = self.filtered_indices.len() - 1;
            self.preview_scroll = 0;
        }
    }

    fn update_filter(&mut self) {
        let search = self.search_input.to_lowercase();
        self.filtered_indices = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                // Apply agent filter
                let agent_ok = self
                    .agent_filter
                    .as_ref()
                    .map(|f| s.agent.as_str() == f.as_str())
                    .unwrap_or(true);
                // Apply search filter
                let search_ok = search.is_empty()
                    || s.title
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&search))
                        .unwrap_or(false)
                    || s.project_dir
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_lowercase().contains(&search))
                        .unwrap_or(false);
                agent_ok && search_ok
            })
            .map(|(i, _)| i)
            .collect();

        // Keep selection in bounds
        if self.filtered_indices.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len() - 1;
        }
        self.preview_scroll = 0;
    }

    fn load_preview(&mut self) {
        let Some(idx) = self.selected_session_index() else {
            self.preview = None;
            return;
        };

        // Return early if already cached for this session
        if let Some(ref p) = self.preview {
            if p.session_index == idx {
                return;
            }
        }

        let session = &self.sessions[idx];
        match claude_code::convert(session) {
            Ok(spool_file) => {
                self.preview = Some(PreviewData {
                    session_index: idx,
                    entries: spool_file.entries,
                });
            }
            Err(_) => {
                self.preview = None;
            }
        }
    }

    fn export_selected(&mut self, redact: bool) {
        let Some(session) = self.selected_session().cloned() else {
            return;
        };

        let mut spool_file = match claude_code::convert(&session) {
            Ok(f) => f,
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
                return;
            }
        };

        if redact {
            let detector = SecretDetector::with_defaults();
            let mut count = 0;
            for entry in &mut spool_file.entries {
                match entry {
                    Entry::Prompt(p) => {
                        let (redacted, secrets) = detector.redact(&p.content);
                        count += secrets.len();
                        p.content = redacted;
                    }
                    Entry::Response(r) => {
                        let (redacted, secrets) = detector.redact(&r.content);
                        count += secrets.len();
                        r.content = redacted;
                    }
                    Entry::ToolResult(tr) => {
                        if let Some(ToolOutput::Text(ref mut text)) = tr.output {
                            let (redacted, secrets) = detector.redact(text);
                            count += secrets.len();
                            *text = redacted;
                        }
                    }
                    Entry::Thinking(t) => {
                        let (redacted, secrets) = detector.redact(&t.content);
                        count += secrets.len();
                        t.content = redacted;
                    }
                    _ => {}
                }
            }
            let _ = count; // used implicitly via status message below
        }

        let stem = session
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let filename = if redact {
            format!("{}.redacted.spool", stem)
        } else {
            format!("{}.spool", stem)
        };

        match spool_file.write_to_path(&filename) {
            Ok(()) => {
                self.set_status(format!("Exported to {}", filename));
            }
            Err(e) => {
                self.set_status(format!("Export failed: {}", e));
            }
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Clear stale status messages (older than 3 seconds).
    fn tick_status(&mut self) {
        if let Some((_, ref t)) = self.status_message {
            if t.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }
}

pub fn run(agent_filter: Option<String>) -> Result<()> {
    let sessions: Vec<SessionInfo> = claude_code::find_sessions()
        .context("Failed to discover sessions")?
        .into_iter()
        .filter(|s| {
            // Filter out sessions known to be empty (message_count == 0).
            // Sessions without index data (message_count == None) are kept.
            s.message_count.map(|c| c > 0).unwrap_or(true)
        })
        .collect();

    let mut app = App::new(sessions, agent_filter);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Install panic hook so we restore the terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Main loop
    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // Load preview for current selection
        app.load_preview();

        terminal.draw(|f| ui::draw(f, app))?;

        // Poll with timeout so status messages can expire
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release/repeat)
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                        KeyCode::Char('g') | KeyCode::Home => app.jump_top(),
                        KeyCode::Char('G') | KeyCode::End => app.jump_bottom(),
                        KeyCode::Char('/') => {
                            app.mode = Mode::Search;
                        }
                        KeyCode::Char('e') => app.export_selected(false),
                        KeyCode::Char('r') => app.export_selected(true),
                        KeyCode::Enter => {
                            if let Some(session) = app.selected_session() {
                                let path = session.path.display().to_string();
                                app.should_quit = true;
                                // We'll print the view hint after restoring terminal
                                app.set_status(path);
                            }
                        }
                        // Preview scrolling
                        KeyCode::PageDown | KeyCode::Char('l') => {
                            app.preview_scroll = app.preview_scroll.saturating_add(10);
                        }
                        KeyCode::PageUp | KeyCode::Char('h') => {
                            app.preview_scroll = app.preview_scroll.saturating_sub(10);
                        }
                        _ => {}
                    },
                    Mode::Search => match key.code {
                        KeyCode::Esc => {
                            app.mode = Mode::Normal;
                            app.search_input.clear();
                            app.update_filter();
                        }
                        KeyCode::Enter => {
                            app.mode = Mode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.search_input.pop();
                            app.update_filter();
                        }
                        KeyCode::Char(c) => {
                            app.search_input.push(c);
                            app.update_filter();
                        }
                        _ => {}
                    },
                }
            }
        }

        app.tick_status();

        if app.should_quit {
            // If Enter was pressed, print view hint
            if let Some((ref path, _)) = app.status_message {
                if !path.starts_with("Exported") && !path.starts_with("Export failed") {
                    let path = path.clone();
                    // Clear status so it doesn't interfere with the message
                    app.status_message = None;
                    // We return Ok and the caller prints after restoring terminal
                    // Store the path in status for the caller
                    app.status_message = Some((format!("VIEW:{}", path), Instant::now()));
                }
            }
            break;
        }
    }

    // After restoring terminal (handled by caller), print view command if needed
    if let Some((ref msg, _)) = app.status_message {
        if let Some(path) = msg.strip_prefix("VIEW:") {
            println!("\nTo view this session:\n  spool view {}", path);
        }
    }

    Ok(())
}
