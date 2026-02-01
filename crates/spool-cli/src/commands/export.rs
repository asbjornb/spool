//! Export command - Convert and export sessions to .spool format.

use anyhow::{Context, Result};
use spool_adapters::{claude_code, codex, AgentType};
use spool_format::{SecretDetector, SpoolFile};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn run(source: &Path, output: Option<&Path>, trim: Option<&str>, redact: bool) -> Result<()> {
    println!("📤 Exporting session...");
    println!("   Source: {:?}", source);

    // Determine if source is already a .spool file or an agent log
    let mut file = if source.extension().map(|e| e == "spool").unwrap_or(false) {
        SpoolFile::from_path(source)?
    } else {
        let agent = detect_agent_from_log(source)?;
        let session_info = spool_adapters::SessionInfo {
            path: source.to_path_buf(),
            agent,
            created_at: None,
            modified_at: None,
            title: None,
            project_dir: None,
            message_count: None,
        };
        match agent {
            AgentType::ClaudeCode => claude_code::convert(&session_info)?,
            AgentType::Codex => codex::convert(&session_info)?,
            _ => anyhow::bail!("Unsupported agent log: {:?}", source),
        }
    };

    // Apply trimming if specified
    if let Some(trim_range) = trim {
        let (start, end) = parse_trim_range(trim_range)?;
        println!("   Trimming: {}ms - {}ms", start, end);
        file.trim(start, end);
    }

    // Apply redaction if requested
    if redact {
        println!("   Applying redaction...");
        let detector = SecretDetector::with_defaults();
        let mut redaction_count = 0;

        for entry in &mut file.entries {
            // Redact content in various entry types
            match entry {
                spool_format::Entry::Prompt(p) => {
                    let (redacted, secrets) = detector.redact(&p.content);
                    redaction_count += secrets.len();
                    p.content = redacted;
                }
                spool_format::Entry::Response(r) => {
                    let (redacted, secrets) = detector.redact(&r.content);
                    redaction_count += secrets.len();
                    r.content = redacted;
                }
                spool_format::Entry::ToolResult(tr) => {
                    if let Some(spool_format::ToolOutput::Text(ref mut text)) = tr.output {
                        let (redacted, secrets) = detector.redact(text);
                        redaction_count += secrets.len();
                        *text = redacted;
                    }
                }
                spool_format::Entry::Thinking(t) => {
                    let (redacted, secrets) = detector.redact(&t.content);
                    redaction_count += secrets.len();
                    t.content = redacted;
                }
                _ => {}
            }
        }

        if redaction_count > 0 {
            println!("   Redacted {} secret(s)", redaction_count);
        }
    }

    // Determine output path
    let output_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let stem = source.file_stem().unwrap_or_default().to_string_lossy();
        source.with_file_name(format!("{}.spool", stem))
    });

    // Write output
    file.write_to_path(&output_path)
        .with_context(|| format!("Failed to write: {:?}", output_path))?;

    println!("✅ Exported to: {:?}", output_path);
    println!("   Entries: {}", file.entries.len());

    Ok(())
}

fn detect_agent_from_log(path: &Path) -> Result<AgentType> {
    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse JSON line in {:?}", path))?;
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        return Ok(match kind {
            "session_meta" => AgentType::Codex,
            "user" | "assistant" | "progress" | "summary" | "system" => AgentType::ClaudeCode,
            _ => AgentType::Unknown,
        });
    }
    Ok(AgentType::Unknown)
}

fn parse_trim_range(range: &str) -> Result<(u64, u64)> {
    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid trim range format. Use START-END (e.g., 0:30-2:45 or 30-165)");
    }

    let start = parse_time(parts[0])?;
    let end = parse_time(parts[1])?;

    if start >= end {
        anyhow::bail!("Start time must be less than end time");
    }

    Ok((start, end))
}

fn parse_time(time: &str) -> Result<u64> {
    if time.contains(':') {
        // mm:ss format
        let parts: Vec<&str> = time.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid time format: {}", time);
        }
        let minutes: u64 = parts[0].parse()?;
        let seconds: u64 = parts[1].parse()?;
        Ok((minutes * 60 + seconds) * 1000)
    } else {
        // Seconds format
        let seconds: u64 = time.parse()?;
        Ok(seconds * 1000)
    }
}
