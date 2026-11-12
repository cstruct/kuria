<script lang="ts">
	import type { Component } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = {
		/** What it does: its accessible name and its tooltip. */
		label: string;
		icon: Component<{ size?: number; 'aria-hidden'?: boolean }>;
		size?: 'sm' | 'md';
	} & HTMLButtonAttributes;

	let { label, icon: Icon, size = 'md', ...rest }: Props = $props();
</script>

<button type="button" aria-label={label} title={label} class="icon-button {size}" {...rest}>
	<Icon size={size === 'sm' ? 14 : 16} aria-hidden={true} />
</button>

<style>
	.icon-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
	}

	.md {
		width: 2rem;
		height: 2rem;
	}

	.sm {
		width: 1.5rem;
		height: 1.5rem;
	}

	.icon-button:hover:not(:disabled) {
		background: var(--bg-muted);
		color: var(--text);
	}

	.icon-button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
</style>
