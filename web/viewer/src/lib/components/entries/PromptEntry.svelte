<script lang="ts">
	import type { PromptEntry } from '$lib/types';

	let { entry }: { entry: PromptEntry } = $props();
</script>

<div class="entry entry-prompt">
	<div class="entry-header">
		<span class="entry-badge badge-prompt">User</span>
	</div>
	<div class="entry-content">{entry.content}</div>
</div>
