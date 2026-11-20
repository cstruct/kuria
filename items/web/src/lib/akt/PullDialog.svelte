<script lang="ts">
	// r[impl app.pull]: a parent board's members, several picked at once.
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { ParentBoard } from '#lib/graphql/operations.ts';
	import { offeredFrom } from './pulling';

	interface Props {
		open: boolean;
		/** The parent board, to pull from. */
		parent: { number: string; title: string };
		/** The container pulled into: its number and type's key, and the
		 * types it holds. */
		here: string;
		type: string;
		contained: readonly string[];
		busy: boolean;
		onPull: (numbers: string[]) => void;
	}

	let { open = $bindable(), parent, here, type, contained, busy, onPull }: Props = $props();

	const board = query(ParentBoard, () => ({ number: parent.number }));
	const offered = $derived(
		offeredFrom(board.data?.akt?.board.columns ?? [], here, type, contained)
	);
	let chosen = $state<string[]>([]);
	$effect(() => {
		if (open) chosen = [];
	});
</script>

<Dialog bind:open title="Add from {parent.title}" description="Choose what to add here.">
	{#if board.fetching && !board.data}
		<p role="status">Loading…</p>
	{:else if offered.length === 0}
		<p>{parent.title} has nothing to add.</p>
	{:else}
		<form
			id="pull-form"
			aria-label="Add from {parent.title}"
			onsubmit={(e) => {
				e.preventDefault();
				onPull(chosen);
			}}
		>
			{#each offered as column (column.key)}
				<fieldset>
					<legend>{column.name}</legend>
					{#each column.akter as akt (akt.number)}
						<label class="choice" class:muted={akt.here || akt.finished}>
							<input type="checkbox" value={akt.number} bind:group={chosen} disabled={akt.here} />
							<span class="what">{akt.title}</span>
							{#if akt.here}<Badge>Already here</Badge>
							{:else if akt.elsewhere}<Badge tone="WARNING">In {akt.elsewhere}</Badge>{/if}
						</label>
					{/each}
				</fieldset>
			{/each}
		</form>
	{/if}
	{#snippet footer()}
		<Button variant="primary" type="submit" form="pull-form" disabled={chosen.length === 0} {busy}
			>Add {chosen.length || ''}</Button
		>
	{/snippet}
</Dialog>

<style>
	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	legend {
		margin-bottom: var(--space-1);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.choice {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) 0;
		font-size: var(--text-sm);
	}

	.what {
		flex: 1;
		min-width: 0;
	}

	.muted .what {
		color: var(--text-muted);
	}
</style>
