<script lang="ts">
	import type { SpoolFile } from '$lib/types';
	import { parseSpool } from '$lib/parser';
	import { uploadSession, type UploadResponse } from '$lib/api';
	import PlayerView from '$lib/components/PlayerView.svelte';

	let spool = $state<SpoolFile | null>(null);
	let rawContent = $state<string | null>(null);
	let error = $state<string | null>(null);
	let dragOver = $state(false);

	// Upload state
	let uploading = $state(false);
	let uploadResult = $state<UploadResponse | null>(null);
	let copied = $state(false);

	async function loadFile(file: File) {
		error = null;
		uploadResult = null;
		try {
			const text = await file.text();
			rawContent = text;
			spool = parseSpool(text);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to parse file';
			spool = null;
			rawContent = null;
		}
	}

	async function loadUrl(url: string) {
		error = null;
		uploadResult = null;
		try {
			const res = await fetch(url);
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const text = await res.text();
			rawContent = text;
			spool = parseSpool(text);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load file';
			spool = null;
			rawContent = null;
		}
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

	async function handleUpload() {
		if (!rawContent) return;

		uploading = true;
		error = null;
		try {
			uploadResult = await uploadSession(rawContent);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Upload failed';
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

	function goBack() {
		spool = null;
		rawContent = null;
		uploadResult = null;
		error = null;
	}
</script>

<svelte:head>
	<title>{spool ? (spool.session.title ?? 'Session') + ' - unspool.dev' : 'unspool.dev'}</title>
</svelte:head>

{#if spool}
	<div class="viewer-container">
		<nav class="viewer-nav">
			<button class="nav-back" onclick={goBack}>&larr; Load another</button>
			<div class="nav-actions">
				{#if uploadResult}
					<span class="upload-success">Published!</span>
					<button class="share-btn" onclick={copyUrl}>
						{copied ? '✓ Copied!' : '🔗 Copy link'}
					</button>
					<a href={uploadResult.url} class="view-btn" target="_blank">View →</a>
				{:else}
					<button class="publish-btn" onclick={handleUpload} disabled={uploading}>
						{uploading ? 'Publishing...' : '📤 Publish'}
					</button>
				{/if}
			</div>
		</nav>

		{#if uploadResult}
			<div class="upload-banner">
				<span>Shared at: </span>
				<a href={uploadResult.url}>{uploadResult.url}</a>
				{#if uploadResult.expires_at}
					<span class="expires">(expires {new Date(uploadResult.expires_at).toLocaleDateString()})</span>
				{/if}
			</div>
		{/if}

		{#if error}
			<div class="error-banner">{error}</div>
		{/if}

		<PlayerView {spool} />
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
					<span>Open .spool file</span>
				</label>

				<div class="drop-hint">or drag and drop a .spool file here</div>

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
					<a href="https://github.com/asbjornb/spool" target="_blank">Learn more →</a>
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

	.nav-actions {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.publish-btn,
	.share-btn,
	.view-btn {
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		color: var(--text);
		padding: 0.4rem 0.8rem;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.85rem;
		text-decoration: none;
	}

	.publish-btn:hover,
	.share-btn:hover,
	.view-btn:hover {
		background: var(--border);
	}

	.publish-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.publish-btn {
		background: var(--accent);
		border-color: var(--accent);
		color: #000;
	}

	.publish-btn:hover:not(:disabled) {
		opacity: 0.9;
	}

	.upload-success {
		color: var(--green);
		font-size: 0.85rem;
	}

	.upload-banner {
		padding: 0.5rem 1rem;
		background: var(--bg-elevated);
		border-bottom: 1px solid var(--border);
		font-size: 0.85rem;
	}

	.upload-banner a {
		color: var(--accent);
	}

	.upload-banner .expires {
		color: var(--text-muted);
		margin-left: 0.5rem;
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
</style>
