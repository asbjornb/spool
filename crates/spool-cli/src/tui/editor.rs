//! Editor view - Session replay with playback, trimming, annotations, and info overlay.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};
use spool_format::{AnnotationEntry, AnnotationStyle, Entry, SpoolFile};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use uuid::Uuid;

use super::common::{
    annotation_style_from_key, annotation_style_label, centered_rect, format_duration_ms,
    render_entry_lines, render_info_lines, truncate_str,
};

/// Maximum gap (ms) before a Prompt entry (user think-time).
const MAX_IDLE_GAP_MS: u64 = 2_000;

/// Maximum display time for Thinking entries.
const MAX_THINKING_MS: u64 = 2_000;

/// Available speed multipliers.
const SPEEDS: &[f32] = &[0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// Action returned by the Editor view to the top-level app loop.
pub enum EditorAction {
    /// Go back to the Library view.
    Back,
    /// Quit the application.
    Quit,
    /// No action; continue rendering.
    None,
}

/// A pre-computed playback timeline entry.
struct TimelineEntry {
    /// Index into the original entries vec.
    entry_index: usize,
    /// Compressed playback time (ms) at which this entry appears.
    playback_ms: u64,
}

#[derive(Clone, Copy)]
enum AnnotationStage {
    Text,
    Style,
}

struct AnnotationDraft {
    target_index: usize,
    target_id: spool_format::EntryId,
    target_ts: u64,
    buffer: String,
    style: Option<AnnotationStyle>,
    stage: AnnotationStage,
}

/// Editor view state (session replay/editing).
pub struct EditorState {
    /// Original source path (log or .spool).
    source_path: PathBuf,
    /// Loaded Spool file.
    spool_file: SpoolFile,
    /// Session title for display.
    session_title: String,
    /// Pre-computed playback timeline with compressed timestamps.
    timeline: Vec<TimelineEntry>,
    /// Total compressed duration.
    total_duration_ms: u64,

    // Playback state
    visible_count: usize,
    playing: bool,
    speed_index: usize,
    playback_elapsed_ms: u64,
    last_tick: Instant,

    // Display
    scroll_offset: usize,

    // Trimming state
    trim_start_ms: Option<u64>,
    trim_end_ms: Option<u64>,
    status_message: Option<String>,

    // Annotation state
    annotation_draft: Option<AnnotationDraft>,

    // Info overlay
    show_info: bool,

    /// Whether we came from a Library (true) or were opened directly (false).
    pub has_library: bool,
}

impl EditorState {
    pub fn new(spool_file: SpoolFile, source_path: PathBuf, speed: f32) -> Self {
        let session_title = spool_file
            .session
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());

        let timeline = build_timeline(&spool_file.entries);
        let total_duration_ms = timeline.last().map(|t| t.playback_ms).unwrap_or(0);

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
            .unwrap_or(2);

        EditorState {
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
            annotation_draft: None,
            show_info: false,
            has_library: false,
        }
    }

    /// Start playback immediately.
    pub fn start_playing(&mut self) {
        self.playing = true;
        self.last_tick = Instant::now();
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

    fn current_entry_info(&self) -> Option<(usize, spool_format::EntryId, u64)> {
        if self.visible_count == 0 {
            return None;
        }
        for idx in (0..self.visible_count).rev() {
            let te = self.timeline.get(idx)?;
            let entry = self.spool_file.entries.get(te.entry_index)?;
            if let (Some(id), Some(ts)) = (entry.id(), entry.timestamp()) {
                return Some((te.entry_index, *id, ts));
            }
        }
        None
    }

    fn start_annotation(&mut self) {
        match self.current_entry_info() {
            Some((target_index, target_id, target_ts)) => {
                self.annotation_draft = Some(AnnotationDraft {
                    target_index,
                    target_id,
                    target_ts,
                    buffer: String::new(),
                    style: None,
                    stage: AnnotationStage::Text,
                });
                self.status_message = Some("Annotation: enter text".to_string());
            }
            None => {
                self.status_message = Some("Cannot annotate yet".to_string());
            }
        }
    }

    fn cancel_annotation(&mut self) {
        self.annotation_draft = None;
        self.status_message = Some("Annotation cancelled".to_string());
    }

    fn handle_annotation_key(&mut self, key: KeyCode) {
        let Some(draft) = self.annotation_draft.as_mut() else {
            return;
        };

        match draft.stage {
            AnnotationStage::Text => match key {
                KeyCode::Esc => self.cancel_annotation(),
                KeyCode::Enter => {
                    if draft.buffer.trim().is_empty() {
                        self.status_message = Some("Enter annotation text".to_string());
                    } else {
                        draft.stage = AnnotationStage::Style;
                        self.status_message = Some("Annotation: select style".to_string());
                    }
                }
                KeyCode::Backspace => {
                    draft.buffer.pop();
                }
                KeyCode::Char(ch) => {
                    draft.buffer.push(ch);
                }
                _ => {}
            },
            AnnotationStage::Style => match key {
                KeyCode::Esc => self.cancel_annotation(),
                KeyCode::Enter => {
                    if let Some(style) = draft.style.clone() {
                        let content = draft.buffer.trim().to_string();
                        let target_index = draft.target_index;
                        let target_id = draft.target_id;
                        let target_ts = draft.target_ts;
                        self.add_annotation(target_index, target_id, target_ts, content, style);
                        self.annotation_draft = None;
                        self.status_message = Some("Annotation added".to_string());
                    } else {
                        self.status_message = Some("Select a style".to_string());
                    }
                }
                KeyCode::Char(ch) => {
                    if let Some(style) = annotation_style_from_key(ch) {
                        draft.style = Some(style);
                    }
                }
                _ => {}
            },
        }
    }

    fn add_annotation(
        &mut self,
        target_index: usize,
        target_id: spool_format::EntryId,
        target_ts: u64,
        content: String,
        style: AnnotationStyle,
    ) {
        let annotation = AnnotationEntry {
            id: Uuid::new_v4(),
            ts: target_ts,
            target_id,
            content,
            author: None,
            style: Some(style),
            created_at: Some(chrono::Utc::now()),
            extra: HashMap::new(),
        };

        let insert_at = (target_index + 1).min(self.spool_file.entries.len());
        self.spool_file
            .entries
            .insert(insert_at, Entry::Annotation(annotation));
        self.update_session_entry_count();
        self.rebuild_timeline();
    }

    fn update_session_entry_count(&mut self) {
        let count = self.spool_file.entries.len();
        self.spool_file.session.entry_count = Some(count);
        if let Some(Entry::Session(ref mut session)) = self.spool_file.entries.get_mut(0) {
            session.entry_count = Some(count);
        }
    }

    fn rebuild_timeline(&mut self) {
        self.timeline = build_timeline(&self.spool_file.entries);
        self.total_duration_ms = self.timeline.last().map(|t| t.playback_ms).unwrap_or(0);
        self.visible_count = self
            .timeline
            .iter()
            .take_while(|t| t.playback_ms <= self.playback_elapsed_ms)
            .count();
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

    /// Advance playback based on elapsed real time.
    pub fn tick(&mut self) {
        if !self.playing {
            return;
        }

        let now = Instant::now();
        let real_elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;

        let advance_ms = (real_elapsed.as_millis() as f64 * self.speed() as f64) as u64;
        self.playback_elapsed_ms = self.playback_elapsed_ms.saturating_add(advance_ms);

        while self.visible_count < self.timeline.len() {
            let te = &self.timeline[self.visible_count];
            if te.playback_ms <= self.playback_elapsed_ms {
                self.visible_count += 1;
                self.auto_scroll();
            } else {
                break;
            }
        }

        if self.visible_count >= self.timeline.len() {
            self.playing = false;
            self.playback_elapsed_ms = self.total_duration_ms;
        }
    }

    fn auto_scroll(&mut self) {
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

    /// Handle a key event and return an action.
    pub fn handle_key(&mut self, key: KeyEvent) -> EditorAction {
        if key.kind != KeyEventKind::Press {
            return EditorAction::None;
        }

        if self.annotation_draft.is_some() {
            self.handle_annotation_key(key.code);
            return EditorAction::None;
        }

        // Info overlay: any key dismisses it
        if self.show_info {
            self.show_info = false;
            return EditorAction::None;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.has_library {
                    return EditorAction::Back;
                } else {
                    return EditorAction::Quit;
                }
            }
            KeyCode::Char(' ') => self.toggle_play(),
            KeyCode::Right | KeyCode::Char('l') => self.step_forward(),
            KeyCode::Left | KeyCode::Char('h') => self.step_backward(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.speed_up(),
            KeyCode::Char('-') | KeyCode::Char('_') => self.speed_down(),
            KeyCode::Home | KeyCode::Char('g') => self.jump_to_start(),
            KeyCode::End | KeyCode::Char('G') => self.jump_to_end(),
            KeyCode::PageUp | KeyCode::Char('k') => self.scroll_up(10),
            KeyCode::PageDown | KeyCode::Char('j') => self.scroll_down(10),
            KeyCode::Char('s') => self.mark_trim_start(),
            KeyCode::Char('d') => self.mark_trim_end(),
            KeyCode::Char('e') => self.export_trimmed(),
            KeyCode::Char('a') => self.start_annotation(),
            KeyCode::Char('i') => {
                self.show_info = true;
            }
            _ => {}
        }

        EditorAction::None
    }

    /// Render the Editor view.
    pub fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title bar
                Constraint::Min(1),    // Entry content
                Constraint::Length(1), // Progress bar
                Constraint::Length(1), // Controls
            ])
            .split(f.area());

        self.draw_title_bar(f, chunks[0]);
        self.draw_entries(f, chunks[1]);
        self.draw_progress_bar(f, chunks[2]);
        self.draw_controls(f, chunks[3]);

        if self.annotation_draft.is_some() {
            self.draw_annotation_modal(f);
        }

        if self.show_info {
            self.draw_info_overlay(f);
        }
    }

    fn draw_title_bar(&self, f: &mut Frame, area: Rect) {
        let status_icon = if self.playing { ">" } else { "||" };
        let speed_label = format!("{}x", self.speed());

        let title = Line::from(vec![
            Span::styled(
                format!(" {} ", status_icon),
                Style::default()
                    .fg(if self.playing {
                        Color::Green
                    } else {
                        Color::Yellow
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                self.session_title.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(speed_label, Style::default().fg(Color::Cyan)),
        ]);

        f.render_widget(Paragraph::new(title), area);
    }

    fn draw_entries(&mut self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::NONE);
        let inner = block.inner(area);

        if self.visible_count == 0 {
            let msg = if self.playing {
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

        let mut lines: Vec<Line> = Vec::new();

        for ti in 0..self.visible_count {
            let te = &self.timeline[ti];
            let entry = &self.spool_file.entries[te.entry_index];
            render_entry_lines(entry, &mut lines, inner.width as usize);
        }

        let total_lines = lines.len();
        let view_height = inner.height as usize;
        let max_scroll = total_lines.saturating_sub(view_height);

        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(paragraph, area);
    }

    fn draw_progress_bar(&self, f: &mut Frame, area: Rect) {
        let ratio = self.progress_ratio();
        let label = self.progress_label();

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

    fn draw_controls(&self, f: &mut Frame, area: Rect) {
        let play_key = if self.playing {
            "Space:pause"
        } else {
            "Space:play"
        };
        let trim_label = match (self.trim_start_ms, self.trim_end_ms) {
            (None, None) => "Trim: [unset]".to_string(),
            (Some(start), None) => format!("Trim start: {}", format_duration_ms(start)),
            (None, Some(end)) => format!("Trim end: {}", format_duration_ms(end)),
            (Some(start), Some(end)) => {
                if start < end {
                    let (kept, duration) = self.trim_preview(start, end);
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

        let back_key = if self.has_library { "q:back" } else { "q:quit" };

        let mut text = format!(
            " {}  h/l:step  +/-:speed  j/k:scroll  g/G:start/end  s:trim-start  d:trim-end  e:export  a:annotate  i:info  {}  {}",
            play_key, back_key, trim_label
        );
        if let Some(ref status) = self.status_message {
            text.push_str("  |  ");
            text.push_str(status);
        }

        let paragraph =
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray).bg(Color::Black));
        f.render_widget(paragraph, area);
    }

    fn draw_annotation_modal(&self, f: &mut Frame) {
        let Some(ref draft) = self.annotation_draft else {
            return;
        };

        let area = centered_rect(70, 40, f.area());
        let block = Block::default()
            .title("Annotation")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let content = match draft.stage {
            AnnotationStage::Text => {
                let text = truncate_str(&draft.buffer, area.width.saturating_sub(4) as usize);
                vec![
                    Line::from("Enter annotation text (Enter to continue, Esc to cancel)"),
                    Line::from(""),
                    Line::from(format!("Text: {}", text)),
                ]
            }
            AnnotationStage::Style => {
                let style_label = draft
                    .style
                    .as_ref()
                    .map(annotation_style_label)
                    .unwrap_or("none");
                vec![
                    Line::from("Select style: 1/2/3/4/5 or h/c/p/w/s"),
                    Line::from("Enter to save, Esc to cancel"),
                    Line::from(""),
                    Line::from(format!("Style: {}", style_label)),
                ]
            }
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, area);
    }

    fn draw_info_overlay(&self, f: &mut Frame) {
        let area = centered_rect(70, 70, f.area());
        let block = Block::default()
            .title(" Session Info (press any key to close) ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let mut lines: Vec<Line> = Vec::new();
        render_info_lines(&self.spool_file, &mut lines);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
    }
}

/// Build a compressed timeline from entries.
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

            if matches!(entry, Entry::Prompt(_)) && gap > MAX_IDLE_GAP_MS {
                gap = MAX_IDLE_GAP_MS;
            }

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

    fn make_prompt_with_id(ts: u64, id: Uuid, content: &str) -> Entry {
        Entry::Prompt(PromptEntry {
            id,
            ts,
            content: content.to_string(),
            subagent_id: None,
            attachments: None,
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
        let entries = vec![
            make_session_entry(),
            make_response(1000, "first response"),
            make_prompt(31_000, "second prompt"),
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[0].playback_ms, 0);
        assert_eq!(timeline[1].playback_ms, 1000);
        assert_eq!(timeline[2].playback_ms, 3000);
    }

    #[test]
    fn test_thinking_compression() {
        let entries = vec![
            make_session_entry(),
            make_thinking(1000, "thinking..."),
            make_response(61_000, "done"),
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[0].playback_ms, 0);
        assert_eq!(timeline[1].playback_ms, 1000);
        assert_eq!(timeline[2].playback_ms, 3000);
    }

    #[test]
    fn test_small_gaps_not_compressed() {
        let entries = vec![
            make_session_entry(),
            make_response(500, "response"),
            make_prompt(1000, "prompt"),
        ];
        let timeline = build_timeline(&entries);
        assert_eq!(timeline[2].playback_ms, 1000);
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

        let app = EditorState::new(file, PathBuf::from("session.spool"), 1.0);
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

        let mut app = EditorState::new(file, PathBuf::from("session.spool"), 1.0);
        app.visible_count = 3;

        assert_eq!(app.current_entry_timestamp(), Some(1000));
    }

    #[test]
    fn test_next_trimmed_path_increments() {
        let dir = std::env::temp_dir().join(format!("spool-editor-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("session.spool");
        let first = next_trimmed_path(&source);
        assert_eq!(first, dir.join("session.trimmed.spool"));

        fs::write(&first, "x").unwrap();
        let second = next_trimmed_path(&source);
        assert_eq!(second, dir.join("session.trimmed-1.spool"));
    }

    #[test]
    fn test_add_annotation_inserts_after_target() {
        let session = match make_session_entry() {
            Entry::Session(s) => s,
            _ => unreachable!(),
        };
        let mut file = SpoolFile::new(session);
        let prompt_id = Uuid::new_v4();
        file.add_entry(make_prompt_with_id(1000, prompt_id, "hello"));
        file.add_entry(make_response(2000, "ok"));

        let mut app = EditorState::new(file, PathBuf::from("session.spool"), 1.0);
        app.add_annotation(
            1,
            prompt_id,
            1000,
            "note".to_string(),
            AnnotationStyle::Comment,
        );

        assert_eq!(app.spool_file.entries.len(), 4);
        match &app.spool_file.entries[2] {
            Entry::Annotation(a) => {
                assert_eq!(a.target_id, prompt_id);
                assert_eq!(a.ts, 1000);
                assert_eq!(a.content, "note");
            }
            _ => panic!("Expected annotation entry"),
        }
    }
}
