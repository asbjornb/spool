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
                            // Extract a snippet around the match
                            if let Some(pos) = text.to_lowercase().find(&query_lower) {
                                let start = pos.saturating_sub(40);
                                let end = (pos + query.len() + 40).min(text.len());
                                // Find safe char boundaries
                                let start = text[..start]
                                    .rfind(char::is_whitespace)
                                    .map(|p| p + 1)
                                    .unwrap_or(start);
                                let end = text[end..]
                                    .find(char::is_whitespace)
                                    .map(|p| p + end)
                                    .unwrap_or(end);
                                let snippet = text[start..end].replace('\n', " ");
                                matched_content = Some(format!("...{}...", snippet));
                            }
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
