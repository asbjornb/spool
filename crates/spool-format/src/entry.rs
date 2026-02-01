//! Entry types for the Spool format.
//!
//! Every entry in a Spool file has a unique ID, timestamp, and type.
//! This module defines all entry types as specified in SPEC.md.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

mod schema {
    use schemars::schema::{InstanceType, ObjectValidation, Schema, SchemaObject};
    use schemars::SchemaGenerator;

    pub fn any_value_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Bool(true)
    }

    pub fn extras_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                additional_properties: Some(Box::new(Schema::Bool(true))),
                ..ObjectValidation::default()
            })),
            ..SchemaObject::default()
        })
    }
}

/// A unique identifier for entries.
/// Should be UUID v7 (time-ordered) or v4 (random).
pub type EntryId = Uuid;

/// Timestamp in milliseconds since session start.
pub type Timestamp = u64;

/// An entry in a Spool file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Entry {
    /// Session metadata (must be first entry)
    Session(SessionEntry),
    /// User prompt to the agent
    Prompt(PromptEntry),
    /// Agent's internal reasoning
    Thinking(ThinkingEntry),
    /// Tool invocation by the agent
    ToolCall(ToolCallEntry),
    /// Result from a tool invocation
    ToolResult(ToolResultEntry),
    /// Agent's response to the user
    Response(ResponseEntry),
    /// Error during the session
    Error(ErrorEntry),
    /// Start of a subagent invocation
    SubagentStart(SubagentStartEntry),
    /// End of a subagent invocation
    SubagentEnd(SubagentEndEntry),
    /// Annotation added during editing
    Annotation(AnnotationEntry),
    /// Marker indicating redaction occurred
    RedactionMarker(RedactionMarkerEntry),
    /// Unknown entry type (for forward compatibility)
    #[serde(other)]
    #[schemars(skip)]
    Unknown,
}

impl Entry {
    /// Get the entry's ID
    pub fn id(&self) -> Option<&EntryId> {
        match self {
            Entry::Session(e) => Some(&e.id),
            Entry::Prompt(e) => Some(&e.id),
            Entry::Thinking(e) => Some(&e.id),
            Entry::ToolCall(e) => Some(&e.id),
            Entry::ToolResult(e) => Some(&e.id),
            Entry::Response(e) => Some(&e.id),
            Entry::Error(e) => Some(&e.id),
            Entry::SubagentStart(e) => Some(&e.id),
            Entry::SubagentEnd(e) => Some(&e.id),
            Entry::Annotation(e) => Some(&e.id),
            Entry::RedactionMarker(e) => Some(&e.id),
            Entry::Unknown => None,
        }
    }

    /// Get the entry's timestamp
    pub fn timestamp(&self) -> Option<Timestamp> {
        match self {
            Entry::Session(e) => Some(e.ts),
            Entry::Prompt(e) => Some(e.ts),
            Entry::Thinking(e) => Some(e.ts),
            Entry::ToolCall(e) => Some(e.ts),
            Entry::ToolResult(e) => Some(e.ts),
            Entry::Response(e) => Some(e.ts),
            Entry::Error(e) => Some(e.ts),
            Entry::SubagentStart(e) => Some(e.ts),
            Entry::SubagentEnd(e) => Some(e.ts),
            Entry::Annotation(e) => Some(e.ts),
            Entry::RedactionMarker(e) => Some(e.ts),
            Entry::Unknown => None,
        }
    }
}

/// Session metadata entry. Must be the first entry in every Spool file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SessionEntry {
    /// Unique identifier
    pub id: EntryId,
    /// Timestamp (must be 0)
    pub ts: Timestamp,
    /// Format version (e.g., "1.0")
    pub version: String,
    /// Agent identifier (e.g., "claude-code", "codex")
    pub agent: String,
    /// Wall-clock time when recording started
    pub recorded_at: DateTime<Utc>,

    // Optional fields
    /// Version of the agent software
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_version: Option<String>,
    /// Human-readable session title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Creator's name or handle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Searchable tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Total session duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Total number of entries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_count: Option<usize>,
    /// Tool names invoked during session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_used: Option<Vec<String>>,
    /// File paths modified during the session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_modified: Option<Vec<String>>,
    /// First user prompt text (truncated), useful for browsing/indexing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    /// URL to the specification version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    /// Trimming metadata if file was trimmed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trimmed: Option<TrimmedMetadata>,
    /// How the session ended
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<SessionEndState>,

    /// Extension fields (prefixed with x_)
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Metadata about trimming
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TrimmedMetadata {
    /// Duration of the original recording in milliseconds
    pub original_duration_ms: u64,
    /// Kept range as [start_ms, end_ms]
    pub kept_range: (u64, u64),
}

/// How a session ended
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndState {
    Completed,
    Cancelled,
    Error,
    Timeout,
    Unknown,
}

/// User prompt entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct PromptEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// File attachment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Attachment {
    /// Must be "binary" for binary content
    #[serde(rename = "type")]
    pub attachment_type: String,
    /// MIME type of the content
    pub media_type: String,
    /// Encoding (must be "base64")
    pub encoding: String,
    /// Base64-encoded content
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<usize>,
}

/// Agent thinking entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ThinkingEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Tool call entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ToolCallEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub tool: String,
    #[schemars(schema_with = "schema::any_value_schema")]
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Tool result entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ToolResultEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    /// References the tool_call entry
    pub call_id: EntryId,
    /// Tool output (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<ToolOutput>,
    /// Error message (mutually exclusive with output)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    /// Inline redaction metadata
    #[serde(rename = "_redacted", skip_serializing_if = "Option::is_none")]
    pub redacted: Option<Vec<RedactionInfo>>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Tool output can be a string or binary data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum ToolOutput {
    Text(String),
    Binary(BinaryContent),
}

/// Binary content with base64 encoding
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct BinaryContent {
    #[serde(rename = "type")]
    pub content_type: String, // "binary"
    pub media_type: String,
    pub encoding: String, // "base64"
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

/// Inline redaction information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RedactionInfo {
    pub reason: RedactionReason,
    pub count: usize,
}

/// Token usage information for an API response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
}

/// Agent response entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ResponseEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Error entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ErrorEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "schema::any_value_schema")]
    pub details: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<EntryId>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Standard error codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    RateLimit,
    ApiError,
    Timeout,
    AuthFailed,
    NetworkError,
    ContextOverflow,
    Cancelled,
    InternalError,
    Unknown,
    /// Custom error code
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::RateLimit => write!(f, "rate_limit"),
            ErrorCode::ApiError => write!(f, "api_error"),
            ErrorCode::Timeout => write!(f, "timeout"),
            ErrorCode::AuthFailed => write!(f, "auth_failed"),
            ErrorCode::NetworkError => write!(f, "network_error"),
            ErrorCode::ContextOverflow => write!(f, "context_overflow"),
            ErrorCode::Cancelled => write!(f, "cancelled"),
            ErrorCode::InternalError => write!(f, "internal_error"),
            ErrorCode::Unknown => write!(f, "unknown"),
            ErrorCode::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Subagent start entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SubagentStartEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_subagent_id: Option<EntryId>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Subagent end entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SubagentEndEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub start_id: EntryId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<SubagentStatus>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Subagent completion status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Completed,
    Failed,
    Cancelled,
}

/// Annotation entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct AnnotationEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub target_id: EntryId,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<AnnotationStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Annotation display style
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationStyle {
    Highlight,
    Comment,
    Pin,
    Warning,
    Success,
}

/// Redaction marker entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RedactionMarkerEntry {
    pub id: EntryId,
    pub ts: Timestamp,
    pub target_id: EntryId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<RedactionReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<bool>,
    #[serde(flatten)]
    #[schemars(schema_with = "schema::extras_schema")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Reason for redaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionReason {
    ApiKey,
    Password,
    Email,
    Phone,
    Path,
    IpAddress,
    Pii,
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_entry() {
        let json = r#"{"id":"018d5f2c-0000-7000-8000-000000000000","ts":0,"type":"session","version":"1.0","agent":"claude-code","recorded_at":"2025-01-31T10:30:00Z"}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        match entry {
            Entry::Session(s) => {
                assert_eq!(s.version, "1.0");
                assert_eq!(s.agent, "claude-code");
            }
            _ => panic!("Expected Session entry"),
        }
    }

    #[test]
    fn test_parse_prompt_entry() {
        let json = r#"{"id":"018d5f2c-0000-7000-8000-000000000001","ts":0,"type":"prompt","content":"Hello, world!"}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        match entry {
            Entry::Prompt(p) => {
                assert_eq!(p.content, "Hello, world!");
            }
            _ => panic!("Expected Prompt entry"),
        }
    }

    #[test]
    fn test_unknown_entry_type_preserved() {
        let json = r#"{"id":"018d5f2c-0000-7000-8000-000000000001","ts":100,"type":"x_future_type","data":"unknown"}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();

        assert!(matches!(entry, Entry::Unknown));
    }

    #[test]
    fn test_round_trip_with_extra_fields() {
        let json = r#"{"id":"018d5f2c-0000-7000-8000-000000000001","ts":0,"type":"prompt","content":"Hello","x_custom":"value"}"#;
        let entry: Entry = serde_json::from_str(json).unwrap();
        let output = serde_json::to_string(&entry).unwrap();

        // Extra fields should be preserved
        assert!(output.contains("x_custom"));
    }
}
