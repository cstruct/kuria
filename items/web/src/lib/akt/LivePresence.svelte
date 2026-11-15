<script lang="ts">
	// r[impl app.live]: whether the person's edits are kept, and who else
	// has the live draft open.
	import type { LiveDraft } from './live.svelte';

	let { live }: { live: LiveDraft } = $props();

	const state = $derived(
		live.status !== 'connected'
			? live.status === 'connecting'
				? 'Connecting…'
				: 'Offline: reconnecting, edits kept until then'
			: !live.synced
				? 'Connecting…'
				: live.readOnly
					? 'Live, read-only'
					: live.pending
						? 'Saving…'
						: 'Saved'
	);
	// Each person once, however many windows they have open.
	const people = $derived([...new Map(live.peers.map((p) => [p.id, p])).values()]);
</script>

<div class="presence">
	<p class="state" role="status" data-testid="live-status">{state}</p>
	{#if people.length > 0}
		<ul aria-label="Also editing">
			{#each people as person (person.id)}
				<li style:--person={person.color} data-testid="peer">{person.name}</li>
			{/each}
		</ul>
	{/if}
	{#if live.refusal}
		<p class="refusal" role="alert">Your last edit was undone: {live.refusal}</p>
	{/if}
</div>

<style>
	.presence {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.state {
		margin: 0;
		color: var(--text-muted);
	}

	ul {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	li::before {
		content: '';
		width: 0.6rem;
		height: 0.6rem;
		border-radius: 50%;
		background: var(--person);
	}

	.refusal {
		flex-basis: 100%;
		margin: 0;
		color: var(--danger, currentColor);
	}
</style>
