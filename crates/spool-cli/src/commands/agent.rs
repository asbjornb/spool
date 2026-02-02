//! Shared helpers for detecting agent logs, discovering sessions, and converting to Spool.

use anyhow::{Context, Result};
use spool_adapters::{claude_code, codex, AgentType, SessionInfo};
use spool_format::SpoolFile;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn load_spool_or_log(path: &Path) -> Result<SpoolFile> {
    if path.extension().map(|e| e == "spool").unwrap_or(false) {
        return SpoolFile::from_path(path).with_context(|| format!("Failed to read: {:?}", path));
    }

    let agent = detect_agent_from_log(path)?;
    let session_info = SessionInfo {
        path: path.to_path_buf(),
        agent,
        created_at: None,
        modified_at: None,
        title: None,
        project_dir: None,
        message_count: None,
    };

    match agent {
        AgentType::ClaudeCode => claude_code::convert(&session_info)
            .with_context(|| format!("Failed to convert session: {:?}", path)),
        AgentType::Codex => codex::convert(&session_info)
            .with_context(|| format!("Failed to convert session: {:?}", path)),
        _ => anyhow::bail!("Unsupported agent log: {:?}", path),
    }
}

pub fn detect_agent_from_log(path: &Path) -> Result<AgentType> {
    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON line in {:?}", path))?;
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        return Ok(match kind {
            "session_meta" => AgentType::Codex,
            "user" | "assistant" | "progress" | "summary" | "system" => AgentType::ClaudeCode,
            _ => AgentType::Unknown,
        });
    }
    Ok(AgentType::Unknown)
}

/// Discover all sessions from all known agent log locations.
/// Returns sessions sorted by modified_at (newest first), filtering out empty sessions.
pub fn find_all_sessions() -> Result<Vec<SessionInfo>> {
    let mut sessions =
        claude_code::find_sessions().context("Failed to discover Claude Code sessions")?;
    sessions.extend(codex::find_sessions().context("Failed to discover Codex sessions")?);
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    sessions.retain(|s| s.message_count.map(|c| c > 0).unwrap_or(true));
    Ok(sessions)
}

/// Convert a SessionInfo into a SpoolFile using the appropriate adapter.
pub fn convert_session(session: &SessionInfo) -> Result<SpoolFile> {
    match session.agent {
        AgentType::ClaudeCode => claude_code::convert(session),
        AgentType::Codex => codex::convert(session),
        _ => anyhow::bail!("Unsupported agent: {}", session.agent.as_str()),
    }
}
