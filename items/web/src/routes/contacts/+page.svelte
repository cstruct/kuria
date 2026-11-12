<script lang="ts">
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import { AddContact, Contacts, EditContact, EraseContact } from '#lib/graphql/operations.ts';

	const client = getClient();
	const list = query(Contacts, () => ({}));
	setCrumbs(() => [{ label: 'Contacts' }]);

	let name = $state('');
	let email = $state('');
	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editEmail = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);

	async function done(result: Promise<{ error?: import('@urql/core').CombinedError }>) {
		busy = true;
		const { error } = await result;
		busy = false;
		refusal = error ? refusalOf(error) : null;
		await client.query(Contacts, {}, { requestPolicy: 'network-only' }).toPromise();
		return !error;
	}

	async function add(event: SubmitEvent) {
		event.preventDefault();
		if (await done(mutate(client, AddContact, { input: { name, email: email || null } }))) {
			name = '';
			email = '';
		}
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!editingId) return;
		const id = editingId;
		if (
			await done(
				mutate(client, EditContact, { input: { id, name: editName, email: editEmail || null } })
			)
		) {
			editingId = null;
		}
	}

	async function erase(id: string, shown: string) {
		if (!confirm(`Erase ${shown}'s name and email? Values naming them will show an identifier.`)) {
			return;
		}
		await done(mutate(client, EraseContact, { id }));
	}
</script>

<svelte:head>
	<title>Contacts · Kuria</title>
</svelte:head>

<main>
	<h1>Contacts</h1>
	<p>
		People you name before they are users. Once one signs in with that email, they show as the user.
	</p>
	{#if refusal}<p role="alert">{refusal.message}</p>{/if}

	<form onsubmit={add} aria-label="Add a contact">
		<label>Name <input bind:value={name} required /></label>
		<label>Email <input type="email" bind:value={email} /></label>
		<button type="submit" disabled={busy}>Add contact</button>
	</form>

	{#if list.error && !list.data}
		<p role="alert">{refusalOf(list.error).message}</p>
	{:else if list.data}
		<table>
			<thead>
				<tr
					><th scope="col">Name</th><th scope="col">Email</th><th scope="col">User</th><th
						scope="col"
					></th></tr
				>
			</thead>
			<tbody>
				{#each list.data.contacts as contact (contact.id)}
					<tr>
						{#if editingId === contact.id}
							<td colspan="4">
								<form onsubmit={save} aria-label="Edit {contact.displayName}">
									<label>Name <input bind:value={editName} required /></label>
									<label>Email <input type="email" bind:value={editEmail} /></label>
									<button type="submit" disabled={busy}>Save</button>
									<button type="button" onclick={() => (editingId = null)}>Cancel</button>
								</form>
							</td>
						{:else}
							<td>{contact.displayName}</td>
							<td>{contact.email ?? ''}</td>
							<td>{contact.user ? `Signed in as ${contact.user.name}` : 'Not yet'}</td>
							<td>
								{#if !contact.user}
									<button
										type="button"
										onclick={() => {
											editingId = contact.id;
											editName = contact.name ?? '';
											editEmail = contact.email ?? '';
										}}>Edit {contact.displayName}</button
									>
								{/if}
								<button
									type="button"
									disabled={busy}
									onclick={() => erase(contact.id, contact.displayName)}
									>Erase {contact.displayName}</button
								>
							</td>
						{/if}
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</main>

<style>
	form {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: end;
	}

	table {
		margin-top: 1.5rem;
		border-collapse: collapse;
	}

	th,
	td {
		padding: 0.25rem 0.75rem;
		text-align: start;
	}
</style>
