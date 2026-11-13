<script lang="ts">
	import { Dialog } from 'bits-ui';
	import { MediaQuery } from 'svelte/reactivity';
	import type { Snippet } from 'svelte';
	import { resolve } from '$app/paths';
	import Menu from '@lucide/svelte/icons/menu';
	import LogOut from '@lucide/svelte/icons/log-out';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { query } from '#lib/graphql/context.svelte.ts';
	import { Shell } from '#lib/graphql/operations.ts';
	import Sidebar from './Sidebar.svelte';
	import SearchDialog from './SearchDialog.svelte';
	import { crumbs } from './crumbs.svelte';
	import { NARROW, isSearchShortcut, shortcutLabel } from './shell';

	let { children }: { children: Snippet } = $props();

	const shell = query(Shell, () => ({}));
	const narrow = new MediaQuery(NARROW);
	const shortcut = shortcutLabel(navigator.platform);
	let drawer = $state(false);
	let searching = $state(false);

	function onKeydown(event: KeyboardEvent) {
		if (isSearchShortcut(event)) {
			event.preventDefault();
			searching = true;
		}
	}

	async function signOut() {
		await fetch('/auth/logout', { method: 'POST', credentials: 'same-origin' });
		// The login page ends its own Konklav session too.
		location.assign(`${resolve('/login')}?sign_out`);
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div class="shell" class:narrow={narrow.current}>
	{#if !narrow.current}
		<aside class="side">
			<Sidebar types={shell.data?.aktTypes ?? []} {shortcut} onSearch={() => (searching = true)} />
		</aside>
	{/if}

	<div class="column">
		<header class="top">
			{#if narrow.current}
				<IconButton label="Open the menu" icon={Menu} onclick={() => (drawer = true)} />
			{/if}
			<nav class="crumbs" aria-label="You are here">
				<ol>
					{#each crumbs.trail as crumb, i (i)}
						<li>
							{#if i > 0}<ChevronRight size={14} aria-hidden={true} />{/if}
							{#if crumb.href && i < crumbs.trail.length - 1}
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
								<a href={crumb.href}>{crumb.label}</a>
							{:else}
								<span aria-current={i === crumbs.trail.length - 1 ? 'page' : undefined}
									>{crumb.label}</span
								>
							{/if}
						</li>
					{/each}
				</ol>
			</nav>
			{#if narrow.current}
				<button type="button" class="search-button" onclick={() => (searching = true)}
					>Search</button
				>
			{/if}
			{#if shell.data}
				<span class="viewer" title={shell.data.viewer.subject}>{shell.data.viewer.user.name}</span>
			{/if}
			<IconButton label="Sign out" icon={LogOut} onclick={signOut} />
		</header>
		<div class="content">{@render children()}</div>
	</div>
</div>

{#if narrow.current}
	<Dialog.Root bind:open={drawer}>
		<Dialog.Portal>
			<Dialog.Overlay class="dialog-overlay" />
			<Dialog.Content class="drawer">
				<Dialog.Title class="visually-hidden">Menu</Dialog.Title>
				<Sidebar
					types={shell.data?.aktTypes ?? []}
					{shortcut}
					onSearch={() => {
						drawer = false;
						searching = true;
					}}
					onNavigate={() => (drawer = false)}
				/>
			</Dialog.Content>
		</Dialog.Portal>
	</Dialog.Root>
{/if}

<SearchDialog bind:open={searching} />

<style>
	.shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
		min-height: 100vh;
	}

	.shell.narrow {
		grid-template-columns: minmax(0, 1fr);
	}

	.side {
		position: sticky;
		top: 0;
		height: 100vh;
		border-right: 1px solid var(--border);
		background: var(--bg-subtle);
	}

	.column {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.top {
		position: sticky;
		top: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		gap: var(--space-3);
		min-height: 3.25rem;
		padding: var(--space-2) var(--space-4);
		border-bottom: 1px solid var(--border);
		background: var(--bg);
	}

	.crumbs {
		flex: 1;
		min-width: 0;
	}

	.crumbs ol {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: var(--text-sm);
		color: var(--text-muted);
		overflow: hidden;
		white-space: nowrap;
	}

	.crumbs li {
		display: flex;
		flex-shrink: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		overflow: hidden;
	}

	.crumbs li :global(svg) {
		flex-shrink: 0;
	}

	/* Earlier crumbs give way first. */
	.crumbs li:not(:last-child) {
		flex-shrink: 4;
	}

	.crumbs li a,
	.crumbs li span {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.crumbs li:last-child span {
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.viewer {
		font-size: var(--text-sm);
		color: var(--text-muted);
		max-width: 14rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.narrow .viewer {
		display: none;
	}

	.search-button {
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		padding: var(--space-1) var(--space-3);
		font: inherit;
		font-size: var(--text-sm);
	}

	.content {
		flex: 1;
	}

	:global(.drawer) {
		position: fixed;
		top: 0;
		bottom: 0;
		left: 0;
		z-index: 41;
		width: min(18rem, 85vw);
		background: var(--bg-subtle);
		border-right: 1px solid var(--border);
		box-shadow: var(--shadow-md);
	}
</style>
