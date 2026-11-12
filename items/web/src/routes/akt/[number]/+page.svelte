<script lang="ts">
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import FieldEditor from '#lib/akt/FieldEditor.svelte';
	import FieldShown from '#lib/akt/FieldShown.svelte';
	import {
		availableTransitions,
		containedTypes,
		draftedFields,
		requiredFields
	} from '#lib/akt/document.ts';
	import { settle } from '#lib/akt/editing.ts';
	import { boardLabels } from '#lib/akt/labels.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import type { Person } from '#lib/akt/people.ts';
	import { changes, draftOf, textOf, type Draft, type FieldDef } from '#lib/akt/values.ts';
	import { getClient, mutate, query } from '#lib/graphql/context.svelte.ts';
	import type { AktPageQuery } from '#lib/graphql/generated/graphql.ts';
	import {
		AddToContainer,
		AktPage,
		Contacts,
		DraftField,
		EditAkt,
		MoveAkt,
		PerformTransition,
		RemoveFromContainer,
		TransferMember
	} from '#lib/graphql/operations.ts';

	type Akt = NonNullable<AktPageQuery['akt']>;
	type Transition = Akt['type']['transitions'][number];

	const client = getClient();
	const number = $derived(page.params.number ?? '');
	const view = query(AktPage, () => ({ number }));
	const contacts = query(Contacts, () => ({}));

	const akt = $derived(view.data?.akt ?? undefined);
	setCrumbs(() =>
		akt
			? [
					{ label: akt.type.name, href: resolve('/types/[key]', { key: akt.type.key }) },
					{ label: akt.number }
				]
			: [{ label: number }]
	);
	const fieldNames = $derived(
		Object.fromEntries((akt?.type.fields ?? []).map((f) => [f.key, f.name]))
	);
	const people = $derived<Person[]>([
		...(view.data ? [{ who: { user: view.data.viewer.subject }, name: 'Me' }] : []),
		...(contacts.data?.contacts ?? []).map((c) => ({
			who: c.user ? { user: c.user.id } : { contact: c.id },
			name: c.displayName
		}))
	]);
	const typeNames = $derived(
		Object.fromEntries((view.data?.aktTypes ?? []).map((t) => [t.key, t.name]))
	);
	const contained = $derived(akt ? containedTypes(akt.type.document) : []);
	const drafted = $derived(akt ? draftedFields(akt.type.document) : []);
	const transitions = $derived<Transition[]>(
		akt ? availableTransitions(akt.type.transitions, akt.state) : []
	);

	// What the editors hold, and what they held when the akt was loaded: an
	// edit sends only what differs. Kept across refetches, so a refusal or a
	// change made elsewhere never loses what the person entered.
	let base = $state<Record<string, Draft>>({});
	let drafts = $state<Record<string, Draft>>({});
	let loadedFor = $state('');
	let editing = $state(false);
	let pending = $state<Transition | null>(null);
	let reason = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);
	let containerToJoin = $state('');
	let memberToAdd = $state('');
	let transferFrom = $state('');
	let transferTo = $state('');

	function snapshot(of: Akt): Record<string, Draft> {
		return Object.fromEntries(
			of.type.fields.map((field) => [
				field.key,
				draftOf(
					field,
					of.fields.find((v) => 'field' in v && v.field === field.key)
				)
			])
		);
	}

	function reset(of: Akt) {
		base = snapshot(of);
		drafts = snapshot(of);
		loadedFor = of.number;
	}

	$effect(() => {
		if (akt && loadedFor !== akt.number) reset(akt);
	});

	async function refetch(): Promise<Akt | undefined> {
		const result = await client
			.query(AktPage, { number }, { requestPolicy: 'network-only' })
			.toPromise();
		return result.data?.akt ?? undefined;
	}

	/** Runs a change; on success, reloads the editors from the result. */
	async function run(change: () => Promise<{ error?: import('@urql/core').CombinedError }>) {
		busy = true;
		refusal = null;
		const { error } = await change();
		const fresh = await refetch();
		busy = false;
		const settled = settle(error, fresh ? snapshot(fresh) : undefined, fieldNames);
		refusal = settled.refusal;
		if (settled.base) base = settled.base;
		if (settled.drafts) drafts = settled.drafts;
		if (fresh && !error) loadedFor = fresh.number;
		return !error;
	}

	async function save() {
		if (!akt) return;
		const fields = changes(akt.type.fields, base, drafts);
		if (Object.keys(fields).length === 0) {
			editing = false;
			return;
		}
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, EditAkt, {
					input: { number, revision, fields, reason: reason || null }
				})
			)
		) {
			editing = false;
			reason = '';
		}
	}

	function required(transition: Transition): FieldDef[] {
		return requiredFields(akt?.type.fields ?? [], transition);
	}

	async function perform(transition: Transition) {
		if (!akt) return;
		const fields = changes(required(transition), base, drafts);
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, PerformTransition, {
					input: {
						number,
						revision,
						transition: transition.key,
						fields,
						reason: reason || null
					}
				})
			)
		) {
			pending = null;
			reason = '';
		}
	}

	async function draft(field: string) {
		if (!akt) return;
		const revision = akt.revision;
		await run(() => mutate(client, DraftField, { input: { number, revision, field } }));
	}

	// The inputs below are cleared as a change starts, so the next number
	// can be typed while it runs, and restored if it is refused.
	async function join() {
		const container = containerToJoin.trim();
		if (!container) return;
		containerToJoin = '';
		if (
			!(await run(() => mutate(client, AddToContainer, { input: { container, member: number } })))
		) {
			containerToJoin = container;
		}
	}

	async function leave(container: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container, member: number } }));
	}

	async function transfer() {
		const to = transferTo.trim();
		if (!transferFrom || !to) return;
		const from = transferFrom;
		transferTo = '';
		if (
			await run(() =>
				mutate(client, TransferMember, {
					input: { member: number, from, to, reason: reason || null }
				})
			)
		) {
			reason = '';
		} else {
			transferTo = to;
		}
	}

	async function addMember() {
		const member = memberToAdd.trim();
		if (!member) return;
		memberToAdd = '';
		if (
			!(await run(() => mutate(client, AddToContainer, { input: { container: number, member } })))
		) {
			memberToAdd = member;
		}
	}

	async function removeMember(member: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container: number, member } }));
	}

	async function move(member: string, place: { before?: string; after?: string }) {
		await run(() => mutate(client, MoveAkt, { input: { number: member, ...place } }));
	}

	function summary(member: Akt['board']['columns'][number]['akter'][number]): string {
		const text = member.fields.find((v) => v.__typename === 'TextValue');
		return text ? textOf(text) : '';
	}
</script>

<svelte:head>
	<title>{number} · Kuria</title>
</svelte:head>

<main>
	{#if view.error && !akt}
		<p role="alert">{refusalOf(view.error).message}</p>
	{:else if akt}
		<header>
			<p>
				<a href={resolve('/types/[key]', { key: akt.type.key })}>{akt.type.name}</a>
			</p>
			<h1>{akt.number}</h1>
			<p>State: <strong data-testid="state">{akt.state.name}</strong></p>
		</header>

		{#if refusal}
			<p role="alert">{refusal.message}</p>
		{/if}

		<section aria-labelledby="fields-title">
			<h2 id="fields-title">Fields</h2>
			{#if editing}
				<form
					onsubmit={(e) => {
						e.preventDefault();
						save();
					}}
					aria-label="Edit fields"
				>
					{#each akt.type.fields as field (field.key)}
						<FieldEditor
							{field}
							bind:draft={drafts[field.key]}
							{people}
							error={refusal?.field === field.key ? refusal.message : undefined}
						/>
					{/each}
					<label>Reason <input bind:value={reason} /></label>
					<div class="actions">
						<button type="submit" disabled={busy}>Save</button>
						<button
							type="button"
							onclick={() => {
								editing = false;
								reset(akt);
							}}>Cancel</button
						>
					</div>
				</form>
			{:else}
				<dl>
					{#each akt.type.fields as field (field.key)}
						{@const value = akt.fields.find((v) => 'field' in v && v.field === field.key)}
						<dt>{field.name}</dt>
						<dd data-field={field.key}>
							{#if value}<FieldShown {value} />{:else}<span class="empty">None</span>{/if}
						</dd>
					{/each}
				</dl>
				{#if !akt.state.final}
					<div class="actions">
						<button type="button" onclick={() => (editing = true)}>Edit fields</button>
						{#each drafted as key (key)}
							<button type="button" disabled={busy} onclick={() => draft(key)}>
								Draft {fieldNames[key] ?? key}
							</button>
						{/each}
					</div>
				{/if}
			{/if}
		</section>

		{#if transitions.length > 0}
			<section aria-labelledby="transitions-title">
				<h2 id="transitions-title">Transitions</h2>
				{#if pending}
					{@const transition = pending}
					<form
						aria-label={transition.name}
						onsubmit={(e) => {
							e.preventDefault();
							perform(transition);
						}}
					>
						{#each required(transition) as field (field.key)}
							<FieldEditor
								{field}
								bind:draft={drafts[field.key]}
								{people}
								error={refusal?.field === field.key ? refusal.message : undefined}
							/>
						{/each}
						<label>Reason <input bind:value={reason} /></label>
						<div class="actions">
							<button type="submit" disabled={busy}>{transition.name}</button>
							<button type="button" onclick={() => (pending = null)}>Cancel</button>
						</div>
					</form>
				{:else}
					<div class="actions">
						{#each transitions as transition (transition.key)}
							<button type="button" onclick={() => (pending = transition)}>
								{transition.name}…
							</button>
						{/each}
					</div>
				{/if}
			</section>
		{/if}

		<section aria-labelledby="containers-title">
			<h2 id="containers-title">In</h2>
			{#if akt.containers.length === 0}
				<p>Not in any container.</p>
			{:else}
				<ul aria-label="Containers">
					{#each akt.containers as container (container.id)}
						<li>
							<a href={resolve('/akt/[number]', { number: container.number })}>
								{container.number}
							</a>
							({container.type.name})
							<button type="button" disabled={busy} onclick={() => leave(container.number)}>
								Remove from {container.number}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
			<form
				aria-label="Add to a container"
				onsubmit={(e) => {
					e.preventDefault();
					join();
				}}
			>
				<label>Container number <input bind:value={containerToJoin} /></label>
				<button type="submit" disabled={busy}>Add to container</button>
			</form>
			{#if akt.containers.length > 0}
				<form
					aria-label="Transfer to another container"
					onsubmit={(e) => {
						e.preventDefault();
						transfer();
					}}
				>
					<label>
						From
						<select bind:value={transferFrom}>
							<option value="">Choose…</option>
							{#each akt.containers as container (container.id)}
								<option value={container.number}>{container.number}</option>
							{/each}
						</select>
					</label>
					<label>To container number <input bind:value={transferTo} /></label>
					<button type="submit" disabled={busy}>Transfer</button>
				</form>
			{/if}
		</section>

		{#if contained.length > 0}
			<section aria-labelledby="members-title">
				<h2 id="members-title">Members</h2>
				<div class="actions">
					{#each contained as key (key)}
						<!-- A resolved route, with the container to create it in. -->
						<!-- eslint-disable svelte/no-navigation-without-resolve -->
						<a
							href={`${resolve('/types/[key]/new', { key })}?in=${encodeURIComponent(akt.number)}`}
						>
							New {typeNames[key] ?? key} here
						</a>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					{/each}
				</div>
				<form
					aria-label="Add a member"
					onsubmit={(e) => {
						e.preventDefault();
						addMember();
					}}
				>
					<label>Member number <input bind:value={memberToAdd} /></label>
					<button type="submit" disabled={busy}>Add member</button>
				</form>
				<div class="board">
					{#each akt.board.columns as column (column.key)}
						<section class="column" aria-label={column.name}>
							<h3>{column.name}</h3>
							<ol>
								{#each column.akter as member, i (member.id)}
									<li>
										<a href={resolve('/akt/[number]', { number: member.number })}>
											{member.number}
										</a>
										<span>{summary(member)}</span>
										<span class="state">{member.state.name}</span>
										<span class="actions">
											<button
												type="button"
												aria-label={boardLabels(member.number).up}
												disabled={busy || i === 0}
												onclick={() => move(member.number, { before: column.akter[i - 1].number })}
												>↑</button
											>
											<button
												type="button"
												aria-label={boardLabels(member.number).down}
												disabled={busy || i === column.akter.length - 1}
												onclick={() => move(member.number, { after: column.akter[i + 1].number })}
												>↓</button
											>
											<button
												type="button"
												aria-label={boardLabels(member.number).remove}
												disabled={busy}
												onclick={() => removeMember(member.number)}>✕</button
											>
										</span>
									</li>
								{/each}
							</ol>
						</section>
					{/each}
				</div>
			</section>
		{/if}

		<section aria-labelledby="history-title">
			<h2 id="history-title">History</h2>
			<ol reversed aria-label="Revisions">
				{#each [...akt.revisions].reverse() as revision (revision.revision)}
					<li>
						<strong>{revision.by.name}</strong>
						{#if revision.transition}performed {revision.transition}{/if}
						{#if revision.membership}changed memberships{/if}
						{#if Object.keys((revision.changes as object) ?? {}).length > 0}
							set {Object.keys(revision.changes as object)
								.map((k) => fieldNames[k] ?? k)
								.join(', ')}
						{/if}
						{#if revision.reason}: “{revision.reason}”{/if}
						<time datetime={revision.createdAt}
							>{new Date(revision.createdAt).toLocaleString()}</time
						>
					</li>
				{/each}
			</ol>
		</section>
	{:else if view.data && !view.fetching}
		<p role="alert">There is no akt {number}, or you can’t see it.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	section {
		margin-block: 1.5rem;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		max-width: 40rem;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}

	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.25rem 1rem;
	}

	dd {
		margin: 0;
	}

	.empty,
	.state {
		opacity: 0.7;
	}

	.board {
		display: flex;
		gap: 1rem;
		overflow-x: auto;
	}

	.column {
		min-width: 14rem;
		margin: 0;
	}

	.column ol {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0;
		list-style: none;
	}

	.column li {
		display: flex;
		flex-direction: column;
		padding: 0.5rem;
		border: 1px solid color-mix(in srgb, currentColor 20%, transparent);
		border-radius: 0.25rem;
	}
</style>
