<script lang="ts">
	// r[impl app.reference-preview]: the akt a reference in shown text goes
	// to, previewed while the reference is pointed at or focused.
	import { resolve } from '$app/paths';
	import { Popover } from 'bits-ui';
	import { getClient } from '#lib/graphql/context.svelte.ts';
	import type { ReferencePreviewQuery } from '#lib/graphql/generated/graphql.ts';
	import { ReferencePreview } from '#lib/graphql/operations.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import TypeMark from './TypeMark.svelte';
	import { excerptOf, referenceOfHref } from './previews';
	import type { Reference } from './references';

	/** The shown text whose references are previewed. */
	let { root }: { root: HTMLElement | undefined } = $props();

	const client = getClient();
	const OPEN_AFTER = 400;
	const CLOSE_AFTER = 200;
	const id = $props.id();

	type Previewed = NonNullable<ReferencePreviewQuery['akt']>;

	let open = $state(false);
	let anchor = $state<HTMLAnchorElement | null>(null);
	let shown = $state<{ akt: Previewed; excerpt: string } | null>(null);
	let failed = $state(false);
	let openTimer: ReturnType<typeof setTimeout> | undefined;
	let closeTimer: ReturnType<typeof setTimeout> | undefined;

	/** The path every akt's page is under. */
	const page = `${resolve('/')}akt/`;
	/** The latest load, so that an earlier one answering late is dropped. */
	let loading = 0;

	/** The reference `target` is in, and its link. */
	function referenceAt(target: EventTarget | null): [HTMLAnchorElement, Reference] | null {
		const link = target instanceof Element ? target.closest('a') : null;
		if (!link || !root?.contains(link)) return null;
		const reference = referenceOfHref(link.getAttribute('href') ?? '', page);
		return reference ? [link, reference] : null;
	}

	async function load(reference: Reference) {
		const mine = ++loading;
		shown = null;
		failed = false;
		const result = await client
			.query(ReferencePreview, { number: reference.number, version: reference.version })
			.toPromise();
		if (mine !== loading) return;
		const akt = result.data?.akt;
		if (!akt) {
			failed = true;
			return;
		}
		const texts = akt.fields.flatMap((f) => ('markdown' in f ? [f.markdown] : []));
		const excerpt = reference.heading
			? (texts.map((t) => excerptOf(t, reference.heading)).find(Boolean) ?? '')
			: excerptOf(texts[0] ?? '', null);
		shown = { akt, excerpt };
	}

	function show(link: HTMLAnchorElement, reference: Reference, after: number) {
		clearTimeout(closeTimer);
		if (open && anchor === link) return;
		clearTimeout(openTimer);
		openTimer = setTimeout(() => {
			anchor?.removeAttribute('aria-describedby');
			anchor = link;
			link.setAttribute('aria-describedby', id);
			open = true;
			void load(reference);
		}, after);
	}

	function hide(after = CLOSE_AFTER) {
		clearTimeout(openTimer);
		clearTimeout(closeTimer);
		closeTimer = setTimeout(() => (open = false), after);
	}

	$effect(() => {
		if (!open) anchor?.removeAttribute('aria-describedby');
	});

	$effect(() => {
		const element = root;
		if (!element) return;
		const over = (event: Event) => {
			const found = referenceAt(event.target);
			if (found) show(found[0], found[1], OPEN_AFTER);
		};
		const out = (event: Event) => {
			if (referenceAt(event.target)) hide();
		};
		// Focus shows it sooner: a person tabbing means to read it.
		const focused = (event: Event) => {
			const found = referenceAt(event.target);
			if (found) show(found[0], found[1], 0);
		};
		element.addEventListener('pointerover', over);
		element.addEventListener('pointerout', out);
		element.addEventListener('focusin', focused);
		element.addEventListener('focusout', out);
		return () => {
			clearTimeout(openTimer);
			clearTimeout(closeTimer);
			element.removeEventListener('pointerover', over);
			element.removeEventListener('pointerout', out);
			element.removeEventListener('focusin', focused);
			element.removeEventListener('focusout', out);
		};
	});
</script>

<Popover.Root bind:open>
	<Popover.Portal>
		<Popover.Content
			{id}
			class="reference-preview"
			role="tooltip"
			customAnchor={anchor}
			side="bottom"
			align="start"
			sideOffset={6}
			trapFocus={false}
			onOpenAutoFocus={(e) => e.preventDefault()}
			onCloseAutoFocus={(e) => e.preventDefault()}
			onInteractOutside={() => (open = false)}
			onpointerenter={() => clearTimeout(closeTimer)}
			onpointerleave={() => hide()}
		>
			{#if shown}
				{@const akt = shown.akt}
				<div class="head">
					<TypeMark type={akt.type} named />
					<span class="number">{akt.number}</span>
					<Badge tone={akt.state.tone}>{akt.state.name}</Badge>
				</div>
				<p class="title">{akt.title ?? akt.number}</p>
				{#if akt.version}
					<p class="version">
						Version {akt.version.number}{akt.version.label ? ` (${akt.version.label})` : ''}
					</p>
				{/if}
				{#if shown.excerpt}<p class="excerpt">{shown.excerpt}</p>{/if}
			{:else if failed}
				<p class="excerpt">It couldn’t be read.</p>
			{:else}
				<p class="excerpt">Loading…</p>
			{/if}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>

<style>
	:global(.reference-preview) {
		z-index: 50;
		width: min(22rem, calc(100vw - 2rem));
		padding: var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-raised);
		box-shadow: var(--shadow-md);
		font-size: var(--text-sm);
		color: var(--text);
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}

	.number {
		font-family: var(--font-mono, monospace);
		font-size: var(--text-xs);
		color: var(--text-muted);
	}

	p {
		margin: var(--space-1) 0 0;
	}

	.title {
		font-weight: 600;
	}

	.version,
	.excerpt {
		color: var(--text-muted);
	}
</style>
