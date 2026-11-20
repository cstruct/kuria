<script lang="ts">
	// r[impl app.links]
	import { resolve } from '$app/paths';
	import Link2 from '@lucide/svelte/icons/link-2';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import Button from '#lib/ui/Button.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Combobox, { type Option } from '#lib/ui/Combobox.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import type { LinkKind, LinkedCreation } from './document';
	import type { Tone } from '#lib/graphql/generated/graphql.ts';

	export interface ShownLink {
		kind: string;
		name: string;
		outgoing: boolean;
		/** Kept by a reference in the source's text, so changed there. */
		byReference?: boolean;
		pinnedRevision?: number | null;
		pinned?: {
			revision: number;
			title?: string | null;
			state: { name: string; tone: Tone };
		} | null;
		akt: {
			id: string;
			number: string;
			title?: string | null;
			state: { name: string; tone: Tone };
		};
	}

	interface Props {
		links: readonly ShownLink[];
		/** The kinds this akt's type declares, to link from it. */
		kinds: readonly LinkKind[];
		editable: boolean;
		busy: boolean;
		/** Akter to pick for `kind`, found by number or title. */
		search: (text: string, kind: LinkKind) => Promise<Option[]>;
		onAdd: (kind: string, target: string) => void;
		onRemove: (link: ShownLink) => void;
		/** The akt it is on, by number, and what can be created linked to
		 * it (`link.create.carry`). */
		number?: string;
		creations?: readonly LinkedCreation[];
	}

	let {
		links,
		kinds,
		editable,
		busy,
		search,
		onAdd,
		onRemove,
		number = '',
		creations = []
	}: Props = $props();

	/** Where creating `c` linked to this akt starts: its type's create page,
	 * carrying this akt's values. */
	function creating(c: LinkedCreation): string {
		const param = c.outgoing ? 'link' : 'linkedFrom';
		const query = new URLSearchParams({ [param]: `${c.kind}:${number}`, carry: '1' });
		return `${resolve('/types/[key]/new', { key: c.type })}?${query}`;
	}

	let chosen = $state('');
	/** Whether the way to add a link was asked for, when there are none. */
	let adding = $state(false);
	const kind = $derived(kinds.find((k) => k.key === chosen) ?? kinds[0]);

	/** The links by the name this akt reads them by: outgoing first. */
	const groups = $derived.by(() => {
		const byName: { name: string; links: ShownLink[] }[] = [];
		for (const link of [...links].sort((a, b) => Number(b.outgoing) - Number(a.outgoing))) {
			const group = byName.find((g) => g.name === link.name);
			if (group) group.links.push(link);
			else byName.push({ name: link.name, links: [link] });
		}
		return byName;
	});
</script>

{#snippet creatable()}
	<!-- r[impl link.create.carry]: New …, linked to this akt and carrying its values. -->
	{#if creations.length > 0}
		<div class="create">
			{#each creations as c (`${c.type}:${c.kind}:${c.outgoing}`)}
				<Button size="sm" variant="ghost" href={creating(c)}
					><Plus size={14} aria-hidden={true} /> New {c.typeName}
					<span class="hint">{c.via}</span></Button
				>
			{/each}
		</div>
	{/if}
{/snippet}

<!-- r[impl app.links]: none shown when there are none, only a way to add one. -->
{#if groups.length === 0 && !adding}
	{#if (editable && kind) || creations.length > 0}
		<div class="empty">
			{#if editable && kind}
				<Button size="sm" variant="ghost" onclick={() => (adding = true)}
					><Link2 size={14} aria-hidden={true} /> Add link</Button
				>
			{/if}
			{@render creatable()}
		</div>
	{/if}
{:else}
	<Card title="Links">
		{#if groups.length > 0}
			{#each groups as group (group.name)}
				<h3>{group.name}</h3>
				<ul class="links" aria-label={group.name}>
					{#each group.links as link (`${link.kind}:${link.akt.id}`)}
						<li>
							<span class="what">
								<a href={resolve('/akt/[number]', { number: link.akt.number })}>{link.akt.number}</a
								>
								<span class="hint">{link.akt.title ?? ''}</span>
								{#if link.pinned}
									<span class="hint pinned"
										>At revision {link.pinned.revision}: {link.pinned.title ?? link.akt.number}, {link
											.pinned.state.name}</span
									>
								{/if}
							</span>
							<Badge tone={link.akt.state.tone}>{link.akt.state.name}</Badge>
							{#if link.byReference}
								<span class="hint">From the text</span>
							{:else if editable}
								<IconButton
									label="Remove link {link.name} {link.akt.number}"
									icon={X}
									size="sm"
									disabled={busy}
									onclick={() => onRemove(link)}
								/>
							{/if}
						</li>
					{/each}
				</ul>
			{/each}
		{/if}
		{#if editable && kind}
			<div class="add">
				{#if kinds.length > 1}
					<label>
						Kind of link
						<select bind:value={chosen}>
							{#each kinds as option (option.key)}
								<option value={option.key}>{option.name}</option>
							{/each}
						</select>
					</label>
				{/if}
				{#key kind.key}
					<Combobox
						label="{kind.name}…"
						placeholder="{kind.name} an akt, by number or title…"
						search={(text) => search(text, kind)}
						onPick={(option) => onAdd(kind.key, option.value)}
					/>
				{/key}
			</div>
		{/if}
		{@render creatable()}
	</Card>
{/if}

<style>
	.empty {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	.create {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
		margin-top: var(--space-2);
	}

	h3 {
		margin: var(--space-2) 0 var(--space-1);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.links {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0 0 var(--space-2);
		padding: 0;
		list-style: none;
	}

	.links li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.what {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.hint {
		color: var(--text-muted);
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.add {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}

	.add label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 500;
	}
</style>
