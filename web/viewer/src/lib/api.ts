/**
 * API client for unspool.dev backend
 */

// API base URL - use environment variable or default to custom domain
const API_BASE = import.meta.env.VITE_API_URL || 'https://api.unspool.dev';

export interface SessionMetadata {
	id: string;
	title: string | null;
	agent: string | null;
	entry_count: number;
	duration_ms: number;
	visibility: string;
	size_bytes: number;
	created_at: string;
	expires_at: string | null;
	user: {
		username: string;
		avatar_url: string | null;
	} | null;
}

export interface UploadResponse {
	id: string;
	url: string;
	expires_at: string | null;
}

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
		this.name = 'ApiError';
	}
}

/**
 * Get session metadata by ID
 */
export async function getSession(id: string): Promise<SessionMetadata> {
	const res = await fetch(`${API_BASE}/api/s/${id}`);
	if (!res.ok) {
		const body = await res.json().catch(() => ({}));
		throw new ApiError(res.status, body.error || `HTTP ${res.status}`);
	}
	return res.json();
}

/**
 * Get session content (.spool file)
 * The API returns gzipped content, we decompress client-side
 */
export async function getSessionContent(id: string): Promise<string> {
	const res = await fetch(`${API_BASE}/api/s/${id}/content`);
	if (!res.ok) {
		const body = await res.json().catch(() => ({}));
		throw new ApiError(res.status, body.error || `HTTP ${res.status}`);
	}

	// The content is gzipped, decompress it
	const blob = await res.blob();

	// Check if it's gzipped (starts with 0x1f 0x8b)
	const header = new Uint8Array(await blob.slice(0, 2).arrayBuffer());
	if (header[0] === 0x1f && header[1] === 0x8b) {
		// Decompress using DecompressionStream
		const ds = new DecompressionStream('gzip');
		const decompressed = await new Response(blob.stream().pipeThrough(ds)).text();
		return decompressed;
	}

	// Not gzipped, return as-is
	return blob.text();
}

/**
 * Upload a .spool file
 * Returns the session ID and shareable URL
 */
export async function uploadSession(content: string): Promise<UploadResponse> {
	// Compress the content
	const encoder = new TextEncoder();
	const data = encoder.encode(content);

	// Use CompressionStream if available, otherwise send uncompressed
	let body: BodyInit;
	let contentType: string;

	if (typeof CompressionStream !== 'undefined') {
		const cs = new CompressionStream('gzip');
		const compressed = await new Response(new Blob([data]).stream().pipeThrough(cs)).arrayBuffer();
		body = compressed;
		contentType = 'application/gzip';
	} else {
		body = content;
		contentType = 'application/jsonl';
	}

	const res = await fetch(`${API_BASE}/api/upload`, {
		method: 'POST',
		headers: {
			'Content-Type': contentType
		},
		body
	});

	if (!res.ok) {
		const responseBody = await res.json().catch(() => ({}));
		throw new ApiError(res.status, responseBody.error || `HTTP ${res.status}`);
	}

	return res.json();
}
