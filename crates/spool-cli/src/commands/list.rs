//! List command - List discovered agent sessions (non-interactive).

use anyhow::Result;
use serde::Serialize;

use super::agent::find_all_sessions;

#[derive(Serialize)]
struct SessionRow {
    path: String,
    agent: String,
    title: String,
    modified: String,
    messages: Option<usize>,
    project: Option<String>,
}

pub fn run(agent_filter: Option<&str>, limit: Option<usize>, json: bool) -> Result<()> {
    let sessions = find_all_sessions()?;

    let filtered: Vec<_> = sessions
        .iter()
        .filter(|s| agent_filter.map(|f| s.agent.as_str() == f).unwrap_or(true))
        .collect();

    let limited: Vec<_> = match limit {
        Some(n) => filtered.into_iter().take(n).collect(),
        None => filtered,
    };

    if json {
        let rows: Vec<SessionRow> = limited
            .iter()
            .map(|s| SessionRow {
                path: s.path.to_string_lossy().to_string(),
                agent: s.agent.as_str().to_string(),
                title: s.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                modified: s.modified_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
                messages: s.message_count,
                project: s
                    .project_dir
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        if limited.is_empty() {
            println!("No sessions found.");
            return Ok(());
        }

        println!(
            "{:<6} {:<40} {:<20} {:>5}  PATH",
            "AGENT", "TITLE", "MODIFIED", "MSGS"
        );
        println!("{}", "-".repeat(100));

        for s in &limited {
            let agent_badge = match s.agent.as_str() {
                "claude-code" => "CC",
                "codex" => "CX",
                "cursor" => "CU",
                "aider" => "AI",
                _ => "??",
            };
            let title = s.title.as_deref().unwrap_or("Untitled");
            let title_display = if title.len() > 38 {
                format!("{}...", &title[..35])
            } else {
                title.to_string()
            };
            let modified = s
                .modified_at
                .map(|d| {
                    d.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|| "-".to_string());
            let msgs = s
                .message_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string());
            let path = s.path.to_string_lossy();

            println!(
                "{:<6} {:<40} {:<20} {:>5}  {}",
                agent_badge, title_display, modified, msgs, path
            );
        }

        println!("\n{} session(s) found.", limited.len());
    }

    Ok(())
}
