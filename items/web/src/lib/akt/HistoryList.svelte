<script lang="ts">
	import Button from '#lib/ui/Button.svelte';
	import { ago, describe, merged, type RevisionData } from './history';

	interface Props {
		revisions: RevisionData[];
		fieldNames: Record<string, string>;
		transitionNames: Record<string, string>;
		/** Each field's kind, by key. */
		fieldKinds?: Record<string, string>;
		/** The versions, to say which revisions were released. */
		versions?: readonly { number: number; revision: number; label?: string | null }[];
		/** Each choice field's options' names, by field and option key. */
		optionNames?: Record<string, Record<string, string>>;
	}

	let {
		revisions,
		fieldNames,
		transitionNames,
		fieldKinds = {},
		versions = [],
		optionNames = {}
	}: Props = $props();
	/** How many are shown until all are asked for (`app.history`). */
	const LATEST = 5;
	let all = $state(false);
	/** Whether the history is shown, not only its latest change (`app.history`). */
	let opened = $state(false);
	// r[impl version.history]: a release, said with the revision it names.
	const described = $derived(
		merged(
			[...revisions].reverse().map((r) => {
				const entry = describe(r, fieldNames, transitionNames, fieldKinds, optionNames);
				const released = versions.filter((v) => v.revision === r.revision);
				for (const v of released) {
					entry.what += `, released as version ${v.number}${v.label ? ` (${v.label})` : ''}`;
				}
				return entry;
			})
		)
	);
	const shown = $derived(all ? described : described.slice(0, LATEST));
</script>

<!-- r[impl app.history]: who changed it last and when, until it's opened. -->
{#if !opened}
	{@const latest = described[0]}
	<div class="latest">
		{#if latest}
			<p>
				Last changed by <strong>{latest.who}</strong>
				<time datetime={latest.at} title={new Date(latest.at).toLocaleString()}
					>{ago(latest.at)}</time
				>
			</p>
		{/if}
		<Button size="sm" variant="ghost" aria-expanded={false} onclick={() => (opened = true)}
			>Show history</Button
		>
	</div>
{:else}
	<ol class="timeline" aria-label="Revisions">
		{#each shown as entry (entry.revision)}
			<li>
				<p>
					<strong>{entry.who}</strong>
					{entry.what}
					<time datetime={entry.at} title={new Date(entry.at).toLocaleString()}
						>{ago(entry.at)}</time
					>
				</p>
				{#if entry.changes.length > 0}
					<ul class="changes">
						{#each entry.changes as change (change.field)}
							<li>
								<span class="field">{change.field}</span>
								{#if change.value === null}<span class="cleared">cleared</span
									>{:else}{change.value}{/if}
							</li>
						{/each}
					</ul>
				{/if}
				{#if entry.memberships.length > 0}
					<ul class="changes">
						{#each entry.memberships as membership (membership)}<li>{membership}</li>{/each}
					</ul>
				{/if}
				{#if entry.reason}<p class="reason">“{entry.reason}”</p>{/if}
			</li>
		{/each}
	</ol>
	{#if described.length > shown.length}
		<Button size="sm" variant="ghost" onclick={() => (all = true)}
			>Show all {described.length}</Button
		>
	{/if}
{/if}

<style>
	.latest {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.latest p {
		margin: 0;
	}

	.latest time {
		color: var(--text-muted);
	}

	.timeline {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.timeline > li {
		position: relative;
		padding: 0 0 var(--space-4) var(--space-4);
		border-left: 2px solid var(--border);
		font-size: var(--text-sm);
	}

	.timeline > li::before {
		content: '';
		position: absolute;
		left: -5px;
		top: 0.4rem;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--border-strong);
	}

	.timeline > li:last-child {
		padding-bottom: 0;
	}

	time {
		color: var(--text-muted);
		margin-left: var(--space-1);
	}

	.changes {
		margin: var(--space-1) 0 0;
		padding: 0;
		list-style: none;
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}

	.field {
		color: var(--text);
		margin-right: var(--space-1);
	}

	.field::after {
		content: ':';
	}

	.cleared {
		font-style: italic;
	}

	.reason {
		margin-top: var(--space-1);
		color: var(--text-muted);
		font-style: italic;
	}
</style>
