<script lang="ts">
	import Copy from '@lucide/svelte/icons/copy';
	import Check from '@lucide/svelte/icons/check';
	import { truncateMiddle } from './text';

	interface Props {
		value: string;
		/** At most this many characters are shown; the whole on hover. */
		max?: number;
		/** Offers to copy the whole value. */
		copy?: boolean;
	}

	let { value, max = 24, copy = false }: Props = $props();
	let copied = $state(false);

	async function copyValue() {
		await navigator.clipboard.writeText(value);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

<span class="mono" title={value}>
	<span aria-label={value}>{truncateMiddle(value, max)}</span>
	{#if copy}
		<button type="button" aria-label="Copy {value}" onclick={copyValue}>
			{#if copied}<Check size={12} aria-hidden={true} />{:else}<Copy
					size={12}
					aria-hidden={true}
				/>{/if}
		</button>
	{/if}
</span>

<style>
	.mono {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		font-family: var(--font-mono);
		font-size: 0.9em;
		white-space: nowrap;
	}

	button {
		display: inline-flex;
		padding: 2px;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
	}

	button:hover {
		background: var(--bg-muted);
		color: var(--text);
	}
</style>
