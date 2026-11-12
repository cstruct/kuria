<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { dismiss, toasts } from './toast.svelte';
</script>

<div class="toaster" role="status" aria-live="polite">
	{#each toasts as item (item.id)}
		<div class="toast {item.tone}">
			<span>{item.message}</span>
			<button type="button" aria-label="Dismiss" onclick={() => dismiss(item.id)}>
				<X size={14} aria-hidden={true} />
			</button>
		</div>
	{/each}
</div>

<style>
	.toaster {
		position: fixed;
		right: var(--space-4);
		bottom: var(--space-4);
		z-index: 60;
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		max-width: min(24rem, calc(100vw - 2rem));
	}

	.toast {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
		font-size: var(--text-sm);
	}

	.toast span {
		flex: 1;
	}

	.success {
		border-left: 4px solid var(--tone-success-fg);
	}

	.danger {
		border-left: 4px solid var(--tone-danger-fg);
	}

	button {
		border: 0;
		background: transparent;
		color: var(--text-muted);
		cursor: pointer;
	}
</style>
