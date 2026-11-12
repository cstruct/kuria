<script lang="ts">
	import type { Snippet } from 'svelte';
	/** A state's tone, as the schema's `Tone` enum names it. */
	type Tone = 'NEUTRAL' | 'ACTIVE' | 'SUCCESS' | 'WARNING' | 'DANGER';

	let { tone = 'NEUTRAL', children }: { tone?: Tone; children: Snippet } = $props();
</script>

<span class="badge {tone.toLowerCase()}">{@render children()}</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		padding: 0.125rem var(--space-2);
		border-radius: 999px;
		font-size: var(--text-xs);
		font-weight: 600;
		line-height: 1.4;
		white-space: nowrap;
	}

	.neutral {
		background: var(--tone-neutral-bg);
		color: var(--tone-neutral-fg);
	}

	.active {
		background: var(--tone-active-bg);
		color: var(--tone-active-fg);
	}

	.success {
		background: var(--tone-success-bg);
		color: var(--tone-success-fg);
	}

	.warning {
		background: var(--tone-warning-bg);
		color: var(--tone-warning-fg);
	}

	.danger {
		background: var(--tone-danger-bg);
		color: var(--tone-danger-fg);
	}
</style>
