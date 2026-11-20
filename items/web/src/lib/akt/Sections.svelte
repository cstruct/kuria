<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { judgedType } from './members';
	// r[impl app.sections]
	// r[impl app.verdicts]
	import { resolve } from '$app/paths';
	import type { AktPageQuery } from '#lib/graphql/generated/graphql.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { renderMarkdown } from './markdown';
	import OutcomeBadge from './OutcomeBadge.svelte';
	import { numbersIn, sectionOutcome, type Item, type Section } from './sections';
	import VerdictButtons from './VerdictButtons.svelte';
	import { judgedOf, type Report } from './verdicts';

	type Member = NonNullable<AktPageQuery['akt']>['board']['columns'][number]['akter'][number];

	interface Props {
		items: Item[];
		/** Whether verdicts may be given now (`app.verdicts`). */
		judging?: boolean;
		/** The akt's members the viewer may read. */
		members: readonly Member[];
		/** The viewer's subject. */
		me: string | undefined;
		busy: boolean;
		onVerdict: (member: Member, verdict: Report | null) => void;
		/** The collapsible sections opened, by path, kept by the page so a
		 * change that reloads the akt doesn't close them. */
		opened: Set<string>;
	}

	let { items, members, me, busy, onVerdict, opened, judging = true }: Props = $props();

	const byNumber = $derived(new Map(members.map((m) => [m.number, m])));
	const outcomeOf = (number: string) => byNumber.get(number)?.outcome;
</script>

{#snippet member(number: string)}
	{@const m = byNumber.get(number)}
	<li class="member" data-number={number}>
		{#if m}
			{@const judged = judgedOf(m, me)}
			<div class="line">
				<a href={resolve('/akt/[number]', { number })}>{m.title ?? number}</a>
				{#if m.title}<Mono value={number} max={22} />{/if}
				<!-- r[impl app.members]: a judged member's state says nothing its outcome doesn't. -->
				{#if !judgedType(m.type)}<Badge tone={m.state.tone}>{m.state.name}</Badge>{/if}
				{#if m.outcome}<OutcomeBadge outcome={m.outcome} />{/if}
			</div>
			{#if judged}
				<VerdictButtons
					{number}
					{judged}
					{busy}
					open={judging}
					onGive={(verdict) => onVerdict(m, verdict)}
				/>
			{/if}
		{:else}
			<!-- A member the viewer can't read, or no longer a member. -->
			<span class="hint">{number}</span>
		{/if}
	</li>
{/snippet}

{#snippet body(section: Section, path: number[])}
	{#if section.text}
		<!-- Sanitized by renderMarkdown. -->
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<div class="markdown">{@html renderMarkdown(section.text)}</div>
	{/if}
	{@render list(section.items, path)}
{/snippet}

{#snippet list(within: Item[], at: number[])}
	<ul class="items">
		{#each within as item, i (i)}
			{#if 'akt' in item}
				{@render member(item.akt)}
			{:else}
				{@const section = item.section}
				{@const title = section.title || 'Untitled section'}
				{@const path = [...at, i]}
				{@const key = path.join('.')}
				<li class="section-item">
					{#if section.collapsible}
						<details
							class="section"
							open={opened.has(key)}
							ontoggle={(e) => {
								if (e.currentTarget.open) opened.add(key);
								else opened.delete(key);
							}}
						>
							<summary>
								<!-- r[impl app.sections]: closed, it says it holds something. -->
								<ChevronRight class="chevron" size={14} aria-hidden={true} />
								<span class="title">{title}</span>
								<span class="count">{numbersIn(section.items).length}</span>
								<OutcomeBadge outcome={sectionOutcome(section, outcomeOf)} />
							</summary>
							{@render body(section, path)}
						</details>
					{:else}
						<section class="section" aria-label={title}>
							<div class="heading" role="heading" aria-level={Math.min(at.length + 3, 6)}>
								<span class="title">{title}</span>
								<OutcomeBadge outcome={sectionOutcome(section, outcomeOf)} />
							</div>
							{@render body(section, path)}
						</section>
					{/if}
				</li>
			{/if}
		{/each}
	</ul>
{/snippet}

<div class="sections" aria-label="Sections" role="region">
	{@render list(items, [])}
</div>

<style>
	.items {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.section .items {
		padding-left: var(--space-4);
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--bg-raised, transparent);
	}

	summary,
	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-weight: 600;
	}

	summary {
		cursor: pointer;
	}

	details:not([open]) {
		gap: 0;
	}

	.member {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}

	.member:last-child {
		border-bottom: 0;
	}

	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.hint {
		color: var(--text-muted);
	}

	.markdown :global(p) {
		margin: 0 0 var(--space-2);
	}

	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		cursor: pointer;
		list-style: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary :global(.chevron) {
		flex-shrink: 0;
		color: var(--text-muted);
		transition: transform 0.1s;
	}

	details[open] > summary :global(.chevron) {
		transform: rotate(90deg);
	}

	.count {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}
</style>
