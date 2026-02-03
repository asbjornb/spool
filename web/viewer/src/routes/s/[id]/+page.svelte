<script lang="ts">
	import type { PageData } from './$types';
	import PlayerView from '$lib/components/PlayerView.svelte';
	import { formatDuration } from '$lib/parser';

	let { data }: { data: PageData } = $props();

	function copyUrl() {
		navigator.clipboard.writeText(window.location.href);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}

	let copied = $state(false);
</script>

<svelte:head>
	<title>{data.spool.session.title ?? 'Session'} - unspool.dev</title>
	<meta
		name="description"
		content="AI agent session: {data.spool.session.title ?? 'Untitled'} - {data.spool.session
			.agent}"
	/>
</svelte:head>

<div class="viewer-container">
	<nav class="viewer-nav">
		<a href="/" class="nav-back">&larr; unspool.dev</a>
		<div class="nav-actions">
			<button class="share-btn" onclick={copyUrl} title="Copy link">
				{copied ? '✓ Copied!' : '🔗 Share'}
			</button>
		</div>
	</nav>

	<div class="session-info">
		{#if data.metadata.user}
			<span class="session-author">by {data.metadata.user.username}</span>
		{/if}
		{#if data.metadata.expires_at}
			<span class="session-expires" title="Anonymous uploads expire after 14 days">
				Expires {new Date(data.metadata.expires_at).toLocaleDateString()}
			</span>
		{/if}
	</div>

	<PlayerView spool={data.spool} />
</div>

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
		color: var(--text-muted);
		text-decoration: none;
		font-size: 0.9rem;
	}

	.nav-back:hover {
		color: var(--text);
	}

	.nav-actions {
		display: flex;
		gap: 0.5rem;
	}

	.share-btn {
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		color: var(--text);
		padding: 0.4rem 0.8rem;
		border-radius: 4px;
		cursor: pointer;
		font-size: 0.85rem;
	}

	.share-btn:hover {
		background: var(--border);
	}

	.session-info {
		display: flex;
		gap: 1rem;
		padding: 0.5rem 1rem;
		font-size: 0.85rem;
		color: var(--text-muted);
		background: var(--bg);
		border-bottom: 1px solid var(--border);
	}

	.session-expires {
		opacity: 0.7;
	}
</style>
