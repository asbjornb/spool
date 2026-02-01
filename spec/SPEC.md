# Spool Format Specification

**Version:** 1.0  
**Status:** Draft  
**Last Updated:** 2025-01-31

## Abstract

This document specifies Spool, a file format for recording, replaying, and sharing AI agent sessions. Spool files capture the complete interaction history between users and AI agents, including prompts, agent reasoning, tool invocations, and results. The format is designed to be streamable, human-readable, and interoperable across different agent implementations.

## Table of Contents

1. [Introduction](#1-introduction)
2. [Conventions and Terminology](#2-conventions-and-terminology)
3. [File Identification](#3-file-identification)
4. [File Structure](#4-file-structure)
5. [Common Entry Fields](#5-common-entry-fields)
6. [Entry Types](#6-entry-types)
7. [Forward Compatibility](#7-forward-compatibility)
8. [Canonicalization](#8-canonicalization)
9. [Binary and Special Content](#9-binary-and-special-content)
10. [Security Considerations](#10-security-considerations)
11. [Normative Test Cases](#11-normative-test-cases)
12. [Complete Examples](#12-complete-examples)
13. [References](#13-references)

---

## 1. Introduction

### 1.1 Purpose

Spool provides a standardized format for capturing AI agent sessions, enabling:

- **Replay**: Step through agent sessions at variable speeds
- **Debugging**: Understand agent behavior, find failures, analyze tool usage
- **Sharing**: Publish sessions for education, documentation, or collaboration
- **Interoperability**: Exchange sessions between different agent implementations

### 1.2 Design Principles

1. **JSONL-based**: One JSON object per line, enabling streaming reads and writes
2. **Unique identifiers**: Every entry has a globally unique ID for linking and annotation
3. **Relative timestamps**: Milliseconds from session start, enabling variable-speed playback
4. **Destructive redaction**: Sensitive data is replaced at write time, never stored
5. **Agent-agnostic**: Works across Claude Code, Codex, Cursor, Aider, and future agents
6. **Self-contained**: Single file contains everything needed to render a session

### 1.3 Scope

This specification defines only the `.spool` file format. Command-line tools, web services, and other implementations that consume or produce Spool files are out of scope.

---

## 2. Conventions and Terminology

### 2.1 RFC 2119 Keywords

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

### 2.2 Definitions

- **Entry**: A single JSON object on one line of a Spool file
- **Session**: The complete recording represented by a Spool file
- **Agent**: An AI system that processes user prompts and may invoke tools
- **Tool**: An external capability an agent can invoke (file I/O, shell commands, web search, etc.)
- **Subagent**: An agent spawned by another agent to handle a delegated task
- **Annotation**: Metadata added to an entry after initial recording
- **Redaction**: The process of removing sensitive data and replacing it with placeholder text

### 2.3 Data Types

Throughout this specification:

- **string**: A UTF-8 encoded JSON string
- **integer**: A JSON number with no fractional component
- **boolean**: JSON `true` or `false`
- **array**: A JSON array
- **object**: A JSON object
- **timestamp**: A non-negative integer representing milliseconds since session start
- **uuid**: A string formatted as a UUID (see Section 5.1)
- **iso8601**: A string formatted per ISO 8601 (e.g., `"2025-01-31T10:30:00Z"`)

---

## 3. File Identification

### 3.1 File Extension

Spool files MUST use the `.spool` file extension.

### 3.2 MIME Type

The MIME type for Spool files is:

```
application/vnd.spool+jsonl
```

Implementations MAY also accept `application/jsonl` or `application/x-ndjson` but SHOULD use the specific MIME type when available.

### 3.3 Magic Bytes

Spool files do not define magic bytes. Format detection SHOULD rely on:

1. File extension (`.spool`)
2. Presence of a valid session metadata entry as the first line
3. MIME type when available

Implementations MAY check that the first line is valid JSON containing `"type":"session"` as a heuristic for format detection.

### 3.4 Character Encoding

Spool files MUST be encoded as UTF-8 without a byte order mark (BOM). Writers MUST NOT emit a BOM. Readers SHOULD ignore a leading BOM if present for robustness.

### 3.5 Line Endings

Writers SHOULD use Unix-style line endings (`\n`, U+000A).

Readers MUST accept both Unix-style (`\n`) and Windows-style (`\r\n`, U+000D U+000A) line endings. Readers MUST treat a final line without a trailing line ending as valid.

### 3.6 Whitespace

Each line MUST contain exactly one JSON object with no leading or trailing whitespace outside the JSON structure. Blank lines (lines containing only whitespace) MUST be ignored by readers.

---

## 4. File Structure

### 4.1 Overall Structure

A Spool file consists of one or more JSON objects, each on its own line. The file structure is:

```
<session_entry>
<entry>*
```

Where:
- `<session_entry>` is REQUIRED and MUST be the first non-blank line
- `<entry>*` represents zero or more additional entries

### 4.2 Entry Ordering

Entries SHOULD be ordered by their `ts` (timestamp) field in non-decreasing order. However, readers MUST NOT assume entries are ordered and MUST handle out-of-order entries gracefully.

When multiple entries share the same timestamp, they SHOULD appear in the order they were recorded. For annotations, see Section 6.9.2 for additional ordering rules.

### 4.3 Empty Files

A file containing only whitespace or no content is invalid. A valid Spool file MUST contain at least the session metadata entry.

---

## 5. Common Entry Fields

### 5.1 Entry Identifier (`id`)

Every entry MUST have an `id` field containing a unique identifier.

**Format**: IDs MUST be formatted as lowercase UUID strings with hyphens, following the pattern:

```
xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

Where each `x` is a lowercase hexadecimal digit (`[0-9a-f]`).

**Generation**: Implementations SHOULD use UUID version 7 (time-ordered) for new entries to provide natural ordering. UUID version 4 (random) is also acceptable. Other UUID versions MAY be used.

**Uniqueness**: IDs MUST be unique within a single Spool file. Implementations SHOULD generate IDs that are globally unique to facilitate merging files and cross-file references.

**Examples**:
```
"018d5f2c-8a3b-7def-9123-456789abcdef"  (v7, recommended)
"550e8400-e29b-41d4-a716-446655440000"  (v4, acceptable)
```

### 5.2 Timestamp (`ts`)

Every entry MUST have a `ts` field containing the timestamp.

**Type**: Non-negative integer

**Unit**: Milliseconds since the session start (the `recorded_at` time in the session metadata)

**Constraints**:
- MUST be >= 0
- MUST be representable as a 64-bit signed integer (max value: 2^63 - 1)
- Negative values are invalid

**Duration Calculation**: The duration of an entry's visible effect (e.g., how long thinking text was displayed) is implicitly the difference between its timestamp and the next entry's timestamp. No explicit duration field is required.

### 5.3 Entry Type (`type`)

Every entry MUST have a `type` field identifying the entry type.

**Type**: String

**Values**: One of the types defined in Section 6, or an extension type (see Section 7.2).

---

## 6. Entry Types

### 6.1 Session Metadata (`session`)

The session metadata entry provides information about the recording and MUST be the first entry in every Spool file.

**Type value**: `"session"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | MUST be `0` |
| `type` | string | MUST be `"session"` |
| `version` | string | Spool format version (for this spec: `"1.0"`) |
| `agent` | string | Agent identifier (see Section 6.1.1) |
| `recorded_at` | iso8601 | Wall-clock time when recording started |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `agent_version` | string | Version of the agent software |
| `title` | string | Human-readable session title |
| `author` | string | Creator's name or handle |
| `tags` | array of strings | Searchable tags |
| `duration_ms` | integer | Total session duration in milliseconds |
| `entry_count` | integer | Total number of entries (including this one) |
| `tools_used` | array of strings | Tool names invoked during session |
| `schema_url` | string | URL to the specification version used |
| `trimmed` | object | Present if file was trimmed (see Section 6.1.2) |
| `ended` | string | How the session ended (see Section 6.1.3) |

#### 6.1.1 Agent Identifiers

The `agent` field SHOULD use one of these standardized identifiers when applicable:

- `claude-code` — Anthropic Claude Code
- `codex` — OpenAI Codex CLI
- `cursor` — Cursor IDE agent
- `aider` — Aider
- `github-copilot` — GitHub Copilot
- `cline` — Cline
- `continue` — Continue

For agents not in this list, implementations SHOULD use a lowercase identifier with hyphens (e.g., `my-custom-agent`).

#### 6.1.2 Trimmed Metadata

When a Spool file has been trimmed from a longer recording, the `trimmed` object contains:

| Field | Type | Description |
|-------|------|-------------|
| `original_duration_ms` | integer | Duration of the original recording |
| `kept_range` | array | Two-element array `[start_ms, end_ms]` of kept range |

**Example**:
```json
"trimmed": {"original_duration_ms": 3600000, "kept_range": [120000, 180000]}
```

#### 6.1.3 Session End States

The optional `ended` field indicates how the session concluded:

- `"completed"` — Agent finished normally
- `"cancelled"` — User cancelled the session
- `"error"` — Session ended due to an error (see `error` entry type)
- `"timeout"` — Session exceeded a time limit
- `"unknown"` — End state not recorded

If omitted, the end state is unknown.

**Example session entry**:
```json
{"id":"018d5f2c-8a3b-7def-9123-456789abcdef","ts":0,"type":"session","version":"1.0","agent":"claude-code","agent_version":"1.2.0","recorded_at":"2025-01-31T10:30:00Z","title":"Security review of auth.py","author":"alex","tags":["security","review"],"duration_ms":45000,"entry_count":12,"tools_used":["read_file","bash"],"ended":"completed"}
```

### 6.2 User Prompt (`prompt`)

A prompt entry represents user input to the agent.

**Type value**: `"prompt"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the prompt was submitted |
| `type` | string | MUST be `"prompt"` |
| `content` | string | The user's input text |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |
| `attachments` | array | File attachments (see Section 9.2) |

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000001","ts":0,"type":"prompt","content":"Review this PR for security issues"}
```

### 6.3 Agent Thinking (`thinking`)

A thinking entry represents the agent's internal reasoning process.

**Type value**: `"thinking"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When thinking began |
| `type` | string | MUST be `"thinking"` |
| `content` | string | The thinking text |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `collapsed` | boolean | Hint to collapse in viewers (default: `false`) |
| `truncated` | boolean | Whether content was truncated (default: `false`) |
| `original_bytes` | integer | Original byte length before truncation |
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000002","ts":1200,"type":"thinking","content":"Let me analyze the authentication flow...","collapsed":true}
```

### 6.4 Tool Call (`tool_call`)

A tool call entry represents the agent invoking an external tool.

**Type value**: `"tool_call"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the tool was invoked |
| `type` | string | MUST be `"tool_call"` |
| `tool` | string | Tool name |
| `input` | object | Tool input parameters |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |

#### 6.4.1 Tool Names

Tool names are opaque strings. Different agents may use different names for equivalent functionality (e.g., `bash` vs `shell` vs `execute_command`).

Implementations SHOULD document the tool names they use. When sharing files across agent ecosystems, implementations MAY provide tool name mappings, but this is outside the scope of this specification.

**Common tool names** (informational, not normative):

| Tool | Common Names |
|------|--------------|
| Read file | `read_file`, `read`, `cat`, `view_file` |
| Write file | `write_file`, `write`, `edit_file`, `save_file` |
| Execute command | `bash`, `shell`, `execute`, `run_command`, `terminal` |
| Search | `search`, `grep`, `find`, `ripgrep` |
| Web search | `web_search`, `search_web`, `browser_search` |
| Web fetch | `web_fetch`, `fetch_url`, `http_get` |

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000003","ts":5000,"type":"tool_call","tool":"read_file","input":{"path":"src/auth.py"}}
```

### 6.5 Tool Result (`tool_result`)

A tool result entry contains the output from a tool invocation.

**Type value**: `"tool_result"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the result was received |
| `type` | string | MUST be `"tool_result"` |
| `call_id` | uuid | References the `tool_call` entry this responds to |

**Conditional fields** (exactly one of `output` or `error` MUST be present):

| Field | Type | Description |
|-------|------|-------------|
| `output` | string or object | Tool output (see Section 9 for binary content) |
| `error` | string | Error message if the tool failed |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `truncated` | boolean | Whether output was truncated (default: `false`) |
| `original_bytes` | integer | Original byte length before truncation |
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |

**Example (success)**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000004","ts":5500,"type":"tool_result","call_id":"018d5f2c-8a3b-7def-0001-000000000003","output":"def verify_token(token):\n    ...","truncated":true,"original_bytes":15420}
```

**Example (error)**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000004","ts":5500,"type":"tool_result","call_id":"018d5f2c-8a3b-7def-0001-000000000003","error":"File not found: src/auth.py"}
```

### 6.6 Agent Response (`response`)

A response entry contains the agent's output to the user.

**Type value**: `"response"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the response was generated |
| `type` | string | MUST be `"response"` |
| `content` | string | The response text |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `truncated` | boolean | Whether content was truncated (default: `false`) |
| `original_bytes` | integer | Original byte length before truncation |
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000005","ts":8000,"type":"response","content":"I found a SQL injection vulnerability in auth.py on line 47..."}
```

### 6.7 Error (`error`)

An error entry represents a failure during the session that is not tied to a specific tool invocation.

**Type value**: `"error"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the error occurred |
| `type` | string | MUST be `"error"` |
| `code` | string | Error code (see Section 6.7.1) |
| `message` | string | Human-readable error description |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `recoverable` | boolean | Whether the session continued after this error (default: `false`) |
| `details` | object | Additional error-specific information |
| `subagent_id` | uuid | If within a subagent, references the `subagent_start` entry |

#### 6.7.1 Error Codes

Standard error codes (implementations MAY define additional codes):

| Code | Description |
|------|-------------|
| `rate_limit` | API rate limit exceeded |
| `api_error` | API returned an error |
| `timeout` | Operation timed out |
| `auth_failed` | Authentication failure |
| `network_error` | Network connectivity issue |
| `context_overflow` | Context window exceeded |
| `cancelled` | User cancelled operation |
| `internal_error` | Internal agent error |
| `unknown` | Unclassified error |

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0001-000000000006","ts":10000,"type":"error","code":"rate_limit","message":"API rate limit exceeded, waiting 60 seconds","recoverable":true,"details":{"retry_after_ms":60000}}
```

### 6.8 Subagent Entries

When an agent delegates work to another agent instance, the interaction is wrapped in subagent entries.

#### 6.8.1 Subagent Start (`subagent_start`)

**Type value**: `"subagent_start"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier (referenced by contained entries) |
| `ts` | timestamp | When the subagent was spawned |
| `type` | string | MUST be `"subagent_start"` |
| `agent` | string | Subagent identifier |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `context` | string | Reason for delegation |
| `parent_subagent_id` | uuid | If nested, references the parent `subagent_start` |

#### 6.8.2 Subagent End (`subagent_end`)

**Type value**: `"subagent_end"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | When the subagent completed |
| `type` | string | MUST be `"subagent_end"` |
| `start_id` | uuid | References the corresponding `subagent_start` entry |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `summary` | string | Summary of what the subagent accomplished |
| `status` | string | `"completed"`, `"failed"`, or `"cancelled"` (default: `"completed"`) |

#### 6.8.3 Subagent Nesting

Subagents MAY spawn their own subagents. When this occurs:

1. The nested `subagent_start` entry MUST include `parent_subagent_id` referencing the parent
2. Entries within the nested subagent include `subagent_id` referencing the innermost `subagent_start`
3. There is no specification-defined limit on nesting depth

Implementations SHOULD impose reasonable depth limits (e.g., 10 levels) to prevent resource exhaustion.

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0002-000000000001","ts":9000,"type":"subagent_start","agent":"security-reviewer","context":"Delegating detailed security analysis"}
{"id":"018d5f2c-8a3b-7def-0002-000000000002","ts":9100,"type":"prompt","content":"Analyze auth.py for OWASP Top 10","subagent_id":"018d5f2c-8a3b-7def-0002-000000000001"}
{"id":"018d5f2c-8a3b-7def-0002-000000000003","ts":12000,"type":"response","content":"Found 3 issues...","subagent_id":"018d5f2c-8a3b-7def-0002-000000000001"}
{"id":"018d5f2c-8a3b-7def-0002-000000000004","ts":12100,"type":"subagent_end","start_id":"018d5f2c-8a3b-7def-0002-000000000001","summary":"Security analysis complete","status":"completed"}
```

### 6.9 Annotation (`annotation`)

Annotations are metadata added to entries after initial recording, typically during editing or review.

**Type value**: `"annotation"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | SHOULD match the `ts` of `target_id` entry |
| `type` | string | MUST be `"annotation"` |
| `target_id` | uuid | The entry being annotated |
| `content` | string | Annotation text |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `author` | string | Who created the annotation |
| `style` | string | Presentation hint (see Section 6.9.1) |
| `created_at` | iso8601 | Wall-clock time annotation was created |

#### 6.9.1 Annotation Styles

| Style | Description |
|-------|-------------|
| `highlight` | Yellow background highlight |
| `comment` | Speech bubble or marginal note |
| `pin` | Important marker / bookmark |
| `warning` | Indicates a problem or concern |
| `success` | Indicates something positive |

If `style` is omitted, implementations SHOULD default to `comment`.

#### 6.9.2 Annotation Ordering

When multiple annotations target the same entry, they share the same `ts` value. To establish order:

1. Annotations with `created_at` SHOULD be ordered by that field
2. Annotations without `created_at` SHOULD appear after those with it
3. Among annotations with identical ordering, implementations MAY use any consistent order

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0003-000000000001","ts":3200,"type":"annotation","target_id":"018d5f2c-8a3b-7def-0001-000000000004","content":"SQL injection! Token is interpolated directly.","author":"alex","style":"highlight","created_at":"2025-01-31T11:45:00Z"}
```

### 6.10 Redaction Marker (`redaction_marker`)

Redaction markers are metadata indicating that sensitive content was removed from an entry. The actual redaction is performed at write time; this entry only provides information for display purposes.

**Type value**: `"redaction_marker"`

**Required fields**:

| Field | Type | Description |
|-------|------|-------------|
| `id` | uuid | Entry identifier |
| `ts` | timestamp | SHOULD match the `ts` of `target_id` entry |
| `type` | string | MUST be `"redaction_marker"` |
| `target_id` | uuid | The entry containing redacted content |

**Optional fields**:

| Field | Type | Description |
|-------|------|-------------|
| `reason` | string | Category of redacted content (see Section 6.10.1) |
| `count` | integer | Number of redactions in the target entry (default: 1) |
| `inline` | boolean | If `true`, redaction metadata is also in target entry (default: `false`) |

#### 6.10.1 Redaction Reasons

| Reason | Description |
|--------|-------------|
| `api_key` | API key or token |
| `password` | Password or secret |
| `email` | Email address |
| `phone` | Phone number |
| `path` | File system path |
| `ip_address` | IP address |
| `pii` | Other personally identifiable information |
| `custom` | User-specified redaction |

#### 6.10.2 Inline Redaction

For robustness against marker/target separation, implementations MAY include redaction metadata directly in the target entry using the `_redacted` field:

```json
{"id":"...","ts":5500,"type":"tool_result","call_id":"...","output":"API key: [REDACTED]","_redacted":[{"reason":"api_key","count":1}]}
```

When `inline` is `true` in a `redaction_marker`, the target entry SHOULD contain corresponding `_redacted` metadata.

#### 6.10.3 Orphaned Markers

If a `redaction_marker` references a `target_id` that does not exist in the file, implementations MUST ignore the marker and SHOULD log a warning. The marker MUST be preserved if round-tripping the file.

**Example**:
```json
{"id":"018d5f2c-8a3b-7def-0004-000000000001","ts":5500,"type":"redaction_marker","target_id":"018d5f2c-8a3b-7def-0001-000000000004","reason":"api_key","count":1}
```

---

## 7. Forward Compatibility

### 7.1 Unknown Entry Types

Readers MUST ignore entry types they do not recognize. Unknown entries:

- MUST be preserved when round-tripping (reading and writing) a file
- MUST NOT cause parsing to fail
- MAY be hidden in user interfaces
- SHOULD be logged for debugging purposes

### 7.2 Extension Entry Types

To avoid conflicts with future specification versions, custom entry types:

- MUST be prefixed with `x_` (e.g., `x_custom_metric`)
- SHOULD include a namespace after the prefix (e.g., `x_myapp_checkpoint`)

### 7.3 Unknown Fields

Readers MUST preserve fields they do not recognize when round-tripping a file. Unknown fields:

- MUST NOT cause parsing to fail
- MUST be included when writing the entry back out
- MAY be ignored for display purposes

### 7.4 Extension Fields

To avoid conflicts with future specification versions, custom fields:

- MUST be prefixed with `x_` (e.g., `x_custom_data`)
- SHOULD include a namespace after the prefix (e.g., `x_myapp_priority`)

### 7.5 Version Handling

The `version` field in the session metadata indicates the specification version.

- Readers SHOULD accept files with version `"1.x"` where x >= 0
- Readers MAY reject files with major version > 1
- Future minor versions (1.1, 1.2, etc.) will be backward compatible with 1.0

---

## 8. Canonicalization

### 8.1 Non-Goal

This specification does NOT define a canonical byte-level representation. Two semantically identical Spool files may have different byte sequences due to:

- JSON field ordering
- Numeric representation (e.g., `1.0` vs `1`)
- Unicode normalization
- Whitespace within JSON values

### 8.2 Implications

Because canonicalization is not defined:

- File hashes cannot be used to compare semantic equality
- Digital signatures over file contents require signing the exact bytes
- Content-addressable storage should use semantic comparison, not byte comparison

### 8.3 Recommended Practices

For applications requiring reproducible output, implementations SHOULD:

- Sort JSON object keys alphabetically
- Use minimal whitespace (no indentation, no spaces after colons/commas)
- Represent integers without decimal points
- Use lowercase for hexadecimal in UUIDs
- Normalize Unicode strings to NFC form

These practices are RECOMMENDED but not REQUIRED for conformance.

---

## 9. Binary and Special Content

### 9.1 Text Content

All string fields MUST contain valid UTF-8 text. The following characters MUST be escaped in JSON strings per RFC 8259:

- Quotation mark (`"`) → `\"`
- Reverse solidus (`\`) → `\\`
- Control characters (U+0000 through U+001F) → `\uXXXX`

### 9.2 Binary Content

For binary data (images, PDFs, compiled files, etc.), use the following encoding:

The `output` field of `tool_result` MAY be an object instead of a string:

```json
{
  "output": {
    "type": "binary",
    "media_type": "image/png",
    "encoding": "base64",
    "data": "iVBORw0KGgoAAAANSUhEUgAA...",
    "size_bytes": 15420
  }
}
```

**Binary object fields**:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | MUST be `"binary"` |
| `media_type` | string | Yes | MIME type of the content |
| `encoding` | string | Yes | MUST be `"base64"` |
| `data` | string | Yes | Base64-encoded content |
| `size_bytes` | integer | No | Original size before encoding |
| `filename` | string | No | Original filename if known |
| `truncated` | boolean | No | Whether data was truncated |

### 9.3 Attachments

Prompt entries MAY include file attachments:

```json
{
  "id": "...",
  "ts": 0,
  "type": "prompt",
  "content": "Review this image",
  "attachments": [
    {
      "type": "binary",
      "media_type": "image/png",
      "encoding": "base64",
      "data": "...",
      "filename": "screenshot.png"
    }
  ]
}
```

### 9.4 Invalid UTF-8

If a tool produces output containing invalid UTF-8 sequences:

1. Implementations SHOULD encode the entire output as binary
2. Alternatively, implementations MAY replace invalid sequences with U+FFFD (replacement character) and note this in a `_encoding_errors` field

### 9.5 Large Content

There is no specification-defined size limit for entries or files. However:

- Implementations MAY impose limits for resource management
- Implementations SHOULD use streaming parsers for large files
- The `truncated` and `original_bytes` fields support recording when content was reduced

---

## 10. Security Considerations

### 10.1 Parsing Untrusted Files

Implementations parsing Spool files from untrusted sources MUST:

1. **Limit resource consumption**: Set maximum limits for:
   - Total file size
   - Individual line length
   - Number of entries
   - Nesting depth (for subagents)
   - Base64 decoded size

2. **Validate input**: Check that:
   - JSON is well-formed
   - Required fields are present
   - Field types match expectations
   - UUIDs are properly formatted
   - Timestamps are non-negative

3. **Handle malformed data gracefully**: Invalid entries should be skipped or rejected without crashing

### 10.2 Content Security

#### 10.2.1 Cross-Site Scripting (XSS)

Spool files may contain arbitrary text from user prompts and agent outputs. Implementations displaying this content in web contexts MUST:

- Escape HTML entities in all displayed text
- Use Content Security Policy headers
- Avoid `innerHTML` or equivalent unsafe APIs
- Sanitize any content used in URLs

#### 10.2.2 Path Traversal

Tool inputs and outputs may contain file paths. Implementations MUST:

- Never use paths from Spool files to access the local filesystem
- Validate and sanitize paths if display is necessary
- Treat all paths as untrusted strings

### 10.3 Redaction Limitations

Redaction in Spool is **informational, not security-critical**:

- Redaction markers may become separated from targets
- Original content may exist in other copies of the file
- Pattern-based redaction may miss sensitive content
- Redaction cannot be verified cryptographically

Implementations SHOULD:

- Display redaction markers visually (e.g., 🔒 icon)
- Warn users that redaction is not a security guarantee
- Provide tools for users to verify redaction completeness

### 10.4 Sensitive Metadata

Session metadata may reveal sensitive information:

- `author` may identify individuals
- `recorded_at` reveals timing information
- `tags` may contain project names or code names
- File paths in tool calls reveal directory structure

Users sharing Spool files SHOULD review all content, not just redaction markers.

---

## 11. Normative Test Cases

Implementations MUST correctly handle the following test cases.

### 11.1 Minimal Valid File

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}
```

A file containing only this line MUST be accepted as valid.

### 11.2 Unknown Entry Type (Must Be Ignored)

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}
{"id":"00000000-0000-0000-0000-000000000001","ts":100,"type":"x_future_type","data":"unknown"}
{"id":"00000000-0000-0000-0000-000000000002","ts":200,"type":"prompt","content":"Hello"}
```

Readers MUST:
- Accept this file without error
- Process the `session` and `prompt` entries normally
- Preserve the `x_future_type` entry when writing the file back out

### 11.3 Unknown Fields (Must Be Preserved)

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z","x_custom_field":"value","x_nested":{"a":1}}
```

Readers MUST preserve `x_custom_field` and `x_nested` when round-tripping.

### 11.4 Error Conditions

#### 11.4.1 Empty File (Invalid)

An empty file or file containing only whitespace MUST be rejected.

#### 11.4.2 Missing Session Entry (Invalid)

```json
{"id":"00000000-0000-0000-0000-000000000001","ts":100,"type":"prompt","content":"Hello"}
```

A file not starting with a `session` entry MUST be rejected.

#### 11.4.3 Invalid JSON Line

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}
{invalid json here}
{"id":"00000000-0000-0000-0000-000000000002","ts":200,"type":"prompt","content":"Hello"}
```

Implementations MUST either:
- Reject the entire file, OR
- Skip the invalid line and continue processing (logging a warning)

The chosen behavior SHOULD be documented and consistent.

#### 11.4.4 Duplicate IDs

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}
{"id":"00000000-0000-0000-0000-000000000001","ts":100,"type":"prompt","content":"First"}
{"id":"00000000-0000-0000-0000-000000000001","ts":200,"type":"prompt","content":"Duplicate ID"}
```

Implementations SHOULD accept this file but MAY:
- Log a warning about duplicate IDs
- Use only the first or last entry with a given ID
- Treat references to the duplicate ID as ambiguous

### 11.5 Line Ending Variations

All of the following MUST be accepted:
- Unix line endings (`\n`)
- Windows line endings (`\r\n`)
- Mixed line endings
- Missing final line ending

### 11.6 Out-of-Order Timestamps

```json
{"id":"00000000-0000-0000-0000-000000000000","ts":0,"type":"session","version":"1.0","agent":"test","recorded_at":"2025-01-01T00:00:00Z"}
{"id":"00000000-0000-0000-0000-000000000001","ts":500,"type":"prompt","content":"First"}
{"id":"00000000-0000-0000-0000-000000000002","ts":200,"type":"thinking","content":"Out of order"}
{"id":"00000000-0000-0000-0000-000000000003","ts":800,"type":"response","content":"Response"}
```

Readers MUST accept this file. Display order is implementation-defined but SHOULD present entries in timestamp order.

---

## 12. Complete Examples

### 12.1 Simple Session

```json
{"id":"018d5f2c-0000-7000-8000-000000000000","ts":0,"type":"session","version":"1.0","agent":"claude-code","agent_version":"1.2.0","recorded_at":"2025-01-31T10:30:00Z","title":"Finding the auth bug","duration_ms":8500,"entry_count":6,"tools_used":["read_file"],"ended":"completed"}
{"id":"018d5f2c-0000-7000-8000-000000000001","ts":0,"type":"prompt","content":"There's a security bug in auth.py, can you find it?"}
{"id":"018d5f2c-0000-7000-8000-000000000002","ts":500,"type":"thinking","content":"I'll start by reading the authentication code to understand the current implementation.","collapsed":true}
{"id":"018d5f2c-0000-7000-8000-000000000003","ts":3000,"type":"tool_call","tool":"read_file","input":{"path":"src/auth.py"}}
{"id":"018d5f2c-0000-7000-8000-000000000004","ts":3200,"type":"tool_result","call_id":"018d5f2c-0000-7000-8000-000000000003","output":"def verify_token(t):\n    query = f\"SELECT * FROM users WHERE token={t}\"\n    return db.execute(query)"}
{"id":"018d5f2c-0000-7000-8000-000000000005","ts":8000,"type":"response","content":"Found it! Line 2 has a SQL injection vulnerability. The token is directly interpolated into the query string instead of using parameterized queries.\n\nThe fix is to use parameterized queries:\n```python\nquery = \"SELECT * FROM users WHERE token = ?\"\nreturn db.execute(query, (t,))\n```"}
```

### 12.2 Session with Annotations and Redactions

```json
{"id":"018d5f2c-1000-7000-8000-000000000000","ts":0,"type":"session","version":"1.0","agent":"claude-code","recorded_at":"2025-01-31T14:00:00Z","title":"API Integration","author":"alex","tags":["api","tutorial"]}
{"id":"018d5f2c-1000-7000-8000-000000000001","ts":0,"type":"prompt","content":"Help me set up the Stripe API"}
{"id":"018d5f2c-1000-7000-8000-000000000002","ts":2000,"type":"tool_call","tool":"read_file","input":{"path":"config.json"}}
{"id":"018d5f2c-1000-7000-8000-000000000003","ts":2100,"type":"tool_result","call_id":"018d5f2c-1000-7000-8000-000000000002","output":"{\n  \"stripe_key\": \"[REDACTED]\"\n}","_redacted":[{"reason":"api_key","count":1}]}
{"id":"018d5f2c-1000-7000-8000-000000000004","ts":2100,"type":"redaction_marker","target_id":"018d5f2c-1000-7000-8000-000000000003","reason":"api_key","count":1,"inline":true}
{"id":"018d5f2c-1000-7000-8000-000000000005","ts":5000,"type":"response","content":"I can see your Stripe configuration. Let me help you set up the integration..."}
{"id":"018d5f2c-1000-7000-8000-000000000006","ts":2100,"type":"annotation","target_id":"018d5f2c-1000-7000-8000-000000000003","content":"Note: API key was automatically redacted for security","author":"system","style":"warning","created_at":"2025-01-31T14:01:00Z"}
```

### 12.3 Session with Subagent

```json
{"id":"018d5f2c-2000-7000-8000-000000000000","ts":0,"type":"session","version":"1.0","agent":"claude-code","recorded_at":"2025-01-31T16:00:00Z","title":"Code Review with Security Analysis"}
{"id":"018d5f2c-2000-7000-8000-000000000001","ts":0,"type":"prompt","content":"Review my authentication module for security issues"}
{"id":"018d5f2c-2000-7000-8000-000000000002","ts":1000,"type":"thinking","content":"This requires detailed security analysis. I'll delegate to a specialized security reviewer."}
{"id":"018d5f2c-2000-7000-8000-000000000003","ts":2000,"type":"subagent_start","agent":"security-reviewer","context":"Delegating OWASP Top 10 analysis"}
{"id":"018d5f2c-2000-7000-8000-000000000004","ts":2100,"type":"prompt","content":"Analyze the following code for OWASP Top 10 vulnerabilities","subagent_id":"018d5f2c-2000-7000-8000-000000000003"}
{"id":"018d5f2c-2000-7000-8000-000000000005","ts":2500,"type":"tool_call","tool":"read_file","input":{"path":"auth.py"},"subagent_id":"018d5f2c-2000-7000-8000-000000000003"}
{"id":"018d5f2c-2000-7000-8000-000000000006","ts":2600,"type":"tool_result","call_id":"018d5f2c-2000-7000-8000-000000000005","output":"...code...","subagent_id":"018d5f2c-2000-7000-8000-000000000003"}
{"id":"018d5f2c-2000-7000-8000-000000000007","ts":8000,"type":"response","content":"Security analysis complete. Found 2 issues:\n1. SQL Injection (A03:2021)\n2. Broken Authentication (A07:2021)","subagent_id":"018d5f2c-2000-7000-8000-000000000003"}
{"id":"018d5f2c-2000-7000-8000-000000000008","ts":8100,"type":"subagent_end","start_id":"018d5f2c-2000-7000-8000-000000000003","summary":"Found 2 OWASP Top 10 vulnerabilities","status":"completed"}
{"id":"018d5f2c-2000-7000-8000-000000000009","ts":10000,"type":"response","content":"The security analysis found two critical issues that need to be addressed..."}
```

### 12.4 Session with Error Recovery

```json
{"id":"018d5f2c-3000-7000-8000-000000000000","ts":0,"type":"session","version":"1.0","agent":"claude-code","recorded_at":"2025-01-31T18:00:00Z","title":"Deployment Script","ended":"completed"}
{"id":"018d5f2c-3000-7000-8000-000000000001","ts":0,"type":"prompt","content":"Deploy the application to production"}
{"id":"018d5f2c-3000-7000-8000-000000000002","ts":1000,"type":"tool_call","tool":"bash","input":{"command":"./deploy.sh"}}
{"id":"018d5f2c-3000-7000-8000-000000000003","ts":5000,"type":"error","code":"rate_limit","message":"API rate limit exceeded","recoverable":true,"details":{"retry_after_ms":30000}}
{"id":"018d5f2c-3000-7000-8000-000000000004","ts":35000,"type":"tool_call","tool":"bash","input":{"command":"./deploy.sh"}}
{"id":"018d5f2c-3000-7000-8000-000000000005","ts":45000,"type":"tool_result","call_id":"018d5f2c-3000-7000-8000-000000000004","output":"Deployment successful!"}
{"id":"018d5f2c-3000-7000-8000-000000000006","ts":46000,"type":"response","content":"Deployment completed successfully after recovering from a rate limit."}
```

---

## 13. References

### 13.1 Normative References

- [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) — Key words for use in RFCs to Indicate Requirement Levels
- [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) — The JavaScript Object Notation (JSON) Data Interchange Format
- [RFC 4122](https://www.rfc-editor.org/rfc/rfc4122) — A Universally Unique IDentifier (UUID) URN Namespace
- [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562) — Universally Unique IDentifiers (UUIDs) — includes UUID v7
- [ISO 8601](https://www.iso.org/iso-8601-date-and-time-format.html) — Date and time format
- [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648) — Base64 encoding

### 13.2 Informative References

- [JSON Lines](https://jsonlines.org/) — Informal JSONL specification
- [Newline Delimited JSON](http://ndjson.org/) — NDJSON specification

---

## Appendix A: JSON Schema (Informative)

A JSON Schema for validating Spool entries is available at:

```
https://spool.dev/schema/v1.0/entry.json
```

This schema is informative and may not capture all constraints in this specification.

---

## Appendix B: Change Log

### Version 1.0 (2025-01-31)

- Initial release
