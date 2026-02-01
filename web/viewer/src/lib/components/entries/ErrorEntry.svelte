<script lang="ts">
	import type { ErrorEntry } from '$lib/types';

	let { entry }: { entry: ErrorEntry } = $props();
</script>

<div class="entry entry-error">
	<div class="entry-header">
		<span class="entry-badge badge-error">{entry.code}</span>
		{#if entry.recoverable}
			<span class="entry-meta">recoverable</span>
		{/if}
	</div>
	<div class="entry-content">{entry.message}</div>
</div>
