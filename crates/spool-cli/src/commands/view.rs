//! View command - Display a session file.

use anyhow::{Context, Result};
use spool_format::{Entry, SpoolFile};
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    let file = SpoolFile::from_path(path)
        .with_context(|| format!("Failed to read: {:?}", path))?;

    println!("📼 Session: {}", file.session.title.as_deref().unwrap_or("Untitled"));
    println!("   Agent: {}", file.session.agent);
    println!("   Recorded: {}", file.session.recorded_at);
    println!("   Entries: {}", file.entries.len());
    println!();

    for entry in &file.entries {
        print_entry(entry);
    }

    Ok(())
}

fn print_entry(entry: &Entry) {
    match entry {
        Entry::Session(_) => {
            // Already printed above
        }
        Entry::Prompt(p) => {
            println!("┌─ PROMPT ─────────────────────────────────────");
            for line in p.content.lines().take(5) {
                println!("│ {}", line);
            }
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::Thinking(t) => {
            println!("┌─ THINKING ───────────────────────────────────");
            let preview = if t.content.len() > 100 {
                format!("{}...", &t.content[..100])
            } else {
                t.content.clone()
            };
            println!("│ {}", preview.replace('\n', " "));
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::ToolCall(tc) => {
            println!("┌─ TOOL: {} ─────────────────────────────", tc.tool);
            println!("│ Input: {}", serde_json::to_string(&tc.input).unwrap_or_default());
            println!("└──────────────────────────────────────────────");
        }
        Entry::ToolResult(tr) => {
            let status = if tr.error.is_some() { "ERROR" } else { "OK" };
            println!("│ Result: [{}]", status);
            if let Some(ref output) = tr.output {
                let text = match output {
                    spool_format::ToolOutput::Text(t) => t.clone(),
                    spool_format::ToolOutput::Binary(_) => "[binary content]".to_string(),
                };
                let preview = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text
                };
                for line in preview.lines().take(5) {
                    println!("│ {}", line);
                }
            }
            if let Some(ref err) = tr.error {
                println!("│ Error: {}", err);
            }
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::Response(r) => {
            println!("┌─ RESPONSE ───────────────────────────────────");
            for line in r.content.lines().take(10) {
                println!("│ {}", line);
            }
            if r.content.lines().count() > 10 {
                println!("│ ... ({} more lines)", r.content.lines().count() - 10);
            }
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::Error(e) => {
            println!("┌─ ERROR: {} ─────────────────────────────", e.code);
            println!("│ {}", e.message);
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::Annotation(a) => {
            println!("   💬 @{}: {}", a.author.as_deref().unwrap_or("anonymous"), a.content);
        }
        Entry::RedactionMarker(r) => {
            println!("   🔒 Redacted: {:?}", r.reason);
        }
        Entry::SubagentStart(s) => {
            println!("┌─ SUBAGENT: {} ─────────────────────────", s.agent);
            if let Some(ref ctx) = s.context {
                println!("│ {}", ctx);
            }
        }
        Entry::SubagentEnd(e) => {
            if let Some(ref summary) = e.summary {
                println!("│ Summary: {}", summary);
            }
            println!("└─ SUBAGENT END ────────────────────────────────");
            println!();
        }
        Entry::Unknown => {
            println!("   [Unknown entry type]");
        }
    }
}
