<script lang="ts">
	import { renderMarkdown } from './markdown';
	import { textOf, type Value } from './values';

	/** `multiline` text is rendered as Markdown. */
	let { value, multiline = false }: { value: Value; multiline?: boolean } = $props();
</script>

{#if value.__typename === 'AttachmentValue'}
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

	.markdown :global(code) {
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		background: var(--bg-muted);
	}
</style>
