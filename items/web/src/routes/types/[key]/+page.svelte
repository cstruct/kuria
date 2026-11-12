<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { AkterOfType } from '#lib/graphql/operations.ts';

	const list = query(AkterOfType, () => ({ type: page.params.key ?? '' }));
</script>

<main>
	{#if list.error}
		<p role="alert">{list.error.message}</p>
	{:else if list.data?.aktType}
		<h1>{list.data.aktType.name}</h1>
		<ul aria-label={list.data.aktType.name}>
			{#each list.data.akter as akt (akt.id)}
				<li>
					<a href={resolve('/akt/[number]', { number: akt.number })}>{akt.number}</a>
					<span>{akt.state.name}</span>
				</li>
			{/each}
		</ul>
	{:else if list.data}
		<p role="alert">There is no akt type {page.params.key}.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>
