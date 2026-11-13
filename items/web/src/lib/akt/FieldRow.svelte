<script lang="ts">
	import Pencil from '@lucide/svelte/icons/pencil';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import FieldEditor from './FieldEditor.svelte';
	import FieldShown from './FieldShown.svelte';
	import type { Person } from './people';
	import type { Draft, FieldDef, Value } from './values';

	interface Props {
		field: FieldDef;
		value: Value | undefined;
		draft: Draft;
		reason: string;
		people: Person[];
		/** Whether this field's editor is open. */
		editing: boolean;
		/** Whether the akt can be edited at all. */
		editable: boolean;
		/** Whether the field has a draft recipe. */
		drafted: boolean;
		busy: boolean;
		error?: string;
		onEdit: () => void;
		onSave: () => void;
		onCancel: () => void;
		onDraft: () => void;
	}

	let {
		field,
		value,
		draft = $bindable(),
		reason = $bindable(),
		people,
		editing,
		editable,
		drafted,
		busy,
		error,
		onEdit,
		onSave,
		onCancel,
		onDraft
	}: Props = $props();
</script>

<svelte:window onkeydown={(e) => editing && e.key === 'Escape' && onCancel()} />

<!-- r[impl app.inline-edit]: each field opens its own editor, in place. -->
<!-- A dt and dd pair, as its list requires, whether shown or edited. -->
<div class="row" class:editing>
	{#if editing}
		<dt class="visually-hidden">{field.name}</dt>
		<dd>
			<form
				aria-label="Edit {field.name}"
				onsubmit={(e) => {
					e.preventDefault();
					onSave();
				}}
			>
				<FieldEditor {field} bind:draft {people} {error} />
				<label class="reason">
					Reason <span class="optional">(optional)</span>
					<input bind:value={reason} />
				</label>
				<div class="actions">
					<Button variant="primary" size="sm" type="submit" {busy}>Save</Button>
					<Button variant="ghost" size="sm" onclick={onCancel}>Cancel</Button>
				</div>
			</form>
		</dd>
	{:else}
		<dt>{field.name}</dt>
		<dd data-field={field.key}>
			<div class="value">
				{#if value}
					<FieldShown {value} multiline={field.multiline} />
				{:else}
					<span class="empty">—</span>
				{/if}
			</div>
			{#if editable}
				<div class="tools">
					{#if drafted}
						<IconButton
							label="Draft {field.name}"
							icon={Sparkles}
							size="sm"
							disabled={busy}
							onclick={onDraft}
						/>
					{/if}
					<IconButton label="Edit {field.name}" icon={Pencil} size="sm" onclick={onEdit} />
				</div>
			{/if}
		</dd>
	{/if}
</div>

<style>
	.row {
		display: grid;
		grid-template-columns: minmax(8rem, 12rem) minmax(0, 1fr);
		gap: var(--space-3);
		align-items: start;
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}

	.row:last-child {
		border-bottom: 0;
	}

	.row.editing {
		display: block;
	}

	dt {
		color: var(--text-muted);
		font-size: var(--text-sm);
		padding-top: 2px;
	}

	dd {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		margin: 0;
		min-width: 0;
	}

	.editing dd {
		display: block;
	}

	.value {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.empty {
		color: var(--text-muted);
	}

	.tools {
		display: flex;
		flex-shrink: 0;
		gap: var(--space-1);
		opacity: 0.6;
	}

	.row:hover .tools,
	.tools:focus-within {
		opacity: 1;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-2) 0;
	}

	.reason {
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

	.actions {
		display: flex;
		gap: var(--space-2);
	}

	@media (max-width: 599px) {
		.row {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
