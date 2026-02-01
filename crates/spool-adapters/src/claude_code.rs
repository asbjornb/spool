//! Claude Code adapter.
//!
//! Parses Claude Code session logs and converts them to Spool format.
//!
//! ## Log Locations
//!
//! Claude Code stores sessions in:
//! - macOS: `~/.claude/projects/<project-hash>/sessions/`
//! - Linux: `~/.claude/projects/<project-hash>/sessions/`
//! - Windows: `%USERPROFILE%\.claude\projects\<project-hash>\sessions\`
//!
//! Each session is a JSON file with an array of conversation turns.

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
use std::path::PathBuf;
use uuid::Uuid;

/// Find all Claude Code sessions on the system.
pub fn find_sessions() -> Result<Vec<SessionInfo>> {
    let base_dir = get_claude_dir()?;
    let projects_dir = base_dir.join("projects");

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    // Iterate through project directories
    for project_entry in fs::read_dir(&projects_dir)? {
        let project_entry = project_entry?;
        let project_path = project_entry.path();

        if !project_path.is_dir() {
            continue;
        }

        let sessions_dir = project_path.join("sessions");
        if !sessions_dir.exists() {
            continue;
        }

        // Find session files
        for session_entry in fs::read_dir(&sessions_dir)? {
            let session_entry = session_entry?;
            let session_path = session_entry.path();

            if session_path.extension().map(|e| e == "json").unwrap_or(false) {
                let metadata = fs::metadata(&session_path)?;
                let modified_at = metadata.modified().ok().map(DateTime::<Utc>::from);
                let created_at = metadata.created().ok().map(DateTime::<Utc>::from);

                sessions.push(SessionInfo {
                    path: session_path,
                    agent: AgentType::ClaudeCode,
                    created_at,
                    modified_at,
                    title: None, // Could parse first prompt
                    project_dir: Some(project_path.clone()),
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
    let content = fs::read_to_string(&session.path)
        .with_context(|| format!("Failed to read session file: {:?}", session.path))?;

    let raw_session: RawClaudeSession = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse session file: {:?}", session.path))?;

    convert_raw_session(&raw_session, session)
}

/// Get the Claude Code base directory.
fn get_claude_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".claude"))
}

// ============================================================================
// Raw Claude Code format types (these may need adjustment based on actual format)
// ============================================================================

/// Raw Claude Code session format.
/// NOTE: This is a best-guess structure. Verify against actual Claude Code logs.
#[derive(Debug, Deserialize)]
struct RawClaudeSession {
    #[serde(default)]
    messages: Vec<RawMessage>,
    #[serde(default)]
    metadata: Option<RawSessionMetadata>,
}

#[derive(Debug, Deserialize)]
struct RawSessionMetadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    role: String,
    content: RawContent,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawContent {
    Text(String),
    Blocks(Vec<RawContentBlock>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Unknown,
}

// ============================================================================
// Conversion logic
// ============================================================================

fn convert_raw_session(raw: &RawClaudeSession, info: &SessionInfo) -> Result<spool_format::SpoolFile> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut ts: u64 = 0;
    let ts_increment: u64 = 1000; // 1 second between entries as placeholder

    // Track tool call IDs for mapping
    let mut tool_id_map: HashMap<String, EntryId> = HashMap::new();

    // Create session entry
    let session_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
    let recorded_at = info
        .created_at
        .or(info.modified_at)
        .unwrap_or_else(Utc::now);

    let title = raw
        .metadata
        .as_ref()
        .and_then(|m| m.title.clone())
        .or_else(|| extract_title_from_messages(&raw.messages));

    let session_entry = SessionEntry {
        id: session_id,
        ts: 0,
        version: "1.0".to_string(),
        agent: "claude-code".to_string(),
        recorded_at,
        agent_version: None, // Could try to detect
        title,
        author: None,
        tags: None,
        duration_ms: None, // Will be updated after processing
        entry_count: None, // Will be updated after processing
        tools_used: None,  // Will be updated after processing
        schema_url: None,
        trimmed: None,
        ended: Some(spool_format::SessionEndState::Unknown),
        extra: HashMap::new(),
    };
    entries.push(Entry::Session(session_entry));

    // Process messages
    for message in &raw.messages {
        match message.role.as_str() {
            "user" | "human" => {
                let content = extract_text_content(&message.content);
                if !content.is_empty() {
                    entries.push(Entry::Prompt(PromptEntry {
                        id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                        ts,
                        content,
                        subagent_id: None,
                        attachments: None,
                        extra: HashMap::new(),
                    }));
                    ts += ts_increment;
                }
            }
            "assistant" => {
                // Process content blocks
                match &message.content {
                    RawContent::Text(text) => {
                        if !text.is_empty() {
                            entries.push(Entry::Response(ResponseEntry {
                                id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                ts,
                                content: text.clone(),
                                truncated: None,
                                original_bytes: None,
                                subagent_id: None,
                                extra: HashMap::new(),
                            }));
                            ts += ts_increment;
                        }
                    }
                    RawContent::Blocks(blocks) => {
                        for block in blocks {
                            match block {
                                RawContentBlock::Text { text } => {
                                    if !text.is_empty() {
                                        entries.push(Entry::Response(ResponseEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            content: text.clone(),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            extra: HashMap::new(),
                                        }));
                                        ts += ts_increment;
                                    }
                                }
                                RawContentBlock::Thinking { thinking } => {
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
                                    ts += ts_increment;
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
                                    ts += ts_increment;
                                }
                                RawContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } => {
                                    let call_id = tool_id_map
                                        .get(tool_use_id)
                                        .copied()
                                        .unwrap_or_else(Uuid::nil);

                                    let entry = if *is_error {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            call_id,
                                            output: None,
                                            error: Some(content.clone()),
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    } else {
                                        ToolResultEntry {
                                            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
                                            ts,
                                            call_id,
                                            output: Some(ToolOutput::Text(content.clone())),
                                            error: None,
                                            truncated: None,
                                            original_bytes: None,
                                            subagent_id: None,
                                            redacted: None,
                                            extra: HashMap::new(),
                                        }
                                    };
                                    entries.push(Entry::ToolResult(entry));
                                    ts += ts_increment;
                                }
                                RawContentBlock::Unknown => {}
                            }
                        }
                    }
                }
            }
            _ => {} // Ignore unknown roles
        }
    }

    // Create the SpoolFile
    let session = match &entries[0] {
        Entry::Session(s) => s.clone(),
        _ => unreachable!(),
    };

    let mut file = spool_format::SpoolFile {
        session,
        entries,
        unparsed_lines: Vec::new(),
    };

    // Update session metadata
    let entry_count = file.entries.len();
    let tools_used = file.tools_used();
    if let Entry::Session(ref mut s) = file.entries[0] {
        s.duration_ms = Some(ts);
        s.entry_count = Some(entry_count);
        s.tools_used = Some(tools_used);
    }
    file.session = match &file.entries[0] {
        Entry::Session(s) => s.clone(),
        _ => unreachable!(),
    };

    Ok(file)
}

fn extract_text_content(content: &RawContent) -> String {
    match content {
        RawContent::Text(text) => text.clone(),
        RawContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                RawContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn extract_title_from_messages(messages: &[RawMessage]) -> Option<String> {
    // Use first user message as title, truncated
    for message in messages {
        if message.role == "user" || message.role == "human" {
            let content = extract_text_content(&message.content);
            if !content.is_empty() {
                let title = content.lines().next().unwrap_or(&content);
                let title = if title.len() > 60 {
                    format!("{}...", &title[..57])
                } else {
                    title.to_string()
                };
                return Some(title);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_content_simple() {
        let content = RawContent::Text("Hello, world!".to_string());
        assert_eq!(extract_text_content(&content), "Hello, world!");
    }

    #[test]
    fn test_extract_text_content_blocks() {
        let content = RawContent::Blocks(vec![
            RawContentBlock::Text {
                text: "First".to_string(),
            },
            RawContentBlock::Thinking {
                thinking: "Ignored".to_string(),
            },
            RawContentBlock::Text {
                text: "Second".to_string(),
            },
        ]);
        assert_eq!(extract_text_content(&content), "First\nSecond");
    }

    #[test]
    fn test_extract_title_truncation() {
        let title = "This is a very long title that should be truncated because it exceeds sixty characters";
        let messages = vec![RawMessage {
            role: "user".to_string(),
            content: RawContent::Text(title.to_string()),
            timestamp: None,
        }];
        let extracted = extract_title_from_messages(&messages);
        assert!(extracted.is_some());
        assert!(extracted.unwrap().len() <= 63); // 60 + "..."
    }
}
