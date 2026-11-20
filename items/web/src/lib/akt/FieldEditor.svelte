<script lang="ts">
	import MarkdownEditor from './editor/MarkdownEditor.svelte';
	import PeopleSelect from './PeopleSelect.svelte';
	import SectionsEditor from './SectionsEditor.svelte';
	import SomeoneSelect from './SomeoneSelect.svelte';
	import { legendOf } from './labels';
	import { upload } from './upload';
	import type { Person } from './people';
	import type { Draft, FieldDef } from './values';
	import { bindTextarea } from './editor/live';
	import type { LiveDraft } from './live.svelte';

	interface Props {
		field: FieldDef;
		draft: Draft;
		people: Person[];
		/** The field's refusal, if its value was refused. */
		error?: string;
		/** The akt's members, which a sections field places. */
		members?: readonly { number: string; title?: string | null }[];
		/** The live draft whose text the field's editor edits, if it is a
		 * text field of one (`app.live`). */
		live?: LiveDraft | null;
	}

	let { field, draft = $bindable(), people, error, members = [], live = null }: Props = $props();

	/** A text area bound to the live draft's text of the field. */
	function boundTo(area: HTMLTextAreaElement, draft: LiveDraft) {
		return { destroy: bindTextarea(area, draft.text(field.key), draft.origin) };
	}

	let uploading = $state(false);
	let uploadError = $state('');
	const id = $derived(`field-${field.key}`);

	async function attach(event: Event & { currentTarget: HTMLInputElement }) {
		const file = event.currentTarget.files?.[0];
		if (!file || draft.kind !== 'ATTACHMENT') return;
		uploading = true;
		uploadError = '';
		try {
			const uploaded = await upload(file);
			draft = { kind: 'ATTACHMENT', id: uploaded.id, name: uploaded.name };
		} catch (e) {
			uploadError = (e as Error).message;
		} finally {
			uploading = false;
		}
	}
</script>

<!-- r[impl app.create]: labelled once, above the control, its hint below. -->
<fieldset
	class="field"
	aria-describedby={[field.description ? `${id}-hint` : '', error ? `${id}-error` : '']
		.filter(Boolean)
		.join(' ') || undefined}
>
	<!-- A checkbox is labelled by its own name, beside it. -->
	<legend class:visually-hidden={draft.kind === 'CHECKBOX'}>
		<span aria-hidden="true"
			>{field.name}{#if field.required}<span class="required"> *</span>{/if}</span
		><span class="visually-hidden">{legendOf(field)}</span>
	</legend>

	{#if draft.kind === 'TEXT'}
		{#if field.multiline && live}
			<textarea aria-label={field.name} rows="6" use:boundTo={live}></textarea>
		{:else if field.multiline}
			<textarea aria-label={field.name} rows="6" bind:value={draft.text}></textarea>
		{:else}
			<input aria-label={field.name} bind:value={draft.text} />
		{/if}
	{:else if draft.kind === 'MARKDOWN'}
		<MarkdownEditor
			bind:text={draft.text}
			pictures={draft.pictures}
			drawings={draft.drawings}
			diagrams={field.diagrams}
			label={field.name}
			live={live ? { draft: live, field: field.key } : null}
		/>
	{:else if draft.kind === 'CHOICE'}
		<select aria-label={field.name} bind:value={draft.key}>
			<option value="">None</option>
			{#each field.options as option (option.key)}
				<option value={option.key}>{option.name}</option>
			{/each}
		</select>
	{:else if draft.kind === 'CHOICES'}
		{#each field.options as option (option.key)}
			<label class="inline">
				<input type="checkbox" value={option.key} bind:group={draft.keys} />
				{option.name}
			</label>
		{/each}
	{:else if draft.kind === 'DATE'}
		<input type="date" aria-label={field.name} bind:value={draft.date} />
	{:else if draft.kind === 'USER'}
		<SomeoneSelect
			label={field.name}
			bind:who={draft.who}
			{people}
			none="Nobody"
			labelled={false}
		/>
	{:else if draft.kind === 'USERS'}
		<PeopleSelect bind:who={draft.who} {people} />
	{:else if draft.kind === 'SECTIONS'}
		<SectionsEditor bind:items={draft.items} {members} />
	{:else if draft.kind === 'CHECKBOX'}
		<label class="inline">
			<input type="checkbox" bind:checked={draft.checked} />
			{field.name}
		</label>
		{#if draft.checked}
			<SomeoneSelect label="Checked for" bind:who={draft.for} {people} none="Me" />
		{/if}
	{:else if draft.kind === 'REFERENCE'}
		<label>URL <input type="url" bind:value={draft.url} /></label>
		<label>Label <input bind:value={draft.label} /></label>
	{:else if draft.kind === 'GIT'}
		<label>Repository <input bind:value={draft.repository} /></label>
		<label>Commit SHA <input bind:value={draft.sha} /></label>
		<label>Tag <input bind:value={draft.tag} /></label>
	{:else if draft.kind === 'DOCKER'}
		<label>Repository <input bind:value={draft.repository} /></label>
		<label>Digest <input bind:value={draft.digest} placeholder="sha256:…" /></label>
		<label>Tag <input bind:value={draft.tag} /></label>
	{:else if draft.kind === 'ATTACHMENT'}
		{#if draft.name}<p>{draft.name}</p>{/if}
		<label>
			{draft.id ? 'Replace' : 'Upload'}
			{field.name}
			<input
				type="file"
				accept={field.format === 'SBOM' ? 'application/json,.json' : undefined}
				onchange={attach}
				disabled={uploading}
			/>
		</label>
		{#if draft.id}
			<button type="button" onclick={() => (draft = { kind: 'ATTACHMENT', id: null, name: null })}>
				Remove {field.name}
			</button>
		{/if}
		{#if uploading}<p role="status">Uploading…</p>{/if}
		{#if uploadError}<p role="alert">{uploadError}</p>{/if}
	{/if}

	{#if field.description}<p class="hint" id="{id}-hint">{field.description}</p>{/if}
	{#if error}<p class="error" role="alert" id="{id}-error">{error}</p>{/if}
</fieldset>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	legend {
		margin-bottom: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.required {
		margin-left: 2px;
		color: var(--tone-danger-fg);
	}

	.field :global(label) {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.field :global(label.inline) {
		flex-direction: row;
		gap: var(--space-2);
		align-items: center;
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
