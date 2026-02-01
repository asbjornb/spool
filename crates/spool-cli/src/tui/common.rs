//! Shared TUI helpers used by both Library and Editor views.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use spool_format::{AnnotationStyle, Entry, ToolOutput};

/// Truncate a string to fit within `max_len` bytes, respecting char boundaries.
pub fn truncate_str(s: &str, max_len: usize) -> &str {
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
pub fn format_tool_input(input: &serde_json::Value, max_len: usize) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (key, val) in map {
                let val_str = match val {
                    serde_json::Value::String(s) => truncate_str(s, 40).to_string(),
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

/// Format a duration in milliseconds as `m:ss`.
pub fn format_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{}:{:02}", minutes, seconds)
}

/// Render a single entry into styled lines for the Editor view.
pub fn render_entry_lines(entry: &Entry, lines: &mut Vec<Line>, width: usize) {
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

/// Create a centered popup rectangle.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);
    horizontal[1]
}

/// Map a key character to an annotation style.
pub fn annotation_style_from_key(ch: char) -> Option<AnnotationStyle> {
    match ch {
        '1' | 'h' | 'H' => Some(AnnotationStyle::Highlight),
        '2' | 'c' | 'C' => Some(AnnotationStyle::Comment),
        '3' | 'p' | 'P' => Some(AnnotationStyle::Pin),
        '4' | 'w' | 'W' => Some(AnnotationStyle::Warning),
        '5' | 's' | 'S' => Some(AnnotationStyle::Success),
        _ => None,
    }
}

/// Human-readable label for an annotation style.
pub fn annotation_style_label(style: &AnnotationStyle) -> &'static str {
    match style {
        AnnotationStyle::Highlight => "highlight",
        AnnotationStyle::Comment => "comment",
        AnnotationStyle::Pin => "pin",
        AnnotationStyle::Warning => "warning",
        AnnotationStyle::Success => "success",
    }
}

/// Render info overlay lines for a SpoolFile (used by Editor `i` key).
pub fn render_info_lines(file: &spool_format::SpoolFile, lines: &mut Vec<Line>) {
    lines.push(Line::from(Span::styled(
        "Session Information",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(format!(
        "  Title:      {}",
        file.session.title.as_deref().unwrap_or("Untitled")
    )));
    lines.push(Line::from(format!("  Agent:      {}", file.session.agent)));
    if let Some(ref ver) = file.session.agent_version {
        lines.push(Line::from(format!("  Agent Ver:  {}", ver)));
    }
    lines.push(Line::from(format!(
        "  Recorded:   {}",
        file.session.recorded_at
    )));
    if let Some(ref author) = file.session.author {
        lines.push(Line::from(format!("  Author:     {}", author)));
    }
    lines.push(Line::from(format!(
        "  Format:     v{}",
        file.session.version
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Statistics",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(format!("  Entries:    {}", file.entries.len())));
    lines.push(Line::from(format!(
        "  Duration:   {} ({:.1}s)",
        format_duration_ms(file.duration_ms()),
        file.duration_ms() as f64 / 1000.0
    )));
    lines.push(Line::from(format!(
        "  Prompts:    {}",
        file.prompts().len()
    )));
    lines.push(Line::from(format!(
        "  Responses:  {}",
        file.responses().len()
    )));
    lines.push(Line::from(format!(
        "  Tool calls: {}",
        file.tool_calls().len()
    )));
    lines.push(Line::from(format!("  Errors:     {}", file.errors().len())));
    lines.push(Line::from(format!(
        "  Annotations:{}",
        file.annotations().len()
    )));
    lines.push(Line::from(""));

    let tools = file.tools_used();
    if !tools.is_empty() {
        lines.push(Line::from(Span::styled(
            "Tools Used",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        for tool in &tools {
            let count = file
                .tool_calls()
                .iter()
                .filter(|tc| tc.tool == *tool)
                .count();
            lines.push(Line::from(format!("  {} ({}x)", tool, count)));
        }
        lines.push(Line::from(""));
    }

    if let Some(ref tags) = file.session.tags {
        if !tags.is_empty() {
            lines.push(Line::from(format!("  Tags: {}", tags.join(", "))));
            lines.push(Line::from(""));
        }
    }

    if let Some(ref trimmed) = file.session.trimmed {
        lines.push(Line::from(Span::styled(
            "Trimmed",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!(
            "  Original duration: {}",
            format_duration_ms(trimmed.original_duration_ms)
        )));
        lines.push(Line::from(format!(
            "  Kept range: {}-{}",
            format_duration_ms(trimmed.kept_range.0),
            format_duration_ms(trimmed.kept_range.1)
        )));
    }
}

/// Truncate a string and add "..." if it exceeds the limit.
/// Used by the Library preview panel where ellipsis is needed.
pub fn truncate_str_with_ellipsis(s: &str, max_len: usize) -> String {
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
    fn truncate_str_ascii_at_limit() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_ascii_over_limit() {
        assert_eq!(truncate_str("hello world", 8), "hello wo");
    }

    #[test]
    fn truncate_str_multibyte_arrow_at_boundary() {
        assert_eq!(truncate_str("a\u{2192}b", 4), "a\u{2192}");
    }

    #[test]
    fn truncate_str_multibyte_arrow_mid_char() {
        assert_eq!(truncate_str("a\u{2192}b", 3), "a");
    }

    #[test]
    fn truncate_str_emoji_4byte() {
        assert_eq!(truncate_str("a\u{1f512}b", 5), "a\u{1f512}");
        assert_eq!(truncate_str("a\u{1f512}b", 3), "a");
    }

    #[test]
    fn truncate_str_all_multibyte() {
        assert_eq!(
            truncate_str("\u{2192}\u{2192}\u{2192}", 6),
            "\u{2192}\u{2192}"
        );
        assert_eq!(truncate_str("\u{2192}\u{2192}\u{2192}", 4), "\u{2192}");
    }

    #[test]
    fn truncate_str_empty_input() {
        assert_eq!(truncate_str("", 10), "");
        assert_eq!(truncate_str("", 0), "");
    }

    #[test]
    fn truncate_str_zero_max() {
        assert_eq!(truncate_str("hello", 0), "");
    }

    #[test]
    fn truncate_str_with_ellipsis_ascii() {
        assert_eq!(truncate_str_with_ellipsis("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_str_with_ellipsis_multibyte() {
        assert_eq!(truncate_str_with_ellipsis("abc\u{2192}def", 7), "abc...");
        assert_eq!(
            truncate_str_with_ellipsis("abc\u{2192}def", 9),
            "abc\u{2192}def"
        );
    }

    #[test]
    fn truncate_str_with_ellipsis_small_max() {
        assert_eq!(truncate_str_with_ellipsis("hello", 3), "hel");
        assert_eq!(truncate_str_with_ellipsis("hello", 0), "");
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0), "0:00");
        assert_eq!(format_duration_ms(1000), "0:01");
        assert_eq!(format_duration_ms(60_000), "1:00");
        assert_eq!(format_duration_ms(90_000), "1:30");
        assert_eq!(format_duration_ms(3_661_000), "61:01");
    }
}
