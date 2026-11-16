<script lang="ts">
	// r[impl app.versions]: two versions, or a version and the living
	// version, compared field by field, text line by line.
	import { untrack } from 'svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { getClient } from '#lib/graphql/context.svelte.ts';
	import { AktFieldsAt } from '#lib/graphql/operations.ts';
	import { lineDiff, type DiffLine } from './diff';
	import { textOf, type FieldDef, type Value } from './values';

	interface Props {
		open: boolean;
		number: string;
		from: number;
		/** `null` for the living version. */
		to: number | null;
		fields: readonly FieldDef[];
	}

	let { open = $bindable(), number, from, to, fields }: Props = $props();
	const client = getClient();
	type Read = { before: readonly Value[]; after: readonly Value[] };
	let read = $state<Read | null>(null);
	let failed = $state(false);

	function text(value: Value | undefined): string {
		if (!value) return '';
		if (value.__typename === 'MarkdownValue') return value.markdown;
		return textOf(value);
	}

	// Read when it opens or the versions change, and only then. Reading the
	// living version refreshes the page's akt in the cache, so every prop is
	// handed down again; a string signals only when it changes, so the reads
	// follow it rather than the props, and the diff is worked out from the
	// fields without reading again.
	const asked = $derived(open ? JSON.stringify([number, from, to]) : null);

	$effect(() => {
		if (!asked) return;
		const [akt, before, after] = JSON.parse(asked) as [string, number, number | null];
		let current = true;
		untrack(() => {
			read = null;
			failed = false;
		});
		const at = (version: number | null) =>
			client
				.query(AktFieldsAt, { number: akt, version }, { requestPolicy: 'network-only' })
				.toPromise();
		Promise.all([at(before), at(after)]).then(([a, b]) => {
			if (!current) return;
			if (!a.data?.akt || !b.data?.akt) {
				failed = true;
				return;
			}
			read = { before: a.data.akt.fields, after: b.data.akt.fields };
		});
		return () => {
			current = false;
		};
	});

	const compared = $derived.by(() => {
		if (!read) return null;
		const { before, after } = read;
		const valueOf = (values: readonly Value[], key: string) =>
			values.find((v) => 'field' in v && v.field === key);
		return fields.flatMap((field): { name: string; lines: DiffLine[] }[] => {
			const was = text(valueOf(before, field.key));
			const is = text(valueOf(after, field.key));
			return was === is ? [] : [{ name: field.name, lines: lineDiff(was, is) }];
		});
	});
</script>

<Dialog bind:open title="Version {from} → {to ? `version ${to}` : 'now'}">
	{#if failed}
		<p role="alert">Those versions couldn’t be read.</p>
	{:else if !compared}
		<p>Comparing…</p>
	{:else if compared.length === 0}
		<p>Nothing changed.</p>
	{:else}
		{#each compared as field (field.name)}
			<h3>{field.name}</h3>
			<pre class="diff" aria-label="Changes to {field.name}">{#each field.lines as line, i (i)}<span
						class={line.tag === '+' ? 'added' : line.tag === '-' ? 'removed' : ''}
						>{line.tag === '+' ? '+ ' : line.tag === '-' ? '− ' : '  '}{line.text}
</span>{/each}</pre>
		{/each}
	{/if}
</Dialog>

<style>
	h3 {
		margin: var(--space-3) 0 var(--space-1);
		font-size: var(--text-md);
	}

	.diff {
		overflow-x: auto;
		margin: 0;
		padding: var(--space-2);
		border-radius: var(--radius-sm);
		background: var(--bg-muted);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		white-space: pre-wrap;
	}

	.added {
		color: var(--tone-success-fg);
	}

	.removed {
		color: var(--tone-danger-fg);
		text-decoration: line-through;
	}
</style>
