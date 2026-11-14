<script lang="ts">
	// r[impl app.fields]: several users or contacts for a `multiple` field.
	import { keyOf, type Person } from './people';
	import type { Who } from './values';

	interface Props {
		who: Who[];
		people: Person[];
	}

	let { who = $bindable(), people }: Props = $props();

	// Whoever the field names that the pickers don't offer, by id.
	const others = $derived(who.filter((w) => !people.some((p) => keyOf(p.who) === keyOf(w))));
	const chosen = (w: Who) => who.some((c) => keyOf(c) === keyOf(w));

	function toggle(w: Who, on: boolean) {
		who = on ? [...who, w] : who.filter((c) => keyOf(c) !== keyOf(w));
	}
</script>

<div class="people">
	{#each people as person (keyOf(person.who))}
		<label class="inline">
			<input
				type="checkbox"
				checked={chosen(person.who)}
				onchange={(e) => toggle(person.who, e.currentTarget.checked)}
			/>
			{person.name}
		</label>
	{/each}
	{#each others as other (keyOf(other))}
		<label class="inline">
			<input type="checkbox" checked onchange={() => toggle(other, false)} />
			{'user' in other ? other.user : other.contact}
		</label>
	{/each}
	{#if people.length === 0 && others.length === 0}
		<p class="none">No one to choose.</p>
	{/if}
</div>

<style>
	.people {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.inline {
		display: flex;
		gap: var(--space-2);
		align-items: center;
	}

	.none {
		margin: 0;
		color: var(--text-muted);
	}
</style>
