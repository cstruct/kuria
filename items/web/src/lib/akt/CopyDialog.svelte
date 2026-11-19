<script lang="ts">
	// r[impl app.copy]: an akt copied with the sections and members chosen,
	// all of them at first.
	import { SvelteSet } from 'svelte/reactivity';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { numbersIn, onlyOf, type Item } from './sections';

	interface Props {
		open: boolean;
		number: string;
		/** Its sections, if it has any. */
		items: Item[];
		/** Its direct members. */
		members: readonly { number: string; title?: string | null }[];
		busy: boolean;
		error?: string;
		/** Copies it, keeping only those given, or everything with `null`. */
		onCopy: (only: string[] | null) => void;
	}

	let { open = $bindable(), number, items, members, busy, error, onCopy }: Props = $props();

	const all = $derived(members.map((m) => m.number));
	const placed = $derived(new Set(numbersIn(items)));
	const unplaced = $derived(members.filter((m) => !placed.has(m.number)));
	const chosen = new SvelteSet<string>();

	$effect(() => {
		if (!open) return;
		chosen.clear();
		for (const n of all) chosen.add(n);
	});

	const titleOf = (n: string) => members.find((m) => m.number === n)?.title;
	const some = (numbers: string[]) => numbers.some((n) => chosen.has(n));
	const every = (numbers: string[]) => numbers.length > 0 && numbers.every((n) => chosen.has(n));

	function choose(numbers: string[], on: boolean) {
		for (const n of numbers) {
			if (on) chosen.add(n);
			else chosen.delete(n);
		}
	}
</script>

{#snippet memberChoice(n: string)}
	<li>
		<label class="inline">
			<input
				type="checkbox"
				checked={chosen.has(n)}
				onchange={(e) => choose([n], e.currentTarget.checked)}
			/>
			{titleOf(n) ?? n}
			{#if titleOf(n)}<span class="hint">{n}</span>{/if}
		</label>
	</li>
{/snippet}

{#snippet outline(within: Item[])}
	<ul class="choices">
		{#each within as item, i (i)}
			{#if 'akt' in item}
				{#if all.includes(item.akt)}{@render memberChoice(item.akt)}{/if}
			{:else}
				{@const inside = numbersIn(item.section.items).filter((n) => all.includes(n))}
				{@const title = item.section.title || 'Untitled section'}
				<li>
					<label class="inline section">
						<input
							type="checkbox"
							checked={every(inside)}
							indeterminate={some(inside) && !every(inside)}
							disabled={inside.length === 0}
							onchange={(e) => choose(inside, e.currentTarget.checked)}
						/>
						{title}
					</label>
					{@render outline(item.section.items)}
				</li>
			{/if}
		{/each}
	</ul>
{/snippet}

<Dialog
	bind:open
	title="Copy {number}"
	description="A new one, starting over, with copies of what you keep, in the same places."
>
	{#if members.length > 0}
		<fieldset class="keep">
			<legend>Keep</legend>
			<div class="all">
				<Button size="sm" variant="ghost" onclick={() => choose(all, true)}>All</Button>
				<Button size="sm" variant="ghost" onclick={() => choose(all, false)}>None</Button>
			</div>
			{@render outline(items)}
			{#if unplaced.length > 0}
				<ul class="choices">
					{#each unplaced as member (member.number)}
						{@render memberChoice(member.number)}
					{/each}
				</ul>
			{/if}
		</fieldset>
	{/if}
	{#if error}<p role="alert">{error}</p>{/if}
	{#snippet footer()}
		<Button variant="primary" {busy} onclick={() => onCopy(onlyOf(all, chosen))}>Copy</Button>
	{/snippet}
</Dialog>

<style>
	.keep {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		border: 0;
		margin: 0;
		padding: 0;
	}

	legend {
		font-weight: 500;
		font-size: var(--text-sm);
	}

	.all {
		display: flex;
		gap: var(--space-1);
	}

	.choices {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.choices .choices {
		padding-left: var(--space-5);
	}

	.inline {
		display: flex;
		gap: var(--space-2);
		align-items: center;
	}

	.section {
		font-weight: 600;
	}

	.hint {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}
</style>
