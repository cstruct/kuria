<script lang="ts">
	import { textOf, type Value } from './values';

	let { value }: { value: Value } = $props();
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
{:else if value.__typename === 'TextValue'}
	<span class="text">{value.text}</span>
{:else}
	<span>{textOf(value)}</span>
{/if}

<style>
	.text {
		white-space: pre-wrap;
	}
</style>
