# Spool CLI Reference

Spool has two modes of operation:

- **Interactive (TUI)**: `spool` or `spool <path>` — for humans browsing and replaying sessions
- **CLI commands**: `spool list`, `spool info`, etc. — for agents, scripts, and pipelines

## Quick Reference

```
spool                          # Open interactive TUI browser
spool <path>                   # Open file in interactive TUI editor
spool list [--json]            # List sessions to stdout
spool info <path> [--json]     # Show session metadata
spool view <path> [--json]     # Print session content
spool search <query> [--json]  # Search sessions
spool export <path> [options]  # Convert/trim/redact to .spool
spool validate <path>          # Check .spool file validity
```

---

## Commands for Agents / Scripts

These commands print to stdout and never enter interactive mode. All support `--json` for machine-readable output.

### `spool list`

List all discovered agent sessions (Claude Code, Codex, etc.).

```bash
# Human-readable table
spool list

# JSON output for parsing
spool list --json

# Filter by agent
spool list --agent claude-code --json

# Limit results
spool list -n 5 --json
```

**JSON output schema:**
```json
[
  {
    "path": "/home/user/.claude/projects/.../abc123.jsonl",
    "agent": "claude-code",
    "title": "Fix authentication bug",
    "modified": "2026-02-01T14:32:00+00:00",
    "messages": 47,
    "project": "/home/user/myproject"
  }
]
```

### `spool info <path>`

Show metadata and statistics for a session.

```bash
# Human-readable
spool info ~/.claude/projects/myproject/abc123.jsonl

# JSON output
spool info session.spool --json
```

**JSON output schema:**
```json
{
  "title": "Fix authentication bug",
  "agent": "claude-code",
  "agent_version": "2.1.29",
  "recorded_at": "2026-02-01T14:32:00+00:00",
  "format_version": "1.0",
  "duration_ms": 185000,
  "duration_display": "3:05",
  "entry_count": 47,
  "prompts": 5,
  "responses": 5,
  "tool_calls": 32,
  "errors": 0,
  "annotations": 0,
  "tools_used": ["Bash", "Read", "Edit", "Grep"],
  "tags": null,
  "files_modified": ["src/auth.rs", "tests/auth_test.rs"],
  "trimmed": false
}
```

### `spool view <path>`

Print session content to stdout.

```bash
# Full session (human-readable)
spool view session.spool

# JSON array of all entries
spool view session.spool --json

# Filter to specific entry types
spool view session.spool --type prompt
spool view session.spool --type response --json
spool view session.spool --type tool_call --json
```

**Entry type filters:** `prompt` (alias: `user`), `response` (alias: `assistant`), `thinking`, `tool_call`, `tool_result`, `error`, `annotation` (alias: `note`), `session`

**JSON output:** Returns the raw spool entry objects as a JSON array. Each entry has a `type` field (`prompt`, `response`, `tool_call`, etc.) and type-specific fields.

### `spool search <query>`

Search sessions by title, project directory, or content (prompts and responses).

```bash
# Search by keyword
spool search "authentication" --json

# Filter by agent
spool search "refactor" --agent codex --json

# Limit results
spool search "bug fix" -n 5 --json
```

**JSON output schema:**
```json
[
  {
    "path": "/home/user/.claude/projects/.../abc123.jsonl",
    "agent": "claude-code",
    "title": "Fix authentication bug",
    "modified": "2026-02-01T14:32:00+00:00",
    "matched_content": "...found the authentication bug in the middleware..."
  }
]
```

### `spool export <path>`

Convert agent logs to `.spool` format with optional trimming and redaction.

```bash
# Basic export
spool export session.jsonl --output session.spool

# With trimming (mm:ss format)
spool export session.jsonl --trim 0:30-2:45 --output trimmed.spool

# With redaction (removes API keys, tokens, emails, etc.)
spool export session.jsonl --redact --output redacted.spool
```

### `spool validate <path>`

Check a `.spool` file for format errors.

```bash
spool validate session.spool
```

---

## Interactive TUI Mode

For humans who want to browse, replay, and edit sessions interactively.

```bash
spool           # Opens the Library view (session browser)
spool <path>    # Opens the Editor view directly on a file
```

### Library View (Session Browser)

| Key | Action |
|-----|--------|
| j/k, Up/Down | Navigate sessions |
| Enter | Open selected session |
| / | Search by title/project |
| g/G | Jump to top/bottom |
| h/l | Scroll preview |
| q | Quit |

### Editor View (Session Replay)

| Key | Action |
|-----|--------|
| Space | Play/pause |
| h/l | Step backward/forward |
| +/- | Adjust speed |
| j/k | Scroll content |
| s | Mark trim start |
| d | Mark trim end |
| e | Export trimmed session |
| a | Add annotation |
| i | Show session info |
| q | Back to library / quit |

---

## Common Patterns

### Agent workflow: find and analyze a session

```bash
# Find recent sessions
path=$(spool list --json | jq -r '.[0].path')

# Get metadata
spool info "$path" --json

# Extract just the prompts
spool view "$path" --type prompt --json

# Extract tool calls
spool view "$path" --type tool_call --json
```

### Search and export

```bash
# Find sessions about a topic
spool search "database migration" --json | jq -r '.[].path'

# Export with redaction
spool export session.jsonl --redact --output clean.spool
```

### Pipe session content

```bash
# Get all responses as text
spool view session.spool --type response --json | jq -r '.[].content'

# Count tool calls by type
spool view session.spool --type tool_call --json | jq -r '.[].tool' | sort | uniq -c | sort -rn
```
