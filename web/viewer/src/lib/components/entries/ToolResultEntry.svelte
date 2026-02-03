<script lang="ts">
	import type { ToolResultEntry } from '$lib/types';

	let { entry }: { entry: ToolResultEntry } = $props();
	let expanded = $state(false);

	const content = $derived(
		entry.error
			? entry.error
			: typeof entry.output === 'string'
				? entry.output
				: entry.output
					? `[Binary: ${entry.output.media_type}]`
					: '[No output]'
	);
	const isError = $derived(!!entry.error);
</script>

<div class="entry entry-tool-result" class:tool-error={isError}>
	<button class="entry-header tool-toggle" onclick={() => (expanded = !expanded)}>
		<span class="entry-badge" class:badge-error={isError} class:badge-tool-result={!isError}>
			{isError ? 'Error' : 'Result'}
		</span>
		{#if entry._redacted?.length}
			<span class="entry-meta redacted-badge">redacted</span>
		{/if}
		<span class="toggle-icon">{expanded ? '\u25BC' : '\u25B6'}</span>
	</button>
	{#if expanded}
		<pre class="entry-content tool-output">{content}</pre>
	{/if}
</div>
