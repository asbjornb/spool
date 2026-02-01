# Spool: Design Goals & Philosophy

> This document captures the product vision for Spool. It's a living document that guides decisions.

## The Core Insight

Capabilities

1. Edit context files across repos (CLAUDE.md, skills, commands)
2. Run parallel AI reviews on diffs
3. Analyze agent sessions for failures
4. Share agent logs, skills and commands with coworkers

Today, context is scattered across repos, machines, Slack threads, and people's heads. Spool is a **context layer for AI-assisted development**.

## The Three Modes

### 1. Watch — See what the AI actually did

Agent session replay. Not logs — *replay*. You see the AI's reasoning, tool calls, failures, recoveries. Like a flight recorder for AI sessions.

**The magic**: Pattern detection. "This tool fails 40% of the time when X." "This skill contradicts that CLAUDE.md." The app learns what breaks and tells you before it breaks again.

### 2. Share — Team context as a product

Skills, commands, session recordings — packaged and versioned. Not "file sharing" — a catalog. With trust indicators, usage stats, and one-click install.

**The magic**: When someone on your team builds a skill that works, you get a quiet notification. "Sarah's SQL review skill caught 12 issues this week. Add it?"

### 3. Shape — Edit the AI's understanding

One unified view of everything that shapes AI behavior: CLAUDE.md files, skills, commands, system prompts. Across all repos. With git-aware diffing where available, snapshots where not.

**The magic**: You edit in one place, it syncs everywhere intelligently. Conflict? It shows you, suggests resolution, learns from your choice.

## Why Watch First?

We're building Watch first because:

1. **No one else does this well** — debugging AI failures is miserable
2. **The pain is acute** — you can feel it every day
3. **It's visually demonstrable** — easy to show, easy to share
4. **It naturally leads to Shape and Share** — fix the context, propagate fixes

## The Differentiation

Compared to existing tools:

| Existing Tools | Spool |
|----------------|-------|
| Hosted-only | Open format + self-host + optional hosted |
| Import via CLI hook | Browse existing logs (picker UI) |
| Full sessions only | Trim to moments (select a range) |
| View-only | Annotations & highlights (first-class) |
| Static scroll | Playback mode (with thinking compression) |
| No embeds | Embeddable component |
| No cross-session search | Search across all your sessions |

### The Killer Feature: Retrospective Sharing

Existing tools require recording a session. You have to decide upfront to record.

Spool is different: "Oh that was cool, let me share it" → open picker → browse existing logs → select → trim → annotate → publish.

**Sharing is retrospective, not premeditated.** This is huge.

## Design Principles

### 1. Start with existing logs

Agents already store logs. Don't make users change their workflow. Find the logs where they are:
- Claude Code: `~/.claude/projects/*/sessions/*.json`
- Codex: `~/.codex/logs/` (TBD)

### 2. Destructive redaction

Secrets are replaced *before* export, never stored. The original logs stay on the user's machine. The exported `.spool` file is a derivative that's safe to share.

### 3. IDs everywhere

Every entry has a unique ID. This enables:
- Deep links (`spool.dev/abc123#e_047`)
- Annotations that reference specific moments
- Subagent nesting
- Filtering and search

### 4. Semantic, not visual

We don't record pixels. We record *meaning*:
- This is a prompt
- This is thinking
- This is a tool call (with inputs)
- This is a result

### 5. Open format

The `.spool` format is documented and open. Anyone can:
- Build viewers
- Build editors
- Build adapters for new agents
- Self-host without lock-in

## The Positioning

**Spool**: "Understand what your AI agent actually did. Replay sessions, find failures, share patterns."

This makes Spool useful even if you never share anything. The sharing is a bonus. That's how you get daily active usage vs. occasional "look at this cool thing."

## The Pitch (30 seconds)

*"AI coding assistants are only as good as the context you give them. But context is scattered—across repos, teams, and time. Spool is a context layer. One place to shape what your AI knows, watch how it reasons, and share what works with your team. It's the difference between an AI that writes code and an AI that understands your code."*

## Non-Goals (For Now)

- **Not a git client** — We're not building repo navigation
- **Not real-time** — We replay recordings, not live sessions
- **Not an IDE** — We complement existing tools, not replace them
- **Not enterprise-first** — Build for individual developers, teams follow

## Open Questions

1. **Editor integration** — VS Code extension? JetBrains plugin?
2. **Mobile viewer** — Read-only session viewing on mobile?

---

*Last updated: January 2025*
