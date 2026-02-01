//! Error types for Spool format operations.

use thiserror::Error;

/// Errors that can occur when working with Spool files.
#[derive(Debug, Error)]
pub enum SpoolError {
    /// File I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error on line {line}: {message}")]
    Json {
        line: usize,
        message: String,
        #[source]
        source: serde_json::Error,
    },

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    /// Missing required session entry
    #[error("File must start with a session entry")]
    MissingSessionEntry,

    /// Invalid entry reference
    #[error("Entry {0} references non-existent entry {1}")]
    InvalidReference(String, String),
}

/// Validation errors for Spool entries.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Missing required field
    #[error("Missing required field: {field} in {entry_type} entry")]
    MissingField { entry_type: String, field: String },

    /// Invalid field value
    #[error("Invalid value for {field}: {message}")]
    InvalidValue { field: String, message: String },

    /// Negative timestamp
    #[error("Timestamp cannot be negative: {0}")]
    NegativeTimestamp(i64),

    /// Invalid UUID format
    #[error("Invalid UUID format: {0}")]
    InvalidUuid(String),

    /// Session entry not at position 0
    #[error("Session entry must have ts=0, found ts={0}")]
    SessionTimestampNotZero(u64),

    /// Duplicate entry ID
    #[error("Duplicate entry ID: {0}")]
    DuplicateId(String),

    /// Tool result without matching tool call
    #[error("Tool result {result_id} references unknown tool call {call_id}")]
    OrphanedToolResult { result_id: String, call_id: String },

    /// Subagent end without matching start
    #[error("Subagent end {end_id} references unknown subagent start {start_id}")]
    OrphanedSubagentEnd { end_id: String, start_id: String },
}

/// Result type for Spool operations.
pub type SpoolResult<T> = Result<T, SpoolError>;
