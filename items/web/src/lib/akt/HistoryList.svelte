<script lang="ts">
	import { ago, describe, type RevisionData } from './history';

	interface Props {
		revisions: RevisionData[];
		fieldNames: Record<string, string>;
		transitionNames: Record<string, string>;
	}

	let { revisions, fieldNames, transitionNames }: Props = $props();
	const described = $derived(
		[...revisions].reverse().map((r) => describe(r, fieldNames, transitionNames))
	);
</script>

<ol class="timeline" aria-label="Revisions">
	{#each described as entry (entry.revision)}
		<li>
			<p>
				<strong>{entry.who}</strong>
				{entry.what}
				<time datetime={entry.at} title={new Date(entry.at).toLocaleString()}>{ago(entry.at)}</time>
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

<style>
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
