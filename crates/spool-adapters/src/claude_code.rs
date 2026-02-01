//! Claude Code adapter.
//!
//! Parses Claude Code session logs and converts them to Spool format.
//!
//! ## Log Locations
//!
//! Claude Code stores sessions in:
//! - `~/.claude/projects/<project-path-slug>/`
//!
//! Each session is a JSONL file (`<session-id>.jsonl`) where each line is a JSON
//! object with a top-level `type` field. The project directory may also contain:
//! - `sessions-index.json` — quick metadata index for all sessions
//! - `<session-id>/subagents/agent-<id>.jsonl` — subagent session files
//! - `agent-<id>.jsonl` — older-format subagent files at project root

use crate::{AgentType, SessionInfo};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use spool_format::{
    Entry, EntryId, PromptEntry, ResponseEntry, SessionEntry, ThinkingEntry, ToolCallEntry,
    ToolOutput, ToolResultEntry,
};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use uuid::Uuid;

// ============================================================================
// Public API
// ============================================================================

/// Find all Claude Code sessions on the system.
pub fn find_sessions() -> Result<Vec<SessionInfo>> {
    let base_dir = get_claude_dir()?;
    let projects_dir = base_dir.join("projects");

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for project_entry in fs::read_dir(&projects_dir)? {
        let project_entry = project_entry?;
        let project_path = project_entry.path();

        if !project_path.is_dir() {
            continue;
        }

        // Try to use sessions-index.json for fast metadata
        let index = read_sessions_index(&project_path);

        // Find .jsonl session files directly in the project directory
        for file_entry in fs::read_dir(&project_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();

            // Only .jsonl files
            if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                // Skip subagent files (agent-*.jsonl)
                let filename = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if filename.starts_with("agent-") {
                    continue;
                }

                let session_id = filename.to_string();
                let metadata = fs::metadata(&file_path).ok();
                let modified_at = metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(DateTime::<Utc>::from);
                let created_at = metadata
                    .as_ref()
                    .and_then(|m| m.created().ok())
                    .map(DateTime::<Utc>::from);

                // Look up metadata from index if available
                let index_entry = index
                    .as_ref()
                    .and_then(|idx| idx.entries.iter().find(|e| e.session_id == session_id));

                let title = index_entry.and_then(|e| {
                    // Prefer summary over firstPrompt
                    e.summary
                        .clone()
                        .or_else(|| e.first_prompt.clone())
                        .filter(|s| s != "No prompt")
                });

                let project_dir = index_entry
                    .and_then(|e| e.project_path.as_ref().map(PathBuf::from))
                    .or_else(|| Some(project_path.clone()));

                // Use index timestamps if available (more accurate)
                let created_at = index_entry
                    .and_then(|e| e.created.as_ref())
                    .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                    .or(created_at);
                let modified_at = index_entry
                    .and_then(|e| e.modified.as_ref())
                    .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                    .or(modified_at);

                let message_count = index_entry.and_then(|e| e.message_count);

                sessions.push(SessionInfo {
                    path: file_path,
                    agent: AgentType::ClaudeCode,
                    created_at,
                    modified_at,
                    title,
                    project_dir,
                    message_count,
                });
            }
        }
    }

    // Sort by modified time, newest first
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
}

/// Convert a Claude Code session to Spool format.
pub fn convert(session: &SessionInfo) -> Result<spool_format::SpoolFile> {
    let file = fs::File::open(&session.path)
        .with_context(|| format!("Failed to open session file: {:?}", session.path))?;
    let reader = BufReader::new(file);

    let mut raw_lines: Vec<RawLine> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<RawLine>(trimmed) {
            Ok(parsed) => raw_lines.push(parsed),
            Err(_) => continue, // Skip unparseable lines
        }
    }

    convert_raw_lines(&raw_lines, session)
}

// ============================================================================
// Sessions index
// ============================================================================

#[derive(Debug, Deserialize)]
struct SessionsIndex {
    #[serde(default)]
    entries: Vec<SessionsIndexEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionsIndexEntry {
    session_id: String,
    #[serde(default)]
    first_prompt: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    message_count: Option<usize>,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    modified: Option<String>,
    #[serde(default)]
    project_path: Option<String>,
}

fn read_sessions_index(project_dir: &std::path::Path) -> Option<SessionsIndex> {
    let index_path = project_dir.join("sessions-index.json");
    let content = fs::read_to_string(&index_path).ok()?;
    serde_json::from_str(&content).ok()
}

// ============================================================================
// Raw Claude Code JSONL format types
// ============================================================================

/// A single line from a Claude Code JSONL session file.
/// Each line has a `type` field that determines its structure.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawLine {
    #[serde(rename = "user")]
    User(RawUserLine),
    #[serde(rename = "assistant")]
    Assistant(RawAssistantLine),
    #[serde(rename = "system")]
    System(RawSystemLine),
    #[serde(rename = "summary")]
    Summary(RawSummaryLine),
    // Types we skip during conversion
    #[serde(rename = "progress")]
    Progress(RawProgressLine),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot {},
    #[serde(rename = "update")]
    Update {},
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct RawUserLine {
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(rename = "isMeta", default)]
    is_meta: bool,
    #[serde(rename = "toolUseResult", default)]
    tool_use_result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawAssistantLine {
    #[serde(default)]
    message: Option<RawApiMessage>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSystemLine {
    #[serde(default)]
    subtype: Option<String>,
    #[serde(rename = "durationMs", default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSummaryLine {
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProgressLine {
    #[serde(default)]
    data: Option<serde_json::Value>,
}

/// The `message` field inside a user line.
#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<RawContent>,
}

/// The `message` field inside an assistant line.
/// This matches the Anthropic API response format.
#[derive(Debug, Deserialize)]
struct RawApiMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    content: Option<Vec<RawContentBlock>>,
    #[serde(default)]
    usage: Option<serde_json::Value>,
    #[serde(default)]
    stop_reason: Option<String>,
}

/// User message content can be a string (prompt) or array (tool results).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawContent {
    Text(String),
    Blocks(Vec<RawToolResultBlock>),
}

/// A content block in an assistant message.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Unknown,
}

/// A tool_result block in a user message content array.
#[derive(Debug, Deserialize)]
struct RawToolResultBlock {
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    content: Option<RawToolResultContent>,
    #[serde(default)]
    is_error: Option<bool>,
}

/// Tool result content can be a string or array of text blocks.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawToolResultContent {
    Text(String),
    Blocks(Vec<RawTextBlock>),
}

#[derive(Debug, Deserialize)]
struct RawTextBlock {
    #[serde(default)]
    text: Option<String>,
}

// ============================================================================
// Conversion logic
// ============================================================================

fn convert_raw_lines(
    raw_lines: &[RawLine],
    info: &SessionInfo,
) -> Result<spool_format::SpoolFile> {
    let mut entries: Vec<Entry> = Vec::new();

    // Track tool call IDs: Claude's tool_use id -> our spool EntryId
    let mut tool_id_map: HashMap<String, EntryId> = HashMap::new();

    // Parse timestamps to compute relative ms
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut summary_text: Option<String> = None;
    let mut agent_version: Option<String> = None;
    let mut model_name: Option<String> = None;

    // First pass: find metadata
    for line in raw_lines {
        match line {
            RawLine::Summary(s) => {
                if summary_text.is_none() {
                    summary_text = s.summary.clone();
                }
            }
            RawLine::User(u) => {
                if first_timestamp.is_none() && !u.is_meta {
                    if let Some(ref ts) = u.timestamp {
                        first_timestamp = ts.parse::<DateTime<Utc>>().ok();
                    }
                }
            }
            RawLine::Assistant(a) => {
                if let Some(ref msg) = a.message {
                    if model_name.is_none() {
                        model_name = msg.model.clone();
                    }
                }
            }
            _ => {}
        }
    }

    // Use session info timestamps as fallback
    let session_start = first_timestamp
        .or(info.created_at)
        .or(info.modified_at)
        .unwrap_or_else(Utc::now);

    // Create session entry
    let title = info
        .title
        .clone()
        .or(summary_text)
        .or_else(|| extract_title_from_lines(raw_lines));

    let session_entry = SessionEntry {
        id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
        ts: 0,
        version: "1.0".to_string(),
        agent: "claude-code".to_string(),
        recorded_at: session_start,
        agent_version: agent_version.clone(),
        title,
        author: None,
        tags: None,
        duration_ms: None,
        entry_count: None,
        tools_used: None,
        schema_url: None,
        trimmed: None,
        ended: Some(spool_format::SessionEndState::Unknown),
        extra: if let Some(ref model) = model_name {
            let mut m = HashMap::new();
            m.insert(
                "x_model".to_string(),
                serde_json::Value::String(model.clone()),
            );
            m
        } else {
            HashMap::new()
        },
    };
    entries.push(Entry::Session(session_entry));

    // Second pass: convert entries
    for line in raw_lines {
        match line {
            RawLine::User(u) => {
                if u.is_meta {
                    continue;
                }
                let ts = compute_relative_ts(&u.timestamp, &session_start);

                if let Some(ref msg) = u.message {
                    match &msg.content {
                        Some(RawContent::Text(text)) => {
                            // Skip command messages (slash commands, local command output)
                            if text.contains("<command-name>")
                                || text.contains("<local-command-stdout>")
                                || text.contains("<local-command-caveat>")
                            {
                                continue;
                            }

                            let clean = strip_system_tags(text);
                            if !clean.is_empty() {
                                entries.push(Entry::Prompt(PromptEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    content: clean,
                                    subagent_id: None,
                                    attachments: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        Some(RawContent::Blocks(blocks)) => {
                            // Tool results
                            for block in blocks {
                                if block
                                    .block_type
                                    .as_deref()
                                    .map(|t| t == "tool_result")
                                    .unwrap_or(false)
                                {
                                    let tool_use_id =
                                        block.tool_use_id.as_deref().unwrap_or("");
                                    let call_id = tool_id_map
                                        .get(tool_use_id)
                                        .copied()
                                        .unwrap_or_else(Uuid::nil);
                                    let is_error =
                                        block.is_error.unwrap_or(false);
                                    let content_text = extract_tool_result_text(&block.content);

                                    let entry = if is_error {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(
                                                uuid::Timestamp::now(uuid::NoContext),
                                            ),
                                            ts,
                                            call_id,
                                            output: None,
                                            error: Some(content_text),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    } else {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(
                                                uuid::Timestamp::now(uuid::NoContext),
                                            ),
                                            ts,
                                            call_id,
                                            output: Some(ToolOutput::Text(content_text)),
                                            error: None,
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    };
                                    entries.push(Entry::ToolResult(entry));
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
            RawLine::Assistant(a) => {
                let ts = compute_relative_ts(&a.timestamp, &session_start);

                if let Some(ref msg) = a.message {
                    if let Some(ref blocks) = msg.content {
                        for block in blocks {
                            match block {
                                RawContentBlock::Text { text } => {
                                    if !text.is_empty() {
                                        entries.push(Entry::Response(ResponseEntry {
                                            id: Uuid::new_v7(
                                                uuid::Timestamp::now(uuid::NoContext),
                                            ),
                                            ts,
                                            content: text.clone(),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            extra: HashMap::new(),
                                        }));
                                    }
                                }
                                RawContentBlock::Thinking { thinking, .. } => {
                                    if !thinking.is_empty() {
                                        entries.push(Entry::Thinking(ThinkingEntry {
                                            id: Uuid::new_v7(
                                                uuid::Timestamp::now(uuid::NoContext),
                                            ),
                                            ts,
                                            content: thinking.clone(),
                                            collapsed: Some(true),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            extra: HashMap::new(),
                                        }));
                                    }
                                }
                                RawContentBlock::ToolUse { id, name, input } => {
                                    let entry_id =
                                        Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
                                    tool_id_map.insert(id.clone(), entry_id);

                                    entries.push(Entry::ToolCall(ToolCallEntry {
                                        id: entry_id,
                                        ts,
                                        tool: name.clone(),
                                        input: input.clone(),
                                        subagent_id: None,
                                        extra: HashMap::new(),
                                    }));
                                }
                                RawContentBlock::Unknown => {}
                            }
                        }
                    }
                }
            }
            // Skip other line types during conversion
            _ => {}
        }
    }

    // Compute final metadata
    let last_ts = entries
        .iter()
        .filter_map(|e| e.timestamp())
        .max()
        .unwrap_or(0);
    let entry_count = entries.len();
    let tools_used = {
        let mut tools: Vec<String> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::ToolCall(tc) => Some(tc.tool.clone()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tools.sort();
        tools
    };

    // Update session entry with computed metadata
    if let Entry::Session(ref mut s) = entries[0] {
        s.duration_ms = Some(last_ts);
        s.entry_count = Some(entry_count);
        if !tools_used.is_empty() {
            s.tools_used = Some(tools_used);
        }
        s.ended = Some(spool_format::SessionEndState::Completed);
    }

    let session = match &entries[0] {
        Entry::Session(s) => s.clone(),
        _ => unreachable!(),
    };

    Ok(spool_format::SpoolFile {
        session,
        entries,
        unparsed_lines: Vec::new(),
    })
}

/// Compute relative timestamp in milliseconds from session start.
fn compute_relative_ts(timestamp: &Option<String>, session_start: &DateTime<Utc>) -> u64 {
    timestamp
        .as_ref()
        .and_then(|ts| ts.parse::<DateTime<Utc>>().ok())
        .map(|ts| {
            let duration = ts.signed_duration_since(*session_start);
            duration.num_milliseconds().max(0) as u64
        })
        .unwrap_or(0)
}

/// Extract text from tool result content (can be string or array of text blocks).
fn extract_tool_result_text(content: &Option<RawToolResultContent>) -> String {
    match content {
        Some(RawToolResultContent::Text(s)) => s.clone(),
        Some(RawToolResultContent::Blocks(blocks)) => blocks
            .iter()
            .filter_map(|b| b.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

/// Strip system-injected XML tags from user messages to get clean prompt text.
fn strip_system_tags(text: &str) -> String {
    // Remove common system-injected tags
    let mut result = text.to_string();
    // Remove <system-reminder>...</system-reminder> blocks
    while let (Some(start), Some(end)) = (
        result.find("<system-reminder>"),
        result.find("</system-reminder>"),
    ) {
        let end = end + "</system-reminder>".len();
        if start < end {
            result.replace_range(start..end, "");
        } else {
            break;
        }
    }
    result.trim().to_string()
}

/// Extract a title from the first real user prompt.
fn extract_title_from_lines(lines: &[RawLine]) -> Option<String> {
    for line in lines {
        if let RawLine::User(u) = line {
            if u.is_meta {
                continue;
            }
            if let Some(ref msg) = u.message {
                if let Some(RawContent::Text(ref text)) = msg.content {
                    if text.contains("<command-name>")
                        || text.contains("<local-command-stdout>")
                        || text.contains("<local-command-caveat>")
                    {
                        continue;
                    }
                    let clean = strip_system_tags(text);
                    if !clean.is_empty() {
                        let first_line = clean.lines().next().unwrap_or(&clean);
                        let title = if first_line.len() > 60 {
                            format!("{}...", &first_line[..57])
                        } else {
                            first_line.to_string()
                        };
                        return Some(title);
                    }
                }
            }
        }
    }
    None
}

fn get_claude_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".claude"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_system_tags() {
        let input = "Hello world";
        assert_eq!(strip_system_tags(input), "Hello world");

        let input = "Before <system-reminder>hidden</system-reminder> After";
        assert_eq!(strip_system_tags(input), "Before  After");

        let input = "<system-reminder>all hidden</system-reminder>";
        assert_eq!(strip_system_tags(input), "");
    }

    #[test]
    fn test_extract_tool_result_text_string() {
        let content = Some(RawToolResultContent::Text("hello".to_string()));
        assert_eq!(extract_tool_result_text(&content), "hello");
    }

    #[test]
    fn test_extract_tool_result_text_blocks() {
        let content = Some(RawToolResultContent::Blocks(vec![
            RawTextBlock {
                text: Some("first".to_string()),
            },
            RawTextBlock {
                text: Some("second".to_string()),
            },
        ]));
        assert_eq!(extract_tool_result_text(&content), "first\nsecond");
    }

    #[test]
    fn test_extract_tool_result_text_none() {
        assert_eq!(extract_tool_result_text(&None), "");
    }

    #[test]
    fn test_compute_relative_ts() {
        let start = "2026-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();

        // 5 seconds later
        let ts = Some("2026-01-01T00:00:05Z".to_string());
        assert_eq!(compute_relative_ts(&ts, &start), 5000);

        // No timestamp
        assert_eq!(compute_relative_ts(&None, &start), 0);

        // Before start (should clamp to 0)
        let ts = Some("2025-12-31T23:59:59Z".to_string());
        assert_eq!(compute_relative_ts(&ts, &start), 0);
    }

    #[test]
    fn test_extract_title_from_lines() {
        let lines = vec![
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text(
                        "<local-command-caveat>skip this</local-command-caveat>".to_string(),
                    )),
                }),
                timestamp: None,
                uuid: None,
                is_meta: true,
                tool_use_result: None,
            }),
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text("Fix the auth bug in login.py".to_string())),
                }),
                timestamp: None,
                uuid: None,
                is_meta: false,
                tool_use_result: None,
            }),
        ];
        assert_eq!(
            extract_title_from_lines(&lines),
            Some("Fix the auth bug in login.py".to_string())
        );
    }
}
