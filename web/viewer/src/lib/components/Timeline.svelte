<script lang="ts">
	import { formatDuration } from '$lib/parser';

	let {
		progress = 0,
		elapsed = 0,
		total = 0,
		onseek
	}: {
		progress: number;
		elapsed: number;
		total: number;
		onseek: (p: number) => void;
	} = $props();

	function handleClick(e: MouseEvent) {
		const bar = e.currentTarget as HTMLElement;
		const rect = bar.getBoundingClientRect();
		const p = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		onseek(p);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowRight') {
			onseek(Math.min(1, progress + 0.02));
		} else if (e.key === 'ArrowLeft') {
			onseek(Math.max(0, progress - 0.02));
		}
	}
</script>

<div class="timeline">
	<span class="timeline-time">{formatDuration(elapsed)}</span>
	<div
		class="timeline-bar"
		role="slider"
		tabindex="0"
		aria-label="Playback position"
		aria-valuemin={0}
		aria-valuemax={100}
		aria-valuenow={Math.round(progress * 100)}
		onclick={handleClick}
		onkeydown={handleKeydown}
	>
		<div class="timeline-fill" style="width: {progress * 100}%"></div>
	</div>
	<span class="timeline-time">{formatDuration(total)}</span>
</div>
