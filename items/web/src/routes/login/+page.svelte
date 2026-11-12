<script lang="ts">
	import { page } from '$app/state';
	import { proof } from '#lib/login/dpop.ts';
	import { Konklav } from '#lib/login/konklav.ts';
	import { resolve } from '$app/paths';
	import { cancel, signIn, signOut, start, type Step } from '#lib/login/flow.ts';

	// Konklav's login application: it signs the person in to Konklav and
	// accepts the sign-in another client (Kuria, say) started.
	const konklav = new Konklav(location.origin, proof);
	const id = page.url.searchParams.get('login_request');

	let step = $state<Step>({ kind: 'message', text: 'Checking for a session…' });
	let email = $state('');
	let password = $state('');
	let busy = $state(false);

	function go(next: Step) {
		if (next.kind === 'redirect') {
			location.replace(next.to);
			return;
		}
		if (next.kind === 'form' && !email && next.request.login_hint) {
			email = next.request.login_hint;
		}
		step = next;
		busy = false;
	}

	const signingOut = page.url.searchParams.has('sign_out');

	$effect(() => {
		(signingOut ? signOut(konklav) : start(konklav, id)).then(go, (error: Error) =>
			go({ kind: 'message', text: `Something went wrong: ${error.message}` })
		);
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (step.kind !== 'form' || !id) return;
		busy = true;
		go(await signIn(konklav, id, step.request, email, password));
	}

	async function refuse() {
		if (!id) return;
		busy = true;
		go(await cancel(konklav, id));
	}
</script>

<svelte:head>
	<title>Sign in</title>
</svelte:head>

<main class="login">
	{#if step.kind === 'form'}
		<form onsubmit={submit} aria-labelledby="sign-in-title">
			<h1 id="sign-in-title">Sign in to continue to {step.request.client_id}</h1>
			{#if step.error}
				<p role="alert">{step.error}</p>
			{/if}
			<label>
				Email
				<input type="email" name="email" autocomplete="username" required bind:value={email} />
			</label>
			<label>
				Password
				<input
					type="password"
					name="password"
					autocomplete="current-password"
					required
					bind:value={password}
				/>
			</label>
			<div class="actions">
				<button type="submit" disabled={busy}>Sign in</button>
				<button type="button" onclick={refuse} disabled={busy}>Cancel</button>
			</div>
		</form>
	{:else if step.kind === 'message'}
		<p role="status">{step.text}</p>
		{#if signingOut}<p><a href={resolve('/')}>Sign in again</a></p>{/if}
	{/if}
</main>

<style>
	.login {
		max-width: 24rem;
		margin-top: 10vh;
	}

	form,
	label {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	form {
		gap: 1rem;
	}

	.actions {
		display: flex;
		gap: 0.5rem;
	}
</style>
