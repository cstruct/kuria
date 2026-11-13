<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ArrowRightLeft from '@lucide/svelte/icons/arrow-right-left';
	import X from '@lucide/svelte/icons/x';
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import FieldRow from '#lib/akt/FieldRow.svelte';
	import HistoryList from '#lib/akt/HistoryList.svelte';
	import TransitionDialog from '#lib/akt/TransitionDialog.svelte';
	import {
		availableTransitions,
		containedTypes,
		draftedFields,
		requiredFields
	} from '#lib/akt/document.ts';
	import { oneField, settle } from '#lib/akt/editing.ts';
	import { refusalOf, type Refusal } from '#lib/akt/errors.ts';
	import { actionsOf, headingOf } from '#lib/akt/header.ts';
	import { boardLabels } from '#lib/akt/labels.ts';
	import type { Person } from '#lib/akt/people.ts';
	import { pickable } from '#lib/akt/pickers.ts';
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
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { toast } from '#lib/ui/toast.svelte.ts';

	type Akt = NonNullable<AktPageQuery['akt']>;
	type Transition = Akt['type']['transitions'][number];
	type Member = Akt['board']['columns'][number]['akter'][number];

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
	const transitionNames = $derived(
		Object.fromEntries((akt?.type.transitions ?? []).map((t) => [t.key, t.name]))
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
	const actions = $derived(
		actionsOf(akt ? availableTransitions(akt.type.transitions, akt.state) : [])
	);
	const editable = $derived(Boolean(akt && !akt.state.final));

	// What the editors hold, and what they held when the akt was loaded.
	// Kept across refetches, so a refusal or a change made elsewhere never
	// loses what the person entered.
	let base = $state<Record<string, Draft>>({});
	let drafts = $state<Record<string, Draft>>({});
	let loadedFor = $state('');
	let editingField = $state<string | null>(null);
	let pending = $state<Transition | null>(null);
	let transitionOpen = $state(false);
	let reason = $state('');
	let refusal = $state<Refusal | null>(null);
	let busy = $state(false);
	let transferOpen = $state(false);
	let transferFrom = $state('');

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

	$effect(() => {
		if (akt && loadedFor !== akt.number) {
			base = snapshot(akt);
			drafts = snapshot(akt);
			loadedFor = akt.number;
			editingField = null;
		}
	});

	async function refetch(): Promise<Akt | undefined> {
		const result = await client
			.query(AktPage, { number }, { requestPolicy: 'network-only' })
			.toPromise();
		return result.data?.akt ?? undefined;
	}

	/** Runs a change; on success, reloads the editors from the result. A
	 * refusal shows where it belongs: in the open editor or dialog, else as a
	 * toast. */
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
		if (refusal && !editingField && !transitionOpen) toast(refusal.message, 'danger');
		return !error;
	}

	async function save(key: string) {
		if (!akt) return;
		const fields = oneField(key, base[key], drafts[key]);
		if (Object.keys(fields).length === 0) {
			editingField = null;
			return;
		}
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, EditAkt, { input: { number, revision, fields, reason: reason || null } })
			)
		) {
			editingField = null;
			reason = '';
		}
	}

	function cancelEdit(key: string) {
		drafts[key] = structuredClone($state.snapshot(base[key]));
		editingField = null;
		refusal = null;
		reason = '';
	}

	function required(transition: Transition): FieldDef[] {
		return requiredFields(akt?.type.fields ?? [], transition);
	}

	function begin(transition: Transition) {
		refusal = null;
		pending = transition;
		transitionOpen = true;
	}

	async function perform() {
		if (!akt || !pending) return;
		const transition = pending;
		const fields = changes(required(transition), base, drafts);
		const revision = akt.revision;
		if (
			await run(() =>
				mutate(client, PerformTransition, {
					input: { number, revision, transition: transition.key, fields, reason: reason || null }
				})
			)
		) {
			transitionOpen = false;
			pending = null;
			reason = '';
			toast(`${transition.name}: done`, 'success');
		}
	}

	async function draft(field: string) {
		if (!akt) return;
		const revision = akt.revision;
		if (await run(() => mutate(client, DraftField, { input: { number, revision, field } }))) {
			toast(`Drafted ${fieldNames[field] ?? field}`, 'success');
		}
	}

	async function join(container: string) {
		await run(() => mutate(client, AddToContainer, { input: { container, member: number } }));
	}

	async function leave(container: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container, member: number } }));
	}

	async function transfer(to: string) {
		if (!transferFrom) return;
		const from = transferFrom;
		if (await run(() => mutate(client, TransferMember, { input: { member: number, from, to } }))) {
			transferOpen = false;
			toast(`Moved from ${from} to ${to}`, 'success');
		}
	}

	async function addMember(member: string) {
		await run(() => mutate(client, AddToContainer, { input: { container: number, member } }));
	}

	async function removeMember(member: string) {
		await run(() => mutate(client, RemoveFromContainer, { input: { container: number, member } }));
	}

	async function move(member: string, place: { before?: string; after?: string }) {
		await run(() => mutate(client, MoveAkt, { input: { number: member, ...place } }));
	}

	function summary(member: Member): string {
		if (member.title) return member.title;
		const text = member.fields.find((v) => v.__typename === 'TextValue');
		return text ? textOf(text) : '';
	}
</script>

<svelte:head>
	<title>{akt ? `${headingOf(akt)} · ${akt.number}` : number} · Kuria</title>
</svelte:head>

<main class="akt">
	{#if view.error && !akt}
		<p role="alert">{refusalOf(view.error).message}</p>
	{:else if akt}
		<!-- r[impl app.header] -->
		<header class="head">
			<div class="identity">
				<p class="eyebrow">
					<a href={resolve('/types/[key]', { key: akt.type.key })}>{akt.type.name}</a>
					<span data-testid="number"><Mono value={akt.number} max={40} copy /></span>
				</p>
				<h1>{headingOf(akt)}</h1>
				<Badge tone={akt.state.tone}><span data-testid="state">{akt.state.name}</span></Badge>
			</div>
			{#if actions.primary}
				{@const primary = actions.primary}
				<div class="actions">
					{#each actions.others as transition (transition.key)}
						<Button onclick={() => begin(transition)}>{transition.name}</Button>
					{/each}
					<Button variant="primary" onclick={() => begin(primary)}>{primary.name}</Button>
				</div>
			{/if}
		</header>

		<div class="columns">
			<div class="main">
				<Card title="Details">
					<dl class="fields">
						{#each akt.type.fields as field (field.key)}
							<FieldRow
								{field}
								value={akt.fields.find((v) => 'field' in v && v.field === field.key)}
								bind:draft={drafts[field.key]}
								bind:reason
								{people}
								editing={editingField === field.key}
								{editable}
								drafted={drafted.includes(field.key)}
								{busy}
								error={editingField === field.key && refusal ? refusal.message : undefined}
								onEdit={() => {
									if (editingField) cancelEdit(editingField);
									editingField = field.key;
								}}
								onSave={() => save(field.key)}
								onCancel={() => cancelEdit(field.key)}
								onDraft={() => draft(field.key)}
							/>
						{/each}
					</dl>
				</Card>

				{#if contained.length > 0}
					<section class="members" aria-labelledby="members-title">
						<header>
							<h2 id="members-title">Members</h2>
							{#if editable}
								<Menu
									label="New member"
									text="New"
									items={contained.map((key) => ({
										label: `New ${typeNames[key] ?? key}`,
										onSelect: () => {
											location.assign(
												`${resolve('/types/[key]/new', { key })}?in=${encodeURIComponent(akt.number)}`
											);
										}
									}))}
								/>
							{/if}
						</header>
						{#if editable}
							<Combobox
								label="Add a member"
								placeholder="Add an existing akt by number or title…"
								search={(text) => pickable(client, text, [akt.number], contained)}
								onPick={(option) => addMember(option.value)}
							/>
						{/if}
						<div class="board">
							{#each akt.board.columns as column (column.key)}
								<section class="column" aria-label={column.name}>
									<h3>{column.name} <span class="count">{column.akter.length}</span></h3>
									<ol>
										{#each column.akter as member, i (member.id)}
											{@const labels = boardLabels(member.number)}
											<li class="card">
												<a href={resolve('/akt/[number]', { number: member.number })}
													>{member.number}</a
												>
												<span class="summary">{summary(member)}</span>
												<span class="foot">
													<Badge tone={member.state.tone}>{member.state.name}</Badge>
													<span class="tools">
														<IconButton
															label={labels.up}
															icon={ArrowUp}
															size="sm"
															disabled={busy || i === 0}
															onclick={() =>
																move(member.number, { before: column.akter[i - 1].number })}
														/>
														<IconButton
															label={labels.down}
															icon={ArrowDown}
															size="sm"
															disabled={busy || i === column.akter.length - 1}
															onclick={() =>
																move(member.number, { after: column.akter[i + 1].number })}
														/>
														<IconButton
															label={labels.remove}
															icon={X}
															size="sm"
															disabled={busy || !editable}
															onclick={() => removeMember(member.number)}
														/>
													</span>
												</span>
											</li>
										{/each}
									</ol>
								</section>
							{/each}
						</div>
					</section>
				{/if}
			</div>

			<aside class="side">
				<Card title="In">
					{#if akt.containers.length === 0}
						<p class="none">Not in any container.</p>
					{:else}
						<ul class="containers" aria-label="Containers">
							{#each akt.containers as container (container.id)}
								<li>
									<span>
										<a href={resolve('/akt/[number]', { number: container.number })}
											>{container.number}</a
										>
										<span class="hint">{container.title ?? container.type.name}</span>
									</span>
									<IconButton
										label="Remove from {container.number}"
										icon={X}
										size="sm"
										disabled={busy}
										onclick={() => leave(container.number)}
									/>
								</li>
							{/each}
						</ul>
					{/if}
					<div class="container-tools">
						<Combobox
							label="Add to a container"
							placeholder="Add to a container…"
							search={(text) =>
								pickable(client, text, [akt.number, ...akt.containers.map((c) => c.number)])}
							onPick={(option) => join(option.value)}
						/>
						{#if akt.containers.length > 0}
							<Button
								size="sm"
								onclick={() => {
									transferFrom = akt.containers[0].number;
									transferOpen = true;
								}}
							>
								<ArrowRightLeft size={14} aria-hidden={true} /> Move to another container
							</Button>
						{/if}
					</div>
				</Card>

				<Card title="History">
					<HistoryList revisions={akt.revisions} {fieldNames} {transitionNames} />
				</Card>
			</aside>
		</div>

		{#if pending}
			<TransitionDialog
				bind:open={transitionOpen}
				name={pending.name}
				fields={required(pending)}
				bind:drafts
				bind:reason
				{people}
				{refusal}
				{busy}
				onPerform={perform}
			/>
		{/if}

		<Dialog
			bind:open={transferOpen}
			title="Move to another container"
			description="One change, recorded on this akt and on both containers."
		>
			<label class="from">
				From
				<select bind:value={transferFrom}>
					{#each akt.containers as container (container.id)}
						<option value={container.number}>{container.number}</option>
					{/each}
				</select>
			</label>
			<Combobox
				label="To"
				placeholder="The container to move it to…"
				search={(text) =>
					pickable(client, text, [akt.number, ...akt.containers.map((c) => c.number)])}
				onPick={(option) => transfer(option.value)}
			/>
		</Dialog>
	{:else if view.data && !view.fetching}
		<p role="alert">There is no akt {number}, or you can’t see it.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	.akt {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		max-width: 80rem;
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4);
	}

	.identity {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		min-width: 0;
	}

	.eyebrow {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	h1 {
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(16rem, 22rem);
		gap: var(--space-5);
		align-items: start;
	}

	@media (max-width: 1099px) {
		.columns {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	.main,
	.side {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
		min-width: 0;
	}

	.fields {
		margin: 0;
	}

	.members {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.members > header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.board {
		display: flex;
		gap: var(--space-3);
		overflow-x: auto;
		padding-bottom: var(--space-2);
	}

	.column {
		flex: 0 0 15rem;
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--bg-subtle);
	}

	.column h3 {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}

	.count {
		color: var(--text-muted);
		font-weight: 400;
	}

	.column ol {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		box-shadow: var(--shadow-sm);
		font-size: var(--text-sm);
	}

	.card a {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.summary {
		overflow-wrap: anywhere;
	}

	.foot {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.tools {
		display: flex;
	}

	.containers {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0 0 var(--space-3);
		padding: 0;
		list-style: none;
	}

	.containers li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.containers li > span {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.hint {
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	.none {
		margin-bottom: var(--space-3);
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.container-tools {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
	}

	.container-tools :global(.combobox) {
		align-self: stretch;
	}

	.from {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}
</style>
