//! Search command - Search sessions by title, project, or content (non-interactive).

use anyhow::Result;
use serde::Serialize;

use super::agent::{convert_session, find_all_sessions};

#[derive(Serialize)]
struct SearchResult {
    path: String,
    agent: String,
    title: String,
    modified: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_content: Option<String>,
}

pub fn run(
    query: &str,
    agent_filter: Option<&str>,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let sessions = find_all_sessions()?;
    let query_lower = query.to_lowercase();

    let mut results: Vec<SearchResult> = Vec::new();
    let max = limit.unwrap_or(20);

    for session in &sessions {
        if results.len() >= max {
            break;
        }

        if let Some(filter) = agent_filter {
            if session.agent.as_str() != filter {
                continue;
            }
        }

        // Check title match
        let title = session.title.as_deref().unwrap_or("Untitled");
        let title_matches = title.to_lowercase().contains(&query_lower);

        // Check project dir match
        let project_matches = session
            .project_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_lowercase().contains(&query_lower))
            .unwrap_or(false);

        // Check content match (search prompts and responses)
        let mut matched_content: Option<String> = None;
        if !title_matches && !project_matches {
            if let Ok(spool_file) = convert_session(session) {
                for entry in &spool_file.entries {
                    let text = match entry {
                        spool_format::Entry::Prompt(p) => Some(&p.content),
                        spool_format::Entry::Response(r) => Some(&r.content),
                        _ => None,
                    };
                    if let Some(text) = text {
                        if text.to_lowercase().contains(&query_lower) {
                            // Extract a snippet around the match using char indices for UTF-8 safety
                            matched_content = extract_snippet(text, &query_lower);
                            break;
                        }
                    }
                }
                if matched_content.is_none() {
                    continue; // No match in content either
                }
            } else {
                continue;
            }
        }

        results.push(SearchResult {
            path: session.path.to_string_lossy().to_string(),
            agent: session.agent.as_str().to_string(),
            title: title.to_string(),
            modified: session
                .modified_at
                .map(|d| d.to_rfc3339())
                .unwrap_or_default(),
            matched_content,
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No sessions matching \"{}\".", query);
            return Ok(());
        }

        for r in &results {
            let badge = match r.agent.as_str() {
                "claude-code" => "CC",
                "codex" => "CX",
                "cursor" => "CU",
                "aider" => "AI",
                _ => "??",
            };
            println!("[{}] {}", badge, r.title);
            println!("     {}", r.path);
            if let Some(ref snippet) = r.matched_content {
                println!("     match: {}", snippet);
            }
            println!();
        }

        println!("{} result(s).", results.len());
    }

    Ok(())
}

/// Extract a snippet around the query match, using char indices for UTF-8 safety.
fn extract_snippet(text: &str, query_lower: &str) -> Option<String> {
    let text_lower = text.to_lowercase();
    let match_start = text_lower.find(query_lower)?;

    // Build a mapping of char indices to byte positions
    let char_indices: Vec<(usize, usize)> = text
        .char_indices()
        .enumerate()
        .map(|(char_idx, (byte_idx, _))| (char_idx, byte_idx))
        .collect();

    // Find char index of match start
    let match_char_idx = char_indices
        .iter()
        .position(|(_, byte_idx)| *byte_idx >= match_start)
        .unwrap_or(0);

    // Calculate snippet boundaries in char space (40 chars context)
    let start_char = match_char_idx.saturating_sub(40);
    let end_char = (match_char_idx + query_lower.chars().count() + 40).min(char_indices.len());

    // Convert back to byte positions
    let start_byte = char_indices.get(start_char).map(|(_, b)| *b).unwrap_or(0);
    let end_byte = char_indices
        .get(end_char)
        .map(|(_, b)| *b)
        .unwrap_or(text.len());

    // Find word boundaries for cleaner snippets
    let start_byte = text[..start_byte]
        .rfind(char::is_whitespace)
        .map(|p| p + 1)
        .unwrap_or(start_byte);
    let end_byte = text[end_byte..]
        .find(char::is_whitespace)
        .map(|p| p + end_byte)
        .unwrap_or(end_byte);

    let snippet = text[start_byte..end_byte].replace('\n', " ");
    Some(format!("...{}...", snippet))
}
