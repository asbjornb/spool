/**
 * Export a SpoolFile to JSONL string format for upload.
 *
 * Handles trimming, annotation injection, and redaction — all client-side.
 */

import type { Entry, SpoolFile, SessionEntry, AnnotationEntry, TrimmedMetadata } from './types';
import { detectSecrets, applyRedactions, type DetectedSecret } from './redaction';

/** Serialize a SpoolFile to .spool JSONL string */
export function spoolToJsonl(spool: SpoolFile): string {
	return spool.entries.map((entry) => JSON.stringify(entry)).join('\n') + '\n';
}

/** Trim a SpoolFile to a time range, returning a new SpoolFile */
export function trimSpool(spool: SpoolFile, startMs: number, endMs: number): SpoolFile {
	const originalDuration = spool.session.duration_ms ?? 0;

	const kept = spool.entries.filter((entry, i) => {
		if (i === 0) return true; // Always keep session entry
		return entry.ts >= startMs && entry.ts <= endMs;
	});

	const session: SessionEntry = {
		...spool.session,
		entry_count: kept.length,
		duration_ms: endMs - startMs,
		trimmed: {
			original_duration_ms: originalDuration,
			kept_range: [startMs, endMs]
		} as TrimmedMetadata
	};

	kept[0] = session;

	return { session, entries: kept };
}

/** Detect secrets across all entries in a SpoolFile */
export function detectSecretsInSpool(spool: SpoolFile): { entryIndex: number; entryId: string; secrets: DetectedSecret[] }[] {
	const results: { entryIndex: number; entryId: string; secrets: DetectedSecret[] }[] = [];

	for (let i = 0; i < spool.entries.length; i++) {
		const entry = spool.entries[i];
		const text = getEntryText(entry);
		if (!text) continue;

		const secrets = detectSecrets(text);
		if (secrets.length > 0) {
			results.push({ entryIndex: i, entryId: entry.id, secrets });
		}
	}

	return results;
}

/** Apply redactions to a SpoolFile, returning a new copy */
export function redactSpool(
	spool: SpoolFile,
	allSecrets: { entryIndex: number; secrets: DetectedSecret[] }[]
): SpoolFile {
	const entries = spool.entries.map((entry, i) => {
		const match = allSecrets.find((s) => s.entryIndex === i);
		if (!match) return entry;

		const confirmed = match.secrets.filter((s) => s.confirmed);
		if (confirmed.length === 0) return entry;

		return redactEntry(entry, confirmed);
	});

	const session = entries[0] as SessionEntry;
	return { session, entries };
}

/** Add an annotation to a SpoolFile, returning a new copy */
export function addAnnotation(
	spool: SpoolFile,
	targetId: string,
	targetTs: number,
	content: string,
	style: 'highlight' | 'comment' | 'pin' | 'warning' | 'success'
): SpoolFile {
	const annotation: AnnotationEntry = {
		id: crypto.randomUUID(),
		ts: targetTs,
		type: 'annotation',
		target_id: targetId,
		content,
		style,
		created_at: new Date().toISOString()
	};

	// Find the target entry and insert after it
	const targetIndex = spool.entries.findIndex((e) => e.id === targetId);
	const insertAt = targetIndex >= 0 ? targetIndex + 1 : spool.entries.length;

	const entries = [...spool.entries];
	entries.splice(insertAt, 0, annotation);

	const session: SessionEntry = {
		...spool.session,
		entry_count: entries.length
	};
	entries[0] = session;

	return { session, entries };
}

/** Remove an annotation from a SpoolFile */
export function removeAnnotation(spool: SpoolFile, annotationId: string): SpoolFile {
	const entries = spool.entries.filter((e) => e.id !== annotationId);
	const session: SessionEntry = {
		...spool.session,
		entry_count: entries.length
	};
	entries[0] = session;
	return { session, entries };
}

// ============================================================================
// Helpers
// ============================================================================

function getEntryText(entry: Entry): string | null {
	switch (entry.type) {
		case 'prompt':
			return (entry as { content: string }).content;
		case 'response':
			return (entry as { content: string }).content;
		case 'thinking':
			return (entry as { content: string }).content;
		case 'tool_result': {
			const tr = entry as { output?: string; error?: string };
			return typeof tr.output === 'string' ? tr.output : tr.error ?? null;
		}
		case 'error':
			return (entry as { message: string }).message;
		case 'annotation':
			return (entry as { content: string }).content;
		default:
			return null;
	}
}

function redactEntry(entry: Entry, secrets: DetectedSecret[]): Entry {
	const text = getEntryText(entry);
	if (!text) return entry;

	const redacted = applyRedactions(text, secrets);
	const copy = { ...entry };

	switch (copy.type) {
		case 'prompt':
		case 'response':
		case 'thinking':
		case 'annotation':
			(copy as { content: string }).content = redacted;
			break;
		case 'tool_result': {
			const tr = copy as { output?: string; error?: string };
			if (typeof tr.output === 'string') {
				tr.output = redacted;
			} else if (tr.error) {
				tr.error = redacted;
			}
			break;
		}
		case 'error':
			(copy as { message: string }).message = redacted;
			break;
	}

	return copy as Entry;
}
