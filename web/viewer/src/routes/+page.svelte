<script lang="ts">
	import type { SpoolFile } from '$lib/types';
	import { parseSpool } from '$lib/parser';
	import PlayerView from '$lib/components/PlayerView.svelte';

	let spool = $state<SpoolFile | null>(null);
	let error = $state<string | null>(null);
	let dragOver = $state(false);

	async function loadFile(file: File) {
		error = null;
		try {
			const text = await file.text();
			spool = parseSpool(text);
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
			spool = parseSpool(text);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load file';
			spool = null;
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
</script>

<svelte:head>
	<title>{spool ? (spool.session.title ?? 'Session') + ' - Spool Viewer' : 'Spool Viewer'}</title>
</svelte:head>

{#if spool}
	<div class="viewer-container">
		<nav class="viewer-nav">
			<button class="nav-back" onclick={() => (spool = null)}>&larr; Load another</button>
			<span class="nav-title">Spool Viewer</span>
		</nav>
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
			<h1>Spool Viewer</h1>
			<p class="subtitle">Browse and replay AI agent sessions</p>

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
		</div>
	</div>
{/if}
