<script lang="ts">
	import type { SubagentStartEntry, SubagentEndEntry } from '$lib/types';

	let { entry }: { entry: SubagentStartEntry | SubagentEndEntry } = $props();
</script>

<div class="entry entry-subagent">
	<div class="entry-header">
		{#if entry.type === 'subagent_start'}
			<span class="entry-badge badge-subagent">Subagent Start</span>
			<span class="entry-meta">{entry.agent}</span>
		{:else}
			<span class="entry-badge badge-subagent">Subagent End</span>
			{#if entry.status}
				<span class="entry-meta">{entry.status}</span>
			{/if}
		{/if}
	</div>
	{#if entry.type === 'subagent_start' && entry.context}
		<div class="entry-content">{entry.context}</div>
	{/if}
	{#if entry.type === 'subagent_end' && entry.summary}
		<div class="entry-content">{entry.summary}</div>
	{/if}
</div>
