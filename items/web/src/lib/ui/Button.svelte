<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';

	type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';

	type Props = {
		variant?: Variant;
		size?: 'sm' | 'md';
		/** Busy: disabled, and saying so to assistive technology. */
		busy?: boolean;
		children: Snippet;
	} & (({ href: string } & HTMLAnchorAttributes) | ({ href?: undefined } & HTMLButtonAttributes));

	let {
		variant = 'secondary',
		size = 'md',
		busy = false,
		children,
		href,
		...rest
	}: Props = $props();
</script>

{#if href}
	<!-- Callers pass hrefs they resolved. -->
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
	<a {href} class="button {variant} {size}" {...rest as HTMLAnchorAttributes}>
		{@render children()}
	</a>
{:else}
	{@const attributes = rest as HTMLButtonAttributes}
	<button
		type="button"
		{...attributes}
		class="button {variant} {size}"
		disabled={busy || attributes.disabled}
		aria-busy={busy || undefined}
	>
		{@render children()}
	</button>
{/if}

<style>
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		font: inherit;
		font-weight: 500;
		line-height: 1;
		white-space: nowrap;
		cursor: pointer;
		text-decoration: none;
		transition:
			background 0.1s,
			border-color 0.1s;
	}

	.md {
		min-height: 2.25rem;
		padding: 0 var(--space-4);
		font-size: var(--text-sm);
	}

	.sm {
		min-height: 1.75rem;
		padding: 0 var(--space-3);
		font-size: var(--text-xs);
	}

	.primary {
		background: var(--accent);
		color: var(--on-accent);
	}

	.primary:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.secondary {
		background: var(--surface);
		border-color: var(--border-strong);
		color: var(--text);
	}

	.secondary:hover:not(:disabled) {
		background: var(--bg-muted);
	}

	.ghost {
		background: transparent;
		color: var(--text);
	}

	.ghost:hover:not(:disabled) {
		background: var(--bg-muted);
	}

	.danger {
		background: var(--danger);
		color: var(--on-danger);
	}

	.danger:hover:not(:disabled) {
		background: var(--danger-hover);
	}

	.button:hover {
		text-decoration: none;
	}

	.button:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.button[aria-busy='true'] {
		cursor: progress;
	}
</style>
