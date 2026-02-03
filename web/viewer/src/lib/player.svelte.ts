import type { Entry, SpoolFile } from './types';

/** Idle gap compression: cap gaps before prompts to this value */
const MAX_IDLE_GAP_MS = 2000;
/** Thinking compression: cap thinking duration to this value */
const MAX_THINKING_GAP_MS = 2000;

export type PlaybackState = 'stopped' | 'playing' | 'paused';

export const SPEED_PRESETS = [0.25, 0.5, 1, 2, 4, 8, 16] as const;

/** Compute compressed playback timestamps for entries */
function computePlaybackTimes(entries: Entry[]): number[] {
	const times: number[] = [];
	for (let i = 0; i < entries.length; i++) {
		if (i === 0) {
			times.push(0);
			continue;
		}
		const gap = entries[i].ts - entries[i - 1].ts;
		const entry = entries[i];
		const prev = entries[i - 1];

		let compressed = gap;
		// Compress idle gaps before prompts (user think-time)
		if (entry.type === 'prompt' && gap > MAX_IDLE_GAP_MS) {
			compressed = MAX_IDLE_GAP_MS;
		}
		// Compress thinking gaps
		if (prev.type === 'thinking' && gap > MAX_THINKING_GAP_MS) {
			compressed = MAX_THINKING_GAP_MS;
		}
		times.push(times[i - 1] + compressed);
	}
	return times;
}

/** Playback engine for .spool files */
export class Player {
	// Reactive state (Svelte 5 runes)
	spool = $state<SpoolFile | null>(null);
	state = $state<PlaybackState>('stopped');
	currentIndex = $state(0);
	speed = $state(1);
	elapsed = $state(0);

	// Internal
	private playbackTimes: number[] = [];
	private animationFrame: number | null = null;
	private lastFrameTime: number | null = null;

	/** Visible entries (up to current index) */
	get visibleEntries(): Entry[] {
		if (!this.spool) return [];
		return this.spool.entries.slice(0, this.currentIndex + 1);
	}

	/** Total compressed duration */
	get totalDuration(): number {
		if (this.playbackTimes.length === 0) return 0;
		return this.playbackTimes[this.playbackTimes.length - 1];
	}

	/** Progress as 0-1 */
	get progress(): number {
		if (this.totalDuration === 0) return 0;
		return Math.min(this.elapsed / this.totalDuration, 1);
	}

	/** Whether we're at the end */
	get atEnd(): boolean {
		return this.spool !== null && this.currentIndex >= this.spool.entries.length - 1;
	}

	/** Load a spool file */
	load(spool: SpoolFile) {
		this.stop();
		this.spool = spool;
		this.playbackTimes = computePlaybackTimes(spool.entries);
		this.currentIndex = 0;
		this.elapsed = 0;
	}

	/** Start or resume playback */
	play() {
		if (!this.spool) return;
		if (this.atEnd) {
			// Restart from beginning
			this.currentIndex = 0;
			this.elapsed = 0;
		}
		this.state = 'playing';
		this.lastFrameTime = performance.now();
		this.tick();
	}

	/** Pause playback */
	pause() {
		this.state = 'paused';
		this.cancelFrame();
	}

	/** Toggle play/pause */
	toggle() {
		if (this.state === 'playing') {
			this.pause();
		} else {
			this.play();
		}
	}

	/** Stop and reset */
	stop() {
		this.state = 'stopped';
		this.cancelFrame();
		this.currentIndex = 0;
		this.elapsed = 0;
	}

	/** Step forward one entry */
	stepForward() {
		if (!this.spool) return;
		if (this.currentIndex < this.spool.entries.length - 1) {
			this.currentIndex++;
			this.elapsed = this.playbackTimes[this.currentIndex];
		}
	}

	/** Step backward one entry */
	stepBackward() {
		if (this.currentIndex > 0) {
			this.currentIndex--;
			this.elapsed = this.playbackTimes[this.currentIndex];
		}
	}

	/** Jump to start */
	jumpToStart() {
		this.currentIndex = 0;
		this.elapsed = 0;
	}

	/** Jump to end (show all entries) */
	jumpToEnd() {
		if (!this.spool) return;
		this.currentIndex = this.spool.entries.length - 1;
		this.elapsed = this.totalDuration;
		this.pause();
	}

	/** Seek to a progress value (0-1) */
	seek(progress: number) {
		if (!this.spool) return;
		const targetTime = progress * this.totalDuration;
		this.elapsed = targetTime;
		// Find the entry index at this time
		for (let i = this.playbackTimes.length - 1; i >= 0; i--) {
			if (this.playbackTimes[i] <= targetTime) {
				this.currentIndex = i;
				break;
			}
		}
	}

	/** Set playback speed */
	setSpeed(speed: number) {
		this.speed = speed;
	}

	/** Cycle to next speed preset */
	cycleSpeed() {
		const idx = SPEED_PRESETS.indexOf(this.speed as (typeof SPEED_PRESETS)[number]);
		const next = (idx + 1) % SPEED_PRESETS.length;
		this.speed = SPEED_PRESETS[next];
	}

	/** Destroy the player */
	destroy() {
		this.cancelFrame();
	}

	private tick() {
		if (this.state !== 'playing') return;

		this.animationFrame = requestAnimationFrame((now) => {
			if (this.lastFrameTime === null) {
				this.lastFrameTime = now;
			}
			const delta = (now - this.lastFrameTime) * this.speed;
			this.lastFrameTime = now;
			this.elapsed += delta;

			// Advance entry index based on elapsed time
			while (
				this.spool &&
				this.currentIndex < this.spool.entries.length - 1 &&
				this.playbackTimes[this.currentIndex + 1] <= this.elapsed
			) {
				this.currentIndex++;
			}

			if (this.atEnd) {
				this.state = 'paused';
				this.cancelFrame();
				return;
			}

			this.tick();
		});
	}

	private cancelFrame() {
		if (this.animationFrame !== null) {
			cancelAnimationFrame(this.animationFrame);
			this.animationFrame = null;
		}
		this.lastFrameTime = null;
	}
}
