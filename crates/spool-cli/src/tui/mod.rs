//! Unified TUI with Library and Editor views.
//!
//! The top-level app manages the terminal and dispatches between:
//! - **Library**: session browser (browse, search, preview)
//! - **Editor**: session replay (playback, trim, annotate, info overlay)

pub mod common;
pub mod editor;
pub mod library;

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use spool_adapters::AgentType;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use editor::{EditorAction, EditorState};
use library::{LibraryAction, LibraryState};

use crate::commands::agent::load_spool_or_log;

/// Top-level view state.
enum AppView {
    Library(LibraryState),
    Editor(Box<EditorState>),
}

/// Run the TUI application.
///
/// - `initial_path: None` → start in Library view
/// - `initial_path: Some(path)` → start directly in Editor view
pub fn run_tui(initial_path: Option<PathBuf>) -> Result<()> {
    // Build initial view
    let mut view = match initial_path {
        Some(ref path) => {
            let spool_file = load_spool_or_log(path)?;
            if spool_file.entries.is_empty() {
                println!("Session has no entries.");
                return Ok(());
            }
            let mut editor = EditorState::new(spool_file, path.clone(), 1.0);
            editor.has_library = false;
            editor.start_playing();
            AppView::Editor(Box::new(editor))
        }
        None => {
            let library = LibraryState::new(None)?;
            AppView::Library(library)
        }
    };

    // Set up terminal (once)
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Install panic hook so we restore the terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Preserve library state across transitions
    let mut saved_library: Option<LibraryState> = None;

    let result = run_app_loop(&mut terminal, &mut view, &mut saved_library);

    // Restore terminal (once)
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    view: &mut AppView,
    saved_library: &mut Option<LibraryState>,
) -> Result<()> {
    loop {
        // Tick and draw
        match view {
            AppView::Library(ref mut lib) => {
                lib.tick();
                terminal.draw(|f| lib.draw(f))?;
            }
            AppView::Editor(ref mut ed) => {
                ed.tick();
                terminal.draw(|f| ed.draw(f))?;
            }
        }

        // Poll for events
        let poll_duration = match view {
            AppView::Library(_) => Duration::from_millis(200),
            AppView::Editor(_) => Duration::from_millis(50),
        };

        if event::poll(poll_duration)? {
            if let Event::Key(key) = event::read()? {
                match view {
                    AppView::Library(ref mut lib) => match lib.handle_key(key) {
                        LibraryAction::OpenEditor(path, agent) => {
                            match load_session_for_editor(&path, agent) {
                                Ok(spool_file) => {
                                    let mut editor = EditorState::new(spool_file, path, 1.0);
                                    editor.has_library = true;
                                    editor.start_playing();

                                    // Save library state
                                    let old_view =
                                        std::mem::replace(view, AppView::Editor(Box::new(editor)));
                                    if let AppView::Library(lib_state) = old_view {
                                        *saved_library = Some(lib_state);
                                    }
                                }
                                Err(e) => {
                                    lib.set_status(format!("Failed to open: {}", e));
                                }
                            }
                        }
                        LibraryAction::Quit => break,
                        LibraryAction::None => {}
                    },
                    AppView::Editor(ref mut ed) => match ed.handle_key(key) {
                        EditorAction::Back => {
                            // Restore library state
                            if let Some(lib_state) = saved_library.take() {
                                *view = AppView::Library(lib_state);
                            } else {
                                break;
                            }
                        }
                        EditorAction::Quit => break,
                        EditorAction::None => {}
                    },
                }
            }
        }
    }

    Ok(())
}

fn load_session_for_editor(path: &PathBuf, agent: AgentType) -> Result<spool_format::SpoolFile> {
    use spool_adapters::{claude_code, codex, SessionInfo};

    if path.extension().map(|e| e == "spool").unwrap_or(false) {
        return Ok(spool_format::SpoolFile::from_path(path)?);
    }

    let session_info = SessionInfo {
        path: path.clone(),
        agent,
        created_at: None,
        modified_at: None,
        title: None,
        project_dir: None,
        message_count: None,
    };

    match agent {
        AgentType::ClaudeCode => claude_code::convert(&session_info),
        AgentType::Codex => codex::convert(&session_info),
        _ => anyhow::bail!("Unsupported agent: {}", agent.as_str()),
    }
}
