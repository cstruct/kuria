<script lang="ts">
	// r[impl app.sections]: the sections an akt lays its members out in,
	// edited as an outline. A member added to the akt is placed here too.
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import ListIndentDecrease from '@lucide/svelte/icons/list-indent-decrease';
	import ListIndentIncrease from '@lucide/svelte/icons/list-indent-increase';
	import ListPlus from '@lucide/svelte/icons/list-plus';
	import X from '@lucide/svelte/icons/x';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import {
		addItem,
		canIndent,
		editSection,
		indentItem,
		moveItem,
		numbersIn,
		outdentItem,
		removeItem,
		type Item,
		type Path
	} from './sections';

	interface Props {
		items: Item[];
		/** The akt's members, which the sections may place. */
		members: readonly { number: string; title?: string | null }[];
	}

	let { items = $bindable(), members }: Props = $props();

	const placed = $derived(new Set(numbersIn(items)));
	const unplaced = $derived(members.filter((m) => !placed.has(m.number)));
	const titleOf = (number: string) => members.find((m) => m.number === number)?.title;
	const nameOf = (item: Item) =>
		'akt' in item ? item.akt : item.section.title || 'the untitled section';
</script>

{#snippet tools(item: Item, path: Path, count: number)}
	{@const i = path[path.length - 1]}
	{@const name = nameOf(item)}
	<span class="tools">
		<IconButton
			label="Move {name} up"
			icon={ArrowUp}
			size="sm"
			disabled={i === 0}
			onclick={() => (items = moveItem(items, path, -1))}
		/>
		<IconButton
			label="Move {name} down"
			icon={ArrowDown}
			size="sm"
			disabled={i === count - 1}
			onclick={() => (items = moveItem(items, path, 1))}
		/>
		<IconButton
			label="Move {name} into the section above"
			icon={ListIndentIncrease}
			size="sm"
			disabled={!canIndent(items, path)}
			onclick={() => (items = indentItem(items, path))}
		/>
		<IconButton
			label="Move {name} out of its section"
			icon={ListIndentDecrease}
			size="sm"
			disabled={path.length < 2}
			onclick={() => (items = outdentItem(items, path))}
		/>
		<IconButton
			label={'akt' in item
				? `Take ${name} out of the sections`
				: `Remove ${name}, keeping what it holds`}
			icon={X}
			size="sm"
			onclick={() => (items = removeItem(items, path))}
		/>
	</span>
{/snippet}

{#snippet outline(list: Item[], at: Path)}
	<ol class="outline">
		{#each list as item, i (i)}
			{@const path = [...at, i]}
			<li>
				{#if 'akt' in item}
					<div class="member">
						<span
							>{item.akt}{#if titleOf(item.akt)}<span class="hint">
									· {titleOf(item.akt)}</span
								>{/if}</span
						>
						{@render tools(item, path, list.length)}
					</div>
				{:else}
					{@const section = item.section}
					<fieldset class="section">
						<legend class="visually-hidden">Section {section.title}</legend>
						<div class="section-head">
							<label class="grow">
								Title
								<input
									value={section.title}
									oninput={(e) =>
										(items = editSection(items, path, { title: e.currentTarget.value }))}
								/>
							</label>
							{@render tools(item, path, list.length)}
						</div>
						<label>
							Text <span class="hint">(Markdown, optional)</span>
							<textarea
								rows="2"
								value={section.text}
								oninput={(e) => (items = editSection(items, path, { text: e.currentTarget.value }))}
							></textarea>
						</label>
						<label class="inline">
							<input
								type="checkbox"
								checked={section.collapsible}
								onchange={(e) =>
									(items = editSection(items, path, { collapsible: e.currentTarget.checked }))}
							/>
							Collapsible, closed until opened
						</label>
						{#if section.items.length > 0}
							{@render outline(section.items, path)}
						{:else}
							<p class="hint">Empty: move items into it with the indent button.</p>
						{/if}
					</fieldset>
				{/if}
			</li>
		{/each}
	</ol>
{/snippet}

<div class="sections">
	{#if items.length > 0}
		{@render outline(items, [])}
	{:else}
		<p class="hint">No sections yet.</p>
	{/if}
	<div>
		<Button
			size="sm"
			onclick={() =>
				(items = addItem(items, {
					section: { title: '', text: '', collapsible: false, items: [] }
				}))}
		>
			<ListPlus size={14} aria-hidden={true} /> Add a section
		</Button>
	</div>
	{#if unplaced.length > 0}
		<div class="unplaced">
			<p>Not in a section</p>
			<ul>
				{#each unplaced as member (member.number)}
					<li>
						<span
							>{member.number}{#if member.title}<span class="hint">
									· {member.title}</span
								>{/if}</span
						>
						<Button size="sm" onclick={() => (items = addItem(items, { akt: member.number }))}
							>Place {member.number}</Button
						>
					</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.sections,
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.outline {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.section .outline {
		padding-left: var(--space-4);
	}

	.section {
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}

	.member,
	.section-head,
	.unplaced li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.section-head {
		align-items: flex-end;
	}

	.grow,
	label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.grow {
		flex: 1;
	}

	.inline {
		flex-direction: row;
		align-items: center;
		gap: var(--space-2);
	}

	.tools {
		display: flex;
		flex-shrink: 0;
		gap: var(--space-1);
	}

	.hint {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.unplaced ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.unplaced p {
		margin: 0 0 var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}
</style>
