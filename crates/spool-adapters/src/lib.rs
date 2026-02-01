//! # spool-adapters
//!
//! Agent log parsers for converting native agent formats to Spool format.
//!
//! This crate provides adapters for various AI coding agents:
//! - Claude Code
//! - Codex CLI (planned)
//! - Cursor (planned)
//! - Aider (planned)
//!
//! ## Example
//!
//! ```rust,no_run
//! use spool_adapters::{claude_code, SessionInfo};
//!
//! // Find all Claude Code sessions
//! let sessions = claude_code::find_sessions()?;
//!
//! // Convert a session to Spool format
//! let spool_file = claude_code::convert(&sessions[0])?;
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod claude_code;
// pub mod codex; // TODO
// pub mod cursor; // TODO
// pub mod aider; // TODO

use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// Information about a discovered agent session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Path to the session log file
    pub path: PathBuf,
    /// Agent that created this session
    pub agent: AgentType,
    /// When the session was created
    pub created_at: Option<DateTime<Utc>>,
    /// When the session was last modified
    pub modified_at: Option<DateTime<Utc>>,
    /// Session title if available
    pub title: Option<String>,
    /// Project directory if known
    pub project_dir: Option<PathBuf>,
    /// Number of messages in the session (from index), if known
    pub message_count: Option<usize>,
}

/// Supported agent types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    ClaudeCode,
    Codex,
    Cursor,
    Aider,
    GithubCopilot,
    Unknown,
}

impl AgentType {
    /// Get the string identifier for this agent.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentType::ClaudeCode => "claude-code",
            AgentType::Codex => "codex",
            AgentType::Cursor => "cursor",
            AgentType::Aider => "aider",
            AgentType::GithubCopilot => "github-copilot",
            AgentType::Unknown => "unknown",
        }
    }
}

/// Trait for agent adapters.
pub trait Adapter {
    /// Find all sessions for this agent.
    fn find_sessions() -> anyhow::Result<Vec<SessionInfo>>;

    /// Convert a session to Spool format.
    fn convert(session: &SessionInfo) -> anyhow::Result<spool_format::SpoolFile>;
}
