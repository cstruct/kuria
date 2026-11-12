<script lang="ts">
	import favicon from '#lib/assets/favicon.svg';
	import { createClient } from '#lib/graphql/client.ts';
	import { setClient } from '#lib/graphql/context.svelte.ts';
	import { Changes } from '#lib/graphql/operations.ts';
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

{@render children()}
