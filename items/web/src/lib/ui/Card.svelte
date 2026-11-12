<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = {
		/** A heading, shown in the card's header. */
		title?: string;
		/** Actions beside the title. */
		actions?: Snippet;
		padded?: boolean;
		children: Snippet;
	} & HTMLAttributes<HTMLElement>;

	let { title, actions, padded = true, children, ...rest }: Props = $props();
	const id = $props.id();
</script>

<section class="card" aria-labelledby={title ? `${id}-title` : undefined} {...rest}>
	{#if title || actions}
		<header>
			{#if title}<h2 id="{id}-title">{title}</h2>{/if}
			{#if actions}<div class="actions">{@render actions()}</div>{/if}
		</header>
	{/if}
	<div class:padded>{@render children()}</div>
</section>

<style>
	.card {
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
		box-shadow: var(--shadow-sm);
		min-width: 0;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-bottom: 1px solid var(--border);
	}

	h2 {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.actions {
		display: flex;
		gap: var(--space-2);
	}

	.padded {
		padding: var(--space-4);
	}
</style>
