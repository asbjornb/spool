//! Library view - Interactive session browser.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use spool_adapters::{claude_code, codex, AgentType, SessionInfo};
use spool_format::Entry;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::common::{format_tool_input, truncate_str_with_ellipsis};

/// Action returned by the Library view to the top-level app loop.
pub enum LibraryAction {
    /// Open a session in the Editor.
    OpenEditor(PathBuf, AgentType),
    /// Quit the application.
    Quit,
    /// No action; continue rendering.
    None,
}

/// Cached preview data for the selected session.
struct PreviewData {
    session_index: usize,
    entries: Vec<Entry>,
}

/// Input mode.
#[derive(PartialEq)]
enum Mode {
    Normal,
    Search,
}

/// Library view state (the session browser).
pub struct LibraryState {
    sessions: Vec<SessionInfo>,
    filtered_indices: Vec<usize>,
    selected: usize,
    preview: Option<PreviewData>,
    preview_scroll: usize,
    mode: Mode,
    search_input: String,
    agent_filter: Option<String>,
    status_message: Option<(String, Instant)>,
}

impl LibraryState {
    pub fn new(agent_filter: Option<String>) -> Result<Self> {
        let sessions: Vec<SessionInfo> = find_all_sessions()?
            .into_iter()
            .filter(|s| s.message_count.map(|c| c > 0).unwrap_or(true))
            .collect();

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

        Ok(LibraryState {
            sessions,
            filtered_indices,
            selected: 0,
            preview: None,
            preview_scroll: 0,
            mode: Mode::Normal,
            search_input: String::new(),
            agent_filter,
            status_message: None,
        })
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
                let agent_ok = self
                    .agent_filter
                    .as_ref()
                    .map(|f| s.agent.as_str() == f.as_str())
                    .unwrap_or(true);
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

        if let Some(ref p) = self.preview {
            if p.session_index == idx {
                return;
            }
        }

        let session = &self.sessions[idx];
        match convert_session(session) {
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

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    fn tick_status(&mut self) {
        if let Some((_, ref t)) = self.status_message {
            if t.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
            }
        }
    }

    /// Handle a key event and return an action.
    pub fn handle_key(&mut self, key: KeyEvent) -> LibraryAction {
        if key.kind != KeyEventKind::Press {
            return LibraryAction::None;
        }

        match self.mode {
            Mode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    return LibraryAction::Quit;
                }
                KeyCode::Char('j') | KeyCode::Down => self.move_down(),
                KeyCode::Char('k') | KeyCode::Up => self.move_up(),
                KeyCode::Char('g') | KeyCode::Home => self.jump_top(),
                KeyCode::Char('G') | KeyCode::End => self.jump_bottom(),
                KeyCode::Char('/') => {
                    self.mode = Mode::Search;
                }
                KeyCode::Enter => {
                    if let Some(session) = self.selected_session() {
                        let path = session.path.clone();
                        let agent = session.agent;
                        return LibraryAction::OpenEditor(path, agent);
                    }
                }
                KeyCode::PageDown | KeyCode::Char('l') => {
                    self.preview_scroll = self.preview_scroll.saturating_add(10);
                }
                KeyCode::PageUp | KeyCode::Char('h') => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(10);
                }
                _ => {}
            },
            Mode::Search => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    self.search_input.clear();
                    self.update_filter();
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    self.update_filter();
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                    self.update_filter();
                }
                _ => {}
            },
        }

        LibraryAction::None
    }

    /// Called before rendering to ensure preview is loaded and status is ticked.
    pub fn tick(&mut self) {
        self.load_preview();
        self.tick_status();
    }

    /// Render the Library view.
    pub fn draw(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(f.area());

        let main_area = chunks[0];
        let status_area = chunks[1];

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(main_area);

        self.draw_session_list(f, main_chunks[0]);
        self.draw_preview(f, main_chunks[1]);
        self.draw_status_bar(f, status_area);
    }

    fn draw_session_list(&self, f: &mut Frame, area: Rect) {
        let count = self.filtered_indices.len();
        let title = if self.search_input.is_empty() {
            format!(" Sessions ({}) ", count)
        } else {
            format!(" Sessions ({}) [/{}] ", count, self.search_input)
        };

        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .enumerate()
            .map(|(i, &session_idx)| {
                let session = &self.sessions[session_idx];
                let title_text = session.title.as_deref().unwrap_or("Untitled");
                let agent = session.agent.as_str();
                let date = session
                    .modified_at
                    .map(|d| {
                        d.with_timezone(&chrono::Local)
                            .format("%m/%d %H:%M")
                            .to_string()
                    })
                    .unwrap_or_default();

                let agent_badge = match agent {
                    "claude-code" => "CC",
                    "codex" => "CX",
                    "cursor" => "CU",
                    "aider" => "AI",
                    _ => "??",
                };

                let is_selected = i == self.selected;

                let max_title_len = area.width as usize - agent_badge.len() - date.len() - 6;
                let display_title = truncate_str_with_ellipsis(title_text, max_title_len);

                let line = Line::from(vec![
                    Span::styled(
                        if is_selected { "> " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        display_title,
                        if is_selected {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::raw(" "),
                    Span::styled(agent_badge, Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(date, Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let block = Block::default().borders(Borders::ALL).title(title);

        if items.is_empty() {
            let empty_msg = if self.sessions.is_empty() {
                "No sessions found.\n\nLooking in:\n  ~/.claude/projects/"
            } else {
                "No matching sessions."
            };
            let paragraph = Paragraph::new(empty_msg)
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(paragraph, area);
        } else {
            let list = List::new(items)
                .block(block)
                .highlight_style(Style::default().bg(Color::DarkGray));
            f.render_widget(list, area);
        }
    }

    fn draw_preview(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Preview ");

        let Some(ref preview) = self.preview else {
            let msg = if self.filtered_indices.is_empty() {
                "No session selected"
            } else {
                "Loading preview..."
            };
            let paragraph = Paragraph::new(msg)
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(paragraph, area);
            return;
        };

        let mut lines: Vec<Line> = Vec::new();

        for entry in &preview.entries {
            match entry {
                Entry::Session(s) => {
                    lines.push(Line::from(Span::styled(
                        format!("SESSION: {}", s.title.as_deref().unwrap_or("Untitled")),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )));
                    if let Some(ref agent_ver) = s.agent_version {
                        lines.push(Line::from(Span::styled(
                            format!("  Agent: {} v{}", s.agent, agent_ver),
                            Style::default().fg(Color::DarkGray),
                        )));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  Agent: {}", s.agent),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                    lines.push(Line::from(""));
                }
                Entry::Prompt(p) => {
                    lines.push(Line::from(Span::styled(
                        "PROMPT",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )));
                    let preview_lines: Vec<&str> = p.content.lines().take(3).collect();
                    for line in preview_lines {
                        let truncated = truncate_str_with_ellipsis(line, area.width as usize - 4);
                        lines.push(Line::from(format!("  {}", truncated)));
                    }
                    if p.content.lines().count() > 3 {
                        lines.push(Line::from(Span::styled(
                            "  ...",
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                    lines.push(Line::from(""));
                }
                Entry::Thinking(t) => {
                    let preview_text =
                        truncate_str_with_ellipsis(&t.content.replace('\n', " "), 60);
                    lines.push(Line::from(vec![
                        Span::styled(
                            "THINKING ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::DIM),
                        ),
                        Span::styled(preview_text, Style::default().add_modifier(Modifier::DIM)),
                    ]));
                    lines.push(Line::from(""));
                }
                Entry::ToolCall(tc) => {
                    let input_preview = format_tool_input(&tc.input, area.width as usize - 12);
                    let tool_display = if tc.tool == "Task" {
                        if let Some(subagent_type) =
                            tc.input.get("subagent_type").and_then(|v| v.as_str())
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
                            "TOOL: ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(tool_display, Style::default().fg(Color::Blue)),
                    ]));
                    lines.push(Line::from(format!("  {}", input_preview)));
                    lines.push(Line::from(""));
                }
                Entry::ToolResult(tr) => {
                    let status = if tr.error.is_some() {
                        Span::styled("[ERROR]", Style::default().fg(Color::Red))
                    } else {
                        Span::styled("[OK]", Style::default().fg(Color::Green))
                    };
                    lines.push(Line::from(vec![
                        Span::styled("RESULT ", Style::default().fg(Color::Blue)),
                        status,
                    ]));
                    if let Some(ref err) = tr.error {
                        let truncated = truncate_str_with_ellipsis(err, area.width as usize - 4);
                        lines.push(Line::from(Span::styled(
                            format!("  {}", truncated),
                            Style::default().fg(Color::Red),
                        )));
                    } else if let Some(ref output) = tr.output {
                        let text = match output {
                            spool_format::ToolOutput::Text(t) => t.as_str(),
                            spool_format::ToolOutput::Binary(_) => "<binary>",
                        };
                        let preview_lines: Vec<&str> = text.lines().take(3).collect();
                        for line in preview_lines {
                            let truncated =
                                truncate_str_with_ellipsis(line, area.width as usize - 4);
                            lines.push(Line::from(format!("  {}", truncated)));
                        }
                    }
                    lines.push(Line::from(""));
                }
                Entry::Response(r) => {
                    lines.push(Line::from(Span::styled(
                        "RESPONSE",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    )));
                    let preview_lines: Vec<&str> = r.content.lines().take(5).collect();
                    for line in preview_lines {
                        let truncated = truncate_str_with_ellipsis(line, area.width as usize - 4);
                        lines.push(Line::from(format!("  {}", truncated)));
                    }
                    if r.content.lines().count() > 5 {
                        lines.push(Line::from(Span::styled(
                            "  ...",
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                    lines.push(Line::from(""));
                }
                Entry::Error(e) => {
                    lines.push(Line::from(Span::styled(
                        format!("ERROR [{:?}]", e.code),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                    let truncated = truncate_str_with_ellipsis(&e.message, area.width as usize - 4);
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
                    lines.push(Line::from(""));
                }
                Entry::SubagentEnd(e) => {
                    let status = e
                        .status
                        .as_ref()
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_default();
                    lines.push(Line::from(Span::styled(
                        format!("SUBAGENT END {}", status),
                        Style::default().fg(Color::Cyan),
                    )));
                    lines.push(Line::from(""));
                }
                Entry::Annotation(a) => {
                    lines.push(Line::from(Span::styled(
                        format!("NOTE ({})", a.author.as_deref().unwrap_or("anonymous")),
                        Style::default().fg(Color::Yellow),
                    )));
                    let truncated = truncate_str_with_ellipsis(&a.content, area.width as usize - 4);
                    lines.push(Line::from(format!("  {}", truncated)));
                    lines.push(Line::from(""));
                }
                Entry::RedactionMarker(_) => {
                    lines.push(Line::from(Span::styled(
                        "[REDACTED]",
                        Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
                    )));
                    lines.push(Line::from(""));
                }
                Entry::Unknown => {}
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.preview_scroll as u16, 0));

        f.render_widget(paragraph, area);
    }

    fn draw_status_bar(&self, f: &mut Frame, area: Rect) {
        let (left_text, style) = if let Some((ref msg, _)) = self.status_message {
            (msg.clone(), Style::default().fg(Color::Green))
        } else if self.mode == Mode::Search {
            (
                format!(
                    "Search: {}_ | Esc: cancel  Enter: confirm",
                    self.search_input
                ),
                Style::default().fg(Color::Yellow),
            )
        } else {
            (
                "j/k: navigate  /: search  Enter: open  q: quit".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        };

        let bar = Paragraph::new(left_text).style(style);
        f.render_widget(bar, area);
    }
}

fn find_all_sessions() -> Result<Vec<SessionInfo>> {
    let mut sessions =
        claude_code::find_sessions().context("Failed to discover Claude Code sessions")?;
    sessions.extend(codex::find_sessions().context("Failed to discover Codex sessions")?);
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(sessions)
}

pub fn convert_session(session: &SessionInfo) -> Result<spool_format::SpoolFile> {
    match session.agent {
        AgentType::ClaudeCode => claude_code::convert(session),
        AgentType::Codex => codex::convert(session),
        _ => anyhow::bail!("Unsupported agent: {}", session.agent.as_str()),
    }
}
