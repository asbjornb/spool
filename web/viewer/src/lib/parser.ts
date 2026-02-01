import type { Entry, SessionEntry, SpoolFile } from './types';

/** Parse a .spool JSONL string into a SpoolFile */
export function parseSpool(text: string): SpoolFile {
	const lines = text.split('\n').filter((line) => line.trim().length > 0);

	if (lines.length === 0) {
		throw new Error('Empty .spool file');
	}

	const entries: Entry[] = [];

	for (let i = 0; i < lines.length; i++) {
		try {
			const parsed = JSON.parse(lines[i]) as Entry;
			entries.push(parsed);
		} catch {
			// Skip unparseable lines (forward compatibility)
			console.warn(`Skipping unparseable line ${i + 1}`);
		}
	}

	if (entries.length === 0) {
		throw new Error('No valid entries found');
	}

	const first = entries[0];
	if (first.type !== 'session') {
		throw new Error(`First entry must be type "session", got "${first.type}"`);
	}

	return {
		session: first as SessionEntry,
		entries
	};
}

/** Format a timestamp (ms since session start) as mm:ss */
export function formatTimestamp(ms: number): string {
	const totalSeconds = Math.floor(ms / 1000);
	const minutes = Math.floor(totalSeconds / 60);
	const seconds = totalSeconds % 60;
	return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

/** Format a duration in ms as human-readable */
export function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
	const minutes = Math.floor(ms / 60_000);
	const seconds = Math.floor((ms % 60_000) / 1000);
	return `${minutes}m ${seconds}s`;
}

/** Get a display label for an entry type */
export function entryTypeLabel(type: string): string {
	const labels: Record<string, string> = {
		session: 'Session',
		prompt: 'Prompt',
		thinking: 'Thinking',
		tool_call: 'Tool Call',
		tool_result: 'Tool Result',
		response: 'Response',
		error: 'Error',
		subagent_start: 'Subagent Start',
		subagent_end: 'Subagent End',
		annotation: 'Annotation',
		redaction_marker: 'Redacted'
	};
	return labels[type] ?? type;
}
