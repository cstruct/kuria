<script lang="ts">
	import { resolve } from '$app/paths';
	import Plus from '@lucide/svelte/icons/plus';
	import { ago } from '#lib/akt/history.ts';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { counted } from '#lib/home/home.ts';
	import { Home } from '#lib/graphql/operations.ts';
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Mono from '#lib/ui/Mono.svelte';

	const home = query(Home, () => ({}));
	setCrumbs(() => [{ label: 'Home' }]);
</script>

<svelte:head>
	<title>Home · Kuria</title>
</svelte:head>

<main class="home">
	<h1>Home</h1>
	{#if home.error}
		<p role="alert">{home.error.message}</p>
	{:else if home.data}
		<Card title="Recently changed" padded={false}>
			{#if home.data.akter.length === 0}
				<EmptyState
					title="Nothing yet"
					description="Akter you can read show here as they change."
				/>
			{:else}
				<ul class="recent" aria-label="Recently changed">
					{#each home.data.akter as akt (akt.id)}
						<li>
							<a class="what" href={resolve('/akt/[number]', { number: akt.number })}>
								<span class="title">{akt.title ?? akt.number}</span>
								<span class="meta">
									{akt.type.name}
									{#if akt.title}· <Mono value={akt.number} max={32} />{/if}
								</span>
							</a>
							<Badge tone={akt.state.tone}>{akt.state.name}</Badge>
							<span class="when" title={new Date(akt.updatedAt).toLocaleString()}
								>{ago(akt.updatedAt)}</span
							>
						</li>
					{/each}
				</ul>
			{/if}
		</Card>

		<section aria-labelledby="types-title">
			<h2 id="types-title">Akt types</h2>
			{#if home.data.aktTypes.length === 0}
				<EmptyState
					title="No akt types"
					description="An administrator defines them through the API."
				/>
			{:else}
				<ul class="types">
					{#each home.data.aktTypes as type (type.key)}
						<li>
							<a class="name" href={resolve('/types/[key]', { key: type.key })}>{type.name}</a>
							<span class="count">{counted(type.akterCount)}</span>
							<Button
								size="sm"
								href={resolve('/types/[key]/new', { key: type.key })}
								aria-label="New {type.name}"
							>
								<Plus size={14} aria-hidden={true} /> New
							</Button>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	{:else}
		<p>Loading…</p>
	{/if}
</main>

<style>
	.home {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		max-width: 64rem;
	}

	h1 {
		margin: 0;
	}

	h2 {
		margin: 0 0 var(--space-3);
		font-size: var(--text-lg);
	}

	.recent {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.recent li {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-bottom: 1px solid var(--border);
	}

	.recent li:last-child {
		border-bottom: none;
	}

	.what {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		color: var(--text);
		text-decoration: none;
	}

	.what:hover .title {
		text-decoration: underline;
	}

	.title {
		overflow: hidden;
		font-weight: 500;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	.when {
		flex-shrink: 0;
		min-width: 5.5rem;
		color: var(--text-muted);
		font-size: var(--text-sm);
		text-align: right;
	}

	.types {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
		gap: var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.types li {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-2);
		padding: var(--space-4);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface);
		box-shadow: var(--shadow-sm);
	}

	.name {
		color: var(--text);
		font-weight: 600;
		text-decoration: none;
	}

	.name:hover {
		text-decoration: underline;
	}

	.count {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	@media (max-width: 480px) {
		.when {
			display: none;
		}
	}
</style>
