<script lang="ts">
	import type { AnnotationEntry, AnnotationStyle, Entry, SpoolFile } from '$lib/types';
	import { Player, SPEED_PRESETS } from '$lib/player.svelte';
	import { formatDuration, formatTimestamp, entryTypeLabel } from '$lib/parser';
	import { spoolToJsonl, trimSpool, addAnnotation, removeAnnotation, detectSecretsInSpool, redactSpool } from '$lib/spool-export';
	import { uploadSession, type UploadResponse, type Visibility } from '$lib/api';
	import { getReplacement, type DetectedSecret, type SecretCategory } from '$lib/redaction';
	import EntryComponent from './Entry.svelte';
	import Timeline from './Timeline.svelte';

	let { spool: initialSpool, sourceFileName = 'session' }: { spool: SpoolFile; sourceFileName?: string } = $props();

	// Working copy that editing modifies
	let spool = $state<SpoolFile>(initialSpool);

	const player = new Player();

	$effect(() => {
		player.load(spool);
	});

	// ---- Editor state ----
	let mode = $state<'view' | 'trim' | 'annotate' | 'redact'>('view');
	let trimStart = $state<number | null>(null);
	let trimEnd = $state<number | null>(null);
	let annotateTargetId = $state<string | null>(null);
	let annotateText = $state('');
	let annotateStyle = $state<AnnotationStyle>('comment');

	// Redaction state
	type SecretResult = { entryIndex: number; entryId: string; secrets: DetectedSecret[] };
	let detectedSecrets = $state<SecretResult[]>([]);
	let redactSelectedIdx = $state(0);

	// Upload state
	let uploading = $state(false);
	let uploadResult = $state<UploadResponse | null>(null);
	let uploadError = $state<string | null>(null);
	let copied = $state(false);
	let statusMessage = $state<string | null>(null);
	let visibility = $state<Visibility>('unlisted');

	// Build annotation lookup: target_id -> annotations
	const annotationMap = $derived.by(() => {
		const map = new Map<string, AnnotationEntry[]>();
		for (const entry of player.visibleEntries) {
			if (entry.type === 'annotation') {
				const list = map.get(entry.target_id) ?? [];
				list.push(entry);
				map.set(entry.target_id, list);
			}
		}
		return map;
	});

	const displayEntries = $derived(
		player.visibleEntries.filter(
			(e) => e.type !== 'session' && e.type !== 'annotation' && e.type !== 'redaction_marker'
		)
	);

	const allSecrets = $derived(detectedSecrets.flatMap((r) => r.secrets));
	const confirmedCount = $derived(allSecrets.filter((s) => s.confirmed).length);

	// ---- Trim helpers ----
	function markTrimStart() {
		trimStart = player.elapsed;
		// Find the entry ts at the current position
		const entry = spool.entries[player.currentIndex];
		if (entry) trimStart = entry.ts;
		showStatus(`Trim start: ${formatTimestamp(trimStart)}`);
	}

	function markTrimEnd() {
		trimEnd = player.elapsed;
		const entry = spool.entries[player.currentIndex];
		if (entry) trimEnd = entry.ts;
		showStatus(`Trim end: ${formatTimestamp(trimEnd)}`);
	}

	function applyTrim() {
		if (trimStart == null || trimEnd == null || trimStart >= trimEnd) {
			showStatus('Invalid trim range');
			return;
		}
		spool = trimSpool(spool, trimStart, trimEnd);
		trimStart = null;
		trimEnd = null;
		mode = 'view';
		showStatus('Trimmed');
	}

	// ---- Annotation helpers ----
	function startAnnotation(entryId: string) {
		annotateTargetId = entryId;
		annotateText = '';
		annotateStyle = 'comment';
		mode = 'annotate';
	}

	function submitAnnotation() {
		if (!annotateTargetId || !annotateText.trim()) return;
		const target = spool.entries.find((e) => e.id === annotateTargetId);
		if (!target) return;
		spool = addAnnotation(spool, annotateTargetId, target.ts, annotateText.trim(), annotateStyle);
		annotateTargetId = null;
		annotateText = '';
		mode = 'view';
		showStatus('Annotation added');
	}

	function cancelAnnotation() {
		annotateTargetId = null;
		annotateText = '';
		mode = 'view';
	}

	// ---- Redaction helpers ----
	function startRedaction() {
		detectedSecrets = detectSecretsInSpool(spool);
		if (detectedSecrets.length === 0 || allSecrets.length === 0) {
			showStatus('No secrets detected');
			return;
		}
		redactSelectedIdx = 0;
		mode = 'redact';
	}

	function toggleSecret(idx: number) {
		// Toggle the confirmed state of a specific secret
		let flatIdx = 0;
		for (const result of detectedSecrets) {
			for (const secret of result.secrets) {
				if (flatIdx === idx) {
					secret.confirmed = !secret.confirmed;
					// Force reactivity
					detectedSecrets = [...detectedSecrets];
					return;
				}
				flatIdx++;
			}
		}
	}

	function confirmAllSecrets() {
		for (const r of detectedSecrets) {
			for (const s of r.secrets) s.confirmed = true;
		}
		detectedSecrets = [...detectedSecrets];
	}

	function dismissAllSecrets() {
		for (const r of detectedSecrets) {
			for (const s of r.secrets) s.confirmed = false;
		}
		detectedSecrets = [...detectedSecrets];
	}

	function applyRedaction() {
		spool = redactSpool(spool, detectedSecrets);
		detectedSecrets = [];
		mode = 'view';
		showStatus(`Redacted ${confirmedCount} secret(s)`);
	}

	// ---- Upload ----
	async function handleUpload() {
		uploading = true;
		uploadError = null;
		try {
			const content = spoolToJsonl(spool);
			uploadResult = await uploadSession(content, visibility);
		} catch (e) {
			uploadError = e instanceof Error ? e.message : 'Upload failed';
		} finally {
			uploading = false;
		}
	}

	function copyUrl() {
		if (!uploadResult) return;
		navigator.clipboard.writeText(uploadResult.url);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}

	function downloadSpool() {
		const content = spoolToJsonl(spool);
		const blob = new Blob([content], { type: 'application/jsonl' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `${sourceFileName.replace(/\.(jsonl|spool)$/i, '')}.spool`;
		a.click();
		URL.revokeObjectURL(url);
	}

	function showStatus(msg: string) {
		statusMessage = msg;
		setTimeout(() => {
			if (statusMessage === msg) statusMessage = null;
		}, 3000);
	}

	// ---- Keyboard shortcuts ----
	function handleKeydown(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
		if (mode === 'annotate' || mode === 'redact') return;

		switch (e.key) {
			case ' ':
				e.preventDefault();
				player.toggle();
				break;
			case 'l':
			case 'ArrowRight':
				player.stepForward();
				break;
			case 'h':
			case 'ArrowLeft':
				player.stepBackward();
				break;
			case 'g':
				player.jumpToStart();
				break;
			case 'G':
				player.jumpToEnd();
				break;
			case '+':
			case '=':
				player.cycleSpeed();
				break;
			case '[':
				markTrimStart();
				break;
			case ']':
				markTrimEnd();
				break;
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="editor">
	<header class="session-header">
		<h2>{spool.session.title ?? 'Untitled Session'}</h2>
		<div class="session-meta">
			<span>{spool.session.agent}</span>
			{#if spool.session.agent_version}
				<span>v{spool.session.agent_version}</span>
			{/if}
			{#if spool.session.duration_ms}
				<span>{formatDuration(spool.session.duration_ms)}</span>
			{/if}
			{#if spool.session.entry_count}
				<span>{spool.session.entry_count} entries</span>
			{/if}
		</div>
	</header>

	<!-- Playback controls -->
	<div class="controls">
		<button class="control-btn" onclick={() => player.toggle()} title="Play/Pause (Space)">
			{player.state === 'playing' ? '\u23F8' : '\u25B6'}
		</button>
		<button class="control-btn" onclick={() => player.stepBackward()} title="Step Back (h)">
			\u23EE
		</button>
		<button class="control-btn" onclick={() => player.stepForward()} title="Step Forward (l)">
			\u23ED
		</button>
		<button class="control-btn" onclick={() => player.jumpToStart()} title="Jump to Start (g)">
			\u23EA
		</button>
		<button class="control-btn" onclick={() => player.jumpToEnd()} title="Jump to End (G)">
			\u23E9
		</button>
		<button class="control-btn speed-btn" onclick={() => player.cycleSpeed()} title="Speed (+)">
			{player.speed}x
		</button>
		<span class="control-info">
			{player.currentIndex + 1} / {spool.entries.length}
		</span>
	</div>

	<Timeline
		progress={player.progress}
		elapsed={player.elapsed}
		total={player.totalDuration}
		onseek={(p) => player.seek(p)}
	/>

	<!-- Editing toolbar -->
	<div class="editor-toolbar">
		<div class="toolbar-left">
			<button class="toolbar-btn" class:active={mode === 'trim'} onclick={() => { mode = mode === 'trim' ? 'view' : 'trim'; }} title="Trim mode: [/] to set range">
				Trim
			</button>
			<button class="toolbar-btn" onclick={startRedaction} title="Detect and redact secrets">
				Redact
			</button>
		</div>
		<div class="toolbar-right">
			<button class="toolbar-btn" onclick={downloadSpool} title="Download as .spool file">
				Download .spool
			</button>
			{#if uploadResult}
				<span class="upload-success">Published ({visibility})</span>
				<button class="toolbar-btn accent" onclick={copyUrl}>
					{copied ? 'Copied!' : 'Copy link'}
				</button>
				<a href={uploadResult.url} class="toolbar-btn accent" target="_blank">View</a>
			{:else}
				<select class="visibility-select" bind:value={visibility} title="Visibility">
					<option value="unlisted">Unlisted</option>
					<option value="public">Public</option>
				</select>
				<button class="toolbar-btn accent" onclick={handleUpload} disabled={uploading}>
					{uploading ? 'Publishing...' : 'Publish & Share'}
				</button>
			{/if}
		</div>
	</div>

	<!-- Trim controls -->
	{#if mode === 'trim'}
		<div class="trim-bar">
			<span class="trim-label">
				Trim:
				{#if trimStart != null}
					{formatTimestamp(trimStart)}
				{:else}
					<button class="inline-btn" onclick={markTrimStart}>Set start [</button>
				{/if}
				&mdash;
				{#if trimEnd != null}
					{formatTimestamp(trimEnd)}
				{:else}
					<button class="inline-btn" onclick={markTrimEnd}>Set end ]</button>
				{/if}
			</span>
			{#if trimStart != null && trimEnd != null && trimStart < trimEnd}
				<button class="toolbar-btn accent" onclick={applyTrim}>Apply trim</button>
			{/if}
			<button class="toolbar-btn" onclick={() => { trimStart = null; trimEnd = null; }}>Reset</button>
			<button class="toolbar-btn" onclick={() => { mode = 'view'; }}>Cancel</button>
		</div>
	{/if}

	<!-- Status / error messages -->
	{#if statusMessage}
		<div class="status-bar">{statusMessage}</div>
	{/if}
	{#if uploadError}
		<div class="error-banner">{uploadError}</div>
	{/if}

	<!-- Annotation modal -->
	{#if mode === 'annotate'}
		<div class="modal-overlay" role="dialog">
			<div class="modal">
				<h3>Add Annotation</h3>
				<textarea bind:value={annotateText} placeholder="Enter note..." rows="3"></textarea>
				<div class="annotation-styles">
					{#each ['comment', 'highlight', 'warning', 'success', 'pin'] as s}
						<label class="style-option">
							<input type="radio" bind:group={annotateStyle} value={s} />
							{s}
						</label>
					{/each}
				</div>
				<div class="modal-actions">
					<button class="toolbar-btn" onclick={cancelAnnotation}>Cancel</button>
					<button class="toolbar-btn accent" onclick={submitAnnotation} disabled={!annotateText.trim()}>Add</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- Redaction review modal -->
	{#if mode === 'redact'}
		<div class="modal-overlay" role="dialog">
			<div class="modal modal-wide">
				<h3>Review Detected Secrets</h3>
				<p class="modal-hint">Toggle each secret to include/exclude from redaction.</p>

				<div class="secret-list">
					{#each allSecrets as secret, i}
						<button
							class="secret-item"
							class:confirmed={secret.confirmed}
							class:selected={i === redactSelectedIdx}
							onclick={() => toggleSecret(i)}
						>
							<span class="secret-check">{secret.confirmed ? '[x]' : '[ ]'}</span>
							<span class="secret-category">{secret.category}</span>
							<span class="secret-matched">{secret.matched.length > 40 ? secret.matched.slice(0, 40) + '...' : secret.matched}</span>
						</button>
					{/each}
				</div>

				<div class="secret-summary">
					{confirmedCount} / {allSecrets.length} secrets will be redacted
				</div>

				<div class="modal-actions">
					<button class="toolbar-btn" onclick={confirmAllSecrets}>Accept all</button>
					<button class="toolbar-btn" onclick={dismissAllSecrets}>Dismiss all</button>
					<button class="toolbar-btn" onclick={() => { mode = 'view'; detectedSecrets = []; }}>Cancel</button>
					<button class="toolbar-btn accent" onclick={applyRedaction}>Apply redactions</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- Entry list -->
	<div class="entry-list">
		{#each displayEntries as entry (entry.id)}
			<div class="entry-row">
				<EntryComponent {entry} annotations={annotationMap.get(entry.id) ?? []} />
				{#if mode !== 'annotate' && mode !== 'redact'}
					<button
						class="annotate-btn"
						onclick={() => startAnnotation(entry.id)}
						title="Add annotation"
					>+</button>
				{/if}
			</div>
		{/each}
	</div>
</div>

<style>
	.editor {
		padding-bottom: 2rem;
	}

	.editor-toolbar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.5rem 0;
		gap: 0.5rem;
		flex-wrap: wrap;
		border-bottom: 1px solid var(--border);
		margin-bottom: 0.5rem;
	}

	.toolbar-left,
	.toolbar-right {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.toolbar-btn {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		color: var(--text);
		padding: 0.35rem 0.7rem;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.8rem;
		text-decoration: none;
		display: inline-flex;
		align-items: center;
	}

	.toolbar-btn:hover {
		background: var(--bg-elevated);
	}

	.toolbar-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.toolbar-btn.active {
		border-color: var(--accent);
		color: var(--accent);
	}

	.toolbar-btn.accent {
		background: var(--accent);
		border-color: var(--accent);
		color: #000;
	}

	.toolbar-btn.accent:hover:not(:disabled) {
		opacity: 0.85;
	}

	.upload-success {
		color: var(--green);
		font-size: 0.8rem;
	}

	.visibility-select {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		color: var(--text);
		padding: 0.35rem 0.5rem;
		border-radius: 4px;
		font-size: 0.8rem;
		cursor: pointer;
	}

	.visibility-select:hover {
		background: var(--bg-elevated);
	}

	.trim-bar {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 4px;
		margin-bottom: 0.5rem;
		font-size: 0.85rem;
	}

	.trim-label {
		color: var(--text-muted);
	}

	.inline-btn {
		background: none;
		border: none;
		color: var(--accent);
		cursor: pointer;
		padding: 0;
		font-size: 0.85rem;
		text-decoration: underline;
	}

	.status-bar {
		padding: 0.4rem 0.75rem;
		background: rgba(88, 166, 255, 0.1);
		border: 1px solid var(--accent);
		border-radius: 4px;
		color: var(--accent);
		font-size: 0.8rem;
		margin-bottom: 0.5rem;
	}

	.error-banner {
		padding: 0.4rem 0.75rem;
		background: rgba(248, 81, 73, 0.1);
		border: 1px solid var(--red);
		border-radius: 4px;
		color: var(--red);
		font-size: 0.8rem;
		margin-bottom: 0.5rem;
	}

	.entry-row {
		position: relative;
	}

	.annotate-btn {
		position: absolute;
		top: 0.4rem;
		right: -1.5rem;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		color: var(--text-muted);
		width: 1.2rem;
		height: 1.2rem;
		border-radius: 50%;
		cursor: pointer;
		font-size: 0.7rem;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: opacity 0.15s;
	}

	.entry-row:hover .annotate-btn {
		opacity: 1;
	}

	.annotate-btn:hover {
		background: var(--accent);
		border-color: var(--accent);
		color: #000;
	}

	/* Modal styles */
	.modal-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.modal {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 8px;
		padding: 1.25rem;
		max-width: 400px;
		width: 90%;
	}

	.modal-wide {
		max-width: 600px;
	}

	.modal h3 {
		margin-bottom: 0.75rem;
		font-size: 1rem;
	}

	.modal-hint {
		color: var(--text-muted);
		font-size: 0.8rem;
		margin-bottom: 0.75rem;
	}

	.modal textarea {
		width: 100%;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text);
		padding: 0.5rem;
		font-size: 0.85rem;
		resize: vertical;
		margin-bottom: 0.5rem;
	}

	.annotation-styles {
		display: flex;
		gap: 0.75rem;
		margin-bottom: 0.75rem;
		flex-wrap: wrap;
	}

	.style-option {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		cursor: pointer;
	}

	.modal-actions {
		display: flex;
		gap: 0.5rem;
		justify-content: flex-end;
	}

	.secret-list {
		max-height: 300px;
		overflow-y: auto;
		margin-bottom: 0.75rem;
		border: 1px solid var(--border);
		border-radius: 4px;
	}

	.secret-item {
		display: flex;
		gap: 0.5rem;
		padding: 0.4rem 0.6rem;
		border: none;
		background: none;
		color: var(--text);
		cursor: pointer;
		font-size: 0.8rem;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--border);
	}

	.secret-item:last-child {
		border-bottom: none;
	}

	.secret-item:hover {
		background: var(--bg-elevated);
	}

	.secret-check {
		font-family: var(--font-mono);
		flex-shrink: 0;
	}

	.secret-item.confirmed .secret-check {
		color: var(--green);
	}

	.secret-category {
		color: var(--orange);
		flex-shrink: 0;
		min-width: 6rem;
	}

	.secret-matched {
		color: var(--red);
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.secret-summary {
		color: var(--text-muted);
		font-size: 0.8rem;
		margin-bottom: 0.75rem;
	}
</style>
