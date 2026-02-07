/**
 * Claude Code adapter — converts raw .jsonl session logs to Spool format.
 *
 * This is a TypeScript port of crates/spool-adapters/src/claude_code.rs.
 * It runs entirely client-side in the browser.
 */

import type {
	Entry,
	SessionEntry,
	PromptEntry,
	ResponseEntry,
	ThinkingEntry,
	ToolCallEntry,
	ToolResultEntry,
	SubagentStartEntry,
	SubagentEndEntry,
	SpoolFile,
	TokenUsage
} from '../types';

// ============================================================================
// Raw Claude Code JSONL types
// ============================================================================

interface RawUserLine {
	type: 'user';
	message?: {
		role?: string;
		content?: string | RawToolResultBlock[];
	};
	timestamp?: string;
	uuid?: string;
	isMeta?: boolean;
	toolUseResult?: unknown;
	version?: string;
}

interface RawAssistantLine {
	type: 'assistant';
	message?: {
		model?: string;
		content?: RawContentBlock[];
		usage?: {
			input_tokens?: number;
			output_tokens?: number;
			cache_read_input_tokens?: number;
			cache_creation_input_tokens?: number;
		};
		stop_reason?: string;
	};
	timestamp?: string;
	uuid?: string;
	version?: string;
}

interface RawSummaryLine {
	type: 'summary';
	summary?: string;
}

interface RawSystemLine {
	type: 'system';
	subtype?: string;
	durationMs?: number;
	timestamp?: string;
}

type RawContentBlock =
	| { type: 'text'; text: string }
	| { type: 'thinking'; thinking: string; signature?: string }
	| { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }
	| { type: string };

interface RawToolResultBlock {
	type?: string;
	tool_use_id?: string;
	content?: string | { text?: string }[];
	is_error?: boolean;
}

type RawLine = RawUserLine | RawAssistantLine | RawSummaryLine | RawSystemLine | { type: string };

// ============================================================================
// Public API
// ============================================================================

/** Detect whether a JSONL string is a Claude Code log */
export function isClaudeCodeLog(firstLine: string): boolean {
	try {
		const parsed = JSON.parse(firstLine);
		const type = parsed?.type;
		return ['user', 'assistant', 'progress', 'summary', 'system', 'file-history-snapshot'].includes(
			type
		);
	} catch {
		return false;
	}
}

/** Convert a raw Claude Code .jsonl string to a SpoolFile */
export function convertClaudeCodeLog(text: string): SpoolFile {
	const lines = text.split('\n').filter((l) => l.trim().length > 0);
	const rawLines: RawLine[] = [];

	for (const line of lines) {
		try {
			const parsed = JSON.parse(line);
			if (parsed && typeof parsed.type === 'string') {
				rawLines.push(parsed as RawLine);
			}
		} catch {
			// Skip unparseable lines
		}
	}

	if (rawLines.length === 0) {
		throw new Error('No valid JSONL lines found');
	}

	return convertRawLines(rawLines);
}

// ============================================================================
// Conversion logic
// ============================================================================

function convertRawLines(rawLines: RawLine[]): SpoolFile {
	const entries: Entry[] = [];

	// Track tool call IDs: Claude's tool_use id -> our spool EntryId
	const toolIdMap = new Map<string, string>();
	// Track Task tool calls: Claude's tool_use id -> SubagentStart EntryId
	const taskSubagentMap = new Map<string, string>();

	let firstTimestamp: Date | null = null;
	let summaryText: string | null = null;
	let agentVersion: string | null = null;
	let modelName: string | null = null;
	let firstPromptText: string | null = null;

	// First pass: find metadata
	for (const line of rawLines) {
		if (line.type === 'summary') {
			const s = line as RawSummaryLine;
			if (!summaryText && s.summary) {
				summaryText = s.summary;
			}
		} else if (line.type === 'user') {
			const u = line as RawUserLine;
			if (!agentVersion && u.version) {
				agentVersion = u.version;
			}
			if (!u.isMeta) {
				if (!firstTimestamp && u.timestamp) {
					firstTimestamp = new Date(u.timestamp);
					if (isNaN(firstTimestamp.getTime())) firstTimestamp = null;
				}
				if (!firstPromptText && u.message?.content && typeof u.message.content === 'string') {
					const text = u.message.content;
					if (
						!text.includes('<command-name>') &&
						!text.includes('<local-command-stdout>') &&
						!text.includes('<local-command-caveat>')
					) {
						const clean = stripSystemTags(text);
						if (clean.length > 0) {
							firstPromptText = truncateText(clean, 200);
						}
					}
				}
			}
		} else if (line.type === 'assistant') {
			const a = line as RawAssistantLine;
			if (!agentVersion && a.version) {
				agentVersion = a.version;
			}
			if (!modelName && a.message?.model) {
				modelName = a.message.model;
			}
		}
	}

	const sessionStart = firstTimestamp || new Date();

	// Extract title
	const title = summaryText || extractTitleFromLines(rawLines);

	// Create session entry
	const sessionEntry: SessionEntry = {
		id: crypto.randomUUID(),
		ts: 0,
		type: 'session',
		version: '1.0',
		agent: 'claude-code',
		recorded_at: sessionStart.toISOString(),
		agent_version: agentVersion ?? undefined,
		title: title ?? undefined,
		first_prompt: firstPromptText ?? undefined,
		ended: 'unknown'
	};

	if (modelName) {
		(sessionEntry as Record<string, unknown>).x_model = modelName;
	}

	entries.push(sessionEntry);

	// Second pass: convert entries
	for (const line of rawLines) {
		if (line.type === 'user') {
			const u = line as RawUserLine;
			if (u.isMeta) continue;

			const ts = computeRelativeTs(u.timestamp, sessionStart);

			if (u.message) {
				const content = u.message.content;

				if (typeof content === 'string') {
					// Skip command messages
					if (
						content.includes('<command-name>') ||
						content.includes('<local-command-stdout>') ||
						content.includes('<local-command-caveat>')
					) {
						continue;
					}

					const clean = stripSystemTags(content);
					if (clean.length > 0) {
						entries.push({
							id: crypto.randomUUID(),
							ts,
							type: 'prompt',
							content: clean
						} as PromptEntry);
					}
				} else if (Array.isArray(content)) {
					// Tool results
					for (const block of content) {
						if (block.type === 'tool_result') {
							const toolUseId = block.tool_use_id || '';
							const callId = toolIdMap.get(toolUseId) || '00000000-0000-0000-0000-000000000000';
							const isError = block.is_error || false;
							const contentText = extractToolResultText(block.content);

							const subagentStartId = taskSubagentMap.get(toolUseId);

							const entry: ToolResultEntry = {
								id: crypto.randomUUID(),
								ts,
								type: 'tool_result',
								call_id: callId,
								subagent_id: subagentStartId
							};

							if (isError) {
								entry.error = contentText;
							} else {
								entry.output = contentText;
							}

							entries.push(entry);

							// Emit SubagentEnd after ToolResult for Task calls
							if (subagentStartId) {
								entries.push({
									id: crypto.randomUUID(),
									ts,
									type: 'subagent_end',
									start_id: subagentStartId,
									status: isError ? 'failed' : 'completed'
								} as SubagentEndEntry);
							}
						}
					}
				}
			}
		} else if (line.type === 'assistant') {
			const a = line as RawAssistantLine;
			const ts = computeRelativeTs(a.timestamp, sessionStart);

			if (a.message?.content) {
				const msgModel = a.message.model;
				const msgTokenUsage = parseTokenUsage(a.message.usage);
				let firstResponseEmitted = false;

				for (const block of a.message.content) {
					if (block.type === 'text' && 'text' in block) {
						const text = (block as { type: 'text'; text: string }).text;
						if (text.length > 0) {
							const entry: ResponseEntry = {
								id: crypto.randomUUID(),
								ts,
								type: 'response',
								content: text
							};
							if (!firstResponseEmitted) {
								firstResponseEmitted = true;
								if (msgModel) entry.model = msgModel;
								if (msgTokenUsage) entry.token_usage = msgTokenUsage;
							}
							entries.push(entry);
						}
					} else if (block.type === 'thinking' && 'thinking' in block) {
						const thinking = (block as { type: 'thinking'; thinking: string }).thinking;
						if (thinking.length > 0) {
							entries.push({
								id: crypto.randomUUID(),
								ts,
								type: 'thinking',
								content: thinking,
								collapsed: true
							} as ThinkingEntry);
						}
					} else if (block.type === 'tool_use' && 'id' in block && 'name' in block) {
						const b = block as { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> };
						const entryId = crypto.randomUUID();
						toolIdMap.set(b.id, entryId);

						let subagentId: string | undefined;

						// For Task tool calls, emit SubagentStart
						if (b.name === 'Task') {
							const subagentType =
								typeof b.input?.subagent_type === 'string'
									? b.input.subagent_type
									: 'unknown';
							const description =
								typeof b.input?.description === 'string'
									? b.input.description
									: undefined;

							const startId = crypto.randomUUID();
							entries.push({
								id: startId,
								ts,
								type: 'subagent_start',
								agent: subagentType,
								context: description
							} as SubagentStartEntry);

							taskSubagentMap.set(b.id, startId);
							subagentId = startId;
						}

						entries.push({
							id: entryId,
							ts,
							type: 'tool_call',
							tool: b.name,
							input: b.input || {},
							subagent_id: subagentId
						} as ToolCallEntry);
					}
				}
			}
		}
		// Skip other line types
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
			const modifiedPath = extractModifiedPath(tc.tool, tc.input);
			if (modifiedPath) filesModifiedSet.add(modifiedPath);
		}
	}

	// Update session entry
	const session = entries[0] as SessionEntry;
	session.duration_ms = lastTs;
	session.entry_count = entries.length;
	if (toolsUsedSet.size > 0) {
		session.tools_used = [...toolsUsedSet].sort();
	}
	if (filesModifiedSet.size > 0) {
		session.files_modified = [...filesModifiedSet].sort();
	}
	session.ended = 'completed';

	return { session, entries };
}

// ============================================================================
// Helpers
// ============================================================================

function computeRelativeTs(timestamp: string | undefined, sessionStart: Date): number {
	if (!timestamp) return 0;
	const ts = new Date(timestamp);
	if (isNaN(ts.getTime())) return 0;
	const diff = ts.getTime() - sessionStart.getTime();
	return Math.max(0, diff);
}

function parseTokenUsage(
	usage: { input_tokens?: number; output_tokens?: number; cache_read_input_tokens?: number; cache_creation_input_tokens?: number } | undefined
): TokenUsage | undefined {
	if (!usage) return undefined;
	if (typeof usage.input_tokens !== 'number' || typeof usage.output_tokens !== 'number') {
		return undefined;
	}
	return {
		input_tokens: usage.input_tokens,
		output_tokens: usage.output_tokens,
		cache_read_tokens: usage.cache_read_input_tokens,
		cache_creation_tokens: usage.cache_creation_input_tokens
	};
}

function extractToolResultText(content: string | { text?: string }[] | undefined): string {
	if (!content) return '';
	if (typeof content === 'string') return content;
	if (Array.isArray(content)) {
		return content
			.map((b) => b.text || '')
			.filter((t) => t.length > 0)
			.join('\n');
	}
	return '';
}

/** Strip system-injected XML tags from user messages */
function stripSystemTags(text: string): string {
	let result = text;
	// Remove <system-reminder>...</system-reminder> blocks
	const re = /<system-reminder>[\s\S]*?<\/system-reminder>/g;
	result = result.replace(re, '');
	return result.trim();
}

/** Extract a file path from a tool call if the tool modifies files */
function extractModifiedPath(tool: string, input: Record<string, unknown>): string | null {
	switch (tool) {
		case 'Write':
		case 'write':
		case 'write_file':
		case 'Edit':
		case 'edit':
		case 'edit_file': {
			const path = input.file_path ?? input.path;
			return typeof path === 'string' ? path : null;
		}
		case 'NotebookEdit':
		case 'notebook_edit': {
			const path = input.notebook_path;
			return typeof path === 'string' ? path : null;
		}
		default:
			return null;
	}
}

function truncateText(text: string, maxLen: number): string {
	if (text.length <= maxLen) return text;
	return text.slice(0, maxLen) + '...';
}

/** Extract a title from the first real user prompt */
function extractTitleFromLines(lines: RawLine[]): string | null {
	for (const line of lines) {
		if (line.type !== 'user') continue;
		const u = line as RawUserLine;
		if (u.isMeta) continue;
		if (!u.message?.content || typeof u.message.content !== 'string') continue;

		const text = u.message.content;
		if (
			text.includes('<command-name>') ||
			text.includes('<local-command-stdout>') ||
			text.includes('<local-command-caveat>')
		) {
			continue;
		}

		const clean = stripSystemTags(text);
		if (clean.length === 0) continue;

		const firstLine = clean.split('\n')[0];
		if (firstLine.length > 60) {
			return firstLine.slice(0, 57) + '...';
		}
		return firstLine;
	}
	return null;
}
