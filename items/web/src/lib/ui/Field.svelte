<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		hint?: string;
		error?: string;
		required?: boolean;
		/** The control, given the ids to label and describe it with. */
		children: Snippet<[{ id: string; describedBy: string | undefined; invalid: boolean }]>;
	}

	let { label, hint, error, required = false, children }: Props = $props();
	const id = $props.id();
	const describedBy = $derived(
		[hint ? `${id}-hint` : '', error ? `${id}-error` : ''].filter(Boolean).join(' ') || undefined
	);
</script>

<div class="field">
	<label for={id}>
		{label}{#if required}<span class="required" aria-hidden="true"> *</span>{/if}
	</label>
	{@render children({ id, describedBy, invalid: Boolean(error) })}
	{#if hint}<p class="hint" id="{id}-hint">{hint}</p>{/if}
	{#if error}<p class="error" id="{id}-error" role="alert">{error}</p>{/if}
</div>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	label {
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.required {
		color: var(--tone-danger-fg);
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	.error {
		font-size: var(--text-xs);
		color: var(--tone-danger-fg);
	}
</style>
