<script lang="ts">
	// r[impl app.verdicts]: a member's assignees and their verdicts, and an
	// assignee's own verdict given, changed or taken back in place.
	import type { Verdict } from '#lib/graphql/generated/graphql.ts';
	import Button from '#lib/ui/Button.svelte';
	import { VERDICTS, type Judged } from './verdicts';

	interface Props {
		/** The akt it is the verdicts of, for labels. */
		number: string;
		judged: Judged;
		busy: boolean;
		/** Gives one's own verdict, or takes it back with `null`. */
		onGive: (verdict: Verdict | null) => void;
	}

	let { number, judged, busy, onGive }: Props = $props();

	const said = (verdict: Verdict | null) =>
		verdict ? (VERDICTS.find((v) => v.verdict === verdict)?.label ?? verdict) : 'Not yet';
</script>

<div class="verdicts">
	{#if judged.assignees.length === 0}
		<span class="hint">No one assigned</span>
	{:else}
		<ul class="assignees" aria-label="Verdicts on {number}">
			{#each judged.assignees as assignee (assignee.id)}
				<li>
					<span>{assignee.name}</span>
					<span class="verdict {assignee.verdict?.toLowerCase() ?? 'none'}"
						>{said(assignee.verdict)}</span
					>
				</li>
			{/each}
		</ul>
	{/if}
	{#if judged.mine !== undefined}
		<div class="mine" role="group" aria-label="Your verdict on {number}">
			{#each VERDICTS as { verdict, label } (verdict)}
				<Button
					size="sm"
					variant={judged.mine === verdict ? 'primary' : 'secondary'}
					aria-pressed={judged.mine === verdict}
					disabled={busy}
					onclick={() => onGive(judged.mine === verdict ? null : verdict)}>{label}</Button
				>
			{/each}
			{#if judged.mine}
				<Button size="sm" variant="ghost" disabled={busy} onclick={() => onGive(null)}
					>Take back</Button
				>
			{/if}
		</div>
	{/if}
</div>

<style>
	.verdicts {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
	}

	.assignees {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: var(--text-sm);
	}

	.assignees li {
		display: flex;
		gap: var(--space-1);
	}

	.verdict {
		font-weight: 600;
	}

	.verdict.pass {
		color: var(--tone-success-fg);
	}

	.verdict.fail {
		color: var(--tone-danger-fg);
	}

	.verdict.skip {
		color: var(--tone-warning-fg);
	}

	.verdict.none,
	.hint {
		color: var(--text-muted);
	}

	.mine {
		display: flex;
		gap: var(--space-1);
	}
</style>
