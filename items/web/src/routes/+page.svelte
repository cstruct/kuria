<script lang="ts">
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { resolve } from '$app/paths';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { Home } from '#lib/graphql/operations.ts';

	const home = query(Home, () => ({}));
	setCrumbs(() => [{ label: 'Home' }]);
</script>

<main>
	<h1>Kuria</h1>
	{#if home.error}
		<p role="alert">{home.error.message}</p>
	{:else if home.data}
		<nav aria-label="Akt types">
			<ul>
				{#each home.data.aktTypes as type (type.key)}
					<li><a href={resolve('/types/[key]', { key: type.key })}>{type.name}</a></li>
				{/each}
			</ul>
		</nav>
	{:else}
		<p>Loading…</p>
	{/if}
</main>
