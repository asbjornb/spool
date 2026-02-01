//! Validate command - Check a .spool file for errors.

use anyhow::{Context, Result};
use spool_format::{validate_default, SpoolFile};
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    println!("🔍 Validating: {:?}\n", path);

    let file = SpoolFile::from_path(path).with_context(|| format!("Failed to read: {:?}", path))?;

    let result = validate_default(&file);

    if result.is_valid() && result.warnings.is_empty() {
        println!("✅ File is valid!");
        println!("   Version: {}", file.session.version);
        println!("   Entries: {}", file.entries.len());
        return Ok(());
    }

    if !result.errors.is_empty() {
        println!("❌ Errors ({}):", result.errors.len());
        for error in &result.errors {
            println!("   • {}", error);
        }
        println!();
    }

    if !result.warnings.is_empty() {
        println!("⚠️  Warnings ({}):", result.warnings.len());
        for warning in &result.warnings {
            println!("   • {}", warning);
        }
        println!();
    }

    if result.is_valid() {
        println!("✅ File is valid (with warnings)");
        Ok(())
    } else {
        anyhow::bail!("Validation failed with {} error(s)", result.errors.len());
    }
}
