<script lang="ts">
	import Lock from '@lucide/svelte/icons/lock';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import FieldEditor from './FieldEditor.svelte';
	import FieldShown from './FieldShown.svelte';
	import type { Person } from './people';
	import type { Draft, FieldDef, Value } from './values';
	import VerdictButtons from './VerdictButtons.svelte';
	import type { Judged, Report } from './verdicts';
	import type { LiveDraft } from './live.svelte';

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
		/** Whether it is locked in the akt's state and holds a value, so
		 * that changing it needs a reason (`akt.field.locked`). */
		locked?: boolean;
		busy: boolean;
		error?: string;
		onEdit: () => void;
		onSave: () => void;
		onCancel: () => void;
		onDraft: () => void;
		/** The akt's members, which a sections field places. */
		members?: readonly { number: string; title?: string | null }[];
		/** For a verdict field: whether verdicts may be given now
		 * (`app.verdicts`), the akt's number, its verdicts, and giving one. */
		judging?: boolean;
		number?: string;
		judged?: Judged | null;
		onVerdict?: (verdict: Report | null) => void;
		/** Whether the akt has a live draft: an edit changes it, without a
		 * reason (`app.live`). */
		liveDraft?: boolean;
		/** The live draft the field's editor edits as it is typed in, for a
		 * text field of one: no saving. */
		live?: LiveDraft | null;
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
		locked = false,
		busy,
		error,
		onEdit,
		onSave,
		onCancel,
		onDraft,
		members = [],
		number = '',
		judged = null,
		onVerdict = () => {},
		judging = true,
		liveDraft = false,
		live = null
	}: Props = $props();

	// A markdown field's sheets are the page's document: they take the whole
	// row, named only to assistive technology. Empty, it keeps its name.
	const wide = $derived(field.kind === 'MARKDOWN' && Boolean(value));
</script>

<svelte:window onkeydown={(e) => editing && e.key === 'Escape' && onCancel()} />

<!-- r[impl app.live]: a text field of a live draft is edited as it is typed, closed when done. -->

<!-- r[impl app.inline-edit]: each field opens its own editor, in place. -->
<!-- A dt and dd pair, as its list requires, whether shown or edited. -->
<div class="row" class:editing class:wide>
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
				<FieldEditor {field} bind:draft {people} {error} {members} {live} />
				<!-- r[impl app.fields]: a locked field changes only with a reason, asked for first. -->
				{#if locked}
					<label class="reason">
						Reason <span class="optional">(needed: {field.name} is locked)</span>
						<input bind:value={reason} required />
					</label>
				{:else if !liveDraft}
					<label class="reason">
						Reason <span class="optional">(optional)</span>
						<input bind:value={reason} />
					</label>
				{/if}
				<div class="actions">
					{#if live}
						<Button variant="primary" size="sm" onclick={onCancel}>Done</Button>
					{:else}
						<Button
							variant="primary"
							size="sm"
							type="submit"
							{busy}
							disabled={locked && reason.trim() === ''}>Save</Button
						>
						<Button variant="ghost" size="sm" onclick={onCancel}>Cancel</Button>
					{/if}
				</div>
			</form>
		</dd>
	{:else}
		<dt class:visually-hidden={wide}>
			{field.name}{#if locked}<span class="locked" title="Locked: changing it needs a reason"
					><Lock size={12} aria-label="locked" /></span
				>{/if}
		</dt>
		<dd data-field={field.key}>
			<div class="value">
				{#if field.kind === 'VERDICT' && judged}
					<VerdictButtons
						{number}
						{judged}
						busy={busy || !editable}
						open={judging}
						onGive={onVerdict}
					/>
				{:else if value}
					<FieldShown {value} multiline={field.multiline} />
				{:else}
					<span class="empty">—</span>
				{/if}
			</div>
			{#if editable && draft?.kind !== 'SHOWN'}
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

	.row.editing,
	.row.wide {
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

	.locked {
		margin-inline-start: var(--space-1);
		color: var(--text-muted);
		vertical-align: middle;
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
