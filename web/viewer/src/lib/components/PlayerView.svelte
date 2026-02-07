<script lang="ts">
	import type { AnnotationEntry, SpoolFile } from '$lib/types';
	import { Player, SPEED_PRESETS } from '$lib/player.svelte';
	import { formatDuration, entryTypeLabel } from '$lib/parser';
	import EntryComponent from './Entry.svelte';
	import Timeline from './Timeline.svelte';

	let { spool }: { spool: SpoolFile } = $props();

	const player = new Player();

	$effect(() => {
		player.load(spool);
	});

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

	// Filter out session, annotation, and redaction_marker from main list
	const displayEntries = $derived(
		player.visibleEntries.filter(
			(e) => e.type !== 'session' && e.type !== 'annotation' && e.type !== 'redaction_marker'
		)
	);

	function handleKeydown(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

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
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="player">
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
			{#if spool.session.ended}
				<span class="session-status status-{spool.session.ended}">{spool.session.ended}</span>
			{/if}
		</div>
		{#if spool.session.tags?.length}
			<div class="session-tags">
				{#each spool.session.tags as tag}
					<span class="tag">{tag}</span>
				{/each}
			</div>
		{/if}
	</header>

	<div class="controls">
		<button class="control-btn" onclick={() => player.toggle()} title="Play/Pause (Space)">
			{player.state === 'playing' ? '⏸' : '▶'}
		</button>
		<button class="control-btn" onclick={() => player.stepBackward()} title="Step Back (h)">
			⏮
		</button>
		<button class="control-btn" onclick={() => player.stepForward()} title="Step Forward (l)">
			⏭
		</button>
		<button class="control-btn" onclick={() => player.jumpToStart()} title="Jump to Start (g)">
			⏪
		</button>
		<button class="control-btn" onclick={() => player.jumpToEnd()} title="Jump to End (G)">
			⏩
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

	<div class="entry-list">
		{#each displayEntries as entry (entry.id)}
			<EntryComponent {entry} annotations={annotationMap.get(entry.id) ?? []} />
		{/each}
	</div>
</div>
