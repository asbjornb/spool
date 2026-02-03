/** Spool format types — mirrors crates/spool-format/src/types.rs */

export type EntryType =
	| 'session'
	| 'prompt'
	| 'thinking'
	| 'tool_call'
	| 'tool_result'
	| 'response'
	| 'error'
	| 'subagent_start'
	| 'subagent_end'
	| 'annotation'
	| 'redaction_marker';

export type SessionEndState = 'completed' | 'cancelled' | 'error' | 'timeout' | 'unknown';
export type ErrorCode =
	| 'rate_limit'
	| 'api_error'
	| 'timeout'
	| 'auth_failed'
	| 'network_error'
	| 'context_overflow'
	| 'cancelled'
	| 'internal_error'
	| 'unknown'
	| string;
export type RedactionReason =
	| 'api_key'
	| 'password'
	| 'email'
	| 'phone'
	| 'path'
	| 'ip_address'
	| 'pii'
	| 'custom';
export type AnnotationStyle = 'highlight' | 'comment' | 'pin' | 'warning' | 'success';
export type SubagentStatus = 'completed' | 'failed' | 'cancelled';

export interface TrimmedMetadata {
	original_duration_ms: number;
	kept_range: [number, number];
}

export interface TokenUsage {
	input_tokens: number;
	output_tokens: number;
	cache_read_tokens?: number;
	cache_creation_tokens?: number;
}

export interface Attachment {
	type: 'binary';
	media_type: string;
	encoding: 'base64';
	data: string;
	filename?: string;
	size_bytes?: number;
}

export interface RedactionInfo {
	reason: RedactionReason;
	count: number;
}

// Base fields shared by all entries
interface EntryBase {
	id: string;
	ts: number;
	type: EntryType;
	[key: string]: unknown; // forward-compat extra fields
}

export interface SessionEntry extends EntryBase {
	type: 'session';
	version: string;
	agent: string;
	recorded_at: string;
	agent_version?: string;
	title?: string;
	author?: string;
	tags?: string[];
	duration_ms?: number;
	entry_count?: number;
	tools_used?: string[];
	files_modified?: string[];
	first_prompt?: string;
	schema_url?: string;
	trimmed?: TrimmedMetadata;
	ended?: SessionEndState;
}

export interface PromptEntry extends EntryBase {
	type: 'prompt';
	content: string;
	subagent_id?: string;
	attachments?: Attachment[];
}

export interface ThinkingEntry extends EntryBase {
	type: 'thinking';
	content: string;
	collapsed?: boolean;
	truncated?: boolean;
	original_bytes?: number;
	subagent_id?: string;
}

export interface ToolCallEntry extends EntryBase {
	type: 'tool_call';
	tool: string;
	input: Record<string, unknown>;
	subagent_id?: string;
}

export interface ToolResultEntry extends EntryBase {
	type: 'tool_result';
	call_id: string;
	output?: string | Attachment;
	error?: string;
	truncated?: boolean;
	original_bytes?: number;
	subagent_id?: string;
	_redacted?: RedactionInfo[];
}

export interface ResponseEntry extends EntryBase {
	type: 'response';
	content: string;
	truncated?: boolean;
	original_bytes?: number;
	model?: string;
	token_usage?: TokenUsage;
	subagent_id?: string;
}

export interface ErrorEntry extends EntryBase {
	type: 'error';
	code: ErrorCode;
	message: string;
	recoverable?: boolean;
	details?: Record<string, unknown>;
	subagent_id?: string;
}

export interface SubagentStartEntry extends EntryBase {
	type: 'subagent_start';
	agent: string;
	context?: string;
	parent_subagent_id?: string;
}

export interface SubagentEndEntry extends EntryBase {
	type: 'subagent_end';
	start_id: string;
	summary?: string;
	status?: SubagentStatus;
}

export interface AnnotationEntry extends EntryBase {
	type: 'annotation';
	target_id: string;
	content: string;
	author?: string;
	style?: AnnotationStyle;
	created_at?: string;
}

export interface RedactionMarkerEntry extends EntryBase {
	type: 'redaction_marker';
	target_id: string;
	reason?: RedactionReason;
	count?: number;
	inline?: boolean;
}

export type Entry =
	| SessionEntry
	| PromptEntry
	| ThinkingEntry
	| ToolCallEntry
	| ToolResultEntry
	| ResponseEntry
	| ErrorEntry
	| SubagentStartEntry
	| SubagentEndEntry
	| AnnotationEntry
	| RedactionMarkerEntry;

/** A parsed .spool file */
export interface SpoolFile {
	session: SessionEntry;
	entries: Entry[];
}
