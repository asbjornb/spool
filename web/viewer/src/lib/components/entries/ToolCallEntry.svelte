<script lang="ts">
	import type { ToolCallEntry } from '$lib/types';

	let { entry }: { entry: ToolCallEntry } = $props();
	let expanded = $state(false);
</script>

<div class="entry entry-tool-call">
	<button class="entry-header tool-toggle" onclick={() => (expanded = !expanded)}>
		<span class="entry-badge badge-tool">Tool</span>
		<span class="tool-name">{entry.tool}</span>
		<span class="toggle-icon">{expanded ? '\u25BC' : '\u25B6'}</span>
	</button>
	{#if expanded}
		<pre class="entry-content tool-input">{JSON.stringify(entry.input, null, 2)}</pre>
	{/if}
</div>
