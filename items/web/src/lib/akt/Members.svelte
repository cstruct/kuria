<script lang="ts">
	// r[impl app.members]
	import { resolve } from '$app/paths';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import GripVertical from '@lucide/svelte/icons/grip-vertical';
	import X from '@lucide/svelte/icons/x';
	import type { AktPageQuery } from '#lib/graphql/generated/graphql.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Menu, { type MenuItem } from '#lib/ui/Menu.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { truncateMiddle } from '#lib/ui/text.ts';
	import { boardLabels } from './labels';
	import { groupMembers, mappedTypes, shownFields, transitionsInto } from './members';
	import { Reorder, type Place } from './reorder.svelte';

	type Columns = NonNullable<AktPageQuery['akt']>['board']['columns'];
	type Member = Columns[number]['akter'][number];

	interface Props {
		columns: Columns;
		/** The container's akt type document, which says what the board maps. */
		document: unknown;
		/** The akt types the container holds, in order. */
		contained: readonly string[];
		editable: boolean;
		busy: boolean;
		onMove: (number: string, place: Place) => void;
		onRemove: (number: string) => void;
		/** A member dropped on another column, with the transitions that
		 * lead there and the column's name. */
		onInto: (member: Member, transitions: Transition[], column: string) => void;
	}

	type Transition = Member['type']['transitions'][number];

	let { columns, document, contained, editable, busy, onMove, onRemove, onInto }: Props = $props();

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
		return [
			{
				label: labels.up,
				icon: ArrowUp,
				disabled: busy || i === 0,
				onSelect: () => onMove(member.number, { before: group[i - 1].number })
			},
			{
				label: labels.down,
				icon: ArrowDown,
				disabled: busy || i === group.length - 1,
				onSelect: () => onMove(member.number, { after: group[i + 1].number })
			},
			{
				label: labels.remove,
				icon: X,
				danger: true,
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
	<EmptyState title="No members yet" description="Add an existing akt, or create one here." />
{/if}

{#if grouped.columns.length > 0}
	<!-- Focusable, so a keyboard can scroll it sideways when its columns
	     hold nothing to focus. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div class="board" role="region" aria-label="Board" tabindex="0">
		{#each grouped.columns as column (column.key)}
			{@const numbers = column.akter.map((m) => m.number)}
			<section
				class="column"
				class:refuses={reorder.refuses(column.key)}
				class:takes={reorder.takes(column.key)}
				class:over={reorder.into === column.key}
				aria-label={column.name}
				title={reorder.refuses(column.key) ? 'No transition leads here from its state' : undefined}
			>
				<h3>{column.name} <span class="count">{column.akter.length}</span></h3>
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
							draggable={!busy}
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
								<Menu label={boardLabels(member.number).menu} items={itemsFor(column.akter, i)} />
							</div>
							{#if member.type.presentation.card.length > 0}
								<dl class="shown">
									{#each shownFields(member, 'card') as field (field.name)}
										{#if field.text}
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
								<Badge tone={member.state.tone}>{member.state.name}</Badge>
							</div>
						</li>
					{/each}
					{@render dropLine(column.key, column.akter.length)}
				</ol>
			</section>
		{/each}
	</div>
{/if}

{#each grouped.lists as list (list.type)}
	{@const group = `list:${list.type}`}
	{@const numbers = list.akter.map((m) => m.number)}
	{@const listFields = list.akter[0].type.presentation.list.length}
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
						<th scope="col">State</th>
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
							draggable={!busy}
							ondragstart={(e) => reorder.start(e, member.number, group)}
							ondragend={() => reorder.end()}
							ondragover={(e) => reorder.overMember(e, group, i)}
						>
							<td
								><a href={resolve('/akt/[number]', { number: member.number })}
									><Mono value={member.number} max={32} /></a
								></td
							>
							<td>{member.title ?? ''}</td>
							{#each shownFields(member, 'list') as field (field.name)}
								<td>{field.text}</td>
							{/each}
							<td><Badge tone={member.state.tone}>{member.state.name}</Badge></td>
							<td class="menu"
								><Menu
									label={boardLabels(member.number).menu}
									items={itemsFor(list.akter, i)}
								/></td
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
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	.shown div {
		display: flex;
		gap: var(--space-2);
	}

	.shown dd {
		margin: 0;
		color: var(--text);
		overflow-wrap: anywhere;
	}

	.foot {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-xs);
	}

	.foot :global(.badge) {
		margin-left: auto;
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
</style>
