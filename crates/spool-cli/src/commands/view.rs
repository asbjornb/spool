//! View command - Print session content to stdout (non-interactive).

use anyhow::Result;
use spool_format::{Entry, ToolOutput};
use std::path::Path;

use super::agent::load_spool_or_log;

pub fn run(path: &Path, json: bool, entry_type: Option<&str>) -> Result<()> {
    let file = load_spool_or_log(path)?;

    if json {
        // In JSON mode, output entries as a JSON array
        let entries: Vec<&Entry> = file
            .entries
            .iter()
            .filter(|e| match entry_type {
                Some(t) => entry_matches_type(e, t),
                None => true,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    // Plain text mode
    for entry in &file.entries {
        if let Some(t) = entry_type {
            if !entry_matches_type(entry, t) {
                continue;
            }
        }

        match entry {
            Entry::Session(s) => {
                println!(
                    "=== SESSION: {} ===",
                    s.title.as_deref().unwrap_or("Untitled")
                );
                println!("Agent: {}", s.agent);
                if let Some(ref ver) = s.agent_version {
                    println!("Version: {}", ver);
                }
                println!("Recorded: {}", s.recorded_at);
                println!();
            }
            Entry::Prompt(p) => {
                let ts = format_ts(p.ts);
                println!("[{}] USER:", ts);
                println!("{}", p.content);
                println!();
            }
            Entry::Thinking(t) => {
                let ts = format_ts(t.ts);
                let collapsed = t.content.replace('\n', " ");
                let preview = if collapsed.len() > 120 {
                    format!("{}...", &collapsed[..117])
                } else {
                    collapsed
                };
                println!("[{}] THINKING: {}", ts, preview);
            }
            Entry::ToolCall(tc) => {
                let ts = format_ts(tc.ts);
                let input_str = format_input(&tc.input);
                println!("[{}] TOOL: {} {}", ts, tc.tool, input_str);
            }
            Entry::ToolResult(tr) => {
                let ts = format_ts(tr.ts);
                if let Some(ref err) = tr.error {
                    println!("[{}] RESULT [ERROR]: {}", ts, truncate(err, 200));
                } else if let Some(ref output) = tr.output {
                    let text = match output {
                        ToolOutput::Text(t) => truncate(t, 200),
                        ToolOutput::Binary(_) => "<binary>".to_string(),
                    };
                    println!("[{}] RESULT [OK]: {}", ts, text);
                } else {
                    println!("[{}] RESULT [OK]", ts);
                }
            }
            Entry::Response(r) => {
                let ts = format_ts(r.ts);
                println!("[{}] ASSISTANT:", ts);
                println!("{}", r.content);
                if let Some(ref model) = r.model {
                    println!("  [model: {}]", model);
                }
                println!();
            }
            Entry::Error(e) => {
                let ts = format_ts(e.ts);
                println!("[{}] ERROR [{}]: {}", ts, e.code, e.message);
            }
            Entry::SubagentStart(s) => {
                let ts = format_ts(s.ts);
                println!("[{}] SUBAGENT START: {}", ts, s.agent);
            }
            Entry::SubagentEnd(e) => {
                let ts = format_ts(e.ts);
                let status = e
                    .status
                    .as_ref()
                    .map(|s| format!(" ({:?})", s))
                    .unwrap_or_default();
                println!("[{}] SUBAGENT END{}", ts, status);
            }
            Entry::Annotation(a) => {
                let author = a.author.as_deref().unwrap_or("anonymous");
                println!("[NOTE @{}] {}", author, a.content);
            }
            Entry::RedactionMarker(_) => {
                println!("[REDACTED]");
            }
            Entry::Unknown => {}
        }
    }

    Ok(())
}

fn entry_matches_type(entry: &Entry, type_filter: &str) -> bool {
    match type_filter {
        "session" => matches!(entry, Entry::Session(_)),
        "prompt" | "user" => matches!(entry, Entry::Prompt(_)),
        "thinking" => matches!(entry, Entry::Thinking(_)),
        "tool_call" | "tool-call" | "toolcall" => matches!(entry, Entry::ToolCall(_)),
        "tool_result" | "tool-result" | "toolresult" => matches!(entry, Entry::ToolResult(_)),
        "response" | "assistant" => matches!(entry, Entry::Response(_)),
        "error" => matches!(entry, Entry::Error(_)),
        "annotation" | "note" => matches!(entry, Entry::Annotation(_)),
        _ => true,
    }
}

fn format_ts(ms: u64) -> String {
    let total_secs = ms / 1000;
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

fn truncate(s: &str, max: usize) -> String {
    let single_line = s.replace('\n', " ");
    if single_line.len() > max {
        format!("{}...", &single_line[..max.saturating_sub(3)])
    } else {
        single_line
    }
}

fn format_input(input: &serde_json::Value) -> String {
    match input {
        serde_json::Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .take(3)
                .map(|(k, v)| {
                    let val = match v {
                        serde_json::Value::String(s) => truncate(s, 50),
                        other => truncate(&other.to_string(), 50),
                    };
                    format!("{}={}", k, val)
                })
                .collect();
            let result = parts.join(" ");
            if map.len() > 3 {
                format!("{} ...", result)
            } else {
                result
            }
        }
        other => truncate(&other.to_string(), 100),
    }
}
