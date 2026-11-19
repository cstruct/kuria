<script lang="ts">
	import { resolve } from '$app/paths';
	import { renderMarkdown } from './markdown';
	import { pictureOf } from './members';
	import ReferencePreview from './ReferencePreview.svelte';
	import { following } from './following';
	import { referenceOfHref } from './previews';
	import { renderReferenced, type Reference } from './references';
	import { pagesOf, sheetsOf } from './sheets';
	import { textOf, type Value } from './values';

	/** Where a reference goes in the app: the akt's page, at its version and
	 * heading if it names them. */
	function hrefOf(reference: Reference): string {
		const page = resolve('/akt/[number]', { number: reference.number });
		const version = reference.version ? `?version=${reference.version}` : '';
		return `${page}${version}${reference.heading ? `#${reference.heading}` : ''}`;
	}

	/** `multiline` text is rendered as Markdown. */
	let { value, multiline = false }: { value: Value; multiline?: boolean } = $props();
	let sheetsElement: HTMLDivElement | undefined = $state();

	// r[impl app.trail]: a reference followed goes on along the trail.
	const follow = following();
	const akter = `${resolve('/')}akt/`;
	function followReference(event: MouseEvent) {
		const link = event.target instanceof Element ? event.target.closest('a') : null;
		const href = link?.getAttribute('href');
		if (href && referenceOfHref(href, akter)) follow(event, href);
	}
</script>

{#if value.__typename === 'AttachmentValue'}
	{@const picture = pictureOf(value)}
	{#if picture}
		<img class="picture" src={picture} alt={value.attachment.name} />
	{/if}
	<!-- The server's attachment endpoint, not a page of the app. -->
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
	<a href={value.attachment.url} download={value.attachment.name}>{value.attachment.name}</a>
	{#if value.attachment.sbom}
		<span>
			({value.attachment.sbom.format === 'SPDX' ? 'SPDX' : 'CycloneDX'}
			{value.attachment.sbom.specVersion}, {value.attachment.sbom.components} components)
		</span>
	{/if}
{:else if value.__typename === 'ReferenceValue'}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
	<a href={value.url} rel="noreferrer noopener external">{value.label ?? value.url}</a>
{:else if value.__typename === 'MarkdownValue'}
	<!-- r[impl app.sheets]: A4 sheets, a new one at each page break. -->
	{@const sheets = sheetsOf(
		renderReferenced(value.markdown, value.references, hrefOf, value.images, value.diagrams)
	)}
	<!-- Clicks on the links in it, which are its keyboard's too. -->
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="sheets" bind:this={sheetsElement} onclick={followReference}>
		{#each sheets as sheet, i (i)}
			<!-- Sanitized by renderReferenced. -->
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<div class="sheet" use:pagesOf>{@html sheet}</div>
		{/each}
	</div>
	<ReferencePreview root={sheetsElement} />
{:else if value.__typename === 'TextValue' && multiline}
	<!-- Sanitized by renderMarkdown. -->
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	<div class="markdown">{@html renderMarkdown(value.text)}</div>
{:else if value.__typename === 'TextValue'}
	<span class="text">{value.text}</span>
{:else}
	<span>{textOf(value)}</span>
{/if}

<style>
	.text {
		white-space: pre-wrap;
	}

	.markdown :global(:first-child) {
		margin-top: 0;
	}

	.markdown :global(h1),
	.markdown :global(h2),
	.markdown :global(h3) {
		margin: var(--space-4) 0 var(--space-2);
		font-size: var(--text-md);
	}

	.markdown :global(ul),
	.markdown :global(ol) {
		margin: var(--space-1) 0;
		padding-left: var(--space-5);
	}

	.markdown :global(p) {
		margin: var(--space-2) 0;
	}

	/* Told apart from the text around them by more than colour. */
	.markdown :global(a) {
		text-decoration: underline;
	}

	.picture {
		display: block;
		max-width: 100%;
		max-height: 24rem;
		margin-bottom: var(--space-2);
		object-fit: contain;
	}

	.markdown :global(figure) {
		margin: var(--space-3) 0;
	}

	.markdown :global(img) {
		max-width: 100%;
		height: auto;
	}

	.markdown :global(figcaption) {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.markdown :global(section[aria-label='Footnotes']) {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.markdown :global(small) {
		color: var(--tone-danger-fg);
	}

	.markdown :global(code) {
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		background: var(--bg-muted);
	}
</style>
