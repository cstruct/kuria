<script lang="ts">
	import PeopleSelect from './PeopleSelect.svelte';
	import SectionsEditor from './SectionsEditor.svelte';
	import SomeoneSelect from './SomeoneSelect.svelte';
	import { legendOf } from './labels';
	import { upload } from './upload';
	import type { Person } from './people';
	import type { Draft, FieldDef } from './values';

	interface Props {
		field: FieldDef;
		draft: Draft;
		people: Person[];
		/** The field's refusal, if its value was refused. */
		error?: string;
		/** The akt's members, which a sections field places. */
		members?: readonly { number: string; title?: string | null }[];
	}

	let { field, draft = $bindable(), people, error, members = [] }: Props = $props();

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

<fieldset class="field" aria-describedby={error ? `${id}-error` : undefined}>
	<legend>{legendOf(field)}</legend>
	{#if field.description}<p class="hint">{field.description}</p>{/if}

	{#if draft.kind === 'TEXT'}
		{#if field.multiline}
			<textarea aria-label={field.name} rows="6" bind:value={draft.text}></textarea>
		{:else}
			<input aria-label={field.name} bind:value={draft.text} />
		{/if}
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
		<SomeoneSelect label={field.name} bind:who={draft.who} {people} none="Nobody" />
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

	{#if error}<p role="alert" id="{id}-error">{error}</p>{/if}
</fieldset>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		border: 1px solid color-mix(in srgb, currentColor 20%, transparent);
		border-radius: 0.25rem;
	}

	.inline {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.hint {
		margin: 0;
		opacity: 0.75;
	}
</style>
