//! Publish command - Upload a session to spool.dev.

use anyhow::Result;
use std::path::Path;

pub fn run(path: &Path, public: bool) -> Result<()> {
    println!("📤 Publish command");
    println!("   Path: {:?}", path);
    println!("   Public: {}", public);
    println!();
    println!("🌐 spool.dev publishing coming in Phase 2!");
    println!();
    println!("For now, you can:");
    println!("  1. Export your session: spool export <path> --redact");
    println!("  2. Share the .spool file directly");
    println!("  3. Host the viewer yourself (coming soon)");

    Ok(())
}
