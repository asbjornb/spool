use schemars::schema_for;
use schemars::JsonSchema;
use std::fs;
use std::path::{Path, PathBuf};

use spool_format::{
    AnnotationEntry, Entry, ErrorEntry, PromptEntry, RedactionMarkerEntry, ResponseEntry,
    SessionEntry, SubagentEndEntry, SubagentStartEntry, ThinkingEntry, ToolCallEntry,
    ToolResultEntry,
};

fn write_schema<T: JsonSchema>(
    out_dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = schema_for!(T);
    let json = serde_json::to_string_pretty(&schema)?;
    fs::write(out_dir.join(format!("{name}.json")), json)?;
    Ok(())
}

fn schema_output_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../spec/schema")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = schema_output_dir();
    fs::create_dir_all(&out_dir)?;

    write_schema::<Entry>(&out_dir, "entry")?;
    write_schema::<SessionEntry>(&out_dir, "session")?;
    write_schema::<PromptEntry>(&out_dir, "prompt")?;
    write_schema::<ThinkingEntry>(&out_dir, "thinking")?;
    write_schema::<ToolCallEntry>(&out_dir, "tool_call")?;
    write_schema::<ToolResultEntry>(&out_dir, "tool_result")?;
    write_schema::<ResponseEntry>(&out_dir, "response")?;
    write_schema::<ErrorEntry>(&out_dir, "error")?;
    write_schema::<SubagentStartEntry>(&out_dir, "subagent_start")?;
    write_schema::<SubagentEndEntry>(&out_dir, "subagent_end")?;
    write_schema::<AnnotationEntry>(&out_dir, "annotation")?;
    write_schema::<RedactionMarkerEntry>(&out_dir, "redaction_marker")?;

    Ok(())
}
