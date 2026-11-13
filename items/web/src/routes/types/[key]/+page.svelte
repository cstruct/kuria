<script lang="ts">
	// r[impl app.akt-types]
	import TypeMark from '#lib/akt/TypeMark.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import Plus from '@lucide/svelte/icons/plus';
	import { ago } from '#lib/akt/history.ts';
	import { shownFields, sortAkter, type SortBy } from '#lib/akt/members.ts';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { AkterOfType } from '#lib/graphql/operations.ts';
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Mono from '#lib/ui/Mono.svelte';

	const key = $derived(page.params.key ?? '');
	const state = $derived(page.url.searchParams.get('state') ?? '');
	const sort = $derived<SortBy>(page.url.searchParams.get('sort') === 'rank' ? 'rank' : 'updated');
	const list = query(AkterOfType, () => ({ type: key, state: state || null }));
	const type = $derived(list.data?.aktType ?? undefined);
	const akter = $derived(sortAkter(list.data?.akter ?? [], sort));
	const columns = $derived(
		type ? shownFields({ fields: [], type }, 'list').map((f) => f.name) : []
	);
	setCrumbs(() => [{ label: type?.name ?? key }]);

	function show(change: { state?: string; sort?: SortBy }) {
		const params = new SvelteURLSearchParams(page.url.search);
		for (const [name, value] of Object.entries(change)) {
			if (value && !(name === 'sort' && value === 'updated')) params.set(name, value);
			else params.delete(name);
		}
		const search = params.toString();
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved, with a query
		goto(`${resolve('/types/[key]', { key })}${search ? `?${search}` : ''}`, {
			replace: true,
			reset: false
		});
	}
</script>

<svelte:head>
	<title>{type?.name ?? key} · Kuria</title>
</svelte:head>

<main class="list">
	{#if list.error}
		<p role="alert">{list.error.message}</p>
	{:else if type}
		<header>
			<h1><TypeMark {type} size="md" named /></h1>
			<Button variant="primary" href={resolve('/types/[key]/new', { key: type.key })}>
				<Plus size={16} aria-hidden={true} /> New {type.name}
			</Button>
		</header>

		<div class="filters">
			<label>
				State
				<select value={state} onchange={(e) => show({ state: e.currentTarget.value })}>
					<option value="">Any state</option>
					{#each type.states as option (option.key)}
						<option value={option.key}>{option.name}</option>
					{/each}
				</select>
			</label>
			<label>
				Sort by
				<select value={sort} onchange={(e) => show({ sort: e.currentTarget.value as SortBy })}>
					<option value="updated">Recently changed</option>
					<option value="rank">Rank</option>
				</select>
			</label>
		</div>

		{#if akter.length === 0}
			<EmptyState
				title={state ? `No ${type.name} in this state` : `No ${type.name} yet`}
				description={state ? 'Try another state.' : `Create the first ${type.name}.`}
			/>
		{:else}
			<div class="scroll">
				<table aria-label={type.name}>
					<thead>
						<tr>
							<th scope="col">Number</th>
							<th scope="col">Title</th>
							{#each columns as name (name)}
								<th scope="col">{name}</th>
							{/each}
							<th scope="col">State</th>
							<th scope="col">Changed</th>
						</tr>
					</thead>
					<tbody>
						{#each akter as akt (akt.id)}
							<tr>
								<td
									><a href={resolve('/akt/[number]', { number: akt.number })}
										><Mono value={akt.number} max={32} /></a
									></td
								>
								<td class="title">{akt.title ?? ''}</td>
								{#each shownFields(akt, 'list') as field (field.name)}
									<td>{field.text}</td>
								{/each}
								<td><Badge tone={akt.state.tone}>{akt.state.name}</Badge></td>
								<td class="when" title={new Date(akt.updatedAt).toLocaleString()}
									>{ago(akt.updatedAt)}</td
								>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	{:else if list.data}
		<p role="alert">There is no akt type {key}.</p>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
	}

	h1 {
		margin: 0;
	}

	.filters {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
	}

	.filters label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--text-muted);
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	.scroll {
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}

	th {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 500;
		text-align: left;
		white-space: nowrap;
	}

	td {
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border);
	}

	tbody tr:last-child td {
		border-bottom: none;
	}

	tbody tr:hover {
		background: var(--bg-subtle);
	}

	.title {
		min-width: 12rem;
	}

	.when {
		color: var(--text-muted);
		white-space: nowrap;
	}
</style>
