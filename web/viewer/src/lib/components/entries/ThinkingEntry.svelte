<script lang="ts">
	import type { ThinkingEntry } from '$lib/types';

	let { entry }: { entry: ThinkingEntry } = $props();
	let expanded = $state(false);
</script>

<div class="entry entry-thinking">
	<button class="entry-header thinking-toggle" onclick={() => (expanded = !expanded)}>
		<span class="entry-badge badge-thinking">Thinking</span>
		<span class="toggle-icon">{expanded ? '\u25BC' : '\u25B6'}</span>
	</button>
	{#if expanded}
		<div class="entry-content thinking-content">{entry.content}</div>
	{/if}
</div>
