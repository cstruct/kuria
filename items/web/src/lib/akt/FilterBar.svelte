<script lang="ts">
	// r[impl app.filters]
	import { keyOf, type Person } from './people';
	import { filterable, type FilterField } from './filters';

	interface Props {
		fields: readonly FilterField[];
		/** Each filtered field's value, by key. */
		values: Record<string, string>;
		/** Whom user fields and checkboxes can be filtered by. */
		people: readonly Person[];
		onChange: (field: string, value: string) => void;
	}

	let { fields, values, people, onChange }: Props = $props();

	/** How a person is said in a condition: `me`, a subject, or a contact. */
	function equalsOf(person: Person, index: number): string {
		if (index === 0 && person.name === 'Me') return 'me';
		const key = keyOf(person.who);
		return key.startsWith('user:') ? key.slice(5) : key;
	}
</script>

{#each fields.filter(filterable) as field (field.key)}
	{@const value = values[field.key] ?? ''}
	<label class="filter">
		{field.name}
		{#if field.kind === 'CHOICE' || field.kind === 'CHOICES'}
			<select {value} onchange={(e) => onChange(field.key, e.currentTarget.value)}>
				<option value="">Any</option>
				{#each field.options ?? [] as option (option.key)}
					<option value={option.key}>{option.name}</option>
				{/each}
			</select>
		{:else if field.kind === 'USER' || field.kind === 'CHECKBOX'}
			<select {value} onchange={(e) => onChange(field.key, e.currentTarget.value)}>
				<option value="">{field.kind === 'CHECKBOX' ? 'Anyone or no one' : 'Anyone'}</option>
				{#each people as person, i (keyOf(person.who))}
					<option value={equalsOf(person, i)}>{person.name}</option>
				{/each}
			</select>
		{:else if field.kind === 'DATE'}
			<input type="date" {value} onchange={(e) => onChange(field.key, e.currentTarget.value)} />
		{:else}
			<input
				type="search"
				{value}
				placeholder="Exactly…"
				onchange={(e) => onChange(field.key, e.currentTarget.value.trim())}
			/>
		{/if}
	</label>
{/each}

<style>
	.filter {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--text-muted);
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	input[type='search'] {
		width: 10rem;
	}
</style>
