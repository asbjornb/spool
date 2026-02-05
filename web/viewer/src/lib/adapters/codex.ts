/**
 * Codex CLI adapter — converts raw .jsonl session logs to Spool format.
 *
 * This is a TypeScript port of crates/spool-adapters/src/codex.rs.
 * It runs entirely client-side in the browser.
 *
 * Codex logs are JSONL with each line having: { timestamp, type, payload }
 * where type is one of: session_meta, event_msg, response_item, turn_context
 */

import type {
	Entry,
	SessionEntry,
	PromptEntry,
	ResponseEntry,
	ThinkingEntry,
	ToolCallEntry,
	ToolResultEntry,
	SpoolFile
} from '../types';

// ============================================================================
// Raw Codex JSONL types
// ============================================================================

interface RawLine {
	timestamp: string;
	type: string;
	payload: Record<string, unknown>;
}

interface RawSessionMeta {
	id?: string;
	timestamp: string;
	cwd?: string;
	originator?: string;
	cli_version?: string;
	source?: string;
	model_provider?: string;
	git?: {
		commit_hash?: string;
		branch?: string;
		repository_url?: string;
	};
}

interface RawTurnContext {
	model?: string;
}

// event_msg payload has a nested "type" field
type RawEventMsg =
	| { type: 'user_message'; message: string }
	| { type: 'agent_message'; message: string }
	| { type: 'agent_reasoning'; text: string }
	| { type: string };

// response_item payload has a nested "type" field
type RawResponseItem =
	| { type: 'function_call'; name: string; arguments: string; call_id: string }
	| { type: 'function_call_output'; call_id: string; output: string }
	| { type: 'custom_tool_call'; name: string; input: string; call_id: string }
	| { type: 'custom_tool_call_output'; call_id: string; output: string }
	| { type: 'web_search_call'; action: { type?: string; query?: string; queries?: string[] } }
	| { type: string };

// ============================================================================
// Public API
// ============================================================================

const CODEX_LINE_TYPES = ['session_meta', 'event_msg', 'response_item', 'turn_context'];

/** Detect whether a JSONL string is a Codex CLI log */
export function isCodexLog(firstLine: string): boolean {
	try {
		const parsed = JSON.parse(firstLine);
		return (
			typeof parsed?.timestamp === 'string' &&
			typeof parsed?.type === 'string' &&
			CODEX_LINE_TYPES.includes(parsed.type) &&
			parsed?.payload !== undefined
		);
	} catch {
		return false;
	}
}

/** Convert a raw Codex CLI .jsonl string to a SpoolFile */
export function convertCodexLog(text: string): SpoolFile {
	const lines = text.split('\n').filter((l) => l.trim().length > 0);
	const rawLines: RawLine[] = [];

	for (const line of lines) {
		try {
			const parsed = JSON.parse(line);
			if (parsed && typeof parsed.type === 'string' && typeof parsed.timestamp === 'string') {
				rawLines.push(parsed as RawLine);
			}
		} catch {
			// Skip unparseable lines
		}
	}

	if (rawLines.length === 0) {
		throw new Error('No valid Codex JSONL lines found');
	}

	return convertRawLines(rawLines);
}

// ============================================================================
// Conversion logic
// ============================================================================

function convertRawLines(rawLines: RawLine[]): SpoolFile {
	const entries: Entry[] = [];
	const toolIdMap = new Map<string, string>();

	let sessionMeta: RawSessionMeta | null = null;
	let firstPromptText: string | null = null;
	let sessionStart: Date | null = null;
	let sessionEnd: Date | null = null;
	let lastModel: string | null = null;

	// First pass: extract metadata
	for (const line of rawLines) {
		const ts = parseTimestamp(line.timestamp);
		if (ts) {
			if (!sessionStart || ts < sessionStart) sessionStart = ts;
			if (!sessionEnd || ts > sessionEnd) sessionEnd = ts;
		}

		switch (line.type) {
			case 'session_meta': {
				if (!sessionMeta) {
					sessionMeta = line.payload as unknown as RawSessionMeta;
				}
				break;
			}
			case 'event_msg': {
				const event = line.payload as unknown as RawEventMsg;
				if (!firstPromptText && event.type === 'user_message') {
					const msg = (event as { type: 'user_message'; message: string }).message;
					if (msg.trim().length > 0) {
						firstPromptText = msg;
					}
				}
				break;
			}
			case 'turn_context': {
				if (!lastModel) {
					const ctx = line.payload as unknown as RawTurnContext;
					if (ctx.model) lastModel = ctx.model;
				}
				break;
			}
		}
	}

	// Determine session start time
	if (!sessionStart && sessionMeta) {
		sessionStart = parseTimestamp(sessionMeta.timestamp);
	}
	if (!sessionStart) {
		sessionStart = new Date();
	}

	// Build session entry
	const title = firstPromptText ? truncateText(firstPromptText, 200) : undefined;

	const sessionEntry: SessionEntry = {
		id: crypto.randomUUID(),
		ts: 0,
		type: 'session',
		version: '1.0',
		agent: 'codex',
		recorded_at: sessionStart.toISOString(),
		agent_version: sessionMeta?.cli_version ?? undefined,
		title,
		first_prompt: firstPromptText ? truncateText(firstPromptText, 200) : undefined,
		ended: 'unknown'
	};

	// Add extra metadata
	if (sessionMeta?.cwd) {
		(sessionEntry as Record<string, unknown>).x_cwd = sessionMeta.cwd;
	}
	if (sessionMeta?.originator) {
		(sessionEntry as Record<string, unknown>).x_originator = sessionMeta.originator;
	}
	if (sessionMeta?.source) {
		(sessionEntry as Record<string, unknown>).x_source = sessionMeta.source;
	}
	if (sessionMeta?.model_provider) {
		(sessionEntry as Record<string, unknown>).x_model_provider = sessionMeta.model_provider;
	}
	if (sessionMeta?.git) {
		(sessionEntry as Record<string, unknown>).x_git = sessionMeta.git;
	}
	if (lastModel) {
		(sessionEntry as Record<string, unknown>).x_model = lastModel;
	}

	entries.push(sessionEntry);

	// Second pass: convert entries
	let currentModel = lastModel;

	for (const line of rawLines) {
		const ts = computeRelativeTs(line.timestamp, sessionStart);

		switch (line.type) {
			case 'turn_context': {
				const ctx = line.payload as unknown as RawTurnContext;
				if (ctx.model) currentModel = ctx.model;
				break;
			}
			case 'event_msg': {
				const event = line.payload as unknown as RawEventMsg;
				switch (event.type) {
					case 'user_message': {
						const msg = (event as { type: 'user_message'; message: string }).message;
						if (msg.trim().length > 0) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'prompt',
								content: msg
							} as PromptEntry);
						}
						break;
					}
					case 'agent_message': {
						const msg = (event as { type: 'agent_message'; message: string }).message;
						if (msg.trim().length > 0) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'response',
								content: msg,
								model: currentModel ?? undefined
							} as ResponseEntry);
						}
						break;
					}
					case 'agent_reasoning': {
						const text = (event as { type: 'agent_reasoning'; text: string }).text;
						if (text.trim().length > 0) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'thinking',
								content: text,
								collapsed: true
							} as ThinkingEntry);
						}
						break;
					}
				}
				break;
			}
			case 'response_item': {
				const item = line.payload as unknown as RawResponseItem;
				switch (item.type) {
					case 'function_call': {
						const fc = item as {
							type: 'function_call';
							name: string;
							arguments: string;
							call_id: string;
						};
						const input = parseJsonOrString(fc.arguments);
						const entryId = crypto.randomUUID();
						toolIdMap.set(fc.call_id, entryId);
						entries.push({
							id: entryId,
							ts,
							type: 'tool_call',
							tool: fc.name,
							input
						} as ToolCallEntry);
						break;
					}
					case 'function_call_output': {
						const fco = item as { type: 'function_call_output'; call_id: string; output: string };
						const callId = toolIdMap.get(fco.call_id);
						if (callId) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'tool_result',
								call_id: callId,
								output: fco.output
							} as ToolResultEntry);
						}
						break;
					}
					case 'custom_tool_call': {
						const ctc = item as {
							type: 'custom_tool_call';
							name: string;
							input: string;
							call_id: string;
						};
						const entryId = crypto.randomUUID();
						toolIdMap.set(ctc.call_id, entryId);
						const input = parseJsonOrString(ctc.input);
						entries.push({
							id: entryId,
							ts,
							type: 'tool_call',
							tool: ctc.name,
							input
						} as ToolCallEntry);
						break;
					}
					case 'custom_tool_call_output': {
						const ctco = item as {
							type: 'custom_tool_call_output';
							call_id: string;
							output: string;
						};
						const callId = toolIdMap.get(ctco.call_id);
						if (callId) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'tool_result',
								call_id: callId,
								output: ctco.output
							} as ToolResultEntry);
						}
						break;
					}
					case 'web_search_call': {
						const wsc = item as {
							type: 'web_search_call';
							action: { type?: string; query?: string; queries?: string[] };
						};
						entries.push({
							id: crypto.randomUUID(),
							ts,
							type: 'tool_call',
							tool: 'web_search',
							input: wsc.action as unknown as Record<string, unknown>
						} as ToolCallEntry);
						break;
					}
				}
				break;
			}
		}
	}

	// Compute final metadata
	let lastTs = 0;
	const toolsUsedSet = new Set<string>();
	const filesModifiedSet = new Set<string>();

	for (const entry of entries) {
		if (entry.ts > lastTs) lastTs = entry.ts;
		if (entry.type === 'tool_call') {
			const tc = entry as ToolCallEntry;
			toolsUsedSet.add(tc.tool);
			if (tc.tool === 'apply_patch') {
				const input = tc.input;
				// apply_patch input is typically the patch string itself
				const patchStr =
					typeof input === 'string'
						? input
						: typeof (input as Record<string, unknown>).patch === 'string'
							? ((input as Record<string, unknown>).patch as string)
							: JSON.stringify(input);
				collectPatchPaths(patchStr, filesModifiedSet);
			}
		}
	}

	// Update session entry
	const session = entries[0] as SessionEntry;
	if (sessionEnd && sessionStart) {
		const duration = sessionEnd.getTime() - sessionStart.getTime();
		if (duration > 0) session.duration_ms = duration;
	}
	session.entry_count = entries.length;
	if (toolsUsedSet.size > 0) {
		session.tools_used = [...toolsUsedSet].sort();
	}
	if (filesModifiedSet.size > 0) {
		session.files_modified = [...filesModifiedSet].sort();
	}

	return { session, entries };
}

// ============================================================================
// Helpers
// ============================================================================

function parseTimestamp(timestamp: string): Date | null {
	const d = new Date(timestamp);
	return isNaN(d.getTime()) ? null : d;
}

function computeRelativeTs(timestamp: string, sessionStart: Date): number {
	const ts = parseTimestamp(timestamp);
	if (!ts) return 0;
	const diff = ts.getTime() - sessionStart.getTime();
	return Math.max(0, diff);
}

function truncateText(text: string, maxLen: number): string {
	if (text.length <= maxLen) return text;
	return text.slice(0, maxLen) + '...';
}

function parseJsonOrString(input: string): Record<string, unknown> {
	try {
		const parsed = JSON.parse(input);
		if (typeof parsed === 'object' && parsed !== null) return parsed;
		return { value: input };
	} catch {
		return { value: input };
	}
}

function collectPatchPaths(patch: string, filesModified: Set<string>) {
	for (const line of patch.split('\n')) {
		let path: string | undefined;
		if (line.startsWith('*** Update File: ')) {
			path = line.slice('*** Update File: '.length).trim();
		} else if (line.startsWith('*** Add File: ')) {
			path = line.slice('*** Add File: '.length).trim();
		} else if (line.startsWith('*** Delete File: ')) {
			path = line.slice('*** Delete File: '.length).trim();
		}
		if (path) filesModified.add(path);
	}
}
