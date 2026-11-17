<script lang="ts">
	// r[impl app.versions]: an akt's versions, released here with a label,
	// read one at a time, and compared.
	import { resolve } from '$app/paths';
	import Button from '#lib/ui/Button.svelte';
	import Card from '#lib/ui/Card.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { ago } from './history';
	import type { Refusal } from './errors';

	export interface ShownVersion {
		number: number;
		revision: number;
		label?: string | null;
		releasedAt: string;
		releasedBy: { name: string };
	}

	interface Props {
		number: string;
		versions: readonly ShownVersion[];
		/** The akt's `markdown` fields, each version's PDFs. */
		pdfFields?: readonly { key: string; name: string }[];
		/** Whether each version is released with a PDF, uploaded here
		 * (`app.external`). */
		withFile?: boolean;
		/** The version being read, if one is. */
		reading: number | null;
		/** Whether the living version can be released now. */
		releasable: boolean;
		busy: boolean;
		refusal: Refusal | null;
		/** Releases the living version with `label`, together with the
		 * akter `together` names (`version.together`), with `file` if it is
		 * released with one. */
		onRelease: (label: string, together: string[], file: File | null) => Promise<boolean>;
		onCompare: (from: number, to: number | null) => void;
	}

	let {
		number,
		versions,
		pdfFields = [],
		withFile = false,
		reading,
		releasable,
		busy,
		refusal,
		onRelease,
		onCompare
	}: Props = $props();

	let releaseOpen = $state(false);
	let label = $state('');
	let file = $state<File | null>(null);
	/** The akter chosen to release together, of those a refusal named. */
	let together = $derived<string[]>(refusal?.together ?? []);
	let from = $state('');
	let to = $state('');

	// r[impl app.pdf]: each version's PDF, opened where it is shown.
	const pdfOf = (version: number, field: string | null) =>
		`/akter/${encodeURIComponent(number)}/pdf?${field ? `field=${encodeURIComponent(field)}&` : ''}version=${version}`;
	// r[impl app.external]: a version released as a file has that PDF.
	const pdfLinks = $derived(
		withFile
			? [{ key: null, shown: 'Open PDF' }]
			: pdfFields.map((f) => ({
					key: f.key,
					shown: pdfFields.length > 1 ? `Open ${f.name} PDF` : 'Open PDF'
				}))
	);

	const page = (version: number | null) =>
		`${resolve('/akt/[number]', { number })}${version ? `?version=${version}` : ''}`;

	async function release() {
		if (withFile && !file) return;
		if (await onRelease(label, together, file)) {
			releaseOpen = false;
			label = '';
			file = null;
			together = [];
		}
	}
</script>

<Card title="Versions">
	{#if reading}
		<p>
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved, with a query -->
			<a href={page(null)}>Back to the living version</a>
		</p>
	{/if}
	{#if versions.length === 0}
		<p class="hint">No versions yet.</p>
	{:else}
		<ol class="versions" aria-label="Versions">
			{#each versions as v (v.number)}
				<li aria-current={reading === v.number ? 'page' : undefined}>
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved, with a query -->
					<a href={page(v.number)}>Version {v.number}{v.label ? ` (${v.label})` : ''}</a>
					<span class="hint"
						>revision {v.revision} · {v.releasedBy.name} ·
						<time datetime={v.releasedAt} title={new Date(v.releasedAt).toLocaleString()}
							>{ago(v.releasedAt)}</time
						></span
					>
					{#if pdfLinks.length}
						<span class="pdfs">
							{#each pdfLinks as f (f.key)}
								<!-- The server's PDF route, not a page of the app. -->
								<!-- eslint-disable svelte/no-navigation-without-resolve -->
								<a
									href={pdfOf(v.number, f.key)}
									target="_blank"
									rel="noopener"
									aria-label="{f.shown} of version {v.number}">{f.shown}</a
								>
								<!-- eslint-enable svelte/no-navigation-without-resolve -->
							{/each}
						</span>
					{/if}
				</li>
			{/each}
		</ol>
		<form
			class="compare"
			aria-label="Compare versions"
			onsubmit={(e) => {
				e.preventDefault();
				if (from) onCompare(Number(from), to ? Number(to) : null);
			}}
		>
			<label>
				From
				<select bind:value={from}>
					<option value="">Choose…</option>
					{#each versions as v (v.number)}<option value={String(v.number)}
							>Version {v.number}</option
						>{/each}
				</select>
			</label>
			<label>
				To
				<select bind:value={to}>
					<option value="">The living version</option>
					{#each versions as v (v.number)}<option value={String(v.number)}
							>Version {v.number}</option
						>{/each}
				</select>
			</label>
			<Button type="submit" size="sm" disabled={!from}>Compare</Button>
		</form>
	{/if}
	{#if releasable}
		<Button
			disabled={busy}
			onclick={() => {
				releaseOpen = true;
			}}>Release a version</Button
		>
	{/if}
</Card>

<Dialog
	bind:open={releaseOpen}
	title="Release a version"
	description={withFile
		? 'It is the PDF you upload, kept as this version, which then never changes.'
		: 'It names this revision, which then never changes.'}
>
	<form
		id="release-form"
		aria-label="Release a version"
		onsubmit={(e) => {
			e.preventDefault();
			release();
		}}
	>
		{#if withFile}
			<label>
				PDF
				<input
					type="file"
					accept="application/pdf"
					required
					onchange={(e) => (file = e.currentTarget.files?.[0] ?? null)}
				/>
			</label>
		{/if}
		<label>
			Label <span class="optional">(optional)</span>
			<input bind:value={label} maxlength="100" placeholder="2.0" />
		</label>
		{#if refusal}
			<div role="alert">
				<p>{refusal.message}</p>
				{#if refusal.references?.length}
					<ul>
						{#each refusal.references as reference (reference)}<li>{reference}</li>{/each}
					</ul>
				{/if}
			</div>
			{#if refusal.together?.length}
				<!-- r[impl app.versions]: released together, the references between them mended. -->
				<fieldset class="together">
					<legend>Release together with</legend>
					<p class="hint">Each gets its next version, with the same label.</p>
					{#each refusal.together as other (other)}
						<label>
							<input type="checkbox" value={other} bind:group={together} />
							{other}
						</label>
					{/each}
				</fieldset>
			{/if}
		{/if}
	</form>
	{#snippet footer()}
		<Button variant="ghost" onclick={() => (releaseOpen = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="release-form" {busy}>Release</Button>
	{/snippet}
</Dialog>

<style>
	.together {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: var(--space-3) 0 0;
		border: none;
		padding: 0;
	}

	.together legend {
		font-weight: 600;
	}

	.hint {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.versions {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0 0 var(--space-3);
		padding: 0;
		list-style: none;
	}

	.versions li {
		display: flex;
		flex-direction: column;
	}

	.pdfs {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		font-size: var(--text-sm);
	}

	.versions li[aria-current='page'] a {
		font-weight: 600;
	}

	.hint,
	.optional {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.compare {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.compare label,
	form label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	[role='alert'] {
		color: var(--tone-danger-fg);
	}
</style>
