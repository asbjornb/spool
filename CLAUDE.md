# CLAUDE.md

Spool is an open format (`.spool`) and CLI tool for recording, replaying, and sharing AI agent sessions. Rust workspace with three crates.

## Build & Test

```bash
cargo build
cargo test
cargo run --bin spool -- --help
```

## Dual CLI Modes

Spool supports two distinct interaction modes:

- **Interactive TUI** (for humans): `spool` opens an interactive session browser; `spool <path>` opens the replay editor. Keyboard-driven with vim-style navigation.
- **CLI commands** (for agents/scripts): `spool list`, `spool info`, `spool view`, `spool search`, `spool export`, `spool validate`. All output to stdout. Use `--json` for machine-readable output.

See [docs/CLI.md](docs/CLI.md) for the full CLI reference with JSON schemas and usage examples.

## Crate Layout

- `crates/spool-format/` -- Core types, serialization, validation, redaction. The source of truth for the `.spool` format.
- `crates/spool-adapters/` -- Parses native agent logs into `.spool`. Claude Code and Codex adapters implemented.
- `crates/spool-cli/` -- CLI binary with TUI (Library + Editor views) and non-interactive commands (list, info, view, search, export, validate).

## Key Docs

- [TODO.md](TODO.md) -- Current task list with dependencies and status
- [spec/SPEC.md](spec/SPEC.md) -- Format specification (~1000 lines, RFC-style)
- [docs/ROADMAP.md](docs/ROADMAP.md) -- Phased development plan (Week 1-2 done)
- [docs/DESIGN.md](docs/DESIGN.md) -- Product vision: Watch / Share / Shape
- [docs/CLI.md](docs/CLI.md) -- CLI reference for both humans and agents

## Claude Code Log Format

Sessions are `.jsonl` files in `~/.claude/projects/<project-slug>/`. Each line has a top-level `type` field (`user`, `assistant`, `progress`, `summary`, `system`). The adapter in `claude_code.rs` handles all of this -- see the module doc comments for details.

## Conventions

- Conventional commits (`feat:`, `fix:`, `docs:`, `test:`)
- `cargo fmt` and `cargo clippy` before committing
- Dead code warnings in spool-adapters are expected (serde struct fields)
