# Spool

> AI agent sessions are the new code snippets. Spool is Gist for them.

You ran Claude Code for 20 minutes. It found a subtle auth bug, made three failed attempts, then nailed the fix. You want to show your team the exact moment it clicked. Today your options are: copy-paste a wall of terminal text, record a screencast where it thinks for 3 minutes straight, or dig through raw JSON logs.

Spool captures the **structure** of agent sessions -- prompts, reasoning, tool calls, results -- and lets you replay, trim, annotate, and share them.

## What Spool Is

1. **An open format** (`.spool`) -- JSONL where every entry is typed and identified. A tool call is a tool call, not a blob of text. [Read the spec](spec/SPEC.md).
2. **A local tool** (`spool`) -- Browse your existing Claude Code sessions, trim to the interesting part, redact secrets, add annotations, export.
3. **A sharing service** (spool.dev, planned) -- Publish sessions, embed in docs, discover patterns others have found.

## The Format

```jsonl
{"id":"018d5f2c-...","ts":0,"type":"session","agent":"claude-code","title":"Finding the SQL injection bug"}
{"id":"018d5f2c-...","ts":0,"type":"prompt","content":"There's a security bug in auth.py, can you find it?"}
{"id":"018d5f2c-...","ts":500,"type":"thinking","content":"I'll start by reading the authentication code..."}
{"id":"018d5f2c-...","ts":3000,"type":"tool_call","tool":"read_file","input":{"path":"src/auth.py"}}
{"id":"018d5f2c-...","ts":3200,"type":"tool_result","call_id":"...","output":"def verify_token(t):\n    query = f\"SELECT * FROM users WHERE token={t}\""}
{"id":"018d5f2c-...","ts":8000,"type":"response","content":"Found it! Line 2 has a SQL injection vulnerability."}
{"id":"a001","type":"annotation","target_id":"018d5f2c-...","content":"This is where it spots the vulnerability","author":"alex","style":"highlight"}
```

Every entry has a unique ID. IDs enable deep links, annotations, subagent nesting, filtering. Timestamps are relative milliseconds, so playback works at any speed. Redaction is destructive -- secrets are replaced before export, never stored in the `.spool` file.

Full specification: [spec/SPEC.md](spec/SPEC.md)

## How It Works

Spool doesn't require you to record anything upfront. Agents already store their logs:

- **Claude Code**: `~/.claude/projects/*/sessions/`
- **Codex**: `~/.codex/logs/` (planned)

Spool reads those logs, converts them to the structured `.spool` format, and lets you work with them. Sharing is **retrospective, not premeditated** -- you had a good session, now you share it.

```bash
# Browse your Claude Code sessions
spool browse

# View a session in the terminal
spool view session.spool

# Export with trimming and automatic secret redaction
spool export <session> --trim 0:30-2:45 --redact --output highlights.spool

# Show session stats
spool info session.spool

# Validate a .spool file against the spec
spool validate session.spool
```

## Project Status

Early development. Phase 1 is in progress.

| Phase | Focus | Status |
|-------|-------|--------|
| **Watch** | Local CLI/TUI for browsing, replaying, trimming sessions | In progress |
| **Share** | spool.dev, web viewer, embeds, static site generator | Planned |
| **Shape** | CLAUDE.md editing, context file management, team sharing | Planned |

### What works now

- `.spool` format types, serialization, validation
- Secret detection and destructive redaction (API keys, tokens, emails, IPs, etc.)
- Claude Code log adapter (reads existing sessions, converts to `.spool`)
- CLI commands: `view`, `export` (with trim + redact), `info`, `validate`, `browse` (basic list)

### What's next

- TUI browser with keyboard navigation and session preview
- Playback mode with thinking compression
- Interactive trimming and annotation
- Codex adapter
- Web viewer

### Supported Agents

| Agent | Status |
|-------|--------|
| Claude Code | In progress (primary target) |
| Codex CLI | Planned |
| Cursor | Planned |
| Aider | Planned |

## Differentiation

[yaplog.dev](https://yaplog.dev) exists in this space. Spool differentiates on:

| | yaplog | Spool |
|-|--------|-------|
| Format | Opaque / hosted | Open spec, JSONL, self-hostable |
| Recording | CLI hook (`yap import`) | Browse existing agent logs retroactively |
| Scope | Full sessions | Trim to moments -- share a 3-minute segment from a 2-hour session |
| Editing | View-only | First-class annotations and highlights |
| Playback | Static scroll | Playback with thinking compression |
| Embeds | No | Planned (iframe/script tag) |

## Architecture

```
crates/
  spool-format/      Core types, serialization, validation, redaction
  spool-adapters/    Agent log parsers (Claude Code adapter implemented)
  spool-cli/         CLI application (TUI planned)
spec/
  SPEC.md            Format specification (RFC-style, ~1000 lines)
  examples/          Example .spool files
  schema/            JSON Schema (planned)
docs/
  DESIGN.md          Product vision and philosophy
  ROADMAP.md         Development phases
web/
  viewer/            Web viewer component (planned)
service/             Backend for spool.dev (planned)
```

## Building

```bash
# Requires Rust 1.75+
cargo build
cargo test

# Run the CLI
cargo run --bin spool -- --help
cargo run --bin spool -- view spec/examples/simple-session.spool
```

## Documentation

- [Format Specification](spec/SPEC.md) -- Complete `.spool` format reference
- [Design Philosophy](docs/DESIGN.md) -- Product vision, three modes (Watch/Share/Shape)
- [Development Roadmap](docs/ROADMAP.md) -- Phased development plan
- [Contributing](CONTRIBUTING.md) -- How to help

## License

MIT -- see [LICENSE](LICENSE)
