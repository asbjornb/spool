# Spool Development Roadmap

This document outlines the phased development approach for Spool.

## Overview

| Phase | Focus | Timeline | Status |
|-------|-------|----------|--------|
| 1 | **Watch** — Local TUI tool | 6 weeks | 🔨 Near Complete |
| 2 | **Share** — unspool.dev + embeds | 6-8 weeks | 📋 Planned |
| 3 | **Shape** — Context editing | 6-8 weeks | 📋 Planned |

---

## Phase 1: Watch (Local TUI Tool)

**Goal**: A command-line tool to browse, replay, trim, and export agent sessions.

**Why first**: This delivers immediate value without any infrastructure. Users can understand their AI sessions better today.

### Week 1-2: Foundation

- [x] Project setup (Rust workspace, CI/CD, GitHub repo)
- [x] Core format types (`spool-format` crate)
  - [x] Entry types (session, prompt, thinking, tool_call, etc.)
  - [x] Serialization/deserialization
  - [x] Validation
- [x] Claude Code adapter (`spool-adapters` crate)
  - [x] Locate log files (`~/.claude/projects/*/*.jsonl`)
  - [x] Parse Claude Code JSONL format (verified against real logs)
  - [x] Convert to Spool format with real timestamps
  - [x] Use sessions-index.json for fast metadata
- [x] Secret detection and destructive redaction
- [x] CLI commands: browse (basic list), view, export (with trim + redact), info, validate

### Week 3-4: TUI Browser

- [x] Basic TUI framework (ratatui)
- [x] Session list view
  - [x] Sort by date, filter by project
  - [x] Preview on hover/select
- [x] Session detail view
  - [x] Entry list with types
  - [x] Expand/collapse thinking blocks
  - [x] Syntax highlighting for code
- [x] Keyboard navigation
  - [x] vim-style (`j/k`, `/` search)
  - [x] Arrow keys

### Week 5: Playback & Editing

- [x] Playback mode
  - [x] Step through entries
  - [x] Thinking compression (show progress bar, skip duration)
  - [x] Adjustable speed (0.25x to 16x)
- [x] Interactive trim UI
  - [x] Mark start/end points in TUI (`[`/`]`)
  - [x] Preview trimmed result
  - [x] Export selection (`x`)
- [x] Annotation support
  - [x] Add annotations to entries from TUI (`a`)
  - [x] Style selection (highlight/success/warning/info)
  - [x] Annotations persisted in exported .spool file

### Week 6: Export & Polish

- [ ] Interactive redaction review UI (confirm/dismiss detections)
- [x] Codex adapter
- [x] More example .spool files
- [x] JSON Schema for the format
- [x] Format improvements: `files_modified`, `token_usage`, `model` on entries
- [x] Idle gap compression in player (compress user think-time to max 2s)
- [ ] Release v0.1.0

### Deliverables

```bash
# Browse sessions from Claude Code
spool browse

# Replay a session in TUI
spool play <session-id>

# Export with trimming and redaction
spool export <session-id> --trim 0:30-2:45 --redact --output session.spool

# View a .spool file
spool view session.spool
```

---

## Phase 2: Share (unspool.dev + Embeds)

**Goal**: Publish sessions to the web, embed in docs, discover patterns.

**Why second**: Once people can create `.spool` files locally, they'll want to share them. This is the network effect phase.

### Week 1-2: Web Viewer

- [ ] Static web viewer component
  - [ ] Renders `.spool` files
  - [ ] Playback controls
  - [ ] Tool filtering
  - [ ] Search within session
- [ ] Deep link support (`#entry-id`)
- [ ] Responsive design

### Week 3-4: unspool.dev Backend

- [ ] User authentication (GitHub OAuth)
- [ ] Session upload API (accept gzipped .spool, ~125 KB avg per session)
- [ ] Public/private visibility
- [ ] Unique URLs (`unspool.dev/username/slug`)
- [ ] Storage: gzipped spool on object storage (S3/R2). ~12 GB per 1K users.

### Week 5-6: Discovery & Social

- [ ] Browse by agent, tool, tag
- [ ] Full-text search
- [ ] "Trending" sessions
- [ ] Follow creators
- [ ] View counts, stars

### Week 7-8: Embeds & Static Sites

- [ ] Embeddable `<iframe>` / `<script>` tag
- [ ] OpenGraph/Twitter cards for link previews
- [ ] Static site generator (`spool build`)
- [ ] GitHub Pages integration

### Deliverables

```bash
# Publish to unspool.dev
spool publish session.spool
# → https://unspool.dev/alex/auth-bug-discovery

# Generate static site
spool build ./sessions --output ./site

# Embed code
<script src="https://unspool.dev/embed.js" data-session="alex/auth-bug"></script>
```

---

## Phase 3: Shape (Context Editing)

**Goal**: Edit CLAUDE.md files, skills, and commands across all repos from one place.

**Why third**: This builds on Watch (understand context) and Share (propagate context) to complete the vision.

### Week 1-2: Context Discovery

- [ ] Scan for context files
  - [ ] CLAUDE.md files
  - [ ] Skills directories
  - [ ] Command definitions
- [ ] Unified context view
- [ ] Git status integration

### Week 3-4: Editing

- [ ] In-TUI editor for context files
- [ ] Syntax highlighting (markdown, YAML)
- [ ] Validation (schema checking)
- [ ] Git-aware saves (commit message prompts)

### Week 5-6: Snapshots

- [ ] Snapshot system for non-git files
- [ ] Diff viewer
- [ ] Restore from snapshot

### Week 7-8: Team Sharing

- [ ] Package skills/commands for sharing
- [ ] unspool.dev skill catalog
- [ ] One-click install
- [ ] Usage analytics

### Deliverables

```bash
# View all context files
spool context list

# Edit a context file
spool context edit ./CLAUDE.md

# Create snapshot
spool context snapshot

# Share a skill
spool share ./skills/security-review/
```

---

## Technical Decisions

### Language: Rust

- Fast startup (important for TUI)
- Single binary distribution
- Strong typing for format handling
- Good TUI libraries (ratatui)

### Format: JSONL

- Streamable (no need to load entire file)
- Human-readable (can inspect with `cat`)
- Git-friendly (line-based diffs)
- Well-supported (every language has JSON)

### Architecture

```
┌─────────────────┐
│   spool-cli     │  TUI application
├─────────────────┤
│  spool-format   │  Core types, validation
├─────────────────┤
│ spool-adapters  │  Agent log parsers
└─────────────────┘
```

Each crate is independently versioned and can be used as a library.

---

## Success Criteria

### Phase 1 ✓ if:
- ~~Can browse Claude Code sessions~~ ✅
- ~~Can replay with thinking compression~~ ✅
- ~~Can trim and export `.spool` files~~ ✅
- ~~Redaction works reliably~~ ✅ (interactive review UI remaining)

### Phase 2 ✓ if:
- 100+ sessions published to unspool.dev
- Embeds work in GitHub READMEs
- Users discover sessions via search

### Phase 3 ✓ if:
- Users edit context files through Spool
- Teams share skills via unspool.dev
- Measurable time saved debugging

---

## Open Technical Questions

1. ~~**Web viewer tech stack**~~ — SvelteKit or SolidStart (see [UNSPOOL_ARCHITECTURE.md](UNSPOOL_ARCHITECTURE.md))
2. ~~**unspool.dev hosting**~~ — Cloudflare (Pages + Workers + D1 + R2). $0-20/month at moderate scale. See [UNSPOOL_ARCHITECTURE.md](UNSPOOL_ARCHITECTURE.md)
3. **Search backend** — TBD. D1 full-text may suffice initially; evaluate Meilisearch/Typesense at scale.
4. ~~**Authentication**~~ — GitHub + Google OAuth. Custom JWT. Device flow for CLI auth (see architecture doc).
5. **Parallel subagents in replay** — Multiple subagents can run concurrently.
   Flat timeline interleaves their events by timestamp. Viewer could show
   side-by-side panels keyed by subagent ID. No format change needed, but
   the viewer needs to handle this gracefully.
6. ~~**Token usage / cost tracking**~~ — Done. `token_usage` and `model` fields added to Response entries.

---

*Last updated: February 2026*
