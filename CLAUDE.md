# CLAUDE.md

Spool is an open format (`.spool`) and CLI tool for recording, replaying, and sharing AI agent sessions. Rust workspace with three crates.

## Build & Test

```bash
cargo build
cargo test          # 24 tests, all should pass
cargo run --bin spool -- --help
```

## Crate Layout

- `crates/spool-format/` -- Core types, serialization, validation, redaction. The source of truth for the `.spool` format.
- `crates/spool-adapters/` -- Parses native agent logs into `.spool`. Claude Code adapter is implemented and verified against real logs.
- `crates/spool-cli/` -- CLI binary. Commands: browse, view, export, info, validate, play (stub), publish (stub).

## Key Docs

- [TODO.md](TODO.md) -- Current task list with dependencies and status
- [spec/SPEC.md](spec/SPEC.md) -- Format specification (~1000 lines, RFC-style)
- [docs/ROADMAP.md](docs/ROADMAP.md) -- Phased development plan (Week 1-2 done)
- [docs/DESIGN.md](docs/DESIGN.md) -- Product vision: Watch / Share / Shape

## Claude Code Log Format

Sessions are `.jsonl` files in `~/.claude/projects/<project-slug>/`. Each line has a top-level `type` field (`user`, `assistant`, `progress`, `summary`, `system`). The adapter in `claude_code.rs` handles all of this -- see the module doc comments for details.

## Conventions

- Conventional commits (`feat:`, `fix:`, `docs:`, `test:`)
- `cargo fmt` and `cargo clippy` before committing
- Dead code warnings in spool-adapters are expected (serde struct fields)
