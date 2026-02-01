//! Shared helpers for detecting agent logs and converting to Spool.

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
