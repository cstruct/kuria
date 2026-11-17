<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import { createClient } from '#lib/graphql/client.ts';
	import { setClient } from '#lib/graphql/context.svelte.ts';
	import { Changes } from '#lib/graphql/operations.ts';
	import { page } from '$app/state';
	import type { LayoutProps } from './$types';
	import Shell from '#lib/shell/Shell.svelte';
	import Toaster from '#lib/ui/Toaster.svelte';
	import '../app.css';

	let { children }: LayoutProps = $props();

	const client = createClient();
	setClient(client);

	// The login page is Konklav's, not Kuria's: it neither signs in to Kuria
	// nor follows its changes.
	const app = $derived(!page.url.pathname.startsWith('/login'));

	$effect(() => {
		if (!app) return;
		// Its events are applied by the client's cache (invalidateChange).
		const subscription = client.subscription(Changes, {}).subscribe(() => {});
		return () => subscription.unsubscribe();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} type="image/svg+xml" />
	<link rel="apple-touch-icon" href="/apple-touch-icon.png" />
	<title>Kuria</title>
</svelte:head>

{#if app}
	<Shell>{@render children()}</Shell>
{:else}
	{@render children()}
{/if}
<Toaster />
