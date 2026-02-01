//! Browse command - Interactive session browser.

use anyhow::Result;
use spool_adapters::claude_code;

pub fn run(agent_filter: Option<String>) -> Result<()> {
    println!("🔍 Searching for agent sessions...\n");

    // Find Claude Code sessions
    let sessions = claude_code::find_sessions()?;

    if sessions.is_empty() {
        println!("No sessions found.");
        println!("\nLooking in:");
        println!("  - Claude Code: ~/.claude/projects/*/sessions/");
        return Ok(());
    }

    println!("Found {} session(s):\n", sessions.len());

    for (i, session) in sessions.iter().take(10).enumerate() {
        let title = session.title.as_deref().unwrap_or("Untitled");
        let modified = session
            .modified_at
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let agent = session.agent.as_str();

        println!("  {}. [{}] {} ({})", i + 1, agent, title, modified);
    }

    if sessions.len() > 10 {
        println!("  ... and {} more", sessions.len() - 10);
    }

    println!("\n📋 TUI browser coming soon!");
    println!("   For now, use: spool view <path>");

    Ok(())
}
