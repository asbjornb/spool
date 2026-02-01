//! TUI rendering for the session browser.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use spool_format::Entry;

use super::App;

/// Main draw function — renders the entire UI.
pub fn draw(f: &mut Frame, app: &App) {
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

    draw_session_list(f, main_chunks[0], app);
    draw_preview(f, main_chunks[1], app);
    draw_status_bar(f, status_area, app);
}

/// Render the session list panel (left side).
fn draw_session_list(f: &mut Frame, area: Rect, app: &App) {
    let count = app.filtered_indices.len();
    let title = if app.search_input.is_empty() {
        format!(" Sessions ({}) ", count)
    } else {
        format!(" Sessions ({}) [/{}] ", count, app.search_input)
    };

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(i, &session_idx)| {
            let session = &app.sessions[session_idx];
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

            let is_selected = i == app.selected;

            // Truncate title to fit
            let max_title_len = area.width as usize - agent_badge.len() - date.len() - 6;
            let display_title = if title_text.len() > max_title_len && max_title_len > 3 {
                let mut end = max_title_len.saturating_sub(3);
                while end > 0 && !title_text.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &title_text[..end])
            } else {
                title_text.to_string()
            };

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
        let empty_msg = if app.sessions.is_empty() {
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

/// Render the preview panel (right side).
fn draw_preview(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Preview ");

    let Some(ref preview) = app.preview else {
        let msg = if app.filtered_indices.is_empty() {
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
                    let truncated = truncate_str(line, area.width as usize - 4);
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
                let preview_text = truncate_str(&t.content.replace('\n', " "), 60);
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
                    let truncated = truncate_str(err, area.width as usize - 4);
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
                        let truncated = truncate_str(line, area.width as usize - 4);
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
                    let truncated = truncate_str(line, area.width as usize - 4);
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
                let truncated = truncate_str(&e.message, area.width as usize - 4);
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
                let truncated = truncate_str(&a.content, area.width as usize - 4);
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
        .scroll((app.preview_scroll as u16, 0));

    f.render_widget(paragraph, area);
}

/// Render the status bar (bottom).
fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (left_text, style) = if let Some((ref msg, _)) = app.status_message {
        if let Some(stripped) = msg.strip_prefix("VIEW:") {
            (stripped.to_string(), Style::default().fg(Color::DarkGray))
        } else {
            (msg.clone(), Style::default().fg(Color::Green))
        }
    } else if app.mode == super::Mode::Search {
        (
            format!(
                "Search: {}_ | Esc: cancel  Enter: confirm",
                app.search_input
            ),
            Style::default().fg(Color::Yellow),
        )
    } else {
        (
            "j/k: navigate  /: search  e: export  r: export+redact  Enter: view  q: quit"
                .to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    let bar = Paragraph::new(left_text).style(style);
    f.render_widget(bar, area);
}

/// Truncate a string to fit within `max_len` bytes, respecting char boundaries.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let mut end = max_len - 3;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    } else {
        let mut end = max_len;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s[..end].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_str_ascii_within_limit() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_ascii_over_limit() {
        // "hello world" = 11 bytes, max 8 → "hello..."
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_str_multibyte_arrow() {
        // '→' is 3 bytes (E2 86 92). "abc→def" bytes: [61,62,63,E2,86,92,64,65,66] = 9 bytes.
        // max 7 → 7-3=4, boundary(4)? byte 4=0x86 (continuation), walk back to 3. result = "abc..."
        assert_eq!(truncate_str("abc→def", 7), "abc...");
        // max 9 → 9-3=6, boundary(6)=true (start of 'd'). result = "abc→..."
        assert_eq!(truncate_str("abc→def", 9), "abc→def"); // 9 == 9, no truncation
                                                           // max 10 → no truncation
        assert_eq!(truncate_str("abc→def", 10), "abc→def");
    }

    #[test]
    fn truncate_str_emoji_4byte() {
        // "ab🔒cd" = 2+4+2 = 8 bytes.
        // max 7 → 7-3=4, boundary(4)? bytes: [61,62,F0,9F,94,92,63,64].
        // 4 is not boundary (inside emoji), walk to 2. result = "ab..."
        assert_eq!(truncate_str("ab🔒cd", 7), "ab...");
    }

    #[test]
    fn truncate_str_small_max() {
        // max ≤ 3: no room for "...", just truncate
        assert_eq!(truncate_str("hello", 3), "hel");
        assert_eq!(truncate_str("hello", 0), "");
    }
}

/// Format tool input JSON for preview display.
fn format_tool_input(input: &serde_json::Value, max_len: usize) -> String {
    match input {
        serde_json::Value::Object(map) => {
            // Show a compact representation of the most useful fields
            let mut parts = Vec::new();
            for (key, val) in map {
                let val_str = match val {
                    serde_json::Value::String(s) => truncate_str(s, 40),
                    other => {
                        let s = other.to_string();
                        truncate_str(&s, 40)
                    }
                };
                parts.push(format!("{}: {}", key, val_str));
            }
            let joined = parts.join(", ");
            truncate_str(&joined, max_len)
        }
        other => truncate_str(&other.to_string(), max_len),
    }
}
