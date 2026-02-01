//! Spool CLI - Browse, replay, and share AI agent sessions.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(name = "spool")]
#[command(author, version, about = "Browse, replay, and share AI agent sessions", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Browse agent sessions interactively
    Browse {
        /// Agent to filter by (claude-code, codex, etc.)
        #[arg(short, long)]
        agent: Option<String>,
    },

    /// View a session file
    View {
        /// Path to the .spool file or agent session
        path: PathBuf,
    },

    /// Replay a session with playback controls
    Play {
        /// Path to the .spool file or agent session
        path: PathBuf,

        /// Playback speed multiplier
        #[arg(short, long, default_value = "1.0")]
        speed: f32,
    },

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

    /// Show information about a session
    Info {
        /// Path to the .spool file or agent session
        path: PathBuf,
    },

    /// Publish a session to unspool.dev (coming soon)
    Publish {
        /// Path to the .spool file
        path: PathBuf,

        /// Make the session public
        #[arg(long)]
        public: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Browse { agent }) => commands::browse::run(agent),
        Some(Commands::View { path }) => commands::view::run(&path),
        Some(Commands::Play { path, speed }) => commands::play::run(&path, speed),
        Some(Commands::Export {
            source,
            output,
            trim,
            redact,
        }) => commands::export::run(&source, output.as_deref(), trim.as_deref(), redact),
        Some(Commands::Validate { path }) => commands::validate::run(&path),
        Some(Commands::Info { path }) => commands::info::run(&path),
        Some(Commands::Publish { path, public }) => commands::publish::run(&path, public),
        None => {
            // Default to browse
            commands::browse::run(None)
        }
    }
}
