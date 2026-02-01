<script lang="ts">
	import type { ResponseEntry } from '$lib/types';

	let { entry }: { entry: ResponseEntry } = $props();
</script>

<div class="entry entry-response">
	<div class="entry-header">
		<span class="entry-badge badge-response">Assistant</span>
		{#if entry.model}
			<span class="entry-meta">{entry.model}</span>
		{/if}
		{#if entry.token_usage}
			<span class="entry-meta">
				{entry.token_usage.input_tokens + entry.token_usage.output_tokens} tokens
			</span>
		{/if}
	</div>
	<div class="entry-content">{entry.content}</div>
</div>
