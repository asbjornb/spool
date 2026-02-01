//! Validation utilities for Spool files.

use crate::{Entry, SpoolFile, ValidationError};
use std::collections::HashSet;

/// Validation options.
#[derive(Debug, Clone)]
pub struct ValidationOptions {
    /// Check for duplicate entry IDs
    pub check_duplicate_ids: bool,
    /// Check that tool results reference valid tool calls
    pub check_tool_references: bool,
    /// Check that subagent ends reference valid starts
    pub check_subagent_references: bool,
    /// Check that annotations reference valid entries
    pub check_annotation_references: bool,
    /// Warn about out-of-order timestamps (not an error per spec)
    pub warn_out_of_order_timestamps: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            check_duplicate_ids: true,
            check_tool_references: true,
            check_subagent_references: true,
            check_annotation_references: true,
            warn_out_of_order_timestamps: true,
        }
    }
}

/// Validation result.
#[derive(Debug)]
pub struct ValidationResult {
    /// Hard errors that make the file invalid
    pub errors: Vec<ValidationError>,
    /// Warnings that don't make the file invalid
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Returns true if there are no errors.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate a Spool file.
pub fn validate(file: &SpoolFile, options: &ValidationOptions) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Collect all entry IDs
    let mut seen_ids = HashSet::new();
    let mut tool_call_ids = HashSet::new();
    let mut subagent_start_ids = HashSet::new();

    // Check session entry
    if file.session.ts != 0 {
        errors.push(ValidationError::SessionTimestampNotZero(file.session.ts));
    }

    let mut last_ts: Option<u64> = None;

    for entry in &file.entries {
        // Check for duplicate IDs
        if options.check_duplicate_ids {
            if let Some(id) = entry.id() {
                if !seen_ids.insert(*id) {
                    errors.push(ValidationError::DuplicateId(id.to_string()));
                }
            }
        }

        // Track tool calls and subagent starts
        match entry {
            Entry::ToolCall(tc) => {
                tool_call_ids.insert(tc.id);
            }
            Entry::SubagentStart(ss) => {
                subagent_start_ids.insert(ss.id);
            }
            _ => {}
        }

        // Check references
        match entry {
            Entry::ToolResult(tr) => {
                if options.check_tool_references && !tool_call_ids.contains(&tr.call_id) {
                    errors.push(ValidationError::OrphanedToolResult {
                        result_id: tr.id.to_string(),
                        call_id: tr.call_id.to_string(),
                    });
                }
            }
            Entry::SubagentEnd(se) => {
                if options.check_subagent_references && !subagent_start_ids.contains(&se.start_id) {
                    errors.push(ValidationError::OrphanedSubagentEnd {
                        end_id: se.id.to_string(),
                        start_id: se.start_id.to_string(),
                    });
                }
            }
            Entry::Annotation(a) => {
                if options.check_annotation_references && !seen_ids.contains(&a.target_id) {
                    warnings.push(format!(
                        "Annotation {} references unknown entry {}",
                        a.id, a.target_id
                    ));
                }
            }
            _ => {}
        }

        // Check timestamp ordering
        if options.warn_out_of_order_timestamps {
            if let Some(ts) = entry.timestamp() {
                if let Some(last) = last_ts {
                    if ts < last {
                        warnings.push(format!(
                            "Entry {:?} has timestamp {} which is before previous entry's {}",
                            entry.id(),
                            ts,
                            last
                        ));
                    }
                }
                last_ts = Some(ts);
            }
        }
    }

    ValidationResult { errors, warnings }
}

/// Validate a Spool file with default options.
pub fn validate_default(file: &SpoolFile) -> ValidationResult {
    validate(file, &ValidationOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_session() -> SessionEntry {
        SessionEntry {
            id: Uuid::new_v4(),
            ts: 0,
            version: "1.0".to_string(),
            agent: "test".to_string(),
            recorded_at: Utc::now(),
            agent_version: None,
            title: None,
            author: None,
            tags: None,
            duration_ms: None,
            entry_count: None,
            tools_used: None,
            files_modified: None,
            first_prompt: None,
            schema_url: None,
            trimmed: None,
            ended: None,
            extra: HashMap::new(),
        }
    }

    fn make_prompt(id: Uuid, ts: u64, content: &str) -> Entry {
        Entry::Prompt(PromptEntry {
            id,
            ts,
            content: content.to_string(),
            subagent_id: None,
            attachments: None,
            extra: HashMap::new(),
        })
    }

    #[test]
    fn test_valid_file() {
        let session = make_session();
        let mut file = SpoolFile::new(session);
        file.add_entry(make_prompt(Uuid::new_v4(), 100, "Hello"));

        let result = validate_default(&file);
        assert!(result.is_valid());
    }

    #[test]
    fn test_duplicate_ids() {
        let session = make_session();
        let mut file = SpoolFile::new(session);

        let id = Uuid::new_v4();
        file.add_entry(make_prompt(id, 100, "First"));
        file.add_entry(make_prompt(id, 200, "Duplicate"));

        let result = validate_default(&file);
        assert!(!result.is_valid());
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateId(_))));
    }

    #[test]
    fn test_out_of_order_warnings() {
        let session = make_session();
        let mut file = SpoolFile::new(session);
        file.add_entry(make_prompt(Uuid::new_v4(), 200, "Second"));
        file.add_entry(make_prompt(Uuid::new_v4(), 100, "First but later"));

        let result = validate_default(&file);
        assert!(result.is_valid()); // Out of order is a warning, not error
        assert!(!result.warnings.is_empty());
    }
}
