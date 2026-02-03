<script lang="ts">
	import type { Entry, AnnotationEntry } from '$lib/types';
	import { formatTimestamp } from '$lib/parser';
	import PromptEntry from './entries/PromptEntry.svelte';
	import ResponseEntry from './entries/ResponseEntry.svelte';
	import ThinkingEntry from './entries/ThinkingEntry.svelte';
	import ToolCallEntry from './entries/ToolCallEntry.svelte';
	import ToolResultEntry from './entries/ToolResultEntry.svelte';
	import ErrorEntry from './entries/ErrorEntry.svelte';
	import AnnotationBadge from './entries/AnnotationBadge.svelte';
	import SubagentEntry from './entries/SubagentEntry.svelte';

	let {
		entry,
		annotations = []
	}: { entry: Entry; annotations?: AnnotationEntry[] } = $props();
</script>

<div class="entry-wrapper" id="entry-{entry.id}">
	<div class="entry-timestamp">{formatTimestamp(entry.ts)}</div>
	<div class="entry-body">
		{#if entry.type === 'prompt'}
			<PromptEntry {entry} />
		{:else if entry.type === 'response'}
			<ResponseEntry {entry} />
		{:else if entry.type === 'thinking'}
			<ThinkingEntry {entry} />
		{:else if entry.type === 'tool_call'}
			<ToolCallEntry {entry} />
		{:else if entry.type === 'tool_result'}
			<ToolResultEntry {entry} />
		{:else if entry.type === 'error'}
			<ErrorEntry {entry} />
		{:else if entry.type === 'annotation'}
			<AnnotationBadge {entry} />
		{:else if entry.type === 'subagent_start' || entry.type === 'subagent_end'}
			<SubagentEntry {entry} />
		{:else if entry.type === 'session' || entry.type === 'redaction_marker'}
			<!-- Session header and redaction markers handled elsewhere -->
		{/if}

		{#each annotations as ann}
			<AnnotationBadge entry={ann} />
		{/each}
	</div>
</div>
