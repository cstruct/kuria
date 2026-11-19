<script lang="ts">
	// r[impl app.verdicts]: a member's assignees, their verdicts and what
	// they saw, and an assignee's own reported, changed or deleted in a
	// dialog.
	import type { Verdict } from '#lib/graphql/generated/graphql.ts';
	import Button from '#lib/ui/Button.svelte';
	import VerdictDialog from './VerdictDialog.svelte';
	import { VERDICTS, type Judged, type Report } from './verdicts';

	interface Props {
		/** The akt it is the verdicts of, for labels. */
		number: string;
		judged: Judged;
		busy: boolean;
		/** Reports one's own verdict, or deletes it with `null`. */
		onGive: (report: Report | null) => void;
	}

	let { number, judged, busy, onGive }: Props = $props();

	let open = $state(false);

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
					{#if assignee.description}
						<span class="description">{assignee.description}</span>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
	{#if judged.mine !== undefined}
		<div class="mine" role="group" aria-label="Your verdict on {number}">
			<Button size="sm" disabled={busy} onclick={() => (open = true)}
				>{judged.mine ? 'Edit' : 'Report'}</Button
			>
		</div>
		<VerdictDialog bind:open {number} mine={judged.mine} {busy} onSave={onGive} />
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
	.hint,
	.description {
		color: var(--text-muted);
	}

	.description {
		white-space: pre-line;
	}

	.mine {
		display: flex;
		gap: var(--space-1);
	}
</style>
