# Spool TODO

Current state as of 2026-02-01. All items below are ordered by dependency.

## Done

- [x] **Project setup** -- Rust workspace, 3 crates, GitHub repo at asbjornb/spool
- [x] **Format types** -- All 11 entry types, serde, validation, forward compat
- [x] **Redaction engine** -- Regex-based secret detection (API keys, tokens, emails, IPs, JWTs), destructive replacement
- [x] **Claude Code adapter** -- Reads real `.jsonl` logs from `~/.claude/projects/`, uses `sessions-index.json` for metadata, real timestamps, tool call/result correlation
- [x] **CLI commands** -- `browse` (basic list), `view`, `export` (with `--trim` and `--redact`), `info`, `validate`
- [x] **Build is green** -- `cargo build` + `cargo test` (46 tests pass)

## Format Improvements (from Claude Code session analysis)

These are improvements inspired by studying the native Claude Code `.jsonl` format.
Not all need to happen now, but they should be tracked.

### Spec changes

- [x] **Add `files_modified` to Session entry** -- List of file paths the agent touched.
  Useful for pattern analysis, search, and understanding session scope.
  Optional field, adapters populate it from tool call/result data.

- [x] **Add `token_usage` to Response entries** -- `input_tokens`, `output_tokens`,
  `cache_read_tokens`, `cache_creation_tokens`. Enables cost analysis per-session
  and per-turn. Optional field, adapters extract from API usage data.

- [x] **Add `model` to Response entries** -- Which model produced this response.
  Already available in Claude Code logs, useful for comparing model behavior.

- [x] **Document idle gap compression for players** -- Spec should RECOMMEND that
  players compress gaps before Prompt entries (user think-time) to a max of e.g.
  2 seconds. Avoids dead air during replay without changing the format. This is a
  viewer convention, not a format change. Added as Appendix C in SPEC.md.

- [x] **Add `first_prompt` to Session entry** -- First user prompt text (truncated).
  Useful for browsing/indexing when no title is set. Adapters already have this data.

### Adapter improvements

- [x] **Populate `files_modified`** in Claude Code adapter -- Scan ToolCall entries
  for Write/Edit/NotebookEdit operations, collect unique paths.

- [x] **Extract token usage** from Claude Code assistant messages -- The `usage` block
  is already in the raw data, just needs mapping to spool fields.

- [x] **Extract model name** from Claude Code assistant messages -- Available in
  `message.model` field of raw logs.

- [x] **Filter prompt-suggestion subagents** -- These `agent-aprompt_suggestion-*.jsonl`
  files are UI-internal and should not appear in converted sessions.

## Phase 1: Watch -- what's left

### Ready now (no blockers)

- [x] **Build TUI session browser** (ratatui)
  - Session list panel: sorted by date, shows title/agent/date/duration
  - Preview panel: shows entries for selected session
  - Keyboard nav: j/k, arrows, / search, q quit
  - Filter by agent type, search by text
  - Enter to open detail view, e to export, r to export+redact
  - ratatui 0.28 + crossterm 0.28 already in workspace deps

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

- [x] **Playback mode** (`spool play`)
  - Step through entries respecting timestamps
  - Idle gap compression (user think-time capped at 2s)
  - Thinking compression (long thinking gaps capped at 2s)
  - Controls: space pause/resume, h/l step, +/- speed, j/k scroll, g/G start/end, q quit
  - Speed: 0.25x to 16x, progress bar with timeline position
  - Supports .spool files and raw Claude Code session logs

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

Architecture decided: Cloudflare-native stack (Pages + Workers + D1 + R2).
See [docs/UNSPOOL_ARCHITECTURE.md](docs/UNSPOOL_ARCHITECTURE.md) for full design.
Scaffolding (Worker, schema, config) in `service/`.

- [ ] **Web viewer component** (web/viewer/)
  - Renders .spool in browser, playback controls, tool filtering, search
  - Deep links (#entry-id), responsive, embeddable
  - SvelteKit or SolidStart (small bundles, SSR-capable)

- [ ] **unspool.dev backend** (service/)
  - Cloudflare Workers API, D1 metadata, R2 blob storage
  - GitHub + Google OAuth, upload API, public/unlisted/private visibility
  - nanoid short URLs (`unspool.dev/s/:id`)
  - TTL: 14 days anonymous, permanent for authed users
  - Implement `spool publish` command

- [ ] **Embeds + static site generator**
  - `<iframe>` embed support (`/embed/:id` minimal viewer)
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
- ~~The `agent_version` variable in claude_code.rs was not populated.~~
  Fixed: now extracted from the `version` field on raw user/assistant lines (Claude
  Code CLI version like "2.1.29").

## Storage Footprint Estimates (for unspool.dev planning)

Based on analysis of real Claude Code sessions (18 sessions, single user):

- **Raw Claude Code logs**: ~479 KB average, ~427 KB median, 1.8 MB max
- **Spool format**: ~40-50% of raw (strips progress events, snapshots, hooks, repeated metadata)
- **Estimated spool avg**: ~200 KB per session (short sessions), ~1 MB for longer sessions

Extrapolated for unspool.dev (assuming 5x session length for typical users):

| Scale | Uncompressed | Gzipped (~8x) |
|-------|-------------|----------------|
| Per session | ~1 MB | ~125 KB |
| Per user (100 sessions) | ~100 MB | ~12.5 MB |
| 1,000 users | ~100 GB | ~12 GB |
| 10,000 users | ~1 TB | ~120 GB |

Key: JSONL compresses very well. Store gzipped on object storage (S3/R2).
