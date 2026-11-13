<script lang="ts">
	import Link from '@lucide/svelte/icons/link';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Trash from '@lucide/svelte/icons/trash-2';
	import { userOptions } from '#lib/akt/contacts.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import {
		AddContact,
		Contacts,
		EditContact,
		EraseContact,
		LinkContact,
		Users
	} from '#lib/graphql/operations.ts';
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { toast } from '#lib/ui/toast.svelte.ts';

	const client = getClient();
	const list = query(Contacts, () => ({}));
	const users = query(Users, () => ({}));
	setCrumbs(() => [{ label: 'Contacts' }]);

	type Contact = NonNullable<typeof list.data>['contacts'][number];

	let name = $state('');
	let email = $state('');
	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editEmail = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);
	let linking = $state<Contact | null>(null);
	let linkOpen = $state(false);

	const linked = $derived(
		new Set((list.data?.contacts ?? []).flatMap((c) => (c.user ? [c.user.id] : [])))
	);

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

	// r[impl contact.link-by-hand]: for people whose email Konklav never
	// verified, such as those who sign in with a password.
	async function link(user: { value: string; label: string }) {
		if (!linking) return;
		const contact = linking;
		if (
			await done(mutate(client, LinkContact, { input: { contact: contact.id, user: user.value } }))
		) {
			linkOpen = false;
			toast(`${contact.displayName} is ${user.label}`, 'success');
		}
	}
</script>

<svelte:head>
	<title>Contacts · Kuria</title>
</svelte:head>

<main class="contacts">
	<header>
		<h1>Contacts</h1>
		<p class="lead">
			People you name before they are users. When one signs in with a verified email, or you link
			them to the user they are, they show as that user.
		</p>
	</header>
	{#if refusal && !linkOpen}<p role="alert" class="refusal">{refusal.message}</p>{/if}

	<Card title="Add a contact">
		<form class="add" onsubmit={add} aria-label="Add a contact">
			<label>Name <input bind:value={name} required /></label>
			<label>Email <input type="email" bind:value={email} /></label>
			<Button type="submit" variant="primary" {busy}>Add contact</Button>
		</form>
	</Card>

	{#if list.error && !list.data}
		<p role="alert">{refusalOf(list.error).message}</p>
	{:else if list.data && list.data.contacts.length === 0}
		<EmptyState title="No contacts yet" description="Add one to name them in a field." />
	{:else if list.data}
		<div class="scroll">
			<table aria-label="Contacts">
				<thead>
					<tr>
						<th scope="col">Name</th>
						<th scope="col">Email</th>
						<th scope="col">User</th>
						<th scope="col"><span class="visually-hidden">Actions</span></th>
					</tr>
				</thead>
				<tbody>
					{#each list.data.contacts as contact (contact.id)}
						<tr>
							{#if editingId === contact.id}
								<td colspan="4">
									<form class="add" onsubmit={save} aria-label="Edit {contact.displayName}">
										<label>Name <input bind:value={editName} required /></label>
										<label>Email <input type="email" bind:value={editEmail} /></label>
										<Button type="submit" variant="primary" size="sm" {busy}>Save</Button>
										<Button size="sm" variant="ghost" onclick={() => (editingId = null)}
											>Cancel</Button
										>
									</form>
								</td>
							{:else}
								<td>{contact.displayName}</td>
								<td>{contact.email ?? ''}</td>
								<td>
									{#if contact.user}
										<span class="user">
											Signed in as {contact.user.name}
											{#if contact.linkedBy}<span class="hint"
													>linked by {contact.linkedBy.name}</span
												>{/if}
										</span>
									{:else}
										<span class="hint">Not yet</span>
									{/if}
								</td>
								<td class="menu">
									<Menu
										label="Actions for {contact.displayName}"
										items={[
											...(contact.user
												? []
												: [
														{
															label: `Edit ${contact.displayName}`,
															icon: Pencil,
															onSelect: () => {
																editingId = contact.id;
																editName = contact.name ?? '';
																editEmail = contact.email ?? '';
															}
														},
														{
															label: `Link ${contact.displayName} to a user…`,
															icon: Link,
															disabled: !users.data,
															onSelect: () => {
																refusal = null;
																linking = contact;
																linkOpen = true;
															}
														}
													]),
											{
												label: `Erase ${contact.displayName}`,
												icon: Trash,
												danger: true,
												separated: !contact.user,
												disabled: busy,
												onSelect: () => erase(contact.id, contact.displayName)
											}
										]}
									/>
								</td>
							{/if}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	{#if linking}
		{@const contact = linking}
		<Dialog
			bind:open={linkOpen}
			title="Link {contact.displayName} to a user"
			description="Values naming {contact.displayName} will show the user. A link can't be undone."
		>
			{#if refusal}<p role="alert" class="refusal">{refusal.message}</p>{/if}
			<Combobox
				label="User"
				placeholder="Name or email…"
				empty="No user who signed in here matches."
				search={async (text) => userOptions(users.data?.users ?? [], text, linked)}
				onPick={link}
			/>
		</Dialog>
	{/if}
</main>

<style>
	.contacts {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 64rem;
	}

	h1 {
		margin: 0 0 var(--space-2);
	}

	.lead {
		margin: 0;
		color: var(--text-muted);
	}

	.refusal {
		color: var(--tone-danger-fg);
	}

	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: var(--space-3);
	}

	.add label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.scroll {
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	th {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 500;
		text-align: left;
	}

	td {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		vertical-align: middle;
	}

	tbody tr:last-child td {
		border-bottom: none;
	}

	.user {
		display: flex;
		flex-direction: column;
	}

	.hint {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	td.menu {
		width: 1%;
		text-align: right;
	}
</style>
