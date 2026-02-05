/**
 * unspool.dev API Worker
 * 
 * Handles session upload, retrieval, and management.
 */

import { customAlphabet } from 'nanoid';

// URL-safe nanoid for session IDs
const nanoid = customAlphabet(
  '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz',
  10
);

export interface Env {
  DB: D1Database;
  BUCKET: R2Bucket;
  ENVIRONMENT: string;
}

// CORS headers for API responses
const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, PATCH, DELETE, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    const url = new URL(request.url);
    const path = url.pathname;

    try {
      // Route handling
      if (path === '/api/upload' && request.method === 'POST') {
        return await handleUpload(request, env);
      }
      
      if (path === '/api/sessions' && request.method === 'GET') {
        return await handleListPublicSessions(url, env);
      }

      if (path.match(/^\/api\/s\/[a-zA-Z0-9]+$/) && request.method === 'GET') {
        const id = path.split('/').pop()!;
        return await handleGetSession(id, env);
      }
      
      if (path.match(/^\/api\/s\/[a-zA-Z0-9]+\/content$/) && request.method === 'GET') {
        const id = path.split('/')[3];
        return await handleGetContent(id, env);
      }

      if (path === '/api/health') {
        return json({ ok: true, env: env.ENVIRONMENT });
      }

      return json({ error: 'Not found' }, 404);
    } catch (err) {
      console.error('Request error:', err);
      return json({ error: 'Internal server error' }, 500);
    }
  },

  // Cron handler for TTL cleanup
  async scheduled(event: ScheduledEvent, env: Env): Promise<void> {
    console.log('Running TTL cleanup...');
    
    const expired = await env.DB.prepare(`
      SELECT id, storage_key FROM sessions 
      WHERE expires_at IS NOT NULL AND expires_at < datetime('now')
      LIMIT 500
    `).all();

    let deleted = 0;
    for (const session of expired.results as any[]) {
      try {
        await env.BUCKET.delete(session.storage_key);
        await env.DB.prepare('DELETE FROM sessions WHERE id = ?')
          .bind(session.id)
          .run();
        deleted++;
      } catch (err) {
        console.error(`Failed to delete session ${session.id}:`, err);
      }
    }

    console.log(`Cleaned up ${deleted} expired sessions`);
  },
};

// ============ Handlers ============

async function handleUpload(request: Request, env: Env): Promise<Response> {
  // Parse visibility from query string (default: unlisted)
  const uploadUrl = new URL(request.url);
  const visibilityParam = uploadUrl.searchParams.get('visibility') || 'unlisted';
  if (!['public', 'unlisted'].includes(visibilityParam)) {
    return json({ error: 'Invalid visibility (must be "public" or "unlisted")' }, 400);
  }

  const contentType = request.headers.get('content-type') || '';
  
  // Accept both raw gzip and application/json with base64
  let data: ArrayBuffer;
  let isGzipped = false;
  
  if (contentType.includes('application/gzip') || 
      request.headers.get('content-encoding') === 'gzip') {
    data = await request.arrayBuffer();
    isGzipped = true;
  } else if (contentType.includes('application/json')) {
    const body = await request.json() as { content: string };
    data = Uint8Array.from(atob(body.content), c => c.charCodeAt(0)).buffer;
    isGzipped = true; // Assume base64 content is gzipped
  } else {
    // Raw .spool content, we'll compress it
    data = await request.arrayBuffer();
  }

  // Size check (10MB compressed max)
  if (data.byteLength > 10 * 1024 * 1024) {
    return json({ error: 'File too large (max 10MB)' }, 400);
  }

  // Decompress for validation if needed
  let content: string;
  try {
    if (isGzipped) {
      const ds = new DecompressionStream('gzip');
      const decompressed = await new Response(
        new Blob([data]).stream().pipeThrough(ds)
      ).arrayBuffer();
      content = new TextDecoder().decode(decompressed);
    } else {
      content = new TextDecoder().decode(data);
    }
  } catch {
    return json({ error: 'Invalid gzip data' }, 400);
  }

  // Parse and validate .spool format
  const validation = validateSpool(content);
  if (!validation.valid) {
    return json({ error: validation.error }, 400);
  }

  // Compress if not already
  let storedData: ArrayBuffer;
  if (isGzipped) {
    storedData = data;
  } else {
    const cs = new CompressionStream('gzip');
    storedData = await new Response(
      new Blob([data]).stream().pipeThrough(cs)
    ).arrayBuffer();
  }

  // Generate ID and store
  const id = nanoid();
  const storageKey = `sessions/${id}.spool.gz`;
  
  // Check auth (simplified - expand in Phase 2)
  const authHeader = request.headers.get('authorization');
  const userId = authHeader ? await validateToken(authHeader, env) : null;
  
  // Set expiry: 14 days for anonymous, never for authed
  const expiresAt = userId 
    ? null 
    : new Date(Date.now() + 14 * 24 * 60 * 60 * 1000).toISOString();

  // Upload to R2
  await env.BUCKET.put(storageKey, storedData, {
    httpMetadata: { contentType: 'application/gzip' },
  });

  // Insert metadata
  await env.DB.prepare(`
    INSERT INTO sessions (id, title, agent, entry_count, duration_ms, user_id, visibility, storage_key, size_bytes, expires_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `).bind(
    id,
    validation.metadata!.title,
    validation.metadata!.agent,
    validation.metadata!.entryCount,
    validation.metadata!.durationMs,
    userId,
    visibilityParam,
    storageKey,
    storedData.byteLength,
    expiresAt
  ).run();

  return json({
    id,
    url: `https://unspool.dev/s/${id}`,
    expires_at: expiresAt,
  }, 201);
}

async function handleListPublicSessions(url: URL, env: Env): Promise<Response> {
  const limit = Math.min(parseInt(url.searchParams.get('limit') || '20'), 50);
  const offset = Math.max(parseInt(url.searchParams.get('offset') || '0'), 0);

  const result = await env.DB.prepare(`
    SELECT
      s.id, s.title, s.agent, s.entry_count, s.duration_ms,
      s.size_bytes, s.created_at, s.expires_at,
      u.username, u.avatar_url
    FROM sessions s
    LEFT JOIN users u ON s.user_id = u.id
    WHERE s.visibility = 'public'
      AND (s.expires_at IS NULL OR s.expires_at > datetime('now'))
    ORDER BY s.created_at DESC
    LIMIT ? OFFSET ?
  `).bind(limit, offset).all();

  const countResult = await env.DB.prepare(`
    SELECT COUNT(*) as total FROM sessions
    WHERE visibility = 'public'
      AND (expires_at IS NULL OR expires_at > datetime('now'))
  `).first();

  const sessions = (result.results as any[]).map(r => ({
    id: r.id,
    title: r.title,
    agent: r.agent,
    entry_count: r.entry_count,
    duration_ms: r.duration_ms,
    size_bytes: r.size_bytes,
    created_at: r.created_at,
    expires_at: r.expires_at,
    user: r.username ? {
      username: r.username,
      avatar_url: r.avatar_url,
    } : null,
  }));

  return json({
    sessions,
    total: (countResult as any)?.total ?? 0,
    limit,
    offset,
  });
}

async function handleGetSession(id: string, env: Env): Promise<Response> {
  const result = await env.DB.prepare(`
    SELECT 
      s.id, s.title, s.agent, s.entry_count, s.duration_ms,
      s.visibility, s.size_bytes, s.created_at, s.expires_at,
      u.username, u.avatar_url
    FROM sessions s
    LEFT JOIN users u ON s.user_id = u.id
    WHERE s.id = ?
  `).bind(id).first();

  if (!result) {
    return json({ error: 'Session not found' }, 404);
  }

  // Check if private (would need auth check here)
  if (result.visibility === 'private') {
    return json({ error: 'Session is private' }, 403);
  }

  return json({
    id: result.id,
    title: result.title,
    agent: result.agent,
    entry_count: result.entry_count,
    duration_ms: result.duration_ms,
    visibility: result.visibility,
    size_bytes: result.size_bytes,
    created_at: result.created_at,
    expires_at: result.expires_at,
    user: result.username ? {
      username: result.username,
      avatar_url: result.avatar_url,
    } : null,
  });
}

async function handleGetContent(id: string, env: Env): Promise<Response> {
  // First check if session exists and is accessible
  const session = await env.DB.prepare(`
    SELECT storage_key, visibility FROM sessions WHERE id = ?
  `).bind(id).first();

  if (!session) {
    return json({ error: 'Session not found' }, 404);
  }

  if (session.visibility === 'private') {
    return json({ error: 'Session is private' }, 403);
  }

  // Fetch from R2
  const object = await env.BUCKET.get(session.storage_key as string);
  if (!object) {
    return json({ error: 'Session content not found' }, 404);
  }

  return new Response(object.body, {
    headers: {
      ...corsHeaders,
      'Content-Type': 'application/jsonl',
      'Content-Encoding': 'gzip',
      'Cache-Control': 'public, max-age=3600',
    },
  });
}

// ============ Helpers ============

function json(data: any, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      ...corsHeaders,
      'Content-Type': 'application/json',
    },
  });
}

interface ValidationResult {
  valid: boolean;
  error?: string;
  metadata?: {
    title: string | null;
    agent: string | null;
    entryCount: number;
    durationMs: number;
  };
}

function validateSpool(content: string): ValidationResult {
  const lines = content.trim().split('\n');
  
  if (lines.length === 0) {
    return { valid: false, error: 'Empty file' };
  }

  // Parse first line - should be session entry
  let firstEntry: any;
  try {
    firstEntry = JSON.parse(lines[0]);
  } catch {
    return { valid: false, error: 'Invalid JSON on first line' };
  }

  if (firstEntry.type !== 'session') {
    return { valid: false, error: 'First entry must be type "session"' };
  }

  // Quick scan for metadata
  let entryCount = 0;
  let maxTs = 0;
  let title = firstEntry.title || null;
  let agent = firstEntry.agent || null;

  for (const line of lines) {
    if (!line.trim()) continue;
    
    try {
      const entry = JSON.parse(line);
      entryCount++;
      
      if (typeof entry.ts === 'number' && entry.ts > maxTs) {
        maxTs = entry.ts;
      }
      
      // Use first prompt as title if session has no title
      if (!title && entry.type === 'prompt' && entry.content) {
        title = entry.content.slice(0, 100);
      }
    } catch {
      // Skip invalid lines in validation pass
    }
  }

  return {
    valid: true,
    metadata: {
      title,
      agent,
      entryCount,
      durationMs: maxTs,
    },
  };
}

async function validateToken(authHeader: string, env: Env): Promise<string | null> {
  // Simplified token validation - expand in Phase 2
  if (!authHeader.startsWith('Bearer ')) {
    return null;
  }
  
  const token = authHeader.slice(7);
  
  // Hash the token and look it up
  const encoder = new TextEncoder();
  const data = encoder.encode(token);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const tokenHash = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  
  const result = await env.DB.prepare(`
    SELECT user_id FROM tokens 
    WHERE token_hash = ? 
    AND (expires_at IS NULL OR expires_at > datetime('now'))
  `).bind(tokenHash).first();
  
  if (result) {
    // Update last_used_at
    await env.DB.prepare(`
      UPDATE tokens SET last_used_at = datetime('now') WHERE token_hash = ?
    `).bind(tokenHash).run();
    
    return result.user_id as string;
  }
  
  return null;
}
