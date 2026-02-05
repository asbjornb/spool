<script lang="ts">
	import type { SpoolFile } from '$lib/types';
	import { parseSpool } from '$lib/parser';
	import { isClaudeCodeLog, convertClaudeCodeLog } from '$lib/adapters/claude-code';
	import PlayerView from '$lib/components/PlayerView.svelte';
	import EditorView from '$lib/components/EditorView.svelte';

	let spool = $state<SpoolFile | null>(null);
	let error = $state<string | null>(null);
	let dragOver = $state(false);
	let fileName = $state<string>('session');

	/** Whether we loaded a raw log (→ EditorView) or a .spool file (→ PlayerView) */
	let isRawLog = $state(false);

	async function loadFile(file: File) {
		error = null;
		fileName = file.name;
		try {
			const text = await file.text();
			parseAndLoad(text, file.name);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to parse file';
			spool = null;
		}
	}

	async function loadUrl(url: string) {
		error = null;
		try {
			const res = await fetch(url);
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const text = await res.text();
			const name = url.split('/').pop() || 'session';
			fileName = name;
			parseAndLoad(text, name);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load file';
			spool = null;
		}
	}

	function parseAndLoad(text: string, name: string) {
		// Try parsing as .spool first
		const firstLine = text.split('\n').find((l) => l.trim().length > 0) ?? '';

		// Check if it's a raw Claude Code log
		if (isClaudeCodeLog(firstLine)) {
			try {
				const firstParsed = JSON.parse(firstLine);
				// If the first entry is type "session" with a version field, it's already .spool
				if (firstParsed.type === 'session' && firstParsed.version) {
					spool = parseSpool(text);
					isRawLog = false;
					return;
				}
			} catch {
				// Not valid JSON, fall through
			}

			// It's a raw log — convert client-side
			spool = convertClaudeCodeLog(text);
			isRawLog = true;
			return;
		}

		// Default: try as .spool
		spool = parseSpool(text);
		isRawLog = false;
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		const file = e.dataTransfer?.files[0];
		if (file) loadFile(file);
	}

	function handleFileInput(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (file) loadFile(file);
	}

	async function loadDemo(name: string) {
		await loadUrl(`/${name}`);
	}

	function goBack() {
		spool = null;
		error = null;
		isRawLog = false;
	}
</script>

<svelte:head>
	<title>{spool ? (spool.session.title ?? 'Session') + ' - unspool.dev' : 'unspool.dev'}</title>
</svelte:head>

{#if spool}
	<div class="viewer-container">
		<nav class="viewer-nav">
			<button class="nav-back" onclick={goBack}>&larr; Load another</button>
			{#if isRawLog}
				<span class="nav-label">Editing: {fileName}</span>
			{/if}
		</nav>

		{#if error}
			<div class="error-banner">{error}</div>
		{/if}

		{#if isRawLog}
			<EditorView {spool} sourceFileName={fileName} />
		{:else}
			<PlayerView {spool} />
		{/if}
	</div>
{:else}
	<div
		class="landing"
		class:drag-over={dragOver}
		role="presentation"
		ondragover={(e) => {
			e.preventDefault();
			dragOver = true;
		}}
		ondragleave={() => (dragOver = false)}
		ondrop={handleDrop}
	>
		<div class="landing-content">
			<h1>unspool.dev</h1>
			<p class="subtitle">Share and replay AI agent sessions</p>

			<div class="load-options">
				<label class="file-picker">
					<input type="file" accept=".spool,.jsonl" onchange={handleFileInput} />
					<span>Open .spool or .jsonl file</span>
				</label>

				<div class="drop-hint">Drop a .spool or raw Claude Code .jsonl log here</div>

				<div class="format-info">
					<p>Supports:</p>
					<ul>
						<li><strong>.spool</strong> files &mdash; view and share</li>
						<li><strong>.jsonl</strong> Claude Code logs &mdash; edit, redact, trim, then share</li>
					</ul>
				</div>

				<div class="demos">
					<p>Try a demo:</p>
					<button onclick={() => loadDemo('demo.spool')}>Simple Session</button>
					<button onclick={() => loadDemo('debugging-demo.spool')}>Debugging Session</button>
				</div>
			</div>

			{#if error}
				<div class="error-message">{error}</div>
			{/if}

			<div class="about">
				<p>
					<strong>Spool</strong> is an open format for recording AI agent sessions.
					<a href="https://github.com/asbjornb/spool" target="_blank">Learn more &rarr;</a>
				</p>
				<p class="about-detail">
					All processing runs locally in your browser. Files are only uploaded when you
					choose to publish.
				</p>
			</div>
		</div>
	</div>
{/if}

<style>
	.viewer-container {
		height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.viewer-nav {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0.75rem 1rem;
		background: var(--bg-surface);
		border-bottom: 1px solid var(--border);
	}

	.nav-back {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 0.9rem;
		padding: 0;
	}

	.nav-back:hover {
		color: var(--text);
	}

	.nav-label {
		color: var(--text-muted);
		font-size: 0.85rem;
	}

	.error-banner {
		padding: 0.5rem 1rem;
		background: rgba(248, 81, 73, 0.1);
		border-bottom: 1px solid var(--red);
		color: var(--red);
		font-size: 0.85rem;
	}

	.format-info {
		text-align: left;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 0.75rem 1rem;
		font-size: 0.85rem;
		margin-top: 0.5rem;
	}

	.format-info p {
		color: var(--text-muted);
		margin-bottom: 0.25rem;
	}

	.format-info ul {
		list-style: none;
		padding: 0;
	}

	.format-info li {
		color: var(--text);
		padding: 0.15rem 0;
	}

	.about {
		margin-top: 3rem;
		padding-top: 2rem;
		border-top: 1px solid var(--border);
		color: var(--text-muted);
		font-size: 0.9rem;
	}

	.about a {
		color: var(--accent);
	}

	.about-detail {
		margin-top: 0.5rem;
		font-size: 0.8rem;
		opacity: 0.7;
	}
</style>
