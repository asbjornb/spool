//! Play command - Replay a session with TUI controls.

use anyhow::Result;
use std::path::Path;

pub fn run(path: &Path, speed: f32) -> Result<()> {
    println!("▶️  Play command");
    println!("   Path: {:?}", path);
    println!("   Speed: {}x", speed);
    println!();
    println!("📺 TUI playback mode coming soon!");
    println!("   For now, use: spool view {:?}", path);

    Ok(())
}
