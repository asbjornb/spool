//! View command - Display a session file.

use anyhow::Result;
use spool_format::Entry;
use std::path::Path;

use super::agent::load_spool_or_log;

/// Truncate a string to at most `max_bytes` bytes at a char boundary.
fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn run(path: &Path) -> Result<()> {
    let file = load_spool_or_log(path)?;

    println!(
        "📼 Session: {}",
        file.session.title.as_deref().unwrap_or("Untitled")
    );
    println!("   Agent: {}", file.session.agent);
    println!("   Recorded: {}", file.session.recorded_at);
    println!("   Entries: {}", file.entries.len());
    println!();

    for entry in &file.entries {
        print_entry(entry);
    }

    Ok(())
}

pub fn print_entry(entry: &Entry) {
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
                format!("{}...", truncate_str(&t.content, 100))
            } else {
                t.content.clone()
            };
            println!("│ {}", preview.replace('\n', " "));
            println!("└──────────────────────────────────────────────");
            println!();
        }
        Entry::ToolCall(tc) => {
            let tool_display = if tc.tool == "Task" {
                if let Some(subagent_type) = tc.input.get("subagent_type").and_then(|v| v.as_str())
                {
                    format!("Task ({})", subagent_type)
                } else {
                    tc.tool.clone()
                }
            } else {
                tc.tool.clone()
            };
            println!("┌─ TOOL: {} ─────────────────────────────", tool_display);
            println!(
                "│ Input: {}",
                serde_json::to_string(&tc.input).unwrap_or_default()
            );
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
                    format!("{}...", truncate_str(&text, 200))
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
            println!(
                "   💬 @{}: {}",
                a.author.as_deref().unwrap_or("anonymous"),
                a.content
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use spool_format::{PromptEntry, ResponseEntry, ThinkingEntry, ToolOutput, ToolResultEntry};
    use std::collections::HashMap;
    use uuid::Uuid;

    // ── truncate_str unit tests ────────────────────────────────────────

    #[test]
    fn truncate_ascii_within_limit() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_ascii_at_limit() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_ascii_over_limit() {
        // "hello world" is 11 bytes, limit 8 → first 8 bytes (no "..." added by this fn)
        assert_eq!(truncate_str("hello world", 8), "hello wo");
    }

    #[test]
    fn truncate_multibyte_arrow_at_boundary() {
        // '→' is 3 bytes (E2 86 92). "a→b" = [61, E2, 86, 92, 62] = 5 bytes.
        // max_bytes=4: end=4, boundary(4)=true (byte 4 is 'b'). result = "a→"
        assert_eq!(truncate_str("a→b", 4), "a→");
    }

    #[test]
    fn truncate_multibyte_arrow_mid_char() {
        // "a→b" = 5 bytes. max_bytes=3: end=3, boundary(3)? 3=92 (continuation), walk to 1.
        assert_eq!(truncate_str("a→b", 3), "a");
    }

    #[test]
    fn truncate_multibyte_arrow_exact_boundary() {
        // "a→b" = 5 bytes. max_bytes=1: end=1, boundary(1)=true. result = "a"
        assert_eq!(truncate_str("a→b", 1), "a");
    }

    #[test]
    fn truncate_emoji_4byte() {
        // '🔒' is 4 bytes. "a🔒b" = 1+4+1 = 6 bytes.
        // max_bytes=5: end=5, boundary(5)=true. result = "a🔒"
        assert_eq!(truncate_str("a🔒b", 5), "a🔒");
        // max_bytes=3: end=3, not boundary (inside emoji), walk to 1.
        assert_eq!(truncate_str("a🔒b", 3), "a");
    }

    #[test]
    fn truncate_all_multibyte() {
        // "→→→" = 9 bytes. max_bytes=6: end=6, boundary(6)=true. result = "→→"
        assert_eq!(truncate_str("→→→", 6), "→→");
        // max_bytes=4: end=4, not boundary, walk to 3. result = "→"
        assert_eq!(truncate_str("→→→", 4), "→");
    }

    #[test]
    fn truncate_empty_input() {
        assert_eq!(truncate_str("", 10), "");
        assert_eq!(truncate_str("", 0), "");
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate_str("hello", 0), "");
    }

    #[test]
    fn truncate_realistic_cat_n_with_arrows() {
        // Simulating `cat -n` output with → separators, the original crash scenario
        let line = format!("     1\t{}→ some content here", "x".repeat(190));
        // This is >200 bytes. Truncation should not panic.
        let result = truncate_str(&line, 200);
        assert!(result.len() <= 200);
        assert!(result.is_char_boundary(result.len()));
    }

    // ── print_entry integration tests ──────────────────────────────────

    fn make_id() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn print_entry_tool_result_with_arrows() {
        // Tool output with → placed to trigger truncation around byte 200
        let text = format!("{}→ rest of content", "x".repeat(198));
        let entry = Entry::ToolResult(ToolResultEntry {
            id: make_id(),
            ts: 0,
            call_id: make_id(),
            output: Some(ToolOutput::Text(text)),
            error: None,
            truncated: None,
            original_bytes: None,
            subagent_id: None,
            redacted: None,
            extra: HashMap::new(),
        });
        // Should not panic
        print_entry(&entry);
    }

    #[test]
    fn print_entry_tool_result_arrow_at_boundary_198_200() {
        // '→' at bytes 198-200: "x" * 198 + "→" (3 bytes) = 201 bytes
        let text = format!("{}→ extra", "x".repeat(198));
        let entry = Entry::ToolResult(ToolResultEntry {
            id: make_id(),
            ts: 0,
            call_id: make_id(),
            output: Some(ToolOutput::Text(text)),
            error: None,
            truncated: None,
            original_bytes: None,
            subagent_id: None,
            redacted: None,
            extra: HashMap::new(),
        });
        print_entry(&entry);
    }

    #[test]
    fn print_entry_thinking_arrow_at_boundary_99_101() {
        // '→' at bytes 99-101: "x" * 99 + "→" (3 bytes) = 102 bytes (>100 threshold)
        let text = format!("{}→ extra thinking", "x".repeat(99));
        let entry = Entry::Thinking(ThinkingEntry {
            id: make_id(),
            ts: 0,
            content: text,
            collapsed: None,
            truncated: None,
            original_bytes: None,
            subagent_id: None,
            extra: HashMap::new(),
        });
        print_entry(&entry);
    }

    #[test]
    fn print_entry_response_with_unicode() {
        let content = "• First bullet\n→ Arrow point\n📌 Pinned\n🔒 Locked item\n— Dash line\n";
        let entry = Entry::Response(ResponseEntry {
            id: make_id(),
            ts: 0,
            content: content.repeat(3),
            truncated: None,
            original_bytes: None,
            model: None,
            token_usage: None,
            subagent_id: None,
            extra: HashMap::new(),
        });
        print_entry(&entry);
    }

    #[test]
    fn print_entry_prompt_with_cjk() {
        let content = "这是一个测试提示，包含中文字符。每个中文字符占3个字节。\n第二行也有中文。";
        let entry = Entry::Prompt(PromptEntry {
            id: make_id(),
            ts: 0,
            content: content.to_string(),
            subagent_id: None,
            attachments: None,
            extra: HashMap::new(),
        });
        print_entry(&entry);
    }
}
