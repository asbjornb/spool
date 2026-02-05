<script lang="ts">
	import type { SpoolFile } from '$lib/types';
	import type { PageData } from './$types';
	import { parseSpool, formatDuration } from '$lib/parser';
	import { isClaudeCodeLog, convertClaudeCodeLog } from '$lib/adapters/claude-code';
	import { isCodexLog, convertCodexLog } from '$lib/adapters/codex';
	import PlayerView from '$lib/components/PlayerView.svelte';
	import EditorView from '$lib/components/EditorView.svelte';

	let { data }: { data: PageData } = $props();

	let spool = $state<SpoolFile | null>(null);
	let error = $state<string | null>(null);
	let dragOver = $state(false);
	let fileName = $state<string>('session');

	/** Whether we loaded a raw log (→ EditorView) or a .spool file (→ PlayerView) */
	let isRawLog = $state(false);

	function agentLabel(agent: string | null): string {
		if (!agent) return 'Unknown';
		switch (agent) {
			case 'claude-code': return 'Claude Code';
			case 'codex': return 'Codex';
			default: return agent;
		}
	}

	function timeAgo(dateStr: string): string {
		const date = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / 60000);
		if (diffMins < 1) return 'just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		const diffHours = Math.floor(diffMins / 60);
		if (diffHours < 24) return `${diffHours}h ago`;
		const diffDays = Math.floor(diffHours / 24);
		if (diffDays < 30) return `${diffDays}d ago`;
		return date.toLocaleDateString();
	}

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

		// Check if it's already a .spool file (first entry is type "session" with version)
		try {
			const firstParsed = JSON.parse(firstLine);
			if (firstParsed.type === 'session' && firstParsed.version) {
				spool = parseSpool(text);
				isRawLog = false;
				return;
			}
		} catch {
			// Not valid JSON, fall through
		}

		// Check if it's a raw Claude Code log
		if (isClaudeCodeLog(firstLine)) {
			spool = convertClaudeCodeLog(text);
			isRawLog = true;
			return;
		}

		// Check if it's a raw Codex CLI log
		if (isCodexLog(firstLine)) {
			spool = convertCodexLog(text);
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
		class:has-sidebar={data.recentSessions.length > 0}
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
					<span>Open file</span>
				</label>

				<div class="drop-hint">or drop a .spool or .jsonl file here</div>

				<div class="demos">
					<button onclick={() => loadDemo('demo.spool')}>Try a demo</button>
					<button onclick={() => loadDemo('debugging-demo.spool')}>Debugging demo</button>
				</div>

				<div class="explore-link">
					<a href="/explore">Browse all public sessions &rarr;</a>
				</div>
			</div>

			{#if error}
				<div class="error-message">{error}</div>
			{/if}

			<div class="about">
				<p>
					<strong>Spool</strong> is an open format for recording AI agent sessions.
					Supports Claude Code and Codex logs. All processing runs locally.
					<a href="https://github.com/asbjornb/spool" target="_blank">Learn more &rarr;</a>
				</p>
			</div>
		</div>

		{#if data.recentSessions.length > 0}
			<aside class="recent-sidebar">
				<h2 class="sidebar-title">Recent sessions</h2>
				<div class="sidebar-list">
					{#each data.recentSessions as session}
						<a href="/s/{session.id}" class="sidebar-card">
							<div class="sidebar-card-title">{session.title ?? 'Untitled Session'}</div>
							<div class="sidebar-card-meta">
								<span class="sidebar-agent">{agentLabel(session.agent)}</span>
								{#if session.duration_ms}
									<span>{formatDuration(session.duration_ms)}</span>
								{/if}
							</div>
							<div class="sidebar-card-footer">
								{#if session.user}
									<span>by {session.user.username}</span>
								{:else}
									<span>anonymous</span>
								{/if}
								<span>{timeAgo(session.created_at)}</span>
							</div>
						</a>
					{/each}
				</div>
				<a href="/explore" class="sidebar-more">View all &rarr;</a>
			</aside>
		{/if}
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

	.explore-link {
		margin-top: 0.5rem;
	}

	.explore-link a {
		color: var(--accent);
		text-decoration: none;
		font-size: 0.9rem;
	}

	.explore-link a:hover {
		text-decoration: underline;
	}

	/* Two-column landing layout when sidebar is present */
	.landing.has-sidebar {
		flex-direction: row;
		align-items: stretch;
		justify-content: center;
		gap: 3rem;
		padding: 2rem 3rem;
	}

	.landing.has-sidebar .landing-content {
		display: flex;
		flex-direction: column;
		justify-content: center;
		flex: 0 1 480px;
	}

	.recent-sidebar {
		flex: 0 1 320px;
		display: flex;
		flex-direction: column;
		max-height: 100vh;
		padding: 1.5rem 0;
	}

	.sidebar-title {
		font-size: 0.9rem;
		font-weight: 600;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-bottom: 0.75rem;
	}

	.sidebar-list {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		overflow-y: auto;
		flex: 1;
		min-height: 0;
		/* Subtle scrollbar */
		scrollbar-width: thin;
		scrollbar-color: var(--border) transparent;
	}

	.sidebar-card {
		display: block;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 0.6rem 0.75rem;
		text-decoration: none;
		color: var(--text);
		transition: border-color 0.15s;
		flex-shrink: 0;
	}

	.sidebar-card:hover {
		border-color: var(--accent);
	}

	.sidebar-card-title {
		font-size: 0.85rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		margin-bottom: 0.2rem;
	}

	.sidebar-card-meta {
		display: flex;
		gap: 0.5rem;
		font-size: 0.75rem;
		color: var(--text-muted);
		margin-bottom: 0.15rem;
	}

	.sidebar-agent {
		color: var(--accent);
	}

	.sidebar-card-footer {
		display: flex;
		justify-content: space-between;
		font-size: 0.7rem;
		color: var(--text-muted);
		opacity: 0.8;
	}

	.sidebar-more {
		display: block;
		text-align: center;
		color: var(--accent);
		text-decoration: none;
		font-size: 0.85rem;
		padding-top: 0.75rem;
		margin-top: 0.5rem;
		border-top: 1px solid var(--border);
	}

	.sidebar-more:hover {
		text-decoration: underline;
	}

	/* Stack on narrow screens */
	@media (max-width: 768px) {
		.landing.has-sidebar {
			flex-direction: column;
			align-items: center;
			padding: 2rem;
			gap: 2rem;
		}

		.landing.has-sidebar .landing-content {
			flex: none;
			width: 100%;
			max-width: 480px;
		}

		.recent-sidebar {
			flex: none;
			width: 100%;
			max-width: 480px;
			max-height: 400px;
		}
	}
</style>
