//! Play command - Replay a session with TUI playback controls.
//!
//! Steps through entries respecting timestamps, with idle gap compression
//! (gaps before prompts capped at 2s) and thinking compression (thinking
//! blocks shown for at most 2s regardless of original duration).

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use spool_format::{Entry, SpoolFile, ToolOutput};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::agent::load_spool_or_log;
/// Maximum gap (ms) before a Prompt entry (user think-time).
const MAX_IDLE_GAP_MS: u64 = 2_000;

/// Maximum display time for Thinking entries.
const MAX_THINKING_MS: u64 = 2_000;

/// Available speed multipliers.
const SPEEDS: &[f32] = &[0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// A pre-computed playback timeline entry.
struct TimelineEntry {
    /// Index into the original entries vec.
    entry_index: usize,
    /// Compressed playback time (ms) at which this entry appears.
    playback_ms: u64,
}

/// Application state for the player.
struct PlayApp {
    /// Original source path (log or .spool)
    source_path: PathBuf,
    /// Loaded Spool file (for export/trim).
    spool_file: SpoolFile,
    /// Session title for display.
    session_title: String,
    /// Pre-computed playback timeline with compressed timestamps.
    timeline: Vec<TimelineEntry>,
    /// Total compressed duration.
    total_duration_ms: u64,

    // Playback state
    /// How many timeline entries are currently visible (0 = none shown yet).
    visible_count: usize,
    /// Whether playback is running.
    playing: bool,
    /// Index into SPEEDS.
    speed_index: usize,
    /// Accumulated playback time in ms.
    playback_elapsed_ms: u64,
    /// When we last ticked.
    last_tick: Instant,

    // Display
    /// Scroll offset in the rendered entry view.
    scroll_offset: usize,

    // Trimming state
    /// Marked trim start timestamp (ms, original timeline).
    trim_start_ms: Option<u64>,
    /// Marked trim end timestamp (ms, original timeline).
    trim_end_ms: Option<u64>,
    /// Status message for UI feedback.
    status_message: Option<String>,

    should_quit: bool,
}

impl PlayApp {
    fn new(spool_file: SpoolFile, source_path: PathBuf, speed: f32) -> Self {
        let session_title = spool_file
            .session
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());

        // Build compressed timeline
        let timeline = build_timeline(&spool_file.entries);
        let total_duration_ms = timeline.last().map(|t| t.playback_ms).unwrap_or(0);

        // Find the closest speed index
        let speed_index = SPEEDS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((**a) - speed)
                    .abs()
                    .partial_cmp(&((**b) - speed).abs())
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(2); // default 1.0x

        PlayApp {
            source_path,
            spool_file,
            session_title,
            timeline,
            total_duration_ms,
            visible_count: 0,
            playing: false,
            speed_index,
            playback_elapsed_ms: 0,
            last_tick: Instant::now(),
            scroll_offset: 0,
            trim_start_ms: None,
            trim_end_ms: None,
            status_message: None,
            should_quit: false,
        }
    }

    fn speed(&self) -> f32 {
        SPEEDS[self.speed_index]
    }

    fn speed_up(&mut self) {
        if self.speed_index < SPEEDS.len() - 1 {
            self.speed_index += 1;
        }
    }

    fn speed_down(&mut self) {
        if self.speed_index > 0 {
            self.speed_index -= 1;
        }
    }

    fn toggle_play(&mut self) {
        self.playing = !self.playing;
        if self.playing {
            self.last_tick = Instant::now();
            // If at the end, restart
            if self.visible_count >= self.timeline.len() {
                self.visible_count = 0;
                self.playback_elapsed_ms = 0;
                self.scroll_offset = 0;
            }
        }
    }

    fn step_forward(&mut self) {
        self.playing = false;
        if self.visible_count < self.timeline.len() {
            self.visible_count += 1;
            if let Some(te) = self.timeline.get(self.visible_count.saturating_sub(1)) {
                self.playback_elapsed_ms = te.playback_ms;
            }
            self.auto_scroll();
        }
    }

    fn step_backward(&mut self) {
        self.playing = false;
        if self.visible_count > 0 {
            self.visible_count -= 1;
            if self.visible_count > 0 {
                if let Some(te) = self.timeline.get(self.visible_count - 1) {
                    self.playback_elapsed_ms = te.playback_ms;
                }
            } else {
                self.playback_elapsed_ms = 0;
            }
            // Reset scroll when going backward
            self.scroll_offset = 0;
        }
    }

    fn jump_to_start(&mut self) {
        self.playing = false;
        self.visible_count = 0;
        self.playback_elapsed_ms = 0;
        self.scroll_offset = 0;
    }

    fn jump_to_end(&mut self) {
        self.playing = false;
        self.visible_count = self.timeline.len();
        self.playback_elapsed_ms = self.total_duration_ms;
        self.auto_scroll();
    }

    fn current_entry_timestamp(&self) -> Option<u64> {
        if self.visible_count == 0 {
            return None;
        }
        for idx in (0..self.visible_count).rev() {
            let te = self.timeline.get(idx)?;
            if let Some(ts) = self.spool_file.entries.get(te.entry_index)?.timestamp() {
                return Some(ts);
            }
        }
        None
    }

    fn mark_trim_start(&mut self) {
        match self.current_entry_timestamp() {
            Some(ts) => {
                self.trim_start_ms = Some(ts);
                self.status_message = Some(format!("Trim start set to {}", format_duration_ms(ts)));
            }
            None => {
                self.status_message = Some("Cannot mark trim start yet".to_string());
            }
        }
    }

    fn mark_trim_end(&mut self) {
        match self.current_entry_timestamp() {
            Some(ts) => {
                self.trim_end_ms = Some(ts);
                self.status_message = Some(format!("Trim end set to {}", format_duration_ms(ts)));
            }
            None => {
                self.status_message = Some("Cannot mark trim end yet".to_string());
            }
        }
    }

    fn trim_range(&self) -> Option<(u64, u64)> {
        match (self.trim_start_ms, self.trim_end_ms) {
            (Some(start), Some(end)) if start < end => Some((start, end)),
            _ => None,
        }
    }

    fn trim_preview(&self, start: u64, end: u64) -> (usize, u64) {
        let mut kept = 1; // session entry always kept
        for entry in self.spool_file.entries.iter().skip(1) {
            if let Some(ts) = entry.timestamp() {
                if ts >= start && ts <= end {
                    kept += 1;
                }
            }
        }
        (kept, end.saturating_sub(start))
    }

    fn export_trimmed(&mut self) {
        let (start, end) = match self.trim_range() {
            Some(range) => range,
            None => {
                self.status_message = Some("Trim range not set".to_string());
                return;
            }
        };

        let mut trimmed = self.spool_file.clone();
        trimmed.trim(start, end);

        let output_path = next_trimmed_path(&self.source_path);
        match trimmed.write_to_path(&output_path) {
            Ok(()) => {
                let name = output_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("trimmed.spool");
                self.status_message = Some(format!("Exported {}", name));
            }
            Err(err) => {
                self.status_message = Some(format!("Export failed: {}", err));
            }
        }
    }

    fn tick(&mut self) {
        if !self.playing {
            return;
        }

        let now = Instant::now();
        let real_elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;

        let advance_ms = (real_elapsed.as_millis() as f64 * self.speed() as f64) as u64;
        self.playback_elapsed_ms = self.playback_elapsed_ms.saturating_add(advance_ms);

        // Reveal entries whose playback_ms <= playback_elapsed_ms
        while self.visible_count < self.timeline.len() {
            let te = &self.timeline[self.visible_count];
            if te.playback_ms <= self.playback_elapsed_ms {
                self.visible_count += 1;
                self.auto_scroll();
            } else {
                break;
            }
        }

        // Stop at end
        if self.visible_count >= self.timeline.len() {
            self.playing = false;
            self.playback_elapsed_ms = self.total_duration_ms;
        }
    }

    /// Ensure the scroll is positioned to show the latest entry.
    fn auto_scroll(&mut self) {
        // We'll let the draw function handle this based on content height.
        // For now, set a high scroll offset that the renderer will clamp.
        self.scroll_offset = usize::MAX;
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    fn progress_ratio(&self) -> f64 {
        if self.total_duration_ms == 0 {
            if self.visible_count > 0 {
                1.0
            } else {
                0.0
            }
        } else {
            (self.playback_elapsed_ms as f64 / self.total_duration_ms as f64).min(1.0)
        }
    }

    fn progress_label(&self) -> String {
        let current = format_duration_ms(self.playback_elapsed_ms);
        let total = format_duration_ms(self.total_duration_ms);
        format!(
            "{} / {}  [{}/{}]",
            current,
            total,
            self.visible_count,
            self.timeline.len()
        )
    }
}

/// Build a compressed timeline from entries.
///
/// Applies two compressions:
/// 1. Idle gap compression: gaps before Prompt entries are capped at MAX_IDLE_GAP_MS
/// 2. Thinking compression: gaps after Thinking entries are capped at MAX_THINKING_MS
fn build_timeline(entries: &[Entry]) -> Vec<TimelineEntry> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut timeline = Vec::with_capacity(entries.len());
    let mut compressed_time: u64 = 0;
    let mut prev_original_ts: u64 = 0;

    for (i, entry) in entries.iter().enumerate() {
        let original_ts = entry.timestamp().unwrap_or(0);
        let raw_gap = original_ts.saturating_sub(prev_original_ts);

        let compressed_gap = if i == 0 {
            0
        } else {
            let mut gap = raw_gap;

            // Idle gap compression: cap gaps before Prompt entries
            if matches!(entry, Entry::Prompt(_)) && gap > MAX_IDLE_GAP_MS {
                gap = MAX_IDLE_GAP_MS;
            }

            // Thinking compression: cap gaps after Thinking entries
            if i > 0 {
                if let Some(prev_entry) = entries.get(i - 1) {
                    if matches!(prev_entry, Entry::Thinking(_)) && gap > MAX_THINKING_MS {
                        gap = MAX_THINKING_MS;
                    }
                }
            }

            gap
        };

        compressed_time += compressed_gap;

        timeline.push(TimelineEntry {
            entry_index: i,
            playback_ms: compressed_time,
        });

        prev_original_ts = original_ts;
    }

    timeline
}

fn format_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{}:{:02}", minutes, seconds)
}

pub fn run(path: &Path, speed: f32) -> Result<()> {
    // Load session
    let spool_file = load_spool_or_log(path)?;

    if spool_file.entries.is_empty() {
        println!("Session has no entries.");
        return Ok(());
    }

    let mut app = PlayApp::new(spool_file, path.to_path_buf(), speed);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::prelude::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Start playing immediately
    app.playing = true;
    app.last_tick = Instant::now();

    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut ratatui::Terminal<ratatui::prelude::CrosstermBackend<io::Stdout>>,
    app: &mut PlayApp,
) -> Result<()> {
    loop {
        app.tick();

        terminal.draw(|f| draw(f, app))?;

        // Poll with short timeout for smooth playback
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
                    }
                    KeyCode::Char(' ') => app.toggle_play(),
                    KeyCode::Right | KeyCode::Char('l') => app.step_forward(),
                    KeyCode::Left | KeyCode::Char('h') => app.step_backward(),
                    KeyCode::Char('+') | KeyCode::Char('=') => app.speed_up(),
                    KeyCode::Char('-') | KeyCode::Char('_') => app.speed_down(),
                    KeyCode::Home | KeyCode::Char('g') => app.jump_to_start(),
                    KeyCode::End | KeyCode::Char('G') => app.jump_to_end(),
                    KeyCode::PageUp | KeyCode::Char('k') => app.scroll_up(10),
                    KeyCode::PageDown | KeyCode::Char('j') => app.scroll_down(10),
                    KeyCode::Char('[') => app.mark_trim_start(),
                    KeyCode::Char(']') => app.mark_trim_end(),
                    KeyCode::Char('x') => app.export_trimmed(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Main draw function.
fn draw(f: &mut Frame, app: &mut PlayApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(1),    // Entry content
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Controls
        ])
        .split(f.area());

    draw_title_bar(f, chunks[0], app);
    draw_entries(f, chunks[1], app);
    draw_progress_bar(f, chunks[2], app);
    draw_controls(f, chunks[3], app);
}

fn draw_title_bar(f: &mut Frame, area: Rect, app: &PlayApp) {
    let status_icon = if app.playing { ">" } else { "||" };
    let speed_label = format!("{}x", app.speed());

    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", status_icon),
            Style::default()
                .fg(if app.playing {
                    Color::Green
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            app.session_title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(speed_label, Style::default().fg(Color::Cyan)),
    ]);

    f.render_widget(Paragraph::new(title), area);
}

fn draw_entries(f: &mut Frame, area: Rect, app: &mut PlayApp) {
    let block = Block::default().borders(Borders::NONE);
    let inner = block.inner(area);

    if app.visible_count == 0 {
        let msg = if app.playing {
            "Starting playback..."
        } else {
            "Press Space to start playback"
        };
        let paragraph = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(paragraph, area);
        return;
    }

    // Build lines for all visible entries
    let mut lines: Vec<Line> = Vec::new();

    for ti in 0..app.visible_count {
        let te = &app.timeline[ti];
        let entry = &app.spool_file.entries[te.entry_index];

        render_entry_lines(entry, &mut lines, inner.width as usize);
    }

    // Clamp scroll offset
    let total_lines = lines.len();
    let view_height = inner.height as usize;
    let max_scroll = total_lines.saturating_sub(view_height);

    if app.scroll_offset > max_scroll {
        app.scroll_offset = max_scroll;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset as u16, 0));

    f.render_widget(paragraph, area);
}

fn draw_progress_bar(f: &mut Frame, area: Rect, app: &PlayApp) {
    let ratio = app.progress_ratio();
    let label = app.progress_label();

    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(ratio)
        .label(label);

    f.render_widget(gauge, area);
}

fn draw_controls(f: &mut Frame, area: Rect, app: &PlayApp) {
    let play_key = if app.playing {
        "Space:pause"
    } else {
        "Space:play"
    };
    let trim_label = match (app.trim_start_ms, app.trim_end_ms) {
        (None, None) => "Trim: [unset]".to_string(),
        (Some(start), None) => format!("Trim start: {}", format_duration_ms(start)),
        (None, Some(end)) => format!("Trim end: {}", format_duration_ms(end)),
        (Some(start), Some(end)) => {
            if start < end {
                let (kept, duration) = app.trim_preview(start, end);
                format!(
                    "Trim: {}-{} ({} entries, {})",
                    format_duration_ms(start),
                    format_duration_ms(end),
                    kept,
                    format_duration_ms(duration)
                )
            } else {
                format!(
                    "Trim: {}-{} (invalid)",
                    format_duration_ms(start),
                    format_duration_ms(end)
                )
            }
        }
    };

    let mut text = format!(
        " {}  h/l:step  +/-:speed  j/k:scroll  g/G:start/end  [:start  ]:end  x:export  q:quit  {}",
        play_key, trim_label
    );
    if let Some(ref status) = app.status_message {
        text.push_str("  |  ");
        text.push_str(status);
    }

    let paragraph =
        Paragraph::new(text).style(Style::default().fg(Color::DarkGray).bg(Color::Black));
    f.render_widget(paragraph, area);
}

/// Render a single entry into styled lines.
fn render_entry_lines(entry: &Entry, lines: &mut Vec<Line>, width: usize) {
    match entry {
        Entry::Session(s) => {
            lines.push(Line::from(Span::styled(
                format!("SESSION: {}", s.title.as_deref().unwrap_or("Untitled")),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            let mut info_parts = vec![format!("Agent: {}", s.agent)];
            if let Some(ref ver) = s.agent_version {
                info_parts[0] = format!("Agent: {} v{}", s.agent, ver);
            }
            info_parts.push(format!(
                "Recorded: {}",
                s.recorded_at.format("%Y-%m-%d %H:%M")
            ));
            if let Some(dur) = s.duration_ms {
                info_parts.push(format!("Duration: {}", format_duration_ms(dur)));
            }
            if let Some(count) = s.entry_count {
                info_parts.push(format!("Entries: {}", count));
            }
            for part in info_parts {
                lines.push(Line::from(Span::styled(
                    format!("  {}", part),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
        }
        Entry::Prompt(p) => {
            lines.push(Line::from(Span::styled(
                "USER",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in p.content.lines() {
                let truncated = truncate_str(line, width.saturating_sub(2));
                lines.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(Color::Green),
                )));
            }
            lines.push(Line::from(""));
        }
        Entry::Thinking(t) => {
            let collapsed = t.content.replace('\n', " ");
            let preview = truncate_str(&collapsed, 80);
            lines.push(Line::from(vec![
                Span::styled(
                    "THINKING ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    preview.to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]));
        }
        Entry::ToolCall(tc) => {
            let tool_display = if tc.tool == "Task" {
                if let Some(subagent_type) = tc.input.get("subagent_type").and_then(|v| v.as_str())
                {
                    format!("Task ({})", subagent_type)
                } else {
                    tc.tool.clone()
                }
            } else {
                tc.tool.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    "TOOL ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(tool_display, Style::default().fg(Color::Blue)),
            ]));

            let input_preview = format_tool_input(&tc.input, width.saturating_sub(4));
            lines.push(Line::from(Span::styled(
                format!("  {}", input_preview),
                Style::default().fg(Color::DarkGray),
            )));
        }
        Entry::ToolResult(tr) => {
            let status = if tr.error.is_some() {
                Span::styled("[ERROR]", Style::default().fg(Color::Red))
            } else {
                Span::styled("[OK]", Style::default().fg(Color::Green))
            };
            lines.push(Line::from(vec![
                Span::styled("  RESULT ", Style::default().fg(Color::Blue)),
                status,
            ]));

            if let Some(ref err) = tr.error {
                let truncated = truncate_str(err, width.saturating_sub(4));
                lines.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(Color::Red),
                )));
            } else if let Some(ref output) = tr.output {
                let text = match output {
                    ToolOutput::Text(t) => t.as_str(),
                    ToolOutput::Binary(_) => "<binary>",
                };
                for line in text.lines().take(5) {
                    let truncated = truncate_str(line, width.saturating_sub(4));
                    lines.push(Line::from(Span::styled(
                        format!("  {}", truncated),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                let line_count = text.lines().count();
                if line_count > 5 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... ({} more lines)", line_count - 5),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            lines.push(Line::from(""));
        }
        Entry::Response(r) => {
            lines.push(Line::from(Span::styled(
                "ASSISTANT",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in r.content.lines() {
                let truncated = truncate_str(line, width.saturating_sub(2));
                lines.push(Line::from(format!("  {}", truncated)));
            }
            if let Some(ref model) = r.model {
                lines.push(Line::from(Span::styled(
                    format!("  [{}]", model),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
        }
        Entry::Error(e) => {
            lines.push(Line::from(Span::styled(
                format!("ERROR [{}]", e.code),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            let truncated = truncate_str(&e.message, width.saturating_sub(2));
            lines.push(Line::from(Span::styled(
                format!("  {}", truncated),
                Style::default().fg(Color::Red),
            )));
            lines.push(Line::from(""));
        }
        Entry::SubagentStart(s) => {
            lines.push(Line::from(Span::styled(
                format!("SUBAGENT START: {}", s.agent),
                Style::default().fg(Color::Cyan),
            )));
            if let Some(ref ctx) = s.context {
                let truncated = truncate_str(ctx, width.saturating_sub(2));
                lines.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        Entry::SubagentEnd(e) => {
            let status = e
                .status
                .as_ref()
                .map(|s| format!(" ({:?})", s))
                .unwrap_or_default();
            lines.push(Line::from(Span::styled(
                format!("SUBAGENT END{}", status),
                Style::default().fg(Color::Cyan),
            )));
            if let Some(ref summary) = e.summary {
                let truncated = truncate_str(summary, width.saturating_sub(2));
                lines.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
        }
        Entry::Annotation(a) => {
            lines.push(Line::from(Span::styled(
                format!("NOTE @{}", a.author.as_deref().unwrap_or("anonymous")),
                Style::default().fg(Color::Yellow),
            )));
            let truncated = truncate_str(&a.content, width.saturating_sub(2));
            lines.push(Line::from(format!("  {}", truncated)));
            lines.push(Line::from(""));
        }
        Entry::RedactionMarker(_) => {
            lines.push(Line::from(Span::styled(
                "[REDACTED]",
                Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
            )));
        }
        Entry::Unknown => {}
    }
}

/// Truncate a string to fit within `max_len` bytes, respecting char boundaries.
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    if max_len == 0 {
        return "";
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Format tool input JSON for preview display.
fn format_tool_input(input: &serde_json::Value, max_len: usize) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (key, val) in map {
                let val_str = match val {
                    serde_json::Value::String(s) => {
                        let t = truncate_str(s, 40);
                        t.to_string()
                    }
                    other => {
                        let s = other.to_string();
                        truncate_str(&s, 40).to_string()
                    }
                };
                parts.push(format!("{}: {}", key, val_str));
            }
            let joined = parts.join(", ");
            truncate_str(&joined, max_len).to_string()
        }
        other => truncate_str(&other.to_string(), max_len).to_string(),
    }
}

fn next_trimmed_path(source: &Path) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let base = parent.join(format!("{}.trimmed.spool", stem));
    if !base.exists() {
        return base;
    }

    for index in 1..=999 {
        let candidate = parent.join(format!("{}.trimmed-{}.spool", stem, index));
        if !candidate.exists() {
            return candidate;
        }
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use spool_format::{PromptEntry, ResponseEntry, SessionEntry, ThinkingEntry};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_session_entry() -> Entry {
        Entry::Session(SessionEntry {
            id: Uuid::nil(),
            ts: 0,
            version: "1.0".to_string(),
            agent: "test".to_string(),
            recorded_at: chrono::Utc::now(),
            agent_version: None,
            title: Some("Test".to_string()),
            author: None,
            tags: None,
            duration_ms: Some(60_000),
            entry_count: Some(5),
            tools_used: None,
            files_modified: None,
            first_prompt: None,
            schema_url: None,
            trimmed: None,
            ended: None,
            extra: HashMap::new(),
        })
    }

    fn make_prompt(ts: u64, content: &str) -> Entry {
        Entry::Prompt(PromptEntry {
            id: Uuid::new_v4(),
            ts,
            content: content.to_string(),
            subagent_id: None,
            attachments: None,
            extra: HashMap::new(),
        })
    }

    fn make_thinking(ts: u64, content: &str) -> Entry {
        Entry::Thinking(ThinkingEntry {
            id: Uuid::new_v4(),
            ts,
            content: content.to_string(),
            collapsed: None,
            truncated: None,
            original_bytes: None,
            subagent_id: None,
            extra: HashMap::new(),
        })
    }

    fn make_response(ts: u64, content: &str) -> Entry {
        Entry::Response(ResponseEntry {
            id: Uuid::new_v4(),
            ts,
            content: content.to_string(),
            truncated: None,
            original_bytes: None,
            model: None,
            token_usage: None,
            subagent_id: None,
            extra: HashMap::new(),
        })
    }

    #[test]
    fn test_build_timeline_empty() {
        let timeline = build_timeline(&[]);
        assert!(timeline.is_empty());
    }

    #[test]
    fn test_build_timeline_no_compression() {
        let entries = vec![
            make_session_entry(),
            make_prompt(0, "hello"),
            make_response(1000, "hi there"),
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].playback_ms, 0);
        assert_eq!(timeline[1].playback_ms, 0);
        assert_eq!(timeline[2].playback_ms, 1000);
    }

    #[test]
    fn test_idle_gap_compression() {
        // 30-second gap before a prompt should compress to 2s
        let entries = vec![
            make_session_entry(),
            make_response(1000, "first response"),
            make_prompt(31_000, "second prompt"), // 30s gap
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[0].playback_ms, 0);
        assert_eq!(timeline[1].playback_ms, 1000);
        // Gap from 1000 to 31000 = 30000ms, but before a Prompt, capped to 2000
        assert_eq!(timeline[2].playback_ms, 3000); // 1000 + 2000
    }

    #[test]
    fn test_thinking_compression() {
        // Long gap after thinking should compress
        let entries = vec![
            make_session_entry(),
            make_thinking(1000, "thinking..."),
            make_response(61_000, "done"), // 60s gap after thinking
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[0].playback_ms, 0);
        assert_eq!(timeline[1].playback_ms, 1000);
        // Gap from 1000 to 61000 = 60000ms, but after Thinking, capped to 2000
        assert_eq!(timeline[2].playback_ms, 3000); // 1000 + 2000
    }

    #[test]
    fn test_small_gaps_not_compressed() {
        let entries = vec![
            make_session_entry(),
            make_response(500, "response"),
            make_prompt(1000, "prompt"), // 500ms gap - under threshold
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[2].playback_ms, 1000); // No compression
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0), "0:00");
        assert_eq!(format_duration_ms(1000), "0:01");
        assert_eq!(format_duration_ms(60_000), "1:00");
        assert_eq!(format_duration_ms(90_000), "1:30");
        assert_eq!(format_duration_ms(3_661_000), "61:01");
    }

    #[test]
    fn test_trim_preview_counts_entries() {
        let session = match make_session_entry() {
            Entry::Session(s) => s,
            _ => unreachable!(),
        };
        let mut file = SpoolFile::new(session);
        file.add_entry(make_prompt(1000, "hello"));
        file.add_entry(make_response(2000, "ok"));
        file.add_entry(make_prompt(3000, "later"));

        let app = PlayApp::new(file, PathBuf::from("session.spool"), 1.0);
        let (kept, duration) = app.trim_preview(1500, 2500);
        assert_eq!(kept, 2);
        assert_eq!(duration, 1000);
    }

    #[test]
    fn test_current_entry_timestamp_skips_unknown() {
        let session = match make_session_entry() {
            Entry::Session(s) => s,
            _ => unreachable!(),
        };
        let mut file = SpoolFile::new(session);
        file.add_entry(make_prompt(1000, "hello"));
        file.add_entry(Entry::Unknown);

        let mut app = PlayApp::new(file, PathBuf::from("session.spool"), 1.0);
        app.visible_count = 3;

        assert_eq!(app.current_entry_timestamp(), Some(1000));
    }

    #[test]
    fn test_next_trimmed_path_increments() {
        let dir = std::env::temp_dir().join(format!("spool-play-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("session.spool");
        let first = next_trimmed_path(&source);
        assert_eq!(first, dir.join("session.trimmed.spool"));

        fs::write(&first, "x").unwrap();
        let second = next_trimmed_path(&source);
        assert_eq!(second, dir.join("session.trimmed-1.spool"));
    }
}
