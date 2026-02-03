//! Export command - Convert and export sessions to .spool format.

use anyhow::{Context, Result};
use std::path::Path;

use super::agent::load_spool_or_log;
use super::detect::{apply_redactions, detect_secrets};

pub fn run(
    source: &Path,
    output: Option<&Path>,
    trim: Option<&str>,
    redact: bool,
    dry_run: bool,
    skip: Option<&str>,
    json: bool,
) -> Result<()> {
    // Determine if source is already a .spool file or an agent log
    let mut file = load_spool_or_log(source)?;

    // Apply trimming if specified
    if let Some(trim_range) = trim {
        let (start, end) = parse_trim_range(trim_range)?;
        if !dry_run && !json {
            println!("   Trimming: {}ms - {}ms", start, end);
        }
        file.trim(start, end);
    }

    // Handle redaction
    if redact {
        let detections = detect_secrets(&file);

        if dry_run {
            // Dry-run mode: just show what would be redacted
            if json {
                println!("{}", serde_json::to_string_pretty(&detections)?);
            } else if detections.is_empty() {
                println!("No secrets detected.");
            } else {
                println!("Would redact {} secret(s):\n", detections.len());
                for d in &detections {
                    println!(
                        "  [{}] {} in {} (entry {})",
                        d.index, d.category, d.entry_type, d.entry_index
                    );
                    println!("      Match: {}", truncate(&d.matched, 60));
                    println!();
                }
                println!(
                    "Run without --dry-run to apply redactions, or use --skip 0,1,2 to exclude."
                );
            }
            return Ok(());
        }

        // Parse skip indices
        let skip_indices = parse_skip_indices(skip)?;

        if !json {
            println!("📤 Exporting session...");
            println!("   Source: {:?}", source);
            println!("   Applying redaction...");
        }

        let redaction_count = detections
            .iter()
            .filter(|d| !skip_indices.contains(&d.index))
            .count();

        apply_redactions(&mut file, &detections, &skip_indices);

        if !json && redaction_count > 0 {
            println!("   Redacted {} secret(s)", redaction_count);
            if !skip_indices.is_empty() {
                println!("   Skipped {} detection(s)", skip_indices.len());
            }
        }
    } else if !dry_run && !json {
        println!("📤 Exporting session...");
        println!("   Source: {:?}", source);
    }

    if dry_run {
        // If dry_run without redact, nothing to show
        if !json {
            println!("Nothing to preview (use --redact --dry-run to preview redactions)");
        }
        return Ok(());
    }

    // Determine output path
    let output_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let stem = source.file_stem().unwrap_or_default().to_string_lossy();
        source.with_file_name(format!("{}.spool", stem))
    });

    // Write output
    file.write_to_path(&output_path)
        .with_context(|| format!("Failed to write: {:?}", output_path))?;

    if json {
        // JSON output for the export result
        let result = serde_json::json!({
            "output": output_path.to_string_lossy(),
            "entries": file.entries.len(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("✅ Exported to: {:?}", output_path);
        println!("   Entries: {}", file.entries.len());
    }

    Ok(())
}

fn parse_skip_indices(skip: Option<&str>) -> Result<Vec<usize>> {
    match skip {
        None | Some("") => Ok(Vec::new()),
        Some(s) => s
            .split(',')
            .map(|part| {
                part.trim()
                    .parse::<usize>()
                    .with_context(|| format!("Invalid skip index: {}", part))
            })
            .collect(),
    }
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

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
