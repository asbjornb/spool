//! Skill management commands - install/uninstall Claude Code skills.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// The embedded skill content (compiled into the binary).
const SPOOL_SKILL: &str = include_str!("../../../../skills/spool.md");

/// Get the Claude Code commands directory.
fn claude_commands_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude").join("commands"))
}

/// Install the spool skill to Claude Code.
pub fn install() -> Result<()> {
    let commands_dir = claude_commands_dir()?;

    // Create directory if it doesn't exist
    if !commands_dir.exists() {
        fs::create_dir_all(&commands_dir)
            .with_context(|| format!("Failed to create {:?}", commands_dir))?;
        println!("Created {:?}", commands_dir);
    }

    let skill_path = commands_dir.join("spool.md");

    // Check if already installed
    if skill_path.exists() {
        let existing = fs::read_to_string(&skill_path)?;
        if existing == SPOOL_SKILL {
            println!("Skill already installed and up to date: {:?}", skill_path);
            return Ok(());
        }
        println!("Updating existing skill...");
    }

    fs::write(&skill_path, SPOOL_SKILL)
        .with_context(|| format!("Failed to write {:?}", skill_path))?;

    println!("Installed spool skill to {:?}", skill_path);
    println!("\nClaude Code can now use /spool to get help with spool commands.");
    println!("Try asking: \"How do I export a session with redaction?\"");

    Ok(())
}

/// Uninstall the spool skill from Claude Code.
pub fn uninstall() -> Result<()> {
    let commands_dir = claude_commands_dir()?;
    let skill_path = commands_dir.join("spool.md");

    if !skill_path.exists() {
        println!("Skill not installed: {:?}", skill_path);
        return Ok(());
    }

    fs::remove_file(&skill_path).with_context(|| format!("Failed to remove {:?}", skill_path))?;

    println!("Uninstalled spool skill from {:?}", skill_path);

    Ok(())
}

/// Show the skill content.
pub fn show() -> Result<()> {
    println!("{}", SPOOL_SKILL);
    Ok(())
}

/// Show where the skill would be installed.
pub fn path() -> Result<()> {
    let commands_dir = claude_commands_dir()?;
    let skill_path = commands_dir.join("spool.md");

    println!("{}", skill_path.display());

    if skill_path.exists() {
        println!("(installed)");
    } else {
        println!("(not installed)");
    }

    Ok(())
}
