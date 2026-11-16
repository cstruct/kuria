<script lang="ts">
	import { Dialog } from 'bits-ui';
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';

	interface Props {
		open: boolean;
		title: string;
		description?: string;
		children: Snippet;
		/** The dialog's actions, at its foot. */
		footer?: Snippet;
		onOpenChange?: (open: boolean) => void;
		/** Where focus goes when it closes, instead of what opened it: call
		 * `preventDefault` on the event and focus it. */
		onCloseAutoFocus?: (event: Event) => void;
	}

	let {
		open = $bindable(),
		title,
		description,
		children,
		footer,
		onOpenChange,
		onCloseAutoFocus
	}: Props = $props();

	let body = $state<HTMLElement>();

	// r[impl app.dialog-focus]: its first field, rather than the Close button
	// before it; with none, the first thing that takes focus, as bits-ui does.
	const fields =
		'input:not([type="hidden"], :disabled), select:not(:disabled), textarea:not(:disabled), [contenteditable="true"]';

	function onOpenAutoFocus(event: Event) {
		const field = body?.querySelector<HTMLElement>(fields);
		if (!field) return;
		event.preventDefault();
		field.focus();
	}
</script>

<Dialog.Root bind:open {onOpenChange}>
	<Dialog.Portal>
		<Dialog.Overlay class="dialog-overlay" />
		<Dialog.Content class="dialog-content" {onOpenAutoFocus} {onCloseAutoFocus}>
			<header>
				<Dialog.Title class="dialog-title">{title}</Dialog.Title>
				<Dialog.Close class="dialog-close" aria-label="Close">
					<X size={16} aria-hidden={true} />
				</Dialog.Close>
			</header>
			{#if description}
				<Dialog.Description class="dialog-description">{description}</Dialog.Description>
			{/if}
			<div class="dialog-body" bind:this={body}>{@render children()}</div>
			{#if footer}<footer>{@render footer()}</footer>{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	:global(.dialog-overlay) {
		position: fixed;
		inset: 0;
		z-index: 40;
		background: rgb(0 0 0 / 0.45);
	}

	:global(.dialog-content) {
		position: fixed;
		top: 50%;
		left: 50%;
		z-index: 41;
		transform: translate(-50%, -50%);
		width: min(36rem, calc(100vw - 2rem));
		max-height: calc(100vh - 2rem);
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
	}

	header,
	footer {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-4) var(--space-5);
	}

	header {
		justify-content: space-between;
	}

	footer {
		justify-content: flex-end;
		border-top: 1px solid var(--border);
	}

	:global(.dialog-title) {
		font-size: var(--text-lg);
		font-weight: 600;
	}

	:global(.dialog-close) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border: 0;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
	}

	:global(.dialog-close:hover) {
		background: var(--bg-muted);
	}

	:global(.dialog-description) {
		padding: 0 var(--space-5);
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.dialog-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-4) var(--space-5);
	}
</style>
