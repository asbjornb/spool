//! Codex CLI adapter.
//!
//! Parses Codex CLI session logs and converts them to Spool format.
//!
//! Codex stores sessions in:
//! - `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
//! - `~/.codex/history.jsonl` (aggregate history, not used here)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spool_format::{
    Entry, EntryId, PromptEntry, ResponseEntry, SessionEndState, SessionEntry, ThinkingEntry,
    ToolCallEntry, ToolOutput, ToolResultEntry,
};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{AgentType, SessionInfo};

// ============================================================================
// Public API
// ============================================================================

/// Find all Codex CLI sessions on the system.
pub fn find_sessions() -> Result<Vec<SessionInfo>> {
    let base_dir = get_codex_dir()?.join("sessions");

    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let pattern = format!("{}/**/*.jsonl", base_dir.display());

    for entry in glob::glob(&pattern)? {
        let path = match entry {
            Ok(path) => path,
            Err(_) => continue,
        };

        if !path.is_file() {
            continue;
        }

        let metadata = fs::metadata(&path).ok();
        let modified_at = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(DateTime::<Utc>::from);

        let (session_meta, first_prompt) = read_session_meta_and_prompt(&path)?;

        let created_at = session_meta
            .as_ref()
            .and_then(|m| parse_timestamp(&m.timestamp));

        let project_dir = session_meta
            .as_ref()
            .and_then(|m| m.cwd.as_ref())
            .map(PathBuf::from);

        let title = first_prompt
            .as_ref()
            .map(|p| truncate_first_prompt(p, 200))
            .filter(|t| !t.is_empty());

        sessions.push(SessionInfo {
            path,
            agent: AgentType::Codex,
            created_at,
            modified_at,
            title,
            project_dir,
            message_count: None,
        });
    }

    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(sessions)
}

/// Convert a Codex CLI session to Spool format.
pub fn convert(session: &SessionInfo) -> Result<spool_format::SpoolFile> {
    let raw_lines = read_raw_lines(&session.path)?;

    let mut session_meta: Option<RawSessionMeta> = None;
    let mut first_prompt_text: Option<String> = None;
    let mut session_start: Option<DateTime<Utc>> = None;
    let mut session_end: Option<DateTime<Utc>> = None;
    let mut last_model: Option<String> = None;

    for line in &raw_lines {
        if let Some(ts) = parse_timestamp(&line.timestamp) {
            session_start = match session_start {
                Some(current) if current <= ts => Some(current),
                _ => Some(ts),
            };
            session_end = match session_end {
                Some(current) if current >= ts => Some(current),
                _ => Some(ts),
            };
        }

        match line.kind.as_str() {
            "session_meta" => {
                if session_meta.is_none() {
                    session_meta =
                        serde_json::from_value::<RawSessionMeta>(line.payload.clone()).ok();
                }
            }
            "event_msg" => {
                if first_prompt_text.is_none() {
                    if let Ok(RawEventMsg::UserMessage { message, .. }) =
                        serde_json::from_value::<RawEventMsg>(line.payload.clone())
                    {
                        if !message.trim().is_empty() {
                            first_prompt_text = Some(message);
                        }
                    }
                }
            }
            "turn_context" => {
                if last_model.is_none() {
                    if let Ok(ctx) = serde_json::from_value::<RawTurnContext>(line.payload.clone())
                    {
                        last_model = ctx.model;
                    }
                }
            }
            _ => {}
        }
    }

    let session_start = session_start
        .or_else(|| {
            session_meta
                .as_ref()
                .and_then(|m| parse_timestamp(&m.timestamp))
        })
        .or(session.created_at)
        .or(session.modified_at)
        .unwrap_or_else(Utc::now);

    let mut extra = HashMap::new();
    if let Some(meta) = session_meta.as_ref() {
        if let Some(cwd) = meta.cwd.as_ref() {
            extra.insert("x_cwd".to_string(), serde_json::Value::String(cwd.clone()));
        }
        if let Some(originator) = meta.originator.as_ref() {
            extra.insert(
                "x_originator".to_string(),
                serde_json::Value::String(originator.clone()),
            );
        }
        if let Some(source) = meta.source.as_ref() {
            extra.insert(
                "x_source".to_string(),
                serde_json::Value::String(source.clone()),
            );
        }
        if let Some(provider) = meta.model_provider.as_ref() {
            extra.insert(
                "x_model_provider".to_string(),
                serde_json::Value::String(provider.clone()),
            );
        }
        if let Some(git) = meta.git.as_ref() {
            if let Ok(value) = serde_json::to_value(git) {
                extra.insert("x_git".to_string(), value);
            }
        }
    }
    if let Some(model) = last_model.as_ref() {
        extra.insert(
            "x_model".to_string(),
            serde_json::Value::String(model.clone()),
        );
    }

    let title = session.title.clone().or_else(|| {
        first_prompt_text
            .as_ref()
            .map(|t| truncate_first_prompt(t, 200))
    });

    let agent_version = session_meta.as_ref().and_then(|m| m.cli_version.clone());

    let session_entry = SessionEntry {
        id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
        ts: 0,
        version: "1.0".to_string(),
        agent: "codex".to_string(),
        recorded_at: session_start,
        agent_version,
        title,
        author: None,
        tags: None,
        duration_ms: None,
        entry_count: None,
        tools_used: None,
        files_modified: None,
        first_prompt: first_prompt_text.map(|t| truncate_first_prompt(&t, 200)),
        schema_url: None,
        trimmed: None,
        ended: Some(SessionEndState::Unknown),
        extra,
    };

    let mut entries = vec![Entry::Session(session_entry)];
    let mut tool_id_map: HashMap<String, EntryId> = HashMap::new();
    let mut tools_used: BTreeSet<String> = BTreeSet::new();
    let mut files_modified: BTreeSet<String> = BTreeSet::new();

    let mut current_model = last_model;

    for line in raw_lines {
        let ts = compute_relative_ts(&line.timestamp, &session_start);

        match line.kind.as_str() {
            "turn_context" => {
                if let Ok(ctx) = serde_json::from_value::<RawTurnContext>(line.payload) {
                    if ctx.model.is_some() {
                        current_model = ctx.model;
                    }
                }
            }
            "event_msg" => {
                if let Ok(event) = serde_json::from_value::<RawEventMsg>(line.payload) {
                    match event {
                        RawEventMsg::UserMessage { message, .. } => {
                            if !message.trim().is_empty() {
                                entries.push(Entry::Prompt(PromptEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    content: message,
                                    subagent_id: None,
                                    attachments: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        RawEventMsg::AgentMessage { message } => {
                            if !message.trim().is_empty() {
                                entries.push(Entry::Response(ResponseEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    content: message,
                                    truncated: None,
                                    original_bytes: None,
                                    model: current_model.clone(),
                                    token_usage: None,
                                    subagent_id: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        RawEventMsg::AgentReasoning { text } => {
                            if !text.trim().is_empty() {
                                entries.push(Entry::Thinking(ThinkingEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    content: text,
                                    collapsed: None,
                                    truncated: None,
                                    original_bytes: None,
                                    subagent_id: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        _ => {}
                    }
                }
            }
            "response_item" => {
                if let Ok(item) = serde_json::from_value::<RawResponseItem>(line.payload) {
                    match item {
                        RawResponseItem::FunctionCall {
                            name,
                            arguments,
                            call_id,
                        } => {
                            let input = parse_json_or_string(&arguments);
                            let entry_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
                            tools_used.insert(name.clone());
                            tool_id_map.insert(call_id, entry_id);
                            entries.push(Entry::ToolCall(ToolCallEntry {
                                id: entry_id,
                                ts,
                                tool: name,
                                input,
                                subagent_id: None,
                                extra: HashMap::new(),
                            }));
                        }
                        RawResponseItem::FunctionCallOutput { call_id, output } => {
                            if let Some(entry_id) = tool_id_map.get(&call_id).copied() {
                                entries.push(Entry::ToolResult(ToolResultEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    call_id: entry_id,
                                    output: Some(ToolOutput::Text(output)),
                                    error: None,
                                    truncated: None,
                                    original_bytes: None,
                                    subagent_id: None,
                                    redacted: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        RawResponseItem::CustomToolCall {
                            name,
                            input,
                            call_id,
                            ..
                        } => {
                            let entry_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
                            tools_used.insert(name.clone());
                            tool_id_map.insert(call_id, entry_id);
                            let input_value = parse_json_or_string(&input);
                            if name == "apply_patch" {
                                collect_patch_paths(&input, &mut files_modified);
                            }
                            entries.push(Entry::ToolCall(ToolCallEntry {
                                id: entry_id,
                                ts,
                                tool: name,
                                input: input_value,
                                subagent_id: None,
                                extra: HashMap::new(),
                            }));
                        }
                        RawResponseItem::CustomToolCallOutput { call_id, output } => {
                            if let Some(entry_id) = tool_id_map.get(&call_id).copied() {
                                entries.push(Entry::ToolResult(ToolResultEntry {
                                    id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                    ts,
                                    call_id: entry_id,
                                    output: Some(ToolOutput::Text(output)),
                                    error: None,
                                    truncated: None,
                                    original_bytes: None,
                                    subagent_id: None,
                                    redacted: None,
                                    extra: HashMap::new(),
                                }));
                            }
                        }
                        RawResponseItem::WebSearchCall { action, .. } => {
                            let entry_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
                            tools_used.insert("web_search".to_string());
                            entries.push(Entry::ToolCall(ToolCallEntry {
                                id: entry_id,
                                ts,
                                tool: "web_search".to_string(),
                                input: serde_json::to_value(action).unwrap_or_default(),
                                subagent_id: None,
                                extra: HashMap::new(),
                            }));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(end) = session_end {
        if let Ok(duration) = (end - session_start).to_std() {
            if let Entry::Session(ref mut entry) = entries[0] {
                entry.duration_ms = Some(duration.as_millis() as u64);
            }
        }
    }

    let entry_count = entries.len();
    if let Entry::Session(ref mut entry) = entries[0] {
        if !tools_used.is_empty() {
            entry.tools_used = Some(tools_used.into_iter().collect());
        }
        if !files_modified.is_empty() {
            entry.files_modified = Some(files_modified.into_iter().collect());
        }
        entry.entry_count = Some(entry_count);
    }

    Ok(spool_format::SpoolFile {
        session: match &entries[0] {
            Entry::Session(s) => s.clone(),
            _ => unreachable!(),
        },
        entries,
        unparsed_lines: Vec::new(),
    })
}

// ============================================================================
// Raw Codex JSONL format types
// ============================================================================

#[derive(Debug, Deserialize)]
struct RawLine {
    timestamp: String,
    #[serde(rename = "type")]
    kind: String,
    payload: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RawSessionMeta {
    id: Option<String>,
    timestamp: String,
    cwd: Option<String>,
    originator: Option<String>,
    cli_version: Option<String>,
    source: Option<String>,
    model_provider: Option<String>,
    base_instructions: Option<RawBaseInstructions>,
    git: Option<RawGitInfo>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RawBaseInstructions {
    text: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
struct RawGitInfo {
    commit_hash: Option<String>,
    branch: Option<String>,
    repository_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawResponseItem {
    #[serde(rename = "message")]
    Message {
        role: Option<String>,
        content: Option<Vec<RawContentItem>>,
    },
    #[serde(rename = "reasoning")]
    Reasoning {
        summary: Option<Vec<RawSummaryBlock>>,
        content: Option<String>,
        encrypted_content: Option<String>,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        name: String,
        arguments: String,
        call_id: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput { call_id: String, output: String },
    #[serde(rename = "custom_tool_call")]
    CustomToolCall {
        status: Option<String>,
        call_id: String,
        name: String,
        input: String,
    },
    #[serde(rename = "custom_tool_call_output")]
    CustomToolCallOutput { call_id: String, output: String },
    #[serde(rename = "web_search_call")]
    WebSearchCall {
        status: Option<String>,
        action: RawWebSearchAction,
    },
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawEventMsg {
    #[serde(rename = "user_message")]
    UserMessage {
        message: String,
        images: Option<Vec<serde_json::Value>>,
        local_images: Option<Vec<serde_json::Value>>,
        text_elements: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "agent_message")]
    AgentMessage { message: String },
    #[serde(rename = "agent_reasoning")]
    AgentReasoning { text: String },
    #[serde(rename = "turn_aborted")]
    TurnAborted { reason: Option<String> },
    #[serde(rename = "token_count")]
    TokenCount { info: Option<serde_json::Value> },
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
struct RawWebSearchAction {
    #[serde(rename = "type")]
    action_type: Option<String>,
    query: Option<String>,
    queries: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawContentItem {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RawSummaryBlock {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTurnContext {
    model: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

fn read_raw_lines(path: &Path) -> Result<Vec<RawLine>> {
    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("Failed to read line {}", idx + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: RawLine = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON line {}", idx + 1))?;
        lines.push(parsed);
    }

    Ok(lines)
}

fn read_session_meta_and_prompt(path: &Path) -> Result<(Option<RawSessionMeta>, Option<String>)> {
    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let reader = BufReader::new(file);
    let mut session_meta: Option<RawSessionMeta> = None;
    let mut first_prompt: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: RawLine = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        match parsed.kind.as_str() {
            "session_meta" => {
                if session_meta.is_none() {
                    session_meta = serde_json::from_value::<RawSessionMeta>(parsed.payload).ok();
                }
            }
            "event_msg" => {
                if first_prompt.is_none() {
                    if let Ok(RawEventMsg::UserMessage { message, .. }) =
                        serde_json::from_value::<RawEventMsg>(parsed.payload)
                    {
                        if !message.trim().is_empty() {
                            first_prompt = Some(message);
                        }
                    }
                }
            }
            _ => {}
        }

        if session_meta.is_some() && first_prompt.is_some() {
            break;
        }
    }

    Ok((session_meta, first_prompt))
}

fn compute_relative_ts(timestamp: &str, session_start: &DateTime<Utc>) -> u64 {
    let Some(ts) = parse_timestamp(timestamp) else {
        return 0;
    };
    if ts < *session_start {
        return 0;
    }
    (ts - *session_start).num_milliseconds() as u64
}

fn parse_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
    timestamp.parse::<DateTime<Utc>>().ok()
}

fn truncate_first_prompt(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    let mut end = max_bytes;
    while !text.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    let mut truncated = text[..end].to_string();
    truncated.push_str("...");
    truncated
}

fn parse_json_or_string(input: &str) -> serde_json::Value {
    serde_json::from_str(input).unwrap_or_else(|_| serde_json::Value::String(input.to_string()))
}

fn collect_patch_paths(patch: &str, files_modified: &mut BTreeSet<String>) {
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            files_modified.insert(path.trim().to_string());
        } else if let Some(path) = line.strip_prefix("*** Add File: ") {
            files_modified.insert(path.trim().to_string());
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            files_modified.insert(path.trim().to_string());
        }
    }
}

fn get_codex_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Failed to determine home directory")?;
    Ok(home.join(".codex"))
}
