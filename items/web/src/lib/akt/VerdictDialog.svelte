<script lang="ts">
	// r[impl app.verdicts]: one's own verdict reported, changed or deleted:
	// its status and its description, which a fail or a skip must have.
	import type { Verdict } from '#lib/graphql/generated/graphql.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Field from '#lib/ui/Field.svelte';
	import { VERDICTS, needsDescription, reportError, type Report } from './verdicts';

	interface Props {
		open: boolean;
		/** The akt the verdict is on, for the title. */
		number: string;
		/** The viewer's verdict as it is, `null` if they have none yet. */
		mine: Report | null;
		busy: boolean;
		/** Saves the verdict, or deletes it with `null`. */
		onSave: (report: Report | null) => void;
	}

	let { open = $bindable(), number, mine, busy, onSave }: Props = $props();
	// An akt can be listed twice on a page, in its sections and its list.
	const form = $props.id();

	let verdict = $state<Verdict | null>(null);
	let description = $state('');
	/** Whether saving was tried, so what is missing is said. */
	let tried = $state(false);

	$effect(() => {
		if (!open) return;
		verdict = mine?.verdict ?? null;
		description = mine?.description ?? '';
		tried = false;
	});

	const error = $derived(reportError({ verdict, description }));
	const required = $derived(verdict !== null && needsDescription(verdict));

	function save(event: SubmitEvent) {
		event.preventDefault();
		tried = true;
		if (error || !verdict) return;
		open = false;
		onSave({ verdict, description });
	}

	function remove() {
		open = false;
		onSave(null);
	}
</script>

<Dialog bind:open title={mine ? `Edit your verdict on ${number}` : `Report on ${number}`}>
	<form id={form} class="report" onsubmit={save} novalidate>
		<fieldset>
			<legend>Status</legend>
			<div class="statuses">
				{#each VERDICTS as option (option.verdict)}
					<label class="status">
						<input type="radio" name="verdict" value={option.verdict} bind:group={verdict} />
						{option.label}
					</label>
				{/each}
			</div>
			{#if tried && !verdict}<p class="error" role="alert">{error}</p>{/if}
		</fieldset>
		<Field
			label="Description"
			hint={required ? undefined : 'What you saw. A fail or a skip must have one.'}
			{required}
			error={tried && verdict && error ? error : undefined}
		>
			{#snippet children({ id, describedBy, invalid })}
				<textarea
					{id}
					rows="4"
					bind:value={description}
					aria-describedby={describedBy}
					aria-invalid={invalid || undefined}
					aria-required={required || undefined}></textarea>
			{/snippet}
		</Field>
	</form>
	{#snippet start()}
		{#if mine}
			<Button variant="danger" disabled={busy} onclick={remove}>Delete</Button>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button variant="primary" type="submit" {form} {busy}>Save</Button>
	{/snippet}
</Dialog>

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		border: 0;
		margin: 0;
		padding: 0;
	}

	legend {
		padding: 0;
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.statuses {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
	}

	.status {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.error {
		margin: 0;
		color: var(--tone-danger-fg);
		font-size: var(--text-sm);
	}
</style>
