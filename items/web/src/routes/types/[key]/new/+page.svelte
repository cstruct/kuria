<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import FieldEditor from '#lib/akt/FieldEditor.svelte';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import type { Person } from '#lib/akt/people.ts';
	import { draftOf, inputOf, isEmpty, type Draft } from '#lib/akt/values.ts';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import { Contacts, CreateAkt, NewAkt } from '#lib/graphql/operations.ts';

	const client = getClient();
	const key = $derived(page.params.key ?? '');
	/** The containers to create it in, from `?in=`. */
	const inside = $derived(page.url.searchParams.getAll('in'));
	const view = query(NewAkt, () => ({ type: key }));
	const contacts = query(Contacts, () => ({}));
	const type = $derived(view.data?.aktType ?? undefined);
	const people = $derived<Person[]>([
		...(view.data ? [{ who: { user: view.data.viewer.subject }, name: 'Me' }] : []),
		...(contacts.data?.contacts ?? []).map((c) => ({
			who: c.user ? { user: c.user.id } : { contact: c.id },
			name: c.displayName
		}))
	]);

	let drafts = $state<Record<string, Draft>>({});
	let preparedFor = $state('');
	let reason = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (type && preparedFor !== type.key) {
			drafts = Object.fromEntries(type.fields.map((f) => [f.key, draftOf(f, undefined)]));
			preparedFor = type.key;
		}
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (!type) return;
		const fields = Object.fromEntries(
			type.fields
				.filter((f) => drafts[f.key] && !isEmpty(drafts[f.key]))
				.map((f) => [f.key, inputOf(drafts[f.key])])
		);
		busy = true;
		const { data, error } = await mutate(client, CreateAkt, {
			input: { type: type.key, fields, in: inside, reason: reason || null }
		});
		busy = false;
		if (error || !data) {
			refusal = error
				? refusalOf(error, Object.fromEntries(type.fields.map((f) => [f.key, f.name])))
				: null;
			return;
		}
		await goto(resolve('/akt/[number]', { number: data.createAkt.number }));
	}
</script>

<svelte:head>
	<title>New {type?.name ?? key} · Kuria</title>
</svelte:head>

<main>
	{#if view.error && !type}
		<p role="alert">{refusalOf(view.error).message}</p>
	{:else if type}
		<h1>New {type.name}</h1>
		{#if inside.length > 0}<p>In {inside.join(', ')}</p>{/if}
		{#if refusal}<p role="alert">{refusal.message}</p>{/if}
		<form onsubmit={create} aria-label="New {type.name}">
			{#each type.fields as field (field.key)}
				{#if drafts[field.key]}
					<FieldEditor
						{field}
						bind:draft={drafts[field.key]}
						{people}
						error={refusal?.field === field.key ? refusal.message : undefined}
					/>
				{/if}
			{/each}
			<label>Reason <input bind:value={reason} /></label>
			<button type="submit" disabled={busy}>Create</button>
		</form>
	{:else if view.data}
		<p role="alert">There is no akt type {key}, or you can’t see it.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		max-width: 40rem;
	}
</style>
