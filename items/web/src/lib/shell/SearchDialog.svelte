<script lang="ts">
	import { Command, Dialog } from 'bits-ui';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Badge from '#lib/ui/Badge.svelte';
	import { getClient } from '#lib/graphql/context.svelte.ts';
	import { SearchAkter } from '#lib/graphql/operations.ts';
	import type { SearchAkterQuery } from '#lib/graphql/generated/graphql.ts';

	// r[impl app.search]
	let { open = $bindable() }: { open: boolean } = $props();

	const client = getClient();
	let text = $state('');
	let found = $state<SearchAkterQuery['akter']['nodes']>([]);
	let searching = $state(false);
	let asked = 0;

	$effect(() => {
		const typed = text.trim();
		const mine = ++asked;
		if (!typed) {
			found = [];
			searching = false;
			return;
		}
		searching = true;
		const timer = setTimeout(async () => {
			const result = await client
				.query(SearchAkter, { text: typed }, { requestPolicy: 'network-only' })
				.toPromise();
			if (mine === asked) {
				found = result.data?.akter.nodes ?? [];
				searching = false;
			}
		}, 150);
		return () => clearTimeout(timer);
	});

	function openAkt(number: string) {
		open = false;
		text = '';
		goto(resolve('/akt/[number]', { number }));
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="dialog-overlay" />
		<Dialog.Content class="search-content">
			<Dialog.Title class="visually-hidden">Search</Dialog.Title>
			<Command.Root label="Search by number or title" shouldFilter={false} class="search-command">
				<div class="search-input">
					<SearchIcon size={16} aria-hidden={true} />
					<Command.Input bind:value={text} placeholder="Search by number or title…" />
				</div>
				<Command.List class="search-list">
					{#each found as akt (akt.id)}
						<Command.Item
							value={akt.number}
							onSelect={() => openAkt(akt.number)}
							class="search-item"
						>
							<span class="number">{akt.number}</span>
							<span class="title">{akt.title ?? akt.type.name}</span>
							<Badge tone={akt.state.tone}>{akt.state.name}</Badge>
						</Command.Item>
					{/each}
					{#if text.trim() && !searching && found.length === 0}
						<p class="nothing">Nothing matches “{text.trim()}”.</p>
					{:else if !text.trim()}
						<p class="nothing">Type a number, such as RELEASE-2026, or words of a title.</p>
					{/if}
				</Command.List>
			</Command.Root>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	:global(.search-content) {
		position: fixed;
		top: 12vh;
		left: 50%;
		z-index: 41;
		transform: translateX(-50%);
		width: min(40rem, calc(100vw - 2rem));
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
		overflow: hidden;
	}

	.search-input {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-bottom: 1px solid var(--border);
		color: var(--text-muted);
	}

	.search-input :global(input) {
		border: 0;
		outline: none;
		padding: 0;
		min-height: 2rem;
		font-size: var(--text-md);
		background: transparent;
	}

	:global(.search-list) {
		max-height: 24rem;
		overflow: auto;
		padding: var(--space-2);
	}

	:global(.search-item) {
		display: grid;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		cursor: pointer;
	}

	:global(.search-item[data-selected]) {
		background: var(--bg-muted);
	}

	.number {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	.title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.nothing {
		padding: var(--space-4);
		color: var(--text-muted);
		font-size: var(--text-sm);
	}
</style>
