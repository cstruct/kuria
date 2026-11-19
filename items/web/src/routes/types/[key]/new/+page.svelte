<script lang="ts">
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import FieldEditor from '#lib/akt/FieldEditor.svelte';
	import { missingRequired, titleFieldOf } from '#lib/akt/creating.ts';
	import { toast } from '#lib/ui/toast.svelte.ts';
	import Alert from '#lib/ui/Alert.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import { releasedFile } from '#lib/akt/document.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import { creatorDefaults, peopleOf, type Person } from '#lib/akt/people.ts';
	import { draftOf, inputOf, isEmpty, type Draft } from '#lib/akt/values.ts';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import {
		AddLink,
		Contacts,
		CreateAkt,
		NewAkt,
		ParentBoard,
		Users
	} from '#lib/graphql/operations.ts';

	const client = getClient();
	const key = $derived(page.params.key ?? '');
	/** The containers to create it in, from `?in=`. */
	const inside = $derived(page.url.searchParams.getAll('in'));
	/** Whether to go back to the container once it is made, as from its
	 * board (`app.create`). */
	const returning = $derived(page.url.searchParams.has('return') && inside.length > 0);
	const view = query(NewAkt, () => ({ type: key }));
	const contacts = query(Contacts, () => ({}));
	const users = query(Users, () => ({}));
	const type = $derived(view.data?.aktType ?? undefined);
	setCrumbs(() => [
		{ label: type?.name ?? key, href: resolve('/types/[key]', { key }) },
		{ label: 'New' }
	]);
	const people = $derived<Person[]>(
		peopleOf(
			view.data?.viewer.subject,
			users.data?.users,
			contacts.data?.contacts,
			view.data?.viewer.user
		)
	);

	let drafts = $state<Record<string, Draft>>({});
	let preparedFor = $state('');
	let reason = $state('');
	let refusal = $state<Refusal | null>(null);
	/** The required fields left empty when it was last sent, by key. */
	let missing = $state<Record<string, string>>({});
	let busy = $state(false);

	/** The fields asked for here: verdicts are given, and sections laid
	 * out, once it exists; a released file, by releasing it (`version.file`). */
	const asked = $derived(
		(type?.fields ?? []).filter(
			(f) =>
				drafts[f.key] &&
				!['SHOWN', 'SECTIONS'].includes(drafts[f.key].kind) &&
				f.key !== releasedFile(type?.document)
		)
	);
	/** The link kind the type names as its board's parent, if any
	 * (`board.parent`): asked for here, as `app.pull` needs it. */
	const parentKind = $derived.by(() => {
		const document = type?.document as {
			board?: { parent?: string };
			links?: { key: string; name: string; to: string[] }[];
		} | null;
		const kind = document?.board?.parent;
		return kind ? (document?.links?.find((l) => l.key === kind) ?? null) : null;
	});
	/** What the container it is made in holds that it could ship from. */
	const container = query(ParentBoard, () => ({ number: inside[0] ?? '' }));
	const parentChoices = $derived(
		parentKind
			? (container.data?.akt?.board.columns ?? [])
					.flatMap((c) => c.akter)
					.filter((m) => !m.state.final && parentKind.to.includes(m.type.key))
			: []
	);
	let parentChosen = $state<string | null>(null);
	$effect(() => {
		if (parentChosen === null && parentChoices.length > 0) parentChosen = parentChoices[0].number;
	});

	/** Where Cancel goes: the container it was opened from, else its type. */
	const back = $derived(
		inside[0] ? resolve('/akt/[number]', { number: inside[0] }) : resolve('/types/[key]', { key })
	);

	$effect(() => {
		if (type && preparedFor !== type.key) {
			drafts = Object.fromEntries(type.fields.map((f) => [f.key, draftOf(f, undefined)]));
			// A title typed on a board before asking for the whole form.
			const titled = titleFieldOf(type.document);
			const given = page.url.searchParams.get('title');
			if (titled && given && drafts[titled]?.kind === 'TEXT') {
				drafts[titled] = { kind: 'TEXT', text: given };
			}
			// r[impl akt.field.default]: offered as its creator, to keep or change.
			const me = view.data?.viewer.subject;
			for (const key of creatorDefaults(type.document)) {
				if (me && drafts[key]?.kind === 'USER') drafts[key] = { kind: 'USER', who: { user: me } };
			}
			preparedFor = type.key;
		}
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (!type) return;
		// r[impl app.create]: nothing is sent while a required field is empty.
		missing = missingRequired(asked, drafts);
		if (Object.keys(missing).length > 0) {
			refusal = null;
			const first = asked.find((f) => missing[f.key]);
			document
				.querySelector<HTMLElement>(
					`[data-field-editor="${first?.key}"] :is(input, select, textarea, [contenteditable])`
				)
				?.focus();
			return;
		}
		const fields = Object.fromEntries(
			type.fields
				.filter((f) => drafts[f.key] && drafts[f.key].kind !== 'SHOWN' && !isEmpty(drafts[f.key]))
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
		// r[impl app.pull]: linked to the parent board chosen with it.
		if (parentKind && parentChosen) {
			await mutate(client, AddLink, {
				input: { source: data.createAkt.number, kind: parentKind.key, target: parentChosen }
			});
		}
		if (returning) {
			// r[impl app.create]: back to the board it was made from.
			toast(`Added ${data.createAkt.title ?? data.createAkt.number}`, 'success');
			await goto(back);
			return;
		}
		await goto(resolve('/akt/[number]', { number: data.createAkt.number }));
	}
</script>

<svelte:head>
	<title>New {type?.name ?? key} · Kuria</title>
</svelte:head>

<main class="create">
	{#if view.error && !type}
		<Alert>{refusalOf(view.error).message}</Alert>
	{:else if type}
		<header>
			<h1>New {type.name}</h1>
			{#if inside.length > 0}<p class="inside">In {inside.join(', ')}</p>{/if}
		</header>
		<form onsubmit={create} aria-label="New {type.name}" novalidate>
			<Card>
				<div class="fields">
					{#if refusal}<Alert title="It wasn’t created">{refusal.message}</Alert>{/if}
					{#each asked as field (field.key)}
						<div data-field-editor={field.key}>
							<FieldEditor
								{field}
								bind:draft={drafts[field.key]}
								{people}
								error={missing[field.key] ??
									(refusal?.field === field.key ? refusal.message : undefined)}
							/>
						</div>
					{/each}
					{#if parentKind && parentChoices.length > 0}
						<label class="parent">
							{parentKind.name}
							<select bind:value={parentChosen}>
								{#each parentChoices as choice (choice.number)}
									<option value={choice.number}>{choice.title ?? choice.number}</option>
								{/each}
								<option value={null}>None</option>
							</select>
						</label>
					{/if}
					<details class="note">
						<summary>Add a note for the history</summary>
						<label>
							Note <span class="optional">(optional)</span>
							<input bind:value={reason} />
						</label>
					</details>
				</div>
			</Card>
			<div class="actions">
				<Button variant="ghost" href={back}>Cancel</Button>
				<Button variant="primary" type="submit" {busy}>Create</Button>
			</div>
		</form>
	{:else if view.data}
		<Alert>There is nothing called {key} here, or you can’t see it.</Alert>
	{:else}
		<p role="status">Loading…</p>
	{/if}
</main>

<style>
	.create {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 44rem;
	}

	header {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.inside {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.parent {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.note summary {
		cursor: pointer;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.note label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin-top: var(--space-2);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.optional {
		color: var(--text-muted);
		font-weight: 400;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}
</style>
