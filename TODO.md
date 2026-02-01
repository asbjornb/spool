# Spool TODO

Current state as of 2026-02-01. All items below are ordered by dependency.

## Done

- [x] **Project setup** -- Rust workspace, 3 crates, GitHub repo at asbjornb/spool
- [x] **Format types** -- All 11 entry types, serde, validation, forward compat
- [x] **Redaction engine** -- Regex-based secret detection (API keys, tokens, emails, IPs, JWTs), destructive replacement
- [x] **Claude Code adapter** -- Reads real `.jsonl` logs from `~/.claude/projects/`, uses `sessions-index.json` for metadata, real timestamps, tool call/result correlation
- [x] **CLI commands** -- `browse` (basic list), `view`, `export` (with `--trim` and `--redact`), `info`, `validate`
- [x] **Build is green** -- `cargo build` + `cargo test` (24 tests pass)

## Phase 1: Watch -- what's left

### Ready now (no blockers)

- [ ] **Build TUI session browser** (ratatui)
  - Session list panel: sorted by date, shows title/agent/date/duration
  - Preview panel: shows entries for selected session
  - Keyboard nav: j/k, arrows, / search, q quit
  - Filter by agent type, search by text
  - Enter to open detail view, e to export, r to export+redact
  - ratatui 0.28 + crossterm 0.28 already in workspace deps
  - Current `browse.rs` is a println stub -- replace entirely

- [ ] **Build Codex CLI adapter** (`crates/spool-adapters/src/codex.rs`)
  - Research where Codex stores logs (~/.codex/ ?)
  - Implement find_sessions() + convert()
  - Register in lib.rs (currently commented out)
  - Update browse to search both Claude Code and Codex

- [ ] **Add more example .spool files** (spec/examples/)
  - debugging-session.spool -- multi-step debug with iteration
  - refactoring-session.spool -- multi-file changes
  - long-session-trimmed.spool -- shows trim markers

- [ ] **Generate JSON Schema** (spec/schema/)
  - Schema for each entry type, usable by editors and other implementations

### Blocked by TUI browser

- [ ] **Playback mode** (`spool play`)
  - Step through entries respecting timestamps
  - Thinking compression (3min thinking -> 2sec progress bar)
  - Controls: space pause, arrows step, +/- speed, q quit
  - Current `play.rs` is a stub

- [ ] **Interactive trimming**
  - `[` mark start, `]` mark end in TUI
  - Preview trimmed result
  - `x` to export selection

- [ ] **Interactive annotations**
  - `a` on any entry to add annotation
  - Text input, style selection (highlight/success/warning/info)
  - Annotations persisted in exported .spool file

- [ ] **Interactive redaction review**
  - Before export: show detected secrets with context
  - Confirm/dismiss each detection
  - Manually mark additional text
  - Preview redacted output

## Phase 2: Share

- [ ] **Web viewer component** (web/viewer/)
  - Renders .spool in browser, playback controls, tool filtering, search
  - Deep links (#entry-id), responsive, embeddable
  - Tech TBD: Svelte / vanilla JS / Preact

- [ ] **unspool.dev backend** (service/)
  - GitHub OAuth, upload API, public/private, unique URLs
  - Implement `spool publish` command

- [ ] **Embeds + static site generator**
  - `<script>` / `<iframe>` embed support
  - OpenGraph cards for link previews
  - `spool build` generates static site from .spool directory

## Phase 3: Shape

- [ ] Context file discovery (CLAUDE.md, skills, commands)
- [ ] In-TUI editor
- [ ] Snapshot system for non-git files
- [ ] Team sharing via unspool.dev

## Notes

- The Claude Code adapter was fully rewritten and verified against real session logs.
  The key insight: logs are `.jsonl` (not `.json`), stored directly in project dirs
  (not in a `sessions/` subdirectory), and each line has a top-level `type` field.
- `spool.dev` was renamed to `unspool.dev` across the codebase.
- Warnings in spool-adapters are all `dead_code` for struct fields needed by serde
  deserialization -- not worth suppressing.
- The `agent_version` variable in claude_code.rs is declared `mut` but never assigned.
  Remove the `mut` or populate it from the `version` field on raw lines (it's the
  Claude Code CLI version like "2.1.29").
