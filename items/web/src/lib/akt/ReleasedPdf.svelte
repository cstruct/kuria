<script lang="ts">
	// r[impl app.external]: an akt written elsewhere shown as the PDF of the
	// version read, else its latest, in place of its file's field.
	interface Props {
		number: string;
		/** The field's name, for assistive technology. */
		name: string;
		/** What the PDF is called, such as the akt's title. */
		title: string;
		/** The version whose PDF is shown; `null` before the first release. */
		version: number | null;
	}

	let { number, name, title, version }: Props = $props();
	const src = $derived(
		version === null ? null : `/akter/${encodeURIComponent(number)}/pdf?version=${version}`
	);
</script>

<div class="row">
	<dt class="visually-hidden">{name}</dt>
	<dd data-field="released-pdf">
		{#if src}
			<!-- The server's PDF route, on Kuria's own origin. -->
			<iframe {src} title="{title}, version {version}"></iframe>
		{:else}
			<p class="empty">No version has been released yet: release one with its PDF.</p>
		{/if}
	</dd>
</div>

<style>
	.row {
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--border);
	}

	.row:last-child {
		border-bottom: 0;
	}

	dd {
		margin: 0;
	}

	/* An A4 page, as wide as the card allows. */
	iframe {
		display: block;
		width: 100%;
		aspect-ratio: 210 / 297;
		max-height: 80vh;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--bg-subtle);
	}

	.empty {
		margin: 0;
		color: var(--text-muted);
	}
</style>
