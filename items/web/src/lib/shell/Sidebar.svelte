<script lang="ts">
	import Logo from './Logo.svelte';
	import TypeMark from '#lib/akt/TypeMark.svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Users from '@lucide/svelte/icons/users';
	import Home from '@lucide/svelte/icons/house';
	import SearchIcon from '@lucide/svelte/icons/search';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import { groupTypes, moreWasOpen, rememberMore, splitTypes } from './shell';
	import type { ShellQuery } from '#lib/graphql/generated/graphql.ts';

	interface Props {
		types: ShellQuery['aktTypes'];
		shortcut: string;
		onSearch: () => void;
		/** Called when a link is followed, such as to close a drawer. */
		onNavigate?: () => void;
	}

	let { types, shortcut, onSearch, onNavigate }: Props = $props();

	const here = (href: string) =>
		page.url.pathname === href || page.url.pathname.startsWith(`${href}/`);

	// r[impl app.navigation]
	const split = $derived(splitTypes(types));
	// Reading `localStorage` itself throws where site data is blocked.
	const storage = () => {
		try {
			return typeof localStorage === 'undefined' ? undefined : localStorage;
		} catch {
			return undefined;
		}
	};
	let moreOpen = $state(moreWasOpen(storage()));
	function toggleMore() {
		moreOpen = !moreOpen;
		rememberMore(storage(), moreOpen);
	}
</script>

{#snippet typeLinks(listed: ShellQuery['aktTypes'])}
	{#each listed as type (type.key)}
		{@const href = resolve('/types/[key]', { key: type.key })}
		<li>
			<a {href} aria-current={here(href) ? 'page' : undefined} onclick={onNavigate}>
				<span class="name"><TypeMark {type} named /></span>
				{#if type.akterCount !== null}<span class="count">{type.akterCount}</span>{/if}
			</a>
		</li>
	{/each}
{/snippet}

<nav class="sidebar" aria-label="Kuria">
	<a class="brand" href={resolve('/')} onclick={onNavigate}><Logo height={26} /></a>

	<button type="button" class="search" onclick={onSearch}>
		<SearchIcon size={14} aria-hidden={true} />
		<span>Search</span>
		<kbd>{shortcut}</kbd>
	</button>

	<ul>
		<li>
			<a
				href={resolve('/')}
				aria-current={page.url.pathname === '/' ? 'page' : undefined}
				onclick={onNavigate}
			>
				<Home size={16} aria-hidden={true} /> Home
			</a>
		</li>
	</ul>

	<h2 id="types-heading">Work</h2>
	{#each groupTypes(split.main) as group (group.group)}
		{#if group.group}
			<h3 class="group" id="group-{group.group}">{group.group}</h3>
			<ul aria-labelledby="group-{group.group}">
				{@render typeLinks(group.types)}
			</ul>
		{:else}
			<ul aria-labelledby="types-heading">
				{@render typeLinks(group.types)}
			</ul>
		{/if}
	{/each}
	{#if split.more.length > 0}
		<button
			type="button"
			class="more"
			aria-expanded={moreOpen}
			aria-controls="more-types"
			onclick={toggleMore}
		>
			<ChevronRight size={14} aria-hidden={true} />
			<span>Show more</span>
			<span class="count">{split.more.length}</span>
		</button>
		<ul id="more-types" aria-label="More work" hidden={!moreOpen}>
			{@render typeLinks(split.more)}
		</ul>
	{/if}

	<ul class="end">
		<li>
			<a
				href={resolve('/contacts')}
				aria-current={here(resolve('/contacts')) ? 'page' : undefined}
				onclick={onNavigate}
			>
				<Users size={16} aria-hidden={true} /> Contacts
			</a>
		</li>
	</ul>
</nav>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		height: 100%;
		padding: var(--space-4) var(--space-3);
		overflow-y: auto;
	}

	.brand {
		display: inline-flex;
		align-self: flex-start;
		padding: var(--space-1) var(--space-2) var(--space-2);
		border-radius: var(--radius-sm);
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text-muted);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.search span {
		flex: 1;
		text-align: start;
	}

	kbd {
		font-size: var(--text-xs);
	}

	h2 {
		margin-top: var(--space-3);
		padding: 0 var(--space-2);
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-muted);
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.more {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border: 0;
		border-radius: var(--radius-md);
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.more:hover {
		background: var(--bg-muted);
	}

	.more span:first-of-type {
		flex: 1;
		text-align: start;
	}

	.more :global(svg) {
		transition: transform 0.15s;
	}

	.more[aria-expanded='true'] :global(svg) {
		transform: rotate(90deg);
	}

	ul[hidden] {
		display: none;
	}

	.end {
		margin-top: auto;
		padding-top: var(--space-3);
	}

	li a {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border-radius: var(--radius-md);
		color: var(--text);
		font-size: var(--text-sm);
	}

	li a:hover {
		background: var(--bg-muted);
		text-decoration: none;
	}

	li a[aria-current='page'] {
		background: var(--accent-subtle);
		color: var(--on-accent-subtle);
		font-weight: 600;
	}

	.name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.count {
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	li a[aria-current='page'] .count {
		color: inherit;
	}

	.group {
		margin: var(--space-3) 0 var(--space-1);
		padding: 0 var(--space-3);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 500;
	}
</style>
