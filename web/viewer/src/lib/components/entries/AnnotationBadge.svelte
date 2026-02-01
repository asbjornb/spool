<script lang="ts">
	import type { AnnotationEntry } from '$lib/types';

	let { entry }: { entry: AnnotationEntry } = $props();
</script>

<div class="annotation annotation-{entry.style ?? 'comment'}">
	<span class="annotation-icon">
		{#if entry.style === 'highlight'}&#9733;
		{:else if entry.style === 'warning'}&#9888;
		{:else if entry.style === 'success'}&#10003;
		{:else}&#9998;
		{/if}
	</span>
	<span class="annotation-text">{entry.content}</span>
	{#if entry.author}
		<span class="annotation-author">&mdash; {entry.author}</span>
	{/if}
</div>
