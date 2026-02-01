-- unspool.dev D1 Schema
-- Run with: wrangler d1 execute unspool-db --file=./schema.sql

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,              -- nanoid, 10 chars
    
    -- Content metadata (extracted from .spool on upload)
    title TEXT,                       -- From session entry or first prompt
    agent TEXT,                       -- "claude-code", "codex", etc.
    entry_count INTEGER,              -- Number of entries
    duration_ms INTEGER,              -- Total session duration
    
    -- Ownership & visibility
    user_id TEXT,                     -- NULL for anonymous uploads
    visibility TEXT DEFAULT 'unlisted' CHECK (visibility IN ('public', 'unlisted', 'private')),
    
    -- Storage
    storage_key TEXT NOT NULL,        -- R2 key: "sessions/{id}.spool.gz"
    size_bytes INTEGER,               -- Compressed size
    
    -- Lifecycle
    created_at TEXT DEFAULT (datetime('now')),
    expires_at TEXT,                  -- NULL = never expires (authed users)
    
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_visibility ON sessions(visibility) WHERE visibility = 'public';
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_created ON sessions(created_at);

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,              -- nanoid
    
    -- Identity (at least one must be set)
    github_id TEXT UNIQUE,
    google_id TEXT UNIQUE,
    username TEXT UNIQUE NOT NULL,    -- Chosen or derived from OAuth
    display_name TEXT,
    avatar_url TEXT,
    email TEXT,                       -- For notifications, optional
    
    -- Timestamps
    created_at TEXT DEFAULT (datetime('now')),
    last_seen_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_users_github ON users(github_id) WHERE github_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_google ON users(google_id) WHERE google_id IS NOT NULL;

-- API tokens for CLI auth
CREATE TABLE IF NOT EXISTS tokens (
    id TEXT PRIMARY KEY,              -- nanoid
    user_id TEXT NOT NULL,
    name TEXT,                        -- e.g., "CLI on macbook"
    token_hash TEXT NOT NULL,         -- SHA-256 of the actual token
    last_used_at TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    expires_at TEXT,                  -- NULL = never expires
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tokens_user ON tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash);

-- Future: Discovery tables (uncomment when needed)

-- CREATE TABLE IF NOT EXISTS votes (
--     user_id TEXT NOT NULL,
--     session_id TEXT NOT NULL,
--     created_at TEXT DEFAULT (datetime('now')),
--     PRIMARY KEY (user_id, session_id),
--     FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
--     FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
-- );

-- CREATE TABLE IF NOT EXISTS tags (
--     id TEXT PRIMARY KEY,
--     name TEXT UNIQUE NOT NULL,       -- e.g., "debugging", "refactoring"
--     session_count INTEGER DEFAULT 0  -- Denormalized for perf
-- );

-- CREATE TABLE IF NOT EXISTS session_tags (
--     session_id TEXT NOT NULL,
--     tag_id TEXT NOT NULL,
--     PRIMARY KEY (session_id, tag_id),
--     FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
--     FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
-- );
