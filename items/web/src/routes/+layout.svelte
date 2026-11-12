<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import { createClient } from '#lib/graphql/client.ts';
	import { setClient } from '#lib/graphql/context.svelte.ts';
	import { Changes } from '#lib/graphql/operations.ts';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { LayoutProps } from './$types';
	import '../app.css';

	let { children }: LayoutProps = $props();

	const client = createClient();
	setClient(client);

	// The login page is Konklav's, not Kuria's: it neither signs in to Kuria
	// nor follows its changes.
	const app = $derived(!page.url.pathname.startsWith('/login'));

	$effect(() => {
		if (!app) return;
		// r[impl app.live]
		const subscription = client.subscription(Changes, {}).subscribe(() => {});
		return () => subscription.unsubscribe();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Kuria</title>
</svelte:head>

{#if app}
	<nav class="top" aria-label="Kuria">
		<a href={resolve('/')}>Kuria</a>
		<a href={resolve('/contacts')}>Contacts</a>
	</nav>
{/if}
{@render children()}

<style>
	.top {
		display: flex;
		gap: 1rem;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid color-mix(in srgb, currentColor 20%, transparent);
	}
</style>
