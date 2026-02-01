# Spool Development Roadmap

This document outlines the phased development approach for Spool.

## Overview

| Phase | Focus | Timeline | Status |
|-------|-------|----------|--------|
| 1 | **Watch** — Local TUI tool | 6 weeks | 🔨 In Progress |
| 2 | **Share** — spool.dev + embeds | 6-8 weeks | 📋 Planned |
| 3 | **Shape** — Context editing | 6-8 weeks | 📋 Planned |

---

## Phase 1: Watch (Local TUI Tool)

**Goal**: A command-line tool to browse, replay, trim, and export agent sessions.

**Why first**: This delivers immediate value without any infrastructure. Users can understand their AI sessions better today.

### Week 1-2: Foundation

- [ ] Project setup (Rust workspace, CI/CD)
- [ ] Core format types (`spool-format` crate)
  - [ ] Entry types (session, prompt, thinking, tool_call, etc.)
  - [ ] Serialization/deserialization
  - [ ] Validation
- [ ] Claude Code adapter (`spool-adapters` crate)
  - [ ] Locate log files (`~/.claude/projects/*/sessions/`)
  - [ ] Parse Claude Code JSON format
  - [ ] Convert to Spool format

### Week 3-4: TUI Browser

- [ ] Basic TUI framework (ratatui)
- [ ] Session list view
  - [ ] Sort by date, filter by project
  - [ ] Preview on hover/select
- [ ] Session detail view
  - [ ] Entry list with types
  - [ ] Expand/collapse thinking blocks
  - [ ] Syntax highlighting for code
- [ ] Keyboard navigation
  - [ ] vim-style (`j/k`, `/` search)
  - [ ] Arrow keys

### Week 5: Playback & Editing

- [ ] Playback mode
  - [ ] Step through entries
  - [ ] Thinking compression (show progress bar, skip duration)
  - [ ] Adjustable speed (1x, 2x, 4x)
- [ ] Trim functionality
  - [ ] Mark start/end points
  - [ ] Preview trimmed result
- [ ] Annotation support
  - [ ] Add annotations to entries
  - [ ] Edit/delete annotations

### Week 6: Export & Polish

- [ ] Redaction
  - [ ] Auto-detect secrets (regex patterns)
  - [ ] Review/confirm UI
  - [ ] Destructive replacement
- [ ] Export to `.spool` file
- [ ] Codex adapter
- [ ] Documentation & examples
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

## Phase 2: Share (spool.dev + Embeds)

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

### Week 3-4: spool.dev Backend

- [ ] User authentication (GitHub OAuth)
- [ ] Session upload API
- [ ] Public/private visibility
- [ ] Unique URLs (`spool.dev/username/slug`)

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
# Publish to spool.dev
spool publish session.spool
# → https://spool.dev/alex/auth-bug-discovery

# Generate static site
spool build ./sessions --output ./site

# Embed code
<script src="https://spool.dev/embed.js" data-session="alex/auth-bug"></script>
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
- [ ] spool.dev skill catalog
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
- Can browse Claude Code sessions
- Can replay with thinking compression
- Can trim and export `.spool` files
- Redaction works reliably

### Phase 2 ✓ if:
- 100+ sessions published to spool.dev
- Embeds work in GitHub READMEs
- Users discover sessions via search

### Phase 3 ✓ if:
- Users edit context files through Spool
- Teams share skills via spool.dev
- Measurable time saved debugging

---

## Open Technical Questions

1. **Web viewer tech stack** — React? Svelte? Vanilla JS?
2. **spool.dev hosting** — Vercel? Fly.io? Self-managed?
3. **Search backend** — Meilisearch? Typesense? PostgreSQL full-text?
4. **Authentication** — GitHub-only? Add email/password?

---

*Last updated: January 2025*
