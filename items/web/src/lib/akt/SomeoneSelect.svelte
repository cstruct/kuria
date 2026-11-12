<script lang="ts">
	import { untrack } from 'svelte';
	import { keyOf, whoOfKey, type Person } from './people';
	import type { Who } from './values';

	interface Props {
		label: string;
		who: Who | null;
		people: Person[];
		/** What choosing nobody means, such as "Nobody" or "Me". */
		none: string;
	}

	let { label, who = $bindable(), people, none }: Props = $props();

	const OTHER = 'other';
	// Read once, when the picker opens: a subject not among `people` is one
	// typed in by id.
	const initially = untrack(() => {
		const known = people.some((p) => keyOf(p.who) === keyOf(who));
		return who && 'user' in who && !known ? who.user : '';
	});
	let typed = $state(initially);
	let choice = $state(initially ? OTHER : untrack(() => keyOf(who)));

	function pick(next: string) {
		choice = next;
		who = next === OTHER ? (typed.trim() ? { user: typed.trim() } : null) : whoOfKey(next);
	}
</script>

<label>
	{label}
	<select aria-label={label} value={choice} onchange={(e) => pick(e.currentTarget.value)}>
		<option value="">{none}</option>
		{#each people as person (keyOf(person.who))}
			<option value={keyOf(person.who)}>{person.name}</option>
		{/each}
		<option value={OTHER}>Someone else, by subject id…</option>
	</select>
</label>
{#if choice === OTHER}
	<label>
		{label}: subject id
		<input
			aria-label="{label}: subject id"
			bind:value={typed}
			oninput={() => (who = typed.trim() ? { user: typed.trim() } : null)}
			autocomplete="off"
		/>
	</label>
{/if}
