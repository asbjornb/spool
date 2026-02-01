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
    Entry, EntryId, PromptEntry, ResponseEntry, SessionEntry, SubagentEndEntry, SubagentStartEntry,
    SubagentStatus, ThinkingEntry, TokenUsage, ToolCallEntry, ToolOutput, ToolResultEntry,
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
                // Skip subagent files (agent-*.jsonl). This also covers
                // prompt-suggestion files (agent-aprompt_suggestion-*.jsonl).
                let filename = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
struct RawAssistantLine {
    #[serde(default)]
    message: Option<RawApiMessage>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
struct RawProgressLine {
    #[serde(default)]
    data: Option<serde_json::Value>,
}

/// The `message` field inside a user line.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<RawContent>,
}

/// The `message` field inside an assistant line.
/// This matches the Anthropic API response format.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawApiMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    content: Option<Vec<RawContentBlock>>,
    #[serde(default)]
    usage: Option<RawUsage>,
    #[serde(default)]
    stop_reason: Option<String>,
}

/// Token usage from the Anthropic API response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
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
#[allow(dead_code)]
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

fn convert_raw_lines(raw_lines: &[RawLine], info: &SessionInfo) -> Result<spool_format::SpoolFile> {
    let mut entries: Vec<Entry> = Vec::new();

    // Track tool call IDs: Claude's tool_use id -> our spool EntryId
    let mut tool_id_map: HashMap<String, EntryId> = HashMap::new();
    // Track Task tool calls: Claude's tool_use id -> SubagentStart EntryId
    let mut task_subagent_map: HashMap<String, EntryId> = HashMap::new();

    // Parse timestamps to compute relative ms
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut summary_text: Option<String> = None;
    let agent_version: Option<String> = None;
    let mut model_name: Option<String> = None;
    let mut first_prompt_text: Option<String> = None;

    // First pass: find metadata
    for line in raw_lines {
        match line {
            RawLine::Summary(s) => {
                if summary_text.is_none() {
                    summary_text = s.summary.clone();
                }
            }
            RawLine::User(u) => {
                if !u.is_meta {
                    if first_timestamp.is_none() {
                        if let Some(ref ts) = u.timestamp {
                            first_timestamp = ts.parse::<DateTime<Utc>>().ok();
                        }
                    }
                    // Extract first prompt text
                    if first_prompt_text.is_none() {
                        if let Some(ref msg) = u.message {
                            if let Some(RawContent::Text(ref text)) = msg.content {
                                if !text.contains("<command-name>")
                                    && !text.contains("<local-command-stdout>")
                                    && !text.contains("<local-command-caveat>")
                                {
                                    let clean = strip_system_tags(text);
                                    if !clean.is_empty() {
                                        first_prompt_text =
                                            Some(truncate_first_prompt(&clean, 200));
                                    }
                                }
                            }
                        }
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
        files_modified: None,
        first_prompt: first_prompt_text,
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
                                    let tool_use_id = block.tool_use_id.as_deref().unwrap_or("");
                                    let call_id = tool_id_map
                                        .get(tool_use_id)
                                        .copied()
                                        .unwrap_or_else(Uuid::nil);
                                    let is_error = block.is_error.unwrap_or(false);
                                    let content_text = extract_tool_result_text(&block.content);

                                    // Check if this tool result corresponds to a Task subagent
                                    let subagent_start_id =
                                        task_subagent_map.get(tool_use_id).copied();

                                    let entry = if is_error {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            call_id,
                                            output: None,
                                            error: Some(content_text),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: subagent_start_id,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    } else {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            call_id,
                                            output: Some(ToolOutput::Text(content_text)),
                                            error: None,
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: subagent_start_id,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    };
                                    entries.push(Entry::ToolResult(entry));

                                    // Emit SubagentEnd after the ToolResult for Task calls
                                    if let Some(start_id) = subagent_start_id {
                                        let status = if is_error {
                                            Some(SubagentStatus::Failed)
                                        } else {
                                            Some(SubagentStatus::Completed)
                                        };
                                        entries.push(Entry::SubagentEnd(SubagentEndEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            start_id,
                                            summary: None,
                                            status,
                                            extra: HashMap::new(),
                                        }));
                                    }
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
                    // Extract model and token_usage once per message.
                    // Attach to the first ResponseEntry to avoid double-counting.
                    let msg_model = msg.model.clone();
                    let msg_token_usage = msg.usage.as_ref().and_then(|u| {
                        // Require at least input or output tokens to be present
                        match (u.input_tokens, u.output_tokens) {
                            (Some(inp), Some(out)) => Some(TokenUsage {
                                input_tokens: inp,
                                output_tokens: out,
                                cache_read_tokens: u.cache_read_input_tokens,
                                cache_creation_tokens: u.cache_creation_input_tokens,
                            }),
                            _ => None,
                        }
                    });
                    let mut first_response_emitted = false;

                    if let Some(ref blocks) = msg.content {
                        for block in blocks {
                            match block {
                                RawContentBlock::Text { text } => {
                                    if !text.is_empty() {
                                        let (model, token_usage) = if !first_response_emitted {
                                            first_response_emitted = true;
                                            (msg_model.clone(), msg_token_usage.clone())
                                        } else {
                                            (None, None)
                                        };
                                        entries.push(Entry::Response(ResponseEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            content: text.clone(),
                                            truncated: None,
                                            original_bytes: None,
                                            model,
                                            token_usage,
                                            subagent_id: None,
                                            extra: HashMap::new(),
                                        }));
                                    }
                                }
                                RawContentBlock::Thinking { thinking, .. } => {
                                    if !thinking.is_empty() {
                                        entries.push(Entry::Thinking(ThinkingEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
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

                                    // For Task tool calls, emit a SubagentStart entry
                                    let subagent_id = if name == "Task" {
                                        let subagent_type = input
                                            .get("subagent_type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let description = input
                                            .get("description")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());

                                        let start_id =
                                            Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
                                        entries.push(Entry::SubagentStart(SubagentStartEntry {
                                            id: start_id,
                                            ts,
                                            agent: subagent_type,
                                            context: description,
                                            parent_subagent_id: None,
                                            extra: HashMap::new(),
                                        }));

                                        task_subagent_map.insert(id.clone(), start_id);
                                        Some(start_id)
                                    } else {
                                        None
                                    };

                                    entries.push(Entry::ToolCall(ToolCallEntry {
                                        id: entry_id,
                                        ts,
                                        tool: name.clone(),
                                        input: input.clone(),
                                        subagent_id,
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

    // Collect files modified by file-writing tool calls
    let files_modified = {
        let mut paths: Vec<String> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::ToolCall(tc) => extract_modified_path(&tc.tool, &tc.input),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        paths.sort();
        paths
    };

    // Update session entry with computed metadata
    if let Entry::Session(ref mut s) = entries[0] {
        s.duration_ms = Some(last_ts);
        s.entry_count = Some(entry_count);
        if !tools_used.is_empty() {
            s.tools_used = Some(tools_used);
        }
        if !files_modified.is_empty() {
            s.files_modified = Some(files_modified);
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

/// Extract a file path from a tool call if the tool modifies files.
///
/// Recognizes Claude Code tool names that modify files:
/// - `Write`, `write`, `write_file` — `file_path` or `path` parameter
/// - `Edit`, `edit`, `edit_file` — `file_path` or `path` parameter
/// - `NotebookEdit`, `notebook_edit` — `notebook_path` parameter
fn extract_modified_path(tool: &str, input: &serde_json::Value) -> Option<String> {
    match tool {
        "Write" | "write" | "write_file" | "Edit" | "edit" | "edit_file" => input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "NotebookEdit" | "notebook_edit" => input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Truncate text for the first_prompt field, respecting UTF-8 boundaries.
fn truncate_first_prompt(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
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
                            let mut end = 57;
                            while end > 0 && !first_line.is_char_boundary(end) {
                                end -= 1;
                            }
                            format!("{}...", &first_line[..end])
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

    #[test]
    fn test_extract_title_cjk_over_60_bytes() {
        // Each CJK char is 3 bytes. 21 chars = 63 bytes > 60.
        let cjk_title = "这是一个很长的中文标题用来测试多字节字符截断功能是否正常";
        assert!(cjk_title.len() > 60);

        let lines = vec![RawLine::User(RawUserLine {
            message: Some(RawMessage {
                role: Some("user".to_string()),
                content: Some(RawContent::Text(cjk_title.to_string())),
            }),
            timestamp: None,
            uuid: None,
            is_meta: false,
            tool_use_result: None,
        })];
        let result = extract_title_from_lines(&lines).unwrap();
        assert!(result.ends_with("..."));
        // Should not panic and should be valid UTF-8
        assert!(result.len() <= 60);
    }

    #[test]
    fn test_extract_title_arrow_at_byte_57() {
        // Place '→' (3 bytes) starting at byte 56: "x"*56 + "→" + "extra" = 62 bytes > 60
        let title = format!("{}→extra_text_here", "x".repeat(56));
        assert!(title.len() > 60);

        let lines = vec![RawLine::User(RawUserLine {
            message: Some(RawMessage {
                role: Some("user".to_string()),
                content: Some(RawContent::Text(title)),
            }),
            timestamp: None,
            uuid: None,
            is_meta: false,
            tool_use_result: None,
        })];
        let result = extract_title_from_lines(&lines).unwrap();
        assert!(result.ends_with("..."));
        // The '→' starts at byte 56 and ends at 59. Truncating at 57 would be mid-char.
        // The fix should walk back to 56.
        assert!(result.is_char_boundary(result.len() - 3)); // before "..."
    }

    #[test]
    fn test_model_and_token_usage_extraction() {
        let lines = vec![
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text("Hello".to_string())),
                }),
                timestamp: Some("2026-01-01T00:00:00Z".to_string()),
                uuid: None,
                is_meta: false,
                tool_use_result: None,
            }),
            RawLine::Assistant(RawAssistantLine {
                message: Some(RawApiMessage {
                    model: Some("claude-sonnet-4-20250514".to_string()),
                    content: Some(vec![
                        RawContentBlock::Text {
                            text: "First block".to_string(),
                        },
                        RawContentBlock::Text {
                            text: "Second block".to_string(),
                        },
                    ]),
                    usage: Some(RawUsage {
                        input_tokens: Some(100),
                        output_tokens: Some(50),
                        cache_read_input_tokens: Some(80),
                        cache_creation_input_tokens: None,
                    }),
                    stop_reason: Some("end_turn".to_string()),
                }),
                timestamp: Some("2026-01-01T00:00:05Z".to_string()),
                uuid: None,
            }),
        ];

        let info = SessionInfo {
            path: PathBuf::from("/tmp/test.jsonl"),
            agent: AgentType::ClaudeCode,
            created_at: Some("2026-01-01T00:00:00Z".parse().unwrap()),
            modified_at: None,
            title: None,
            project_dir: None,
            message_count: None,
        };

        let spool = convert_raw_lines(&lines, &info).unwrap();

        // Find all response entries
        let responses: Vec<&spool_format::ResponseEntry> = spool
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Response(r) => Some(r),
                _ => None,
            })
            .collect();

        assert_eq!(responses.len(), 2);

        // First response should have model and token_usage
        assert_eq!(
            responses[0].model.as_deref(),
            Some("claude-sonnet-4-20250514")
        );
        let usage = responses[0].token_usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_read_tokens, Some(80));
        assert_eq!(usage.cache_creation_tokens, None);

        // Second response should NOT have model or token_usage (avoid double-counting)
        assert!(responses[1].model.is_none());
        assert!(responses[1].token_usage.is_none());
    }

    #[test]
    fn test_first_prompt_extraction() {
        let lines = vec![
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text(
                        "<local-command-caveat>skip</local-command-caveat>skip this".to_string(),
                    )),
                }),
                timestamp: Some("2026-01-01T00:00:00Z".to_string()),
                uuid: None,
                is_meta: false,
                tool_use_result: None,
            }),
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text(
                        "Fix the authentication bug in login.py".to_string(),
                    )),
                }),
                timestamp: Some("2026-01-01T00:00:01Z".to_string()),
                uuid: None,
                is_meta: false,
                tool_use_result: None,
            }),
            RawLine::Assistant(RawAssistantLine {
                message: Some(RawApiMessage {
                    model: None,
                    content: Some(vec![RawContentBlock::Text {
                        text: "I'll fix that.".to_string(),
                    }]),
                    usage: None,
                    stop_reason: None,
                }),
                timestamp: Some("2026-01-01T00:00:02Z".to_string()),
                uuid: None,
            }),
        ];

        let info = SessionInfo {
            path: PathBuf::from("/tmp/test.jsonl"),
            agent: AgentType::ClaudeCode,
            created_at: Some("2026-01-01T00:00:00Z".parse().unwrap()),
            modified_at: None,
            title: None,
            project_dir: None,
            message_count: None,
        };

        let spool = convert_raw_lines(&lines, &info).unwrap();
        assert_eq!(
            spool.session.first_prompt.as_deref(),
            Some("Fix the authentication bug in login.py")
        );
    }

    #[test]
    fn test_files_modified_extraction() {
        let lines = vec![
            RawLine::User(RawUserLine {
                message: Some(RawMessage {
                    role: Some("user".to_string()),
                    content: Some(RawContent::Text("Edit my files".to_string())),
                }),
                timestamp: Some("2026-01-01T00:00:00Z".to_string()),
                uuid: None,
                is_meta: false,
                tool_use_result: None,
            }),
            RawLine::Assistant(RawAssistantLine {
                message: Some(RawApiMessage {
                    model: None,
                    content: Some(vec![
                        RawContentBlock::ToolUse {
                            id: "tool1".to_string(),
                            name: "Write".to_string(),
                            input: serde_json::json!({
                                "file_path": "/home/user/src/main.rs",
                                "content": "fn main() {}"
                            }),
                        },
                        RawContentBlock::ToolUse {
                            id: "tool2".to_string(),
                            name: "Edit".to_string(),
                            input: serde_json::json!({
                                "file_path": "/home/user/src/lib.rs",
                                "old_string": "old",
                                "new_string": "new"
                            }),
                        },
                        RawContentBlock::ToolUse {
                            id: "tool3".to_string(),
                            name: "Read".to_string(),
                            input: serde_json::json!({
                                "file_path": "/home/user/src/other.rs"
                            }),
                        },
                        // Duplicate Write to same file — should be deduplicated
                        RawContentBlock::ToolUse {
                            id: "tool4".to_string(),
                            name: "Write".to_string(),
                            input: serde_json::json!({
                                "file_path": "/home/user/src/main.rs",
                                "content": "fn main() { println!() }"
                            }),
                        },
                        RawContentBlock::ToolUse {
                            id: "tool5".to_string(),
                            name: "NotebookEdit".to_string(),
                            input: serde_json::json!({
                                "notebook_path": "/home/user/notebook.ipynb",
                                "new_source": "print('hello')"
                            }),
                        },
                    ]),
                    usage: None,
                    stop_reason: None,
                }),
                timestamp: Some("2026-01-01T00:00:05Z".to_string()),
                uuid: None,
            }),
        ];

        let info = SessionInfo {
            path: PathBuf::from("/tmp/test.jsonl"),
            agent: AgentType::ClaudeCode,
            created_at: Some("2026-01-01T00:00:00Z".parse().unwrap()),
            modified_at: None,
            title: None,
            project_dir: None,
            message_count: None,
        };

        let spool = convert_raw_lines(&lines, &info).unwrap();
        let files = spool.session.files_modified.unwrap();

        // Should contain Write, Edit, NotebookEdit targets but not Read
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"/home/user/notebook.ipynb".to_string()));
        assert!(files.contains(&"/home/user/src/lib.rs".to_string()));
        assert!(files.contains(&"/home/user/src/main.rs".to_string()));
        // Should NOT contain the Read target
        assert!(!files.contains(&"/home/user/src/other.rs".to_string()));
    }

    #[test]
    fn test_extract_modified_path() {
        // Write tool
        let path = extract_modified_path(
            "Write",
            &serde_json::json!({"file_path": "/src/main.rs", "content": ""}),
        );
        assert_eq!(path.as_deref(), Some("/src/main.rs"));

        // Edit tool
        let path = extract_modified_path(
            "Edit",
            &serde_json::json!({"file_path": "/src/lib.rs", "old_string": "a", "new_string": "b"}),
        );
        assert_eq!(path.as_deref(), Some("/src/lib.rs"));

        // NotebookEdit tool
        let path = extract_modified_path(
            "NotebookEdit",
            &serde_json::json!({"notebook_path": "/nb.ipynb", "new_source": "x"}),
        );
        assert_eq!(path.as_deref(), Some("/nb.ipynb"));

        // Read tool — not a write operation
        let path = extract_modified_path("Read", &serde_json::json!({"file_path": "/src/main.rs"}));
        assert!(path.is_none());

        // Bash — not tracked
        let path = extract_modified_path(
            "Bash",
            &serde_json::json!({"command": "echo hello > file.txt"}),
        );
        assert!(path.is_none());
    }

    #[test]
    fn test_truncate_first_prompt() {
        // Short text — no truncation
        assert_eq!(truncate_first_prompt("hello", 200), "hello");

        // Exactly at limit
        let text = "x".repeat(200);
        assert_eq!(truncate_first_prompt(&text, 200), text);

        // Over limit
        let text = "x".repeat(210);
        let result = truncate_first_prompt(&text, 200);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 203); // 200 + "..."

        // Multi-byte at boundary
        let text = format!("{}→end", "x".repeat(199)); // → is 3 bytes, starts at 199
        let result = truncate_first_prompt(&text, 200);
        assert!(result.ends_with("..."));
        assert!(result.is_char_boundary(result.len() - 3));
    }
}
