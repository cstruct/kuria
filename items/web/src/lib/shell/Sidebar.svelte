<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Users from '@lucide/svelte/icons/users';
	import Home from '@lucide/svelte/icons/house';
	import SearchIcon from '@lucide/svelte/icons/search';
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
</script>

<nav class="sidebar" aria-label="Kuria">
	<a class="brand" href={resolve('/')} onclick={onNavigate}>Kuria</a>

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

	<h2 id="types-heading">Akt types</h2>
	<ul aria-labelledby="types-heading">
		{#each types as type (type.key)}
			{@const href = resolve('/types/[key]', { key: type.key })}
			<li>
				<a {href} aria-current={here(href) ? 'page' : undefined} onclick={onNavigate}>
					<span class="name">{type.name}</span>
					{#if type.akterCount !== null}<span class="count">{type.akterCount}</span>{/if}
				</a>
			</li>
		{/each}
	</ul>

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
		padding: 0 var(--space-2) var(--space-2);
		font-size: var(--text-lg);
		font-weight: 700;
		color: var(--text);
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
</style>
