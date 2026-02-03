//! Secret detection command and shared detection logic.

use anyhow::Result;
use serde::Serialize;
use spool_format::{Entry, SecretDetector, SpoolFile, ToolOutput};
use std::path::Path;

use super::agent::load_spool_or_log;

/// A detected secret with context for display/selection.
#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    /// Index in the detections list (for --skip)
    pub index: usize,
    /// Index into spool_file.entries
    pub entry_index: usize,
    /// Entry type (prompt, response, etc.)
    pub entry_type: String,
    /// Secret category
    pub category: String,
    /// The matched secret text
    #[serde(rename = "match")]
    pub matched: String,
    /// Replacement text that would be used
    pub replacement: String,
    /// Context before the match (~40 chars)
    pub context_before: String,
    /// Context after the match (~40 chars)
    pub context_after: String,
    /// Byte offset within the entry's text
    pub start: usize,
    /// Byte end offset within the entry's text
    pub end: usize,
}

/// Detect all secrets in a SpoolFile.
/// Returns a list of detections with indices for selective redaction.
pub fn detect_secrets(file: &SpoolFile) -> Vec<Detection> {
    let detector = SecretDetector::with_defaults();
    let mut detections = Vec::new();
    let mut detection_index = 0;

    for (entry_idx, entry) in file.entries.iter().enumerate() {
        let (entry_type, text) = match entry {
            Entry::Prompt(p) => ("prompt", Some(p.content.as_str())),
            Entry::Response(r) => ("response", Some(r.content.as_str())),
            Entry::Thinking(t) => ("thinking", Some(t.content.as_str())),
            Entry::ToolResult(tr) => match &tr.output {
                Some(ToolOutput::Text(t)) => ("tool_result", Some(t.as_str())),
                _ => ("tool_result", None),
            },
            Entry::Error(e) => ("error", Some(e.message.as_str())),
            Entry::SubagentStart(s) => ("subagent_start", s.context.as_deref()),
            Entry::SubagentEnd(e) => ("subagent_end", e.summary.as_deref()),
            Entry::Annotation(a) => ("annotation", Some(a.content.as_str())),
            _ => ("unknown", None),
        };

        if let Some(text) = text {
            let secrets = detector.detect(text);
            for secret in secrets {
                detections.push(Detection {
                    index: detection_index,
                    entry_index: entry_idx,
                    entry_type: entry_type.to_string(),
                    category: format!("{:?}", secret.reason),
                    matched: secret.matched.clone(),
                    replacement: secret.reason.replacement().to_string(),
                    context_before: extract_context_before(text, secret.start, 40),
                    context_after: extract_context_after(text, secret.end, 40),
                    start: secret.start,
                    end: secret.end,
                });
                detection_index += 1;
            }
        }
    }

    detections
}

/// Apply redactions to a SpoolFile, optionally skipping certain detection indices.
pub fn apply_redactions(file: &mut SpoolFile, detections: &[Detection], skip_indices: &[usize]) {
    use std::collections::HashMap;

    // Group detections by entry index, excluding skipped ones
    let mut by_entry: HashMap<usize, Vec<&Detection>> = HashMap::new();
    for detection in detections {
        if !skip_indices.contains(&detection.index) {
            by_entry
                .entry(detection.entry_index)
                .or_default()
                .push(detection);
        }
    }

    // Process each entry that has redactions
    for (entry_idx, mut entry_detections) in by_entry {
        // Sort by position descending so we can replace without offset issues
        entry_detections.sort_by(|a, b| b.start.cmp(&a.start));

        if let Some(entry) = file.entries.get_mut(entry_idx) {
            apply_redactions_to_entry(entry, &entry_detections);
        }
    }
}

fn apply_redactions_to_entry(entry: &mut Entry, detections: &[&Detection]) {
    match entry {
        Entry::Prompt(p) => {
            p.content = apply_redactions_to_text(&p.content, detections);
        }
        Entry::Response(r) => {
            r.content = apply_redactions_to_text(&r.content, detections);
        }
        Entry::Thinking(t) => {
            t.content = apply_redactions_to_text(&t.content, detections);
        }
        Entry::ToolResult(tr) => {
            if let Some(ToolOutput::Text(ref mut text)) = tr.output {
                *text = apply_redactions_to_text(text, detections);
            }
        }
        Entry::Error(e) => {
            e.message = apply_redactions_to_text(&e.message, detections);
        }
        Entry::SubagentStart(s) => {
            if let Some(ref mut ctx) = s.context {
                *ctx = apply_redactions_to_text(ctx, detections);
            }
        }
        Entry::SubagentEnd(e) => {
            if let Some(ref mut summary) = e.summary {
                *summary = apply_redactions_to_text(summary, detections);
            }
        }
        Entry::Annotation(a) => {
            a.content = apply_redactions_to_text(&a.content, detections);
        }
        _ => {}
    }
}

fn apply_redactions_to_text(text: &str, detections: &[&Detection]) -> String {
    let mut result = text.to_string();
    for detection in detections {
        if detection.start < result.len() && detection.end <= result.len() {
            result.replace_range(detection.start..detection.end, &detection.replacement);
        }
    }
    result
}

fn extract_context_before(text: &str, pos: usize, max_len: usize) -> String {
    let start = pos.saturating_sub(max_len);
    let slice = &text[start..pos];
    if let Some(nl) = slice.rfind('\n') {
        slice[nl + 1..].to_string()
    } else {
        slice.to_string()
    }
}

fn extract_context_after(text: &str, pos: usize, max_len: usize) -> String {
    let end = (pos + max_len).min(text.len());
    let slice = &text[pos..end];
    if let Some(nl) = slice.find('\n') {
        slice[..nl].to_string()
    } else {
        slice.to_string()
    }
}

/// Run the detect command.
pub fn run(source: &Path, json: bool) -> Result<()> {
    let file = load_spool_or_log(source)?;
    let detections = detect_secrets(&file);

    if json {
        println!("{}", serde_json::to_string_pretty(&detections)?);
    } else if detections.is_empty() {
        println!("No secrets detected.");
    } else {
        println!("Detected {} secret(s):\n", detections.len());
        for d in &detections {
            println!(
                "  [{}] {} in {} (entry {})",
                d.index, d.category, d.entry_type, d.entry_index
            );
            println!("      Match: {}", truncate(&d.matched, 60));
            println!(
                "      Context: ...{}[MATCH]{}...",
                truncate(&d.context_before, 30),
                truncate(&d.context_after, 30)
            );
            println!();
        }
        println!("Use 'spool export --redact --skip 0,1,2' to exclude specific detections.");
    }

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spool_format::{PromptEntry, SessionEntry};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_test_file() -> SpoolFile {
        let session = SessionEntry {
            id: Uuid::nil(),
            ts: 0,
            version: "1.0".to_string(),
            agent: "test".to_string(),
            recorded_at: chrono::Utc::now(),
            agent_version: None,
            title: Some("Test".to_string()),
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
        };
        let mut file = SpoolFile::new(session);
        file.add_entry(Entry::Prompt(PromptEntry {
            id: Uuid::new_v4(),
            ts: 1000,
            content: "My email is test@example.com and my key is sk-ant-api03-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
            subagent_id: None,
            attachments: None,
            extra: HashMap::new(),
        }));
        file
    }

    #[test]
    fn test_detect_secrets_finds_email_and_api_key() {
        let file = make_test_file();
        let detections = detect_secrets(&file);
        assert_eq!(detections.len(), 2);
        assert_eq!(detections[0].category, "Email");
        assert_eq!(detections[1].category, "ApiKey");
    }

    #[test]
    fn test_apply_redactions_with_skip() {
        let mut file = make_test_file();
        let detections = detect_secrets(&file);

        // Skip the email (index 0), only redact API key
        apply_redactions(&mut file, &detections, &[0]);

        if let Entry::Prompt(p) = &file.entries[1] {
            assert!(p.content.contains("test@example.com")); // Email preserved
            assert!(p.content.contains("[REDACTED:api_key]")); // API key redacted
        } else {
            panic!("Expected prompt entry");
        }
    }

    #[test]
    fn test_apply_redactions_all() {
        let mut file = make_test_file();
        let detections = detect_secrets(&file);

        apply_redactions(&mut file, &detections, &[]);

        if let Entry::Prompt(p) = &file.entries[1] {
            assert!(p.content.contains("[REDACTED:email]"));
            assert!(p.content.contains("[REDACTED:api_key]"));
        } else {
            panic!("Expected prompt entry");
        }
    }
}
