<script lang="ts">
	import { setCrumbs } from '#lib/shell/crumbs.svelte.ts';
	import { resolve } from '$app/paths';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash from '@lucide/svelte/icons/trash-2';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox, { type Option } from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Field from '#lib/ui/Field.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Mono from '#lib/ui/Mono.svelte';
	import { toast } from '#lib/ui/toast.svelte.ts';

	// The app's components on one page, for checking them by eye and with
	// automated accessibility checks, in light and in dark.
	const TONES = ['NEUTRAL', 'ACTIVE', 'SUCCESS', 'WARNING', 'DANGER'] as const;
	const OPTIONS: Option[] = [
		{ value: 'EPIC-001', label: 'Sign-in', hint: 'EPIC-001 · Epic' },
		{ value: 'ITEM-0003', label: 'Password reset', hint: 'ITEM-0003 · Backlog item' },
		{ value: 'ITEM-0001', label: 'Passkeys', hint: 'ITEM-0001 · Backlog item' }
	];

	setCrumbs(() => [{ label: 'Design' }]);
	let dialog = $state(false);
	let picked = $state('');
	let theme = $state<'system' | 'light' | 'dark'>('system');

	$effect(() => {
		if (theme === 'system') document.documentElement.removeAttribute('data-theme');
		else document.documentElement.setAttribute('data-theme', theme);
		return () => document.documentElement.removeAttribute('data-theme');
	});
</script>

<svelte:head>
	<title>Design · Kuria</title>
</svelte:head>

<main class="gallery">
	<header>
		<h1>Design</h1>
		<label class="theme">
			Theme
			<select bind:value={theme}>
				<option value="system">System</option>
				<option value="light">Light</option>
				<option value="dark">Dark</option>
			</select>
		</label>
	</header>

	<Card title="Buttons">
		<div class="row">
			<Button variant="primary">Release</Button>
			<Button>Edit</Button>
			<Button variant="ghost">Cancel</Button>
			<Button variant="danger">Erase</Button>
			<Button variant="primary" busy>Saving</Button>
			<Button size="sm"><Plus size={14} aria-hidden={true} /> Add</Button>
			<Button href={resolve('/design')}>A link</Button>
		</div>
		<div class="row">
			<IconButton label="Move up" icon={ArrowUp} />
			<IconButton label="Remove" icon={Trash} />
			<IconButton label="Move up" icon={ArrowUp} disabled />
			<Menu
				label="More actions"
				items={[
					{ label: 'Draft text', onSelect: () => toast('Drafted text', 'success') },
					{ label: 'Add to container', onSelect: () => toast('Added') },
					{
						label: 'Remove',
						onSelect: () => toast('Removed', 'danger'),
						danger: true,
						separated: true
					}
				]}
			/>
			<Menu
				label="Add a member"
				text="Add"
				items={[{ label: 'New backlog item', onSelect: () => {} }]}
			/>
		</div>
	</Card>

	<Card title="States">
		<div class="row">
			{#each TONES as tone (tone)}<Badge {tone}>{tone.toLowerCase()}</Badge>{/each}
		</div>
	</Card>

	<Card title="Fields">
		<div class="fields">
			<Field label="Title" required hint="What it is, in a few words.">
				{#snippet children({ id, describedBy })}
					<input {id} aria-describedby={describedBy} value="Sign-in" />
				{/snippet}
			</Field>
			<Field label="Version" error="Version is required.">
				{#snippet children({ id, describedBy, invalid })}
					<input {id} aria-describedby={describedBy} aria-invalid={invalid} />
				{/snippet}
			</Field>
			<Field label="Kind">
				{#snippet children({ id })}
					<select {id}><option>Feature</option><option>Fix</option></select>
				{/snippet}
			</Field>
			<Field label="Goal">
				{#snippet children({ id })}<textarea {id}></textarea>{/snippet}
			</Field>
			<Combobox
				label="Find an akt"
				placeholder="Number or title…"
				search={async (text) =>
					OPTIONS.filter((o) => `${o.value} ${o.label}`.toLowerCase().includes(text.toLowerCase()))}
				onPick={(option) => (picked = option.value)}
			/>
			{#if picked}<p>Picked {picked}</p>{/if}
		</div>
	</Card>

	<Card title="Values">
		<p>
			Image <Mono value={`registry.example/shop/backend@sha256:${'e'.repeat(64)}`} max={40} copy />
		</p>
	</Card>

	<Card title="Empty" padded={false}>
		<EmptyState
			title="No members yet"
			description="Add backlog items or epics to plan the release."
		>
			{#snippet action()}<Button variant="primary">Add</Button>{/snippet}
		</EmptyState>
	</Card>

	<Card title="Dialog and toasts">
		<div class="row">
			<Button onclick={() => (dialog = true)}>Open a dialog</Button>
			<Button onclick={() => toast('Saved', 'success')}>Show a toast</Button>
		</div>
	</Card>

	<Dialog
		bind:open={dialog}
		title="Release"
		description="Releasing needs the version, the date and the go."
	>
		<Field label="Version">
			{#snippet children({ id })}<input {id} />{/snippet}
		</Field>
		{#snippet footer()}
			<Button variant="ghost" onclick={() => (dialog = false)}>Cancel</Button>
			<Button variant="primary" onclick={() => (dialog = false)}>Release</Button>
		{/snippet}
	</Dialog>
</main>

<style>
	.gallery {
		display: flex;
		flex-direction: column;
		gap: var(--space-5);
	}

	header {
		display: flex;
		align-items: end;
		justify-content: space-between;
	}

	.theme {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.row + .row {
		margin-top: var(--space-3);
	}

	.fields {
		display: grid;
		gap: var(--space-4);
		max-width: 32rem;
	}
</style>
