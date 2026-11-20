<script lang="ts">
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import Info from '@lucide/svelte/icons/info';
	import type { Snippet } from 'svelte';

	interface Props {
		tone?: 'danger' | 'neutral';
		/** A short heading, such as what was refused. */
		title?: string;
		children: Snippet;
	}

	let { tone = 'danger', title, children }: Props = $props();
</script>

<!-- A message that needs reading before going on: a refusal, or why
     something can't be done here. -->
<div class="alert {tone}" role={tone === 'danger' ? 'alert' : 'status'}>
	{#if tone === 'danger'}
		<CircleAlert size={16} aria-hidden={true} />
	{:else}
		<Info size={16} aria-hidden={true} />
	{/if}
	<div class="body">
		{#if title}<p class="title">{title}</p>{/if}
		{@render children()}
	</div>
</div>

<style>
	.alert {
		display: flex;
		gap: var(--space-2);
		align-items: flex-start;
		padding: var(--space-3);
		border: 1px solid;
		border-radius: var(--radius-md);
		font-size: var(--text-sm);
	}

	.alert :global(svg) {
		flex-shrink: 0;
		margin-top: 2px;
	}

	.danger {
		background: var(--tone-danger-bg);
		border-color: color-mix(in srgb, var(--tone-danger-fg) 30%, transparent);
		color: var(--tone-danger-fg);
	}

	.neutral {
		background: var(--bg-muted);
		border-color: var(--border);
		color: var(--text);
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.title {
		font-weight: 600;
	}
</style>
