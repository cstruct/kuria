<script lang="ts">
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import Avatar from '#lib/ui/Avatar.svelte';
	// r[impl app.members]
	import { resolve } from '$app/paths';
	import Archive from '@lucide/svelte/icons/archive';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import type { AktPageQuery } from '#lib/graphql/generated/graphql.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Menu, { type MenuItem } from '#lib/ui/Menu.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { articled, truncateMiddle } from '#lib/ui/text.ts';
	import { boardLabels } from './labels';
	import {
		groupMembers,
		mappedTypes,
		judgedType,
		showsState,
		shownFields,
		transitionsInto
	} from './members';
	import OutcomeBadge from './OutcomeBadge.svelte';
	import { Reorder, type Place } from './reorder.svelte';
	import VerdictButtons from './VerdictButtons.svelte';
	import { judgedOf, type Report } from './verdicts';

	type Columns = NonNullable<AktPageQuery['akt']>['board']['columns'];
	type Member = Columns[number]['akter'][number];

	interface Props {
		columns: Columns;
		/** The container's akt type document, which says what the board maps. */
		document: unknown;
		/** The akt types the container holds, in order. */
		contained: readonly string[];
		editable: boolean;
		/** Whether members may be reordered: not in an archived container. */
		movable?: boolean;
		busy: boolean;
		onMove: (number: string, place: Place) => void;
		onRemove: (number: string) => void;
		/** A member dropped on another column, with the transitions that
		 * lead there and the column's name. */
		onInto: (member: Member, transitions: Transition[], column: string) => void;
		/** The viewer's subject, for their own verdicts. */
		me?: string;
		/** Gives the viewer's verdict on a member, or takes it back. */
		onVerdict?: (member: Member, verdict: Report | null) => void;
		/** The akter a column offers to create, each with its type's key
		 * and name, the new-akt page that creates it (`app.board-create`),
		 * and whether a title alone makes one (`app.quick-add`). */
		creatable?: (column: string) => { key?: string; name: string; href: string; quick?: boolean }[];
		/** Whether verdicts on members may be given now (`app.verdicts`). */
		judging?: boolean;
		/** Creates an akt of a type, by key, from its title, on the board;
		 * answers whether it was made. */
		onQuickAdd?: (type: string, title: string) => Promise<boolean>;
	}

	type Transition = Member['type']['transitions'][number];

	let {
		columns,
		document,
		contained,
		editable,
		movable = true,
		busy,
		onMove,
		onRemove,
		onInto,
		me,
		onVerdict = () => {},
		creatable = () => [],
		onQuickAdd,
		judging = true
	}: Props = $props();

	/** What each column's quick-add has typed, and the type it adds. */
	let typed = $state<Record<string, string>>({});
	let adding = $state<Record<string, string>>({});
	let quickBusy = $state(false);

	/** Enter adds what was typed and stays for the next; Shift+Enter opens
	 * the whole form with it, to give more (`app.quick-add`). */
	// r[impl app.quick-add]
	async function quickKey(
		event: KeyboardEvent,
		column: string,
		offered: { key?: string; href: string }[]
	) {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		const title = (typed[column] ?? '').trim();
		const type = offered.find((o) => o.key === adding[column]) ?? offered[0];
		if (event.shiftKey) {
			location.assign(`${type.href}${title ? `&title=${encodeURIComponent(title)}` : ''}`);
			return;
		}
		if (!title || !type.key || !onQuickAdd || quickBusy) return;
		quickBusy = true;
		if (await onQuickAdd(type.key, title)) typed[column] = '';
		quickBusy = false;
	}

	const grouped = $derived(groupMembers(columns, mappedTypes(document), contained));
	const memberOf = (number: string) =>
		columns.flatMap((c) => c.akter).find((m) => m.number === number);
	const into = (number: string, column: string): Transition[] => {
		const member = memberOf(number);
		return member ? transitionsInto(document, column, member) : [];
	};
	const reorder = new Reorder(
		(number, place) => onMove(number, place),
		(number, column) => editable && into(number, column).length > 0,
		(number, column) => {
			const member = memberOf(number);
			const name = columns.find((c) => c.key === column)?.name ?? column;
			if (member) onInto(member, into(number, column), name);
		}
	);

	function itemsFor(group: readonly Member[], i: number): MenuItem[] {
		const member = group[i];
		const labels = boardLabels(member.number);
		// r[impl app.board-drop]: the moves a drop makes, by the column they go to.
		const moves: MenuItem[] = grouped.columns
			.filter((c) => c.key !== 'unmapped' && !c.akter.some((m) => m.number === member.number))
			.flatMap((c) => {
				const transitions = into(member.number, c.key);
				return transitions.length > 0
					? [
							{
								label: labels.into(c.name),
								icon: ArrowRight,
								disabled: busy || !editable,
								onSelect: () => onInto(member, transitions, c.name)
							}
						]
					: [];
			});
		return [
			{
				label: labels.up,
				icon: ArrowUp,
				disabled: busy || !movable || i === 0,
				onSelect: () => onMove(member.number, { before: group[i - 1].number })
			},
			{
				label: labels.down,
				icon: ArrowDown,
				disabled: busy || !movable || i === group.length - 1,
				onSelect: () => onMove(member.number, { after: group[i + 1].number })
			},
			...moves.map((m, k) => (k === 0 ? { ...m, separated: true } : m)),
			{
				label: labels.remove,
				icon: X,
				separated: true,
				disabled: busy || !editable,
				onSelect: () => onRemove(member.number)
			}
		];
	}

	function marker(group: string, index: number): boolean {
		return reorder.target?.group === group && reorder.target.index === index;
	}
</script>

{#snippet dropLine(group: string, index: number)}
	{#if marker(group, index)}<li class="drop-line" aria-hidden="true"></li>{/if}
{/snippet}

{#if grouped.columns.length === 0 && grouped.lists.length === 0}
	<EmptyState title="Nothing here yet" description="Add an existing item, or create one here." />
{/if}

<p id="quick-hint" class="visually-hidden">Enter adds it; Shift+Enter opens the form for more.</p>

{#if grouped.columns.length > 0}
	<!-- Focusable, so a keyboard can scroll it sideways when its columns
	     hold nothing to focus. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div class="board" role="region" aria-label="Board" tabindex="0">
		{#each grouped.columns as column (column.key)}
			{@const numbers = column.akter.map((m) => m.number)}
			{@const offered = column.key === 'unmapped' ? [] : creatable(column.key)}
			{@const quick = offered.filter((o) => o.quick && o.key)}
			<section
				class="column"
				class:refuses={reorder.refuses(column.key)}
				class:takes={reorder.takes(column.key)}
				class:over={reorder.into === column.key}
				aria-label={column.name}
			>
				<header class="column-head">
					<h3>{column.name} <span class="count">{column.akter.length}</span></h3>
					<!-- r[impl app.board-create] -->
					{#if offered.length === 1}
						<!-- eslint-disable svelte/no-navigation-without-resolve -- resolved, with a query -->
						<a
							class="create"
							href={offered[0].href}
							aria-label="New {offered[0].name} in {column.name}"
							title="New {offered[0].name} in {column.name}"
							><Plus size={14} aria-hidden={true} /></a
						>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					{:else if offered.length > 1}
						<Menu
							label="New in {column.name}"
							icon={Plus}
							items={offered.map((o) => ({
								label: `New ${o.name}`,
								icon: Plus,
								onSelect: () => location.assign(o.href)
							}))}
						/>
					{/if}
				</header>
				<ol
					ondragover={(e) => reorder.overGroup(e, column.key, numbers.length)}
					ondrop={(e) => reorder.drop(e, column.key, numbers)}
				>
					{#each column.akter as member, i (member.id)}
						{@render dropLine(column.key, i)}
						<li
							class="card"
							data-number={member.number}
							class:dragging={reorder.dragging?.number === member.number}
							draggable={!busy && movable}
							ondragstart={(e) => reorder.start(e, member.number, column.key)}
							ondragend={() => reorder.end()}
							ondragover={(e) => reorder.overMember(e, column.key, i)}
						>
							<div class="top">
								<a
									class="title"
									href={resolve('/akt/[number]', { number: member.number })}
									aria-label={member.title ? undefined : member.number}
									title={member.title ? undefined : member.number}
									>{member.title ?? truncateMiddle(member.number, 26)}</a
								>
								<!-- r[impl app.permitted]: nothing to do with it unless they may edit the container. -->
								{#if editable}
									<Menu label={boardLabels(member.number).menu} items={itemsFor(column.akter, i)} />
								{/if}
							</div>
							{#if shownFields(member, 'card').some((f) => f.text || f.picture)}
								<dl class="shown">
									{#each shownFields(member, 'card') as field (field.name)}
										{#if field.picture}
											<img class="picture" src={field.picture} alt={member.title ?? field.name} />
										{:else if field.text && field.as === 'person'}
											<div class="person" title="{field.name}: {field.text}">
												<dt class="visually-hidden">{field.name}</dt>
												<dd><Avatar name={field.text} />{field.text}</dd>
											</div>
										{:else if field.text && field.as === 'chip'}
											<div class="chip">
												<dt class="visually-hidden">{field.name}</dt>
												<dd><Badge>{field.text}</Badge></dd>
											</div>
										{:else if field.text}
											<div>
												<dt>{field.name}</dt>
												<dd>{field.text}</dd>
											</div>
										{/if}
									{/each}
								</dl>
							{/if}
							<div class="foot">
								<span class="grip" aria-hidden="true"><GripVertical size={12} /></span>
								{#if member.title}<Mono value={member.number} max={22} />{/if}
								<!-- r[impl app.archive]: an archived member marked, in words. -->
								{#if member.archived}<span class="archived"
										><Badge><Archive size={12} aria-hidden={true} /> Archived</Badge></span
									>{/if}
								{#if showsState(document, column.key, member.type.key) && !judgedType(member.type)}
									<Badge tone={member.state.tone}>{member.state.name}</Badge>
								{/if}
								{#if member.outcome}<OutcomeBadge outcome={member.outcome} />{/if}
							</div>
						</li>
					{/each}
					{@render dropLine(column.key, column.akter.length)}
				</ol>
				<!-- r[impl app.board-drop]: why a column refuses the card dragged over it. -->
				{#if reorder.refuses(column.key) && reorder.dragging}
					{@const dragged = memberOf(reorder.dragging.number)}
					<p class="refused" role="status">
						Can’t move here from {dragged?.state.name ?? 'its status'}
					</p>
				{/if}
				{#if editable && onQuickAdd && quick.length > 0}
					{@const chosen = quick.find((o) => o.key === adding[column.key]) ?? quick[0]}
					<div class="quick-add">
						{#if quick.length > 1}
							<select
								aria-label="What to add to {column.name}"
								value={chosen.key}
								onchange={(e) => (adding[column.key] = e.currentTarget.value)}
							>
								{#each quick as o (o.key)}<option value={o.key}>{o.name}</option>{/each}
							</select>
						{/if}
						<input
							aria-label="Add {articled(chosen.name.toLowerCase())} to {column.name}"
							aria-describedby="quick-hint"
							placeholder="Add {articled(chosen.name.toLowerCase())}…"
							bind:value={typed[column.key]}
							aria-busy={quickBusy || undefined}
							onkeydown={(e) => quickKey(e, column.key, [chosen, ...quick])}
						/>
					</div>
				{/if}
			</section>
		{/each}
	</div>
{/if}

{#each grouped.lists as list (list.type)}
	{@const group = `list:${list.type}`}
	{@const numbers = list.akter.map((m) => m.number)}
	{@const listFields = list.akter[0].type.presentation.list.length}
	{@const outcomes = list.akter.some((m) => m.outcome)}
	{@const withVerdicts = list.akter.some((m) => judgedOf(m, me))}
	<!-- r[impl app.members]: a judged member's state says nothing its outcome doesn't. -->
	{@const judgedList = judgedType(list.akter[0].type)}
	<section class="list" aria-label={list.name}>
		<h3>{list.name} <span class="count">{list.akter.length}</span></h3>
		<div class="scroll">
			<table>
				<thead>
					<tr>
						<th scope="col">Number</th>
						<th scope="col">Title</th>
						{#if listFields > 0}
							{#each shownFields(list.akter[0], 'list') as field (field.name)}
								<th scope="col">{field.name}</th>
							{/each}
						{/if}
						{#if !judgedList}<th scope="col">Status</th>{/if}
						{#if outcomes}<th scope="col">Outcome</th>{/if}
						{#if withVerdicts}<th scope="col">Verdicts</th>{/if}
						<th scope="col"><span class="visually-hidden">Actions</span></th>
					</tr>
				</thead>
				<tbody
					ondragover={(e) => reorder.overGroup(e, group, numbers.length)}
					ondrop={(e) => reorder.drop(e, group, numbers)}
				>
					{#each list.akter as member, i (member.id)}
						<tr
							data-number={member.number}
							class:dragging={reorder.dragging?.number === member.number}
							class:drop-before={marker(group, i)}
							class:drop-after={i === list.akter.length - 1 && marker(group, i + 1)}
							draggable={!busy && movable}
							ondragstart={(e) => reorder.start(e, member.number, group)}
							ondragend={() => reorder.end()}
							ondragover={(e) => reorder.overMember(e, group, i)}
						>
							<td
								><a href={resolve('/akt/[number]', { number: member.number })}
									><Mono value={member.number} max={32} /></a
								></td
							>
							<td
								>{member.title ?? ''}{#if member.archived}
									<Badge><Archive size={12} aria-hidden={true} /> Archived</Badge>{/if}</td
							>
							{#each shownFields(member, 'list') as field (field.name)}
								<td
									>{#if field.text && field.as === 'person'}<span class="person"
											><Avatar name={field.text} />{field.text}</span
										>{:else if field.text && field.as === 'chip'}<Badge>{field.text}</Badge
										>{:else}{field.text}{/if}</td
								>
							{/each}
							{#if !judgedList}<td><Badge tone={member.state.tone}>{member.state.name}</Badge></td
								>{/if}
							{#if outcomes}
								<td
									>{#if member.outcome}<OutcomeBadge outcome={member.outcome} />{/if}</td
								>
							{/if}
							{#if withVerdicts}
								{@const judged = judgedOf(member, me)}
								<td
									>{#if judged}<VerdictButtons
											number={member.number}
											{judged}
											open={judging}
											busy={busy || member.state.final === true}
											onGive={(verdict) => onVerdict(member, verdict)}
										/>{/if}</td
								>
							{/if}
							<td class="menu"
								>{#if editable}<Menu
										label={boardLabels(member.number).menu}
										items={itemsFor(list.akter, i)}
									/>{/if}</td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>
{/each}

<style>
	.board:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}

	.board {
		display: flex;
		gap: var(--space-3);
		overflow-x: auto;
		padding-bottom: var(--space-2);
	}

	.column {
		flex: 1 0 14rem;
		min-width: 0;
		max-width: 22rem;
		display: flex;
		flex-direction: column;
		padding: var(--space-3);
		border: 1px dashed transparent;
		border-radius: var(--radius-lg);
		background: var(--bg-subtle);
	}

	.column.refuses {
		border-color: var(--border-strong);
		opacity: 0.6;
	}

	.column.takes {
		border-color: var(--accent);
	}

	.column.over {
		border-style: solid;
		background: var(--accent-subtle);
	}

	h3 {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin: 0 0 var(--space-3);
		font-size: var(--text-sm);
	}

	.column-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: 2rem;
		margin-bottom: var(--space-3);
	}

	.column-head h3 {
		margin: 0;
	}

	.create {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: var(--radius-md);
		color: var(--text-muted);
	}

	.create:hover {
		background: var(--bg-muted);
		color: var(--text);
	}

	.count {
		color: var(--text-muted);
		font-weight: 400;
	}

	ol {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 3rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface);
		box-shadow: var(--shadow-sm);
		font-size: var(--text-sm);
		cursor: grab;
	}

	.card.dragging,
	tr.dragging {
		opacity: 0.4;
	}

	.drop-line {
		height: 2px;
		margin: -1px 0;
		border-radius: 1px;
		background: var(--accent);
	}

	.top {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.title {
		min-width: 0;
		padding-top: var(--space-1);
		color: var(--text);
		font-weight: 500;
		overflow-wrap: anywhere;
		text-decoration: none;
	}

	.title:hover {
		text-decoration: underline;
	}

	.shown {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	.shown div {
		display: flex;
		gap: var(--space-2);
		flex-basis: 100%;
	}

	.shown div.person,
	.shown div.chip {
		flex-basis: auto;
	}

	.shown .person dd,
	.person {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.picture {
		max-width: 100%;
		max-height: 8rem;
		object-fit: contain;
		border-radius: var(--radius-sm);
	}

	.shown dd {
		margin: 0;
		color: var(--text);
		overflow-wrap: anywhere;
	}

	.foot {
		display: flex;
		align-items: center;
		white-space: nowrap;
		gap: var(--space-2);
		font-size: var(--text-xs);
	}

	.foot :global(.badge) {
		margin-left: auto;
	}

	.foot .archived {
		margin-left: auto;
	}

	.foot .archived + :global(.badge) {
		margin-left: 0;
	}

	.grip {
		display: inline-flex;
		color: var(--text-muted);
	}

	.list {
		margin-top: var(--space-2);
	}

	.scroll {
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
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
		white-space: nowrap;
	}

	td {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		vertical-align: middle;
	}

	tbody tr:last-child td {
		border-bottom: none;
	}

	tr[draggable='true'] {
		cursor: grab;
	}

	tr.drop-before td {
		box-shadow: inset 0 2px 0 var(--accent);
	}

	tr.drop-after td {
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	td.menu {
		width: 1%;
		text-align: right;
	}

	.quick-add {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin-top: var(--space-2);
	}

	.quick-add input,
	.quick-add select {
		min-height: 2rem;
		background: var(--surface);
		font-size: var(--text-sm);
	}

	.quick-add input:not(:focus) {
		border-style: dashed;
	}

	.refused {
		margin-top: var(--space-2);
		padding: var(--space-2);
		border-radius: var(--radius-md);
		background: var(--bg-muted);
		color: var(--text-muted);
		font-size: var(--text-xs);
		text-align: center;
	}
</style>
