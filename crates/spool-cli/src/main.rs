//! Spool CLI - Browse, replay, and share AI agent sessions.
//!
//! Two modes of operation:
//! - **Interactive (TUI)**: `spool` or `spool <path>` opens the interactive browser/editor
//! - **CLI**: Subcommands like `list`, `info`, `view`, `search` output to stdout for scripting

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod tui;

#[derive(Parser)]
#[command(name = "spool")]
#[command(author, version, about = "Browse, replay, and share AI agent sessions", long_about = None)]
struct Cli {
    /// Path to open directly in the Editor (TUI mode)
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List discovered agent sessions
    List {
        /// Filter by agent type (e.g., claude-code, codex)
        #[arg(short, long)]
        agent: Option<String>,

        /// Maximum number of sessions to show
        #[arg(short = 'n', long)]
        limit: Option<usize>,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,
    },

    /// Show session metadata and statistics
    Info {
        /// Path to the session (agent log or .spool file)
        path: PathBuf,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,
    },

    /// Print session content to stdout
    View {
        /// Path to the session (agent log or .spool file)
        path: PathBuf,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,

        /// Filter by entry type (prompt, response, tool_call, tool_result, thinking, error, annotation)
        #[arg(short = 't', long = "type")]
        entry_type: Option<String>,
    },

    /// Search sessions by title, project, or content
    Search {
        /// Search query
        query: String,

        /// Filter by agent type (e.g., claude-code, codex)
        #[arg(short, long)]
        agent: Option<String>,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "20")]
        limit: Option<usize>,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,
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

        /// Preview redactions without exporting (use with --redact)
        #[arg(long)]
        dry_run: bool,

        /// Skip specific detection indices (comma-separated, e.g., 0,2,5)
        #[arg(long)]
        skip: Option<String>,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,
    },

    /// Detect secrets in a session (without exporting)
    Detect {
        /// Path to the session (agent log or .spool file)
        path: PathBuf,

        /// Output as JSON (for machine consumption)
        #[arg(long)]
        json: bool,
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
        Some(Commands::List { agent, limit, json }) => {
            commands::list::run(agent.as_deref(), limit, json)
        }
        Some(Commands::Info { path, json }) => commands::info::run(&path, json),
        Some(Commands::View {
            path,
            json,
            entry_type,
        }) => commands::view::run(&path, json, entry_type.as_deref()),
        Some(Commands::Search {
            query,
            agent,
            limit,
            json,
        }) => commands::search::run(&query, agent.as_deref(), limit, json),
        Some(Commands::Export {
            source,
            output,
            trim,
            redact,
            dry_run,
            skip,
            json,
        }) => commands::export::run(
            &source,
            output.as_deref(),
            trim.as_deref(),
            redact,
            dry_run,
            skip.as_deref(),
            json,
        ),
        Some(Commands::Detect { path, json }) => commands::detect::run(&path, json),
        Some(Commands::Validate { path }) => commands::validate::run(&path),
        None => {
            // spool <path> → open directly in Editor (TUI)
            // spool        → open Library (TUI)
            tui::run_tui(cli.path)
        }
    }
}
