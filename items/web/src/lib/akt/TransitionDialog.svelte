<script lang="ts">
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import FieldEditor from './FieldEditor.svelte';
	import type { Refusal } from './errors';
	import type { Person } from './people';
	import type { Draft, FieldDef } from './values';

	interface Props {
		open: boolean;
		/** The transition's name, the dialog's title and its action. */
		name: string;
		/** The fields it requires, asked for here. */
		fields: FieldDef[];
		drafts: Record<string, Draft>;
		reason: string;
		people: Person[];
		refusal: Refusal | null;
		busy: boolean;
		onPerform: () => void;
	}

	let {
		open = $bindable(),
		name,
		fields,
		drafts = $bindable(),
		reason = $bindable(),
		people,
		refusal,
		busy,
		onPerform
	}: Props = $props();

	// A refusal of one of the fields asked for shows by that field; any
	// other, such as a requirement not met, for the whole dialog.
	const general = $derived(
		refusal && !fields.some((f) => f.key === refusal.field) ? refusal : null
	);
</script>

<!-- r[impl app.transitions]: the fields a transition requires, asked for before it is performed. -->
<Dialog
	bind:open
	title={name}
	description={fields.length > 0 ? 'It needs these fields.' : undefined}
>
	<form
		id="transition-form"
		aria-label={name}
		onsubmit={(e) => {
			e.preventDefault();
			onPerform();
		}}
	>
		{#each fields as field (field.key)}
			<FieldEditor
				{field}
				bind:draft={drafts[field.key]}
				{people}
				error={refusal?.field === field.key ? refusal.message : undefined}
			/>
		{/each}
		<label>
			Reason <span class="optional">(optional)</span>
			<input bind:value={reason} />
		</label>
		{#if general}<p role="alert">{general.message}</p>{/if}
	</form>
	{#snippet footer()}
		<Button variant="ghost" onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="transition-form" {busy}>{name}</Button>
	{/snippet}
</Dialog>

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.optional {
		color: var(--text-muted);
		font-weight: 400;
	}

	[role='alert'] {
		color: var(--tone-danger-fg);
		font-size: var(--text-sm);
	}
</style>
