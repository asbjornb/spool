# Spool - AI Session Recording Tool

Spool is a CLI for browsing, replaying, and sharing AI agent sessions. Use it to analyze past Claude Code sessions, export shareable recordings, and detect/redact secrets.

## Quick Reference

```bash
# Interactive TUI (for humans)
spool                    # Browse all sessions
spool <path>             # Open specific session in editor

# CLI commands (for agents/scripts)
spool list [--json]              # List sessions
spool info <path> [--json]       # Session metadata
spool view <path> [--json]       # Print content
spool search <query> [--json]    # Search sessions
spool detect <path> [--json]     # Detect secrets
spool export <path> [options]    # Export to .spool
spool validate <path>            # Validate .spool file
```

## Common Workflows

### Find and analyze a recent session

```bash
# Get the most recent session path
path=$(spool list --json | jq -r '.[0].path')

# View metadata
spool info "$path" --json

# Extract just the prompts
spool view "$path" --type prompt --json

# Extract tool calls
spool view "$path" --type tool_call --json
```

### Export with redaction

```bash
# Preview what secrets would be redacted
spool detect session.jsonl --json

# Export with automatic redaction
spool export session.jsonl --redact --output clean.spool

# Selective redaction (skip false positives by index)
spool export session.jsonl --redact --skip 0,2 --output clean.spool
```

### Export with trimming

```bash
# Export a time range (mm:ss format)
spool export session.jsonl --trim 0:30-2:45 --output trimmed.spool
```

### Search sessions

```bash
# Find sessions mentioning a topic
spool search "authentication" --json

# Filter by agent type
spool search "refactor" --agent claude-code --json
```

## Entry Types

When using `spool view --type`, valid types are:
- `prompt` (alias: `user`) - User messages
- `response` (alias: `assistant`) - Agent responses
- `thinking` - Agent reasoning (extended thinking)
- `tool_call` - Tool invocations
- `tool_result` - Tool outputs
- `error` - Errors during session
- `annotation` (alias: `note`) - Human annotations
- `session` - Session metadata

## Tips

- Always use `--json` when parsing output programmatically
- Session paths from `spool list` work directly with other commands
- The `detect` command shows indices you can use with `--skip`
- Redaction is destructive - secrets are replaced, not masked
