//! Reading and writing Spool files.

use crate::{Entry, SessionEntry, SpoolError, SpoolResult};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// A parsed Spool file.
#[derive(Debug, Clone)]
pub struct SpoolFile {
    /// The session metadata (first entry)
    pub session: SessionEntry,
    /// All entries including the session entry
    pub entries: Vec<Entry>,
    /// Lines that failed to parse (for round-tripping)
    pub unparsed_lines: Vec<(usize, String)>,
}

impl SpoolFile {
    /// Create a new SpoolFile with the given session metadata.
    pub fn new(session: SessionEntry) -> Self {
        Self {
            session: session.clone(),
            entries: vec![Entry::Session(session)],
            unparsed_lines: Vec::new(),
        }
    }

    /// Read a Spool file from a path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> SpoolResult<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_reader(reader)
    }

    /// Read a Spool file from a reader.
    pub fn from_reader<R: BufRead>(reader: R) -> SpoolResult<Self> {
        let mut entries = Vec::new();
        let mut unparsed_lines = Vec::new();
        let mut session: Option<SessionEntry> = None;

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            let line_num = line_num + 1; // 1-indexed

            // Skip blank lines
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Entry>(&line) {
                Ok(entry) => {
                    // First entry must be session
                    if entries.is_empty() {
                        match &entry {
                            Entry::Session(s) => {
                                session = Some(s.clone());
                            }
                            _ => return Err(SpoolError::MissingSessionEntry),
                        }
                    }
                    entries.push(entry);
                }
                Err(e) => {
                    // Store unparsed line for round-tripping
                    unparsed_lines.push((line_num, line));
                    // Log but don't fail - forward compatibility
                    eprintln!("Warning: Failed to parse line {}: {}", line_num, e);
                }
            }
        }

        let session = session.ok_or(SpoolError::MissingSessionEntry)?;

        Ok(Self {
            session,
            entries,
            unparsed_lines,
        })
    }

    /// Write the Spool file to a path.
    pub fn write_to_path<P: AsRef<Path>>(&self, path: P) -> SpoolResult<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        self.write_to(writer)
    }

    /// Write the Spool file to a writer.
    pub fn write_to<W: Write>(&self, mut writer: W) -> SpoolResult<()> {
        for entry in &self.entries {
            let json = serde_json::to_string(entry).map_err(|e| SpoolError::Json {
                line: 0,
                message: e.to_string(),
                source: e,
            })?;
            writeln!(writer, "{}", json)?;
        }
        Ok(())
    }

    /// Add an entry to the file.
    pub fn add_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Get all entries of a specific type.
    pub fn entries_of_type<F, T>(&self, f: F) -> Vec<&T>
    where
        F: Fn(&Entry) -> Option<&T>,
    {
        self.entries.iter().filter_map(f).collect()
    }

    /// Get all tool calls.
    pub fn tool_calls(&self) -> Vec<&crate::ToolCallEntry> {
        self.entries_of_type(|e| match e {
            Entry::ToolCall(tc) => Some(tc),
            _ => None,
        })
    }

    /// Get all prompts.
    pub fn prompts(&self) -> Vec<&crate::PromptEntry> {
        self.entries_of_type(|e| match e {
            Entry::Prompt(p) => Some(p),
            _ => None,
        })
    }

    /// Get all responses.
    pub fn responses(&self) -> Vec<&crate::ResponseEntry> {
        self.entries_of_type(|e| match e {
            Entry::Response(r) => Some(r),
            _ => None,
        })
    }

    /// Get all errors.
    pub fn errors(&self) -> Vec<&crate::ErrorEntry> {
        self.entries_of_type(|e| match e {
            Entry::Error(err) => Some(err),
            _ => None,
        })
    }

    /// Get all annotations.
    pub fn annotations(&self) -> Vec<&crate::AnnotationEntry> {
        self.entries_of_type(|e| match e {
            Entry::Annotation(a) => Some(a),
            _ => None,
        })
    }

    /// Get the total duration of the session in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.entries
            .iter()
            .filter_map(|e| e.timestamp())
            .max()
            .unwrap_or(0)
    }

    /// Get unique tool names used in the session.
    pub fn tools_used(&self) -> Vec<String> {
        let mut tools: Vec<String> = self
            .tool_calls()
            .iter()
            .map(|tc| tc.tool.clone())
            .collect();
        tools.sort();
        tools.dedup();
        tools
    }

    /// Trim the file to a time range.
    pub fn trim(&mut self, start_ms: u64, end_ms: u64) {
        // Keep session entry always
        let session = self.entries.remove(0);

        // Filter entries within range
        self.entries.retain(|e| {
            if let Some(ts) = e.timestamp() {
                ts >= start_ms && ts <= end_ms
            } else {
                false
            }
        });

        // Re-add session at start
        self.entries.insert(0, session);

        // Update session metadata
        if let Entry::Session(ref mut s) = self.entries[0] {
            s.trimmed = Some(crate::TrimmedMetadata {
                original_duration_ms: self.session.duration_ms.unwrap_or(0),
                kept_range: (start_ms, end_ms),
            });
        }
    }
}

/// Parse a single line of JSONL.
pub fn parse_line(line: &str) -> SpoolResult<Entry> {
    serde_json::from_str(line).map_err(|e| SpoolError::Json {
        line: 0,
        message: e.to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn create_test_session() -> SessionEntry {
        SessionEntry {
            id: Uuid::new_v4(),
            ts: 0,
            version: "1.0".to_string(),
            agent: "test".to_string(),
            recorded_at: Utc::now(),
            agent_version: None,
            title: Some("Test Session".to_string()),
            author: None,
            tags: None,
            duration_ms: None,
            entry_count: None,
            tools_used: None,
            schema_url: None,
            trimmed: None,
            ended: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_create_new_file() {
        let session = create_test_session();
        let file = SpoolFile::new(session.clone());
        assert_eq!(file.entries.len(), 1);
        assert_eq!(file.session.title, session.title);
    }

    #[test]
    fn test_parse_minimal_file() {
        let content = r#"{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}"#;
        let reader = std::io::Cursor::new(content);
        let file = SpoolFile::from_reader(reader).unwrap();
        assert_eq!(file.entries.len(), 1);
    }

    #[test]
    fn test_missing_session_entry() {
        let content = r#"{"id":"00000000-0000-0000-0000-000000000001","ts":100,"type":"prompt","content":"Hello"}"#;
        let reader = std::io::Cursor::new(content);
        let result = SpoolFile::from_reader(reader);
        assert!(matches!(result, Err(SpoolError::MissingSessionEntry)));
    }
}
