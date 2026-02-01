# unspool.dev Architecture

## Overview

unspool.dev is a sharing service for `.spool` files—the structured format for AI agent sessions. Think "pastebin meets Gist" with playback.

**Design principles:**
- Anonymous-first, auth optional
- Cloudflare-native (Pages + Workers + D1 + R2)
- $0-20/month at moderate scale, fits in $500/year budget
- Self-hostable later (all infrastructure is standard/portable)

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            unspool.dev                                  │
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     Cloudflare Pages                              │  │
│  │                                                                    │  │
│  │   /                    Landing page                               │  │
│  │   /s/:id               Session viewer (SPA)                       │  │
│  │   /u/:username         User profile (public sessions)             │  │
│  │   /browse              Discovery feed                             │  │
│  │   /embed/:id           Minimal embed viewer                       │  │
│  │                                                                    │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     Cloudflare Workers                            │  │
│  │                        /api/*                                     │  │
│  │                                                                    │  │
│  │   POST   /api/upload          Upload .spool (returns short ID)    │  │
│  │   GET    /api/s/:id           Session metadata                    │  │
│  │   GET    /api/s/:id/content   Raw .spool content (gzipped)        │  │
│  │   PATCH  /api/s/:id           Update visibility/title (authed)    │  │
│  │   DELETE /api/s/:id           Delete session (authed)             │  │
│  │                                                                    │  │
│  │   GET    /api/u/:username     User's public sessions              │  │
│  │   GET    /api/browse          Discovery feed (public sessions)    │  │
│  │                                                                    │  │
│  │   POST   /api/auth/github     GitHub OAuth callback               │  │
│  │   POST   /api/auth/google     Google OAuth callback               │  │
│  │   GET    /api/auth/me         Current user info                   │  │
│  │                                                                    │  │
│  └─────────────────┬────────────────────────┬───────────────────────┘  │
│                    │                        │                           │
│                    ▼                        ▼                           │
│  ┌─────────────────────────┐  ┌─────────────────────────────────────┐  │
│  │    Cloudflare D1        │  │         Cloudflare R2               │  │
│  │    (Metadata DB)        │  │         (Blob Storage)              │  │
│  │                         │  │                                     │  │
│  │  - sessions             │  │   /sessions/:id.spool.gz            │  │
│  │  - users                │  │                                     │  │
│  │  - (future: votes,tags) │  │   Lifecycle rule: delete after      │  │
│  │                         │  │   expires_at (via cron worker)      │  │
│  └─────────────────────────┘  └─────────────────────────────────────┘  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Data Model

### D1 Schema

```sql
-- Sessions table
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,              -- nanoid, 10 chars (e.g., "V1StGXR8_Z")
    
    -- Content metadata (extracted from .spool on upload)
    title TEXT,                       -- From session entry or first prompt
    agent TEXT,                       -- "claude-code", "codex", etc.
    entry_count INTEGER,              -- Number of entries
    duration_ms INTEGER,              -- Total session duration
    
    -- Ownership & visibility
    user_id TEXT,                     -- NULL for anonymous uploads
    visibility TEXT DEFAULT 'unlisted', -- 'public', 'unlisted', 'private'
    
    -- Storage
    storage_key TEXT NOT NULL,        -- R2 key: "sessions/{id}.spool.gz"
    size_bytes INTEGER,               -- Compressed size
    
    -- Lifecycle
    created_at TEXT DEFAULT (datetime('now')),
    expires_at TEXT,                  -- NULL = never expires (authed users)
    
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_visibility ON sessions(visibility);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

-- Users table (Phase 2)
CREATE TABLE users (
    id TEXT PRIMARY KEY,              -- nanoid
    
    -- Identity
    github_id TEXT UNIQUE,
    google_id TEXT UNIQUE,
    username TEXT UNIQUE,             -- Chosen or derived from OAuth
    display_name TEXT,
    avatar_url TEXT,
    
    -- Timestamps
    created_at TEXT DEFAULT (datetime('now')),
    last_seen_at TEXT
);

-- Future: Discovery features (Phase 3+)
-- CREATE TABLE votes (...);
-- CREATE TABLE tags (...);
```

### R2 Storage Layout

```
sessions/
  {id}.spool.gz          # Gzipped .spool content
```

Simple flat namespace. ID collision is effectively impossible with nanoid.

---

## ID Design

Using [nanoid](https://github.com/ai/nanoid) with custom alphabet for URL-friendly IDs:

```typescript
import { customAlphabet } from 'nanoid';

// 10 chars from 62-char alphabet = 62^10 ≈ 8×10^17 combinations
// At 1000 uploads/day, collision probability stays negligible for centuries
const nanoid = customAlphabet(
  '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz',
  10
);

// URLs look like: unspool.dev/s/V1StGXR8_Z
```

---

## Upload Flow

```
┌────────┐         ┌─────────┐         ┌────┐         ┌────┐
│ Client │         │ Worker  │         │ R2 │         │ D1 │
└───┬────┘         └────┬────┘         └─┬──┘         └─┬──┘
    │                   │                │               │
    │ POST /api/upload  │                │               │
    │ (gzipped .spool)  │                │               │
    │──────────────────▶│                │               │
    │                   │                │               │
    │                   │ Validate .spool format         │
    │                   │ (parse first few lines,        │
    │                   │  check for session entry)      │
    │                   │                │               │
    │                   │ Generate ID    │               │
    │                   │ (nanoid)       │               │
    │                   │                │               │
    │                   │ PUT object     │               │
    │                   │───────────────▶│               │
    │                   │                │               │
    │                   │ INSERT session │               │
    │                   │───────────────────────────────▶│
    │                   │                │               │
    │  { id, url }      │                │               │
    │◀──────────────────│                │               │
    │                   │                │               │
```

**Validation on upload:**
1. Check Content-Encoding or decompress if needed
2. Parse first line—must be a valid `session` entry
3. Quick scan for entry types to populate metadata
4. Reject if >10MB compressed (reasonable limit)

---

## TTL Strategy

| User State | TTL | Notes |
|------------|-----|-------|
| Anonymous | 14 days | `expires_at = now + 14d` |
| Authenticated | No expiry | `expires_at = NULL` |

**Cleanup worker** (runs daily via Cron Trigger):

```typescript
export default {
  async scheduled(event: ScheduledEvent, env: Env) {
    // Find expired sessions
    const expired = await env.DB.prepare(`
      SELECT id, storage_key FROM sessions 
      WHERE expires_at < datetime('now')
      LIMIT 1000
    `).all();
    
    // Delete from R2 and D1
    for (const session of expired.results) {
      await env.BUCKET.delete(session.storage_key);
      await env.DB.prepare('DELETE FROM sessions WHERE id = ?')
        .bind(session.id)
        .run();
    }
  }
};
```

---

## API Design

### Upload

```
POST /api/upload
Content-Type: application/gzip
Authorization: Bearer <token>  (optional)

[gzipped .spool content]

Response 201:
{
  "id": "V1StGXR8_Z",
  "url": "https://unspool.dev/s/V1StGXR8_Z",
  "expires_at": "2024-02-15T00:00:00Z"  // null if authed
}
```

### Get Session Metadata

```
GET /api/s/:id

Response 200:
{
  "id": "V1StGXR8_Z",
  "title": "Finding the SQL injection bug",
  "agent": "claude-code",
  "entry_count": 47,
  "duration_ms": 180000,
  "visibility": "unlisted",
  "user": {                    // null if anonymous
    "username": "alexk",
    "avatar_url": "..."
  },
  "created_at": "2024-02-01T12:00:00Z",
  "expires_at": "2024-02-15T00:00:00Z"
}
```

### Get Session Content

```
GET /api/s/:id/content
Accept-Encoding: gzip

Response 200:
Content-Type: application/jsonl
Content-Encoding: gzip

[raw .spool content, gzipped]
```

### Update Session (authed, owner only)

```
PATCH /api/s/:id
Authorization: Bearer <token>
Content-Type: application/json

{
  "title": "New title",
  "visibility": "public"
}

Response 200:
{ "ok": true }
```

---

## Auth Flow (Phase 2)

Using OAuth 2.0 with GitHub and Google. Tokens stored in httpOnly cookies.

```
┌────────┐         ┌─────────┐         ┌────────┐
│ Client │         │ Worker  │         │ GitHub │
└───┬────┘         └────┬────┘         └───┬────┘
    │                   │                   │
    │ GET /auth/github  │                   │
    │──────────────────▶│                   │
    │                   │                   │
    │ 302 → GitHub OAuth│                   │
    │◀──────────────────│                   │
    │                   │                   │
    │ ─────────────────────────────────────▶│
    │                   │                   │
    │ Callback with code│                   │
    │◀─────────────────────────────────────│
    │                   │                   │
    │ GET /auth/github/callback?code=...   │
    │──────────────────▶│                   │
    │                   │                   │
    │                   │ Exchange code     │
    │                   │──────────────────▶│
    │                   │                   │
    │                   │ Access token      │
    │                   │◀──────────────────│
    │                   │                   │
    │                   │ Fetch user info   │
    │                   │──────────────────▶│
    │                   │                   │
    │                   │ Create/update user in D1
    │                   │                   │
    │ Set-Cookie (JWT)  │                   │
    │ 302 → /           │                   │
    │◀──────────────────│                   │
```

JWT contains: `{ sub: user_id, username, exp }`. Validated on each request.

---

## Frontend Architecture

Single Page App, likely **Svelte** or **SolidJS** (small bundle, fast).

```
web/
  viewer/
    src/
      routes/
        +page.svelte           # Landing
        s/[id]/+page.svelte    # Session viewer
        u/[user]/+page.svelte  # User profile
        browse/+page.svelte    # Discovery
        embed/[id]/+page.svelte # Minimal embed
      lib/
        Player.svelte          # Playback component
        Entry.svelte           # Single entry renderer
        Timeline.svelte        # Scrubber/progress
      api.ts                   # API client
    static/
      embed.js                 # Lightweight embed script (future)
```

**Key viewer features:**
- Streaming parse of .spool (don't load entire file into memory)
- Playback with variable speed
- Thinking compression (collapse long thinking blocks)
- Deep linking to specific entries (`/s/abc123#entry-5`)
- Copy-to-clipboard for individual entries

---

## Embed Strategy (Phase 2)

### iframe embed

```html
<iframe 
  src="https://unspool.dev/embed/V1StGXR8_Z" 
  width="100%" 
  height="400"
  frameborder="0"
></iframe>
```

The `/embed/:id` route serves a minimal viewer optimized for embedding:
- No header/nav
- Compact controls
- Respects `prefers-color-scheme`
- Optional URL params: `?autoplay=1&start=30000`

### JS widget (future)

```html
<div data-unspool="V1StGXR8_Z"></div>
<script src="https://unspool.dev/embed.js" async></script>
```

---

## Cost Estimate

| Component | Free Tier | Your Usage | Cost |
|-----------|-----------|------------|------|
| Pages | Unlimited | ✓ | $0 |
| Workers | 100k req/day | <10k req/day | $0 |
| D1 | 5GB, 5M reads/day | <100MB, <100k reads | $0 |
| R2 | 10GB storage | ~2GB (14-day TTL) | $0 |
| R2 egress | Free | ✓ | $0 |
| Domain | - | unspool.dev | ~$12/year |

**Projected monthly cost at launch: $1/month** (domain amortized)

**At 1,000 daily active users:**
- ~5GB R2 storage → still free tier
- ~500k requests/day → may need Workers paid ($5/mo)
- Estimated: **$5-10/month**

---

## Implementation Phases

### Phase 1: Core Upload & View (2-3 weeks)
- [ ] Cloudflare project setup (Pages, Workers, D1, R2)
- [ ] Upload endpoint with validation
- [ ] Basic web viewer (no playback, just scroll)
- [ ] Session metadata API
- [ ] TTL cleanup worker
- [ ] CLI `spool publish` command

### Phase 2: Auth & Playback (2-3 weeks)
- [ ] GitHub OAuth
- [ ] Google OAuth  
- [ ] User profiles
- [ ] Session ownership (update title, delete)
- [ ] Playback mode in viewer
- [ ] Thinking compression

### Phase 3: Sharing Polish (1-2 weeks)
- [ ] iframe embeds
- [ ] Private sessions (authed view)
- [ ] OG meta tags for link previews
- [ ] "Extend TTL" action for anonymous sessions

### Phase 4: Discovery (future)
- [ ] Public session feed
- [ ] Tags
- [ ] Upvotes
- [ ] Search

---

## CLI Integration

Add to `spool-cli`:

```bash
# Publish to unspool.dev
spool publish session.spool
# → Uploaded: https://unspool.dev/s/V1StGXR8_Z
# → Expires: 2024-02-15 (sign in to keep permanently)

# With auth (reads token from ~/.spool/config)
spool publish session.spool --public --title "Bug hunt"

# Check status of uploaded session
spool status V1StGXR8_Z
```

Config stored in `~/.spool/config.toml`:

```toml
[unspool]
token = "..."  # JWT from OAuth flow
default_visibility = "unlisted"
```

---

## Open Questions

1. **Rate limiting**: What's reasonable for anonymous uploads? 10/hour? 50/day per IP?

2. **Max file size**: 10MB compressed seems generous. Could start at 5MB.

3. **Abuse**: How to handle spam/abuse without auth? Probably fine to start without protection, add captcha or IP limits if needed.

4. **CLI auth flow**: Browser-based OAuth redirect, or device flow? Device flow is nicer for terminals.

5. **Static site generator**: You mentioned this in the README. Is the idea to export a session as a standalone HTML file? Could be a nice `spool export --html` feature that bundles the viewer + data.

---

## Tech Stack Summary

| Layer | Choice | Why |
|-------|--------|-----|
| Hosting | Cloudflare Pages | Free, fast, integrated |
| API | Cloudflare Workers | Free tier generous, same platform |
| Database | Cloudflare D1 | SQLite, simple, free tier |
| Blob storage | Cloudflare R2 | No egress fees, lifecycle rules |
| Frontend | SvelteKit or SolidStart | Small bundles, SSR-capable |
| Auth | Custom OAuth | Simple enough, no vendor lock-in |
| IDs | nanoid | URL-safe, collision-resistant |

All portable if you ever need to move off Cloudflare.
