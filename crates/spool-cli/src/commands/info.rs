//! Info command - Show session metadata (non-interactive).

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use super::agent::load_spool_or_log;

#[derive(Serialize)]
struct SessionInfo {
    title: Option<String>,
    agent: String,
    agent_version: Option<String>,
    recorded_at: String,
    author: Option<String>,
    format_version: String,
    duration_ms: u64,
    duration_display: String,
    entry_count: usize,
    prompts: usize,
    responses: usize,
    tool_calls: usize,
    errors: usize,
    annotations: usize,
    tools_used: Vec<String>,
    tags: Option<Vec<String>>,
    files_modified: Option<Vec<String>>,
    trimmed: bool,
}

pub fn run(path: &Path, json: bool) -> Result<()> {
    let file = load_spool_or_log(path)?;

    let duration_ms = file.duration_ms();
    let total_secs = duration_ms / 1000;
    let duration_display = format!("{}:{:02}", total_secs / 60, total_secs % 60);
    let tools = file.tools_used();

    if json {
        let info = SessionInfo {
            title: file.session.title.clone(),
            agent: file.session.agent.clone(),
            agent_version: file.session.agent_version.clone(),
            recorded_at: file.session.recorded_at.to_rfc3339(),
            author: file.session.author.clone(),
            format_version: file.session.version.clone(),
            duration_ms,
            duration_display,
            entry_count: file.entries.len(),
            prompts: file.prompts().len(),
            responses: file.responses().len(),
            tool_calls: file.tool_calls().len(),
            errors: file.errors().len(),
            annotations: file.annotations().len(),
            tools_used: tools,
            tags: file.session.tags.clone(),
            files_modified: file.session.files_modified.clone(),
            trimmed: file.session.trimmed.is_some(),
        };
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!(
            "Title:        {}",
            file.session.title.as_deref().unwrap_or("Untitled")
        );
        println!("Agent:        {}", file.session.agent);
        if let Some(ref ver) = file.session.agent_version {
            println!("Agent Ver:    {}", ver);
        }
        println!("Recorded:     {}", file.session.recorded_at);
        if let Some(ref author) = file.session.author {
            println!("Author:       {}", author);
        }
        println!("Format:       v{}", file.session.version);
        println!();
        println!(
            "Duration:     {} ({:.1}s)",
            duration_display,
            duration_ms as f64 / 1000.0
        );
        println!("Entries:      {}", file.entries.len());
        println!("Prompts:      {}", file.prompts().len());
        println!("Responses:    {}", file.responses().len());
        println!("Tool calls:   {}", file.tool_calls().len());
        println!("Errors:       {}", file.errors().len());
        println!("Annotations:  {}", file.annotations().len());

        if !tools.is_empty() {
            println!();
            println!("Tools used:");
            for tool in &tools {
                let count = file
                    .tool_calls()
                    .iter()
                    .filter(|tc| tc.tool == *tool)
                    .count();
                println!("  {} ({}x)", tool, count);
            }
        }

        if let Some(ref tags) = file.session.tags {
            if !tags.is_empty() {
                println!("\nTags:         {}", tags.join(", "));
            }
        }

        if let Some(ref files) = file.session.files_modified {
            if !files.is_empty() {
                println!("\nFiles modified:");
                for f in files {
                    println!("  {}", f);
                }
            }
        }

        if file.session.trimmed.is_some() {
            println!("\n[trimmed]");
        }
    }

    Ok(())
}
