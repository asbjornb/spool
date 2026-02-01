//! Info command - Show information about a session.

use anyhow::{Context, Result};
use spool_format::SpoolFile;
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    let file = SpoolFile::from_path(path).with_context(|| format!("Failed to read: {:?}", path))?;

    println!("📋 Session Information\n");
    println!("File: {:?}", path);
    println!();

    // Session metadata
    println!(
        "Title:      {}",
        file.session.title.as_deref().unwrap_or("Untitled")
    );
    println!("Agent:      {}", file.session.agent);
    if let Some(ref ver) = file.session.agent_version {
        println!("Agent Ver:  {}", ver);
    }
    println!("Recorded:   {}", file.session.recorded_at);
    if let Some(ref author) = file.session.author {
        println!("Author:     {}", author);
    }
    println!("Format:     v{}", file.session.version);
    println!();

    // Statistics
    println!("Statistics:");
    println!("  Entries:    {}", file.entries.len());
    println!(
        "  Duration:   {} ms ({:.1}s)",
        file.duration_ms(),
        file.duration_ms() as f64 / 1000.0
    );
    println!("  Prompts:    {}", file.prompts().len());
    println!("  Responses:  {}", file.responses().len());
    println!("  Tool calls: {}", file.tool_calls().len());
    println!("  Errors:     {}", file.errors().len());
    println!("  Annotations:{}", file.annotations().len());
    println!();

    // Tools used
    let tools = file.tools_used();
    if !tools.is_empty() {
        println!("Tools Used:");
        for tool in tools {
            let count = file
                .tool_calls()
                .iter()
                .filter(|tc| tc.tool == tool)
                .count();
            println!("  {} ({}x)", tool, count);
        }
        println!();
    }

    // Tags
    if let Some(ref tags) = file.session.tags {
        if !tags.is_empty() {
            println!("Tags: {}", tags.join(", "));
            println!();
        }
    }

    // Trimmed info
    if let Some(ref trimmed) = file.session.trimmed {
        println!("Note: This file was trimmed from a longer session.");
        println!("  Original duration: {} ms", trimmed.original_duration_ms);
        println!(
            "  Kept range: {}-{} ms",
            trimmed.kept_range.0, trimmed.kept_range.1
        );
        println!();
    }

    Ok(())
}
