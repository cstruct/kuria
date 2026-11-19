<script lang="ts">
	// r[impl app.verdicts]: a member's assignees, each with their verdict and
	// what they saw, and an assignee's own given in place: Pass at once,
	// Fail or Skip saying why, before it is saved.
	import type { Verdict } from '#lib/graphql/generated/graphql.ts';
	import Avatar from '#lib/ui/Avatar.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { VERDICTS, needsDescription, reportError, type Judged, type Report } from './verdicts';

	interface Props {
		/** The akt it is the verdicts of, for labels. */
		number: string;
		judged: Judged;
		busy: boolean;
		/** Whether verdicts may be given now: not before its run has started
		 * (`app.verdicts`). */
		open?: boolean;
		/** Reports one's own verdict, or deletes it with `null`. */
		onGive: (report: Report | null) => void;
	}

	let { number, judged, busy, open = true, onGive }: Props = $props();

	/** The status being explained before it is saved: a fail or a skip. */
	let explaining = $state<Verdict | null>(null);
	let description = $state('');
	let tried = $state(false);
	const id = $props.id();

	const tone = (verdict: Verdict | null) =>
		verdict === 'PASS'
			? 'SUCCESS'
			: verdict === 'FAIL'
				? 'DANGER'
				: verdict === 'SKIP'
					? 'WARNING'
					: 'NEUTRAL';
	const said = (verdict: Verdict | null) =>
		verdict ? (VERDICTS.find((v) => v.verdict === verdict)?.label ?? verdict) : 'Not yet';
	const error = $derived(
		explaining && tried ? reportError({ verdict: explaining, description }) : null
	);

	function choose(verdict: Verdict) {
		if (!needsDescription(verdict)) {
			explaining = null;
			if (judged.mine?.verdict !== 'PASS') onGive({ verdict, description: '' });
			return;
		}
		explaining = verdict;
		description = judged.mine?.verdict === verdict ? judged.mine.description : '';
		tried = false;
	}

	function save(event: SubmitEvent) {
		event.preventDefault();
		tried = true;
		if (!explaining || reportError({ verdict: explaining, description })) return;
		onGive({ verdict: explaining, description: description.trim() });
		explaining = null;
	}
</script>

<div class="verdicts">
	{#if judged.assignees.length === 0}
		<span class="hint">No one assigned</span>
	{:else}
		<ul class="assignees" aria-label="Verdicts on {number}">
			{#each judged.assignees as assignee (assignee.id)}
				<li>
					<span class="who"><Avatar name={assignee.name} />{assignee.name}</span>
					<Badge tone={tone(assignee.verdict)}
						><span class="verdict" data-verdict={assignee.verdict?.toLowerCase() ?? 'none'}
							>{said(assignee.verdict)}</span
						></Badge
					>
					{#if assignee.description}
						<p class="description">{assignee.description}</p>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
	{#if judged.mine !== undefined}
		<div class="mine" role="group" aria-label="Your verdict on {number}">
			{#if !open}
				<span class="hint">Results are given once the run has started.</span>
			{:else}
				<div class="choices">
					{#each VERDICTS as option (option.verdict)}
						<Button
							size="sm"
							variant={judged.mine?.verdict === option.verdict ? 'primary' : 'secondary'}
							aria-pressed={judged.mine?.verdict === option.verdict}
							disabled={busy}
							onclick={() => choose(option.verdict)}>{option.label}</Button
						>
					{/each}
					{#if judged.mine}
						<Button size="sm" variant="ghost" disabled={busy} onclick={() => onGive(null)}
							>Clear</Button
						>
					{/if}
				</div>
				{#if explaining}
					<form
						class="why"
						aria-label="Why it {explaining === 'FAIL' ? 'failed' : 'was skipped'}"
						onsubmit={save}
					>
						<label for="{id}-why">
							{explaining === 'FAIL' ? 'What failed' : 'Why it was skipped'}<span
								class="required"
								aria-hidden="true"
							>
								*</span
							>
						</label>
						<textarea
							id="{id}-why"
							rows="2"
							bind:value={description}
							aria-invalid={error ? true : undefined}
							aria-required="true"
							aria-describedby={error ? `${id}-error` : undefined}></textarea>
						{#if error}<p class="error" id="{id}-error">{error}</p>{/if}
						<div class="actions">
							<Button size="sm" variant="ghost" onclick={() => (explaining = null)}>Cancel</Button>
							<Button size="sm" variant="primary" type="submit" {busy}>Save</Button>
						</div>
					</form>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.verdicts {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.assignees {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: var(--text-sm);
	}

	.assignees li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
	}

	.who {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}

	.description {
		flex-basis: 100%;
		margin-left: 1.875rem;
		color: var(--text-muted);
		white-space: pre-line;
	}

	.hint {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.mine {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	.why {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		max-width: 32rem;
		font-size: var(--text-sm);
	}

	.why label {
		font-weight: 500;
	}

	.required,
	.error {
		color: var(--tone-danger-fg);
	}

	.error {
		font-size: var(--text-xs);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-1);
	}
</style>
