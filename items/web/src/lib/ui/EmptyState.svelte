<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, description, action }: { title: string; description?: string; action?: Snippet } =
		$props();
</script>

<div class="empty">
	<p class="title">{title}</p>
	{#if description}<p class="description">{description}</p>{/if}
	{#if action}<div class="action">{@render action()}</div>{/if}
</div>

<style>
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-6) var(--space-4);
		text-align: center;
		color: var(--text-muted);
	}

	.title {
		font-weight: 600;
		color: var(--text);
	}

	.description {
		font-size: var(--text-sm);
		max-width: 32rem;
	}

	.action {
		margin-top: var(--space-2);
	}
</style>
