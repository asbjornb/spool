<script lang="ts">
	import type { PageData } from './$types';
	import { formatDuration } from '$lib/parser';

	let { data }: { data: PageData } = $props();

	const totalPages = $derived(Math.ceil(data.total / data.limit));

	function agentLabel(agent: string | null): string {
		if (!agent) return 'Unknown';
		switch (agent) {
			case 'claude-code':
				return 'Claude Code';
			case 'codex':
				return 'Codex';
			default:
				return agent;
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
</script>

<svelte:head>
	<title>Explore - unspool.dev</title>
	<meta name="description" content="Browse public AI agent sessions on unspool.dev" />
</svelte:head>

<div class="explore-page">
	<nav class="explore-nav">
		<a href="/" class="nav-back">&larr; unspool.dev</a>
	</nav>

	<header class="explore-header">
		<h1>Explore</h1>
		<p class="explore-subtitle">Public AI agent sessions</p>
	</header>

	{#if data.sessions.length === 0}
		<div class="empty-state">
			<p>No public sessions yet.</p>
			<p class="empty-hint">
				Be the first! <a href="/">Upload a session</a> and set it to Public.
			</p>
		</div>
	{:else}
		<div class="session-grid">
			{#each data.sessions as session}
				<a href="/s/{session.id}" class="session-card">
					<div class="card-title">{session.title ?? 'Untitled Session'}</div>
					<div class="card-meta">
						<span class="card-agent">{agentLabel(session.agent)}</span>
						{#if session.duration_ms}
							<span>{formatDuration(session.duration_ms)}</span>
						{/if}
						<span>{session.entry_count} entries</span>
					</div>
					<div class="card-footer">
						{#if session.user}
							<span class="card-author">by {session.user.username}</span>
						{:else}
							<span class="card-author">anonymous</span>
						{/if}
						<span class="card-time">{timeAgo(session.created_at)}</span>
					</div>
				</a>
			{/each}
		</div>

		{#if totalPages > 1}
			<nav class="pagination">
				{#if data.page > 1}
					<a href="/explore?page={data.page - 1}" class="page-link">Previous</a>
				{/if}
				<span class="page-info">Page {data.page} of {totalPages}</span>
				{#if data.page < totalPages}
					<a href="/explore?page={data.page + 1}" class="page-link">Next</a>
				{/if}
			</nav>
		{/if}
	{/if}
</div>

<style>
	.explore-page {
		max-width: 900px;
		margin: 0 auto;
		padding: 0 1rem 4rem;
	}

	.explore-nav {
		display: flex;
		align-items: center;
		padding: 0.75rem 0;
		border-bottom: 1px solid var(--border);
		margin-bottom: 1.5rem;
	}

	.nav-back {
		color: var(--text-muted);
		text-decoration: none;
		font-size: 0.9rem;
	}

	.nav-back:hover {
		color: var(--text);
	}

	.explore-header {
		margin-bottom: 1.5rem;
	}

	.explore-header h1 {
		font-size: 1.5rem;
		font-weight: 600;
		margin-bottom: 0.25rem;
	}

	.explore-subtitle {
		color: var(--text-muted);
		font-size: 0.9rem;
	}

	.empty-state {
		text-align: center;
		padding: 3rem 1rem;
		color: var(--text-muted);
	}

	.empty-hint {
		margin-top: 0.5rem;
		font-size: 0.9rem;
	}

	.empty-hint a {
		color: var(--accent);
	}

	.session-grid {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.session-card {
		display: block;
		background: var(--bg-surface);
		border: 1px solid var(--border);
		border-radius: 6px;
		padding: 0.75rem 1rem;
		text-decoration: none;
		color: var(--text);
		transition: border-color 0.15s;
	}

	.session-card:hover {
		border-color: var(--accent);
	}

	.card-title {
		font-weight: 500;
		margin-bottom: 0.35rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-meta {
		display: flex;
		gap: 0.75rem;
		color: var(--text-muted);
		font-size: 0.8rem;
		margin-bottom: 0.35rem;
	}

	.card-agent {
		color: var(--accent);
	}

	.card-footer {
		display: flex;
		justify-content: space-between;
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	.pagination {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1rem;
		margin-top: 2rem;
		padding-top: 1rem;
		border-top: 1px solid var(--border);
	}

	.page-link {
		color: var(--accent);
		text-decoration: none;
		font-size: 0.9rem;
		padding: 0.4rem 0.8rem;
		border: 1px solid var(--border);
		border-radius: 4px;
		background: var(--bg-surface);
	}

	.page-link:hover {
		background: var(--bg-elevated);
	}

	.page-info {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
</style>
