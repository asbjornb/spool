//! Spool CLI - Browse, replay, and share AI agent sessions.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod tui;

#[derive(Parser)]
#[command(name = "spool")]
#[command(author, version, about = "Browse, replay, and share AI agent sessions", long_about = None)]
struct Cli {
    /// Path to open directly in the Editor
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Export a session to .spool format
    Export {
        /// Path to the source session (agent log or .spool file)
        source: PathBuf,

        /// Output path for the .spool file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Trim to time range (format: START-END in mm:ss or seconds)
        #[arg(long)]
        trim: Option<String>,

        /// Apply automatic redaction
        #[arg(long)]
        redact: bool,
    },

    /// Validate a .spool file
    Validate {
        /// Path to the .spool file
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Export {
            source,
            output,
            trim,
            redact,
        }) => commands::export::run(&source, output.as_deref(), trim.as_deref(), redact),
        Some(Commands::Validate { path }) => commands::validate::run(&path),
        None => {
            // spool <path> → open directly in Editor
            // spool        → open Library
            tui::run_tui(cli.path)
        }
    }
}
