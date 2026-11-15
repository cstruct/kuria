<script lang="ts">
	// r[impl app.editor]: a `markdown` field in a rich-text editor offering
	// only what its Markdown holds, references inserted through a picker.
	import { onMount } from 'svelte';
	import { $createCodeNode as createCode } from '@lexical/code';
	import {
		$createHorizontalRuleNode as createRule,
		INSERT_HORIZONTAL_RULE_COMMAND
	} from '@lexical/extension';
	import { createEmptyHistoryState, registerHistory } from '@lexical/history';
	import { $toggleLink as toggleLink } from '@lexical/link';
	import {
		INSERT_CHECK_LIST_COMMAND,
		INSERT_ORDERED_LIST_COMMAND,
		INSERT_UNORDERED_LIST_COMMAND,
		registerCheckList,
		registerList
	} from '@lexical/list';
	import { registerMarkdownShortcuts } from '@lexical/markdown';
	import {
		$createHeadingNode as createHeading,
		$createQuoteNode as createQuote,
		registerRichText,
		type HeadingTagType
	} from '@lexical/rich-text';
	import { $setBlocksType as setBlocksType } from '@lexical/selection';
	import { INSERT_TABLE_COMMAND, registerTablePlugin } from '@lexical/table';
	import { $insertNodeToNearestRoot as insertAtRoot, mergeRegister } from '@lexical/utils';
	import {
		$createParagraphNode as createParagraph,
		$createTextNode as createText,
		$getRoot as getRoot,
		$getSelection as getSelection,
		$isRangeSelection as isRangeSelection,
		$setSelection as setSelection,
		COMMAND_PRIORITY_EDITOR,
		createEditor,
		FORMAT_TEXT_COMMAND,
		type BaseSelection,
		type LexicalEditor,
		type TextFormatType
	} from 'lexical';
	import Button from '#lib/ui/Button.svelte';
	import Combobox, { type Option } from '#lib/ui/Combobox.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { getClient } from '#lib/graphql/context.svelte.ts';
	import { AktReferable } from '#lib/graphql/operations.ts';
	import { pickable } from '../pickers';
	import { destinationOf } from '../references';
	import {
		$createFootnoteDefinitionNode as createFootnoteDefinition,
		$createFootnoteReferenceNode as createFootnoteReference,
		$fromMarkdown as fromMarkdown,
		$nextFootnoteLabel as nextFootnoteLabel,
		$toMarkdown as toMarkdown,
		NODES,
		TRANSFORMERS
	} from './markdown';

	/** What a reference can name in an akt. */
	export interface Referable {
		title: string | null;
		headings: { id: string; text: string; level: number }[];
		versions: { number: number; label?: string | null }[];
	}

	interface Props {
		/** The field's Markdown. */
		text: string;
		label: string;
	}

	let { text = $bindable(), label }: Props = $props();
	const client = getClient();

	/** Akter to reference, found by number or title. */
	const search = (typed: string) => pickable(client, typed, []);

	/** What the akt `number` has to reference: its headings and versions. */
	async function referable(number: string): Promise<Referable | null> {
		const result = await client
			.query(AktReferable, { number }, { requestPolicy: 'network-only' })
			.toPromise();
		const akt = result.data?.akt;
		if (!akt) return null;
		return {
			title: akt.title ?? null,
			headings: akt.fields.flatMap((f) => ('headings' in f ? f.headings : [])),
			versions: akt.versions
		};
	}

	let root: HTMLDivElement;
	let editor: LexicalEditor | undefined;
	let block = $state('paragraph');

	onMount(() => {
		const original = text;
		const instance = createEditor({
			namespace: 'kuria-markdown',
			nodes: NODES,
			onError: (error) => console.error(error),
			theme: {
				paragraph: 'p',
				quote: 'quote',
				link: 'link',
				code: 'code',
				text: { bold: 'bold', italic: 'italic', strikethrough: 'struck', code: 'inline-code' },
				list: {
					checklist: 'checklist',
					listitemChecked: 'checked',
					listitemUnchecked: 'unchecked'
				},
				table: 'table',
				tableCell: 'cell',
				tableCellHeader: 'header'
			}
		});
		instance.setRootElement(root);
		editor = instance;
		const unregister = mergeRegister(
			registerRichText(instance),
			registerHistory(instance, createEmptyHistoryState(), 300),
			registerList(instance),
			registerCheckList(instance),
			registerTablePlugin(instance),
			registerMarkdownShortcuts(instance, TRANSFORMERS),
			instance.registerCommand(
				INSERT_HORIZONTAL_RULE_COMMAND,
				() => {
					insertAtRoot(createRule());
					return true;
				},
				COMMAND_PRIORITY_EDITOR
			)
		);
		instance.update(() => fromMarkdown(original), { discrete: true });
		// Opened and saved unchanged, the value is what it was, as written.
		const unchanged = instance.getEditorState().read(() => toMarkdown());
		const stop = instance.registerUpdateListener(({ editorState }) => {
			const now = editorState.read(() => toMarkdown());
			text = now === unchanged ? original : now;
		});
		return () => {
			stop();
			unregister();
			instance.setRootElement(null);
		};
	});

	function format(kind: TextFormatType) {
		editor?.dispatchCommand(FORMAT_TEXT_COMMAND, kind);
		editor?.focus();
	}

	function setBlock(kind: string) {
		editor?.update(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;
			if (kind === 'paragraph') setBlocksType(selection, () => createParagraph());
			else if (kind === 'quote') setBlocksType(selection, () => createQuote());
			else if (kind === 'code') setBlocksType(selection, () => createCode());
			else setBlocksType(selection, () => createHeading(kind as HeadingTagType));
		});
		block = 'paragraph';
		editor?.focus();
	}

	// The selection when a dialog opened, put back to insert at.
	let saved: BaseSelection | null = null;
	function keepSelection() {
		saved = editor?.getEditorState().read(() => getSelection()?.clone() ?? null) ?? null;
	}

	/** Links the selection to `url`, or inserts `words` linked to it. */
	function link(url: string, words: string) {
		editor?.update(() => {
			if (saved) setSelection(saved.clone());
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;
			if (selection.isCollapsed()) {
				const node = createText(words);
				selection.insertNodes([node]);
				node.select(0, words.length);
			}
			toggleLink(url);
			// Typing goes on after the link, not over it.
			const after = getSelection();
			if (isRangeSelection(after)) {
				const end = after.isBackward() ? after.anchor : after.focus;
				after.anchor.set(end.key, end.offset, end.type);
				after.focus.set(end.key, end.offset, end.type);
			}
		});
		editor?.focus();
	}

	/** A footnote: the next number at the cursor, and its definition at the
	 * end, where typing goes on. */
	function footnote() {
		editor?.update(() => {
			const selection = getSelection();
			if (!isRangeSelection(selection)) return;
			const label = nextFootnoteLabel();
			selection.insertNodes([createFootnoteReference(label)]);
			const definition = createFootnoteDefinition(label);
			getRoot().append(definition);
			definition.select();
			// The note starts plain, whatever the text before it was.
			const inNote = getSelection();
			if (isRangeSelection(inNote)) {
				inNote.format = 0;
				inNote.style = '';
			}
		});
		editor?.focus();
	}

	let linkOpen = $state(false);
	let url = $state('https://');

	let referenceOpen = $state(false);
	let chosen = $state<{ number: string; label: string } | null>(null);
	let found = $state<Referable | null>(null);
	let heading = $state('');
	let version = $state('');

	async function choose(option: Option) {
		chosen = { number: option.value, label: option.label };
		heading = '';
		version = '';
		found = await referable(option.value);
	}

	/** Typing goes on in the editor once a dialog of its closes. */
	function backToEditor(event: Event) {
		event.preventDefault();
		editor?.focus();
	}

	function insertReference() {
		if (!chosen) return;
		const destination = destinationOf({
			number: chosen.number,
			version: version ? Number(version) : null,
			heading: heading || null
		});
		const named = found?.headings.find((h) => h.id === heading);
		link(destination, named?.text ?? found?.title ?? chosen.number);
		referenceOpen = false;
	}
</script>

<div class="editor">
	<div class="toolbar" role="toolbar" aria-label="Format {label}">
		<select aria-label="Block" bind:value={block} onchange={(e) => setBlock(e.currentTarget.value)}>
			<option value="paragraph">Text</option>
			<option value="h1">Heading 1</option>
			<option value="h2">Heading 2</option>
			<option value="h3">Heading 3</option>
			<option value="quote">Quote</option>
			<option value="code">Code block</option>
		</select>
		<Button size="sm" variant="ghost" onclick={() => format('bold')}>Bold</Button>
		<Button size="sm" variant="ghost" onclick={() => format('italic')}>Italic</Button>
		<Button size="sm" variant="ghost" onclick={() => format('strikethrough')}>Strikethrough</Button>
		<Button size="sm" variant="ghost" onclick={() => format('code')}>Code</Button>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => editor?.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, undefined)}
			>Bulleted list</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => editor?.dispatchCommand(INSERT_ORDERED_LIST_COMMAND, undefined)}
			>Numbered list</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => editor?.dispatchCommand(INSERT_CHECK_LIST_COMMAND, undefined)}
			>Task list</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() =>
				editor?.dispatchCommand(INSERT_TABLE_COMMAND, {
					rows: '3',
					columns: '2',
					includeHeaders: { rows: true, columns: false }
				})}>Table</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => editor?.dispatchCommand(INSERT_HORIZONTAL_RULE_COMMAND, undefined)}
			>Rule</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => {
				keepSelection();
				url = 'https://';
				linkOpen = true;
			}}>Link</Button
		>
		<Button
			size="sm"
			variant="ghost"
			onclick={() => {
				keepSelection();
				chosen = null;
				found = null;
				referenceOpen = true;
			}}>Reference</Button
		>
		<Button size="sm" variant="ghost" onclick={footnote}>Footnote</Button>
	</div>
	<div
		class="content markdown"
		bind:this={root}
		contenteditable="true"
		role="textbox"
		aria-multiline="true"
		aria-label={label}
		tabindex="0"
	></div>
</div>

<Dialog bind:open={linkOpen} title="Link" onCloseAutoFocus={backToEditor}>
	<form
		id="link-form"
		aria-label="Link"
		onsubmit={(e) => {
			e.preventDefault();
			link(url.trim(), url.trim());
			linkOpen = false;
		}}
	>
		<label>Address <input type="url" bind:value={url} required /></label>
	</form>
	{#snippet footer()}
		<Button variant="ghost" onclick={() => (linkOpen = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="link-form">Link</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={referenceOpen}
	title="Reference"
	onCloseAutoFocus={backToEditor}
	description="A link to an akt, one of its headings, and its living version or one of its versions."
>
	<form
		id="reference-form"
		aria-label="Reference"
		onsubmit={(e) => {
			e.preventDefault();
			insertReference();
		}}
	>
		<Combobox label="Akt" placeholder="Number or title" {search} onPick={choose} />
		{#if chosen}
			<p>{chosen.label} <span class="hint">{chosen.number}</span></p>
			<label>
				Heading
				<select bind:value={heading}>
					<option value="">The whole akt</option>
					{#each found?.headings ?? [] as h (h.id)}
						<option value={h.id}>{'· '.repeat(h.level - 1)}{h.text}</option>
					{/each}
				</select>
			</label>
			<label>
				Version
				<select bind:value={version}>
					<option value="">The living version</option>
					{#each found?.versions ?? [] as v (v.number)}
						<option value={String(v.number)}
							>Version {v.number}{v.label ? ` (${v.label})` : ''}</option
						>
					{/each}
				</select>
			</label>
		{/if}
	</form>
	{#snippet footer()}
		<Button variant="ghost" onclick={() => (referenceOpen = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="reference-form" disabled={!chosen}>Insert</Button>
	{/snippet}
</Dialog>

<style>
	.editor {
		border: 1px solid var(--border);
		border-radius: var(--radius-md, var(--radius-sm));
		background: var(--bg);
	}

	.toolbar {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
		padding: var(--space-1);
		border-bottom: 1px solid var(--border);
	}

	.content {
		min-height: 10rem;
		padding: var(--space-3);
		outline: none;
	}

	.content:focus-visible {
		outline: 2px solid var(--focus, var(--accent));
		outline-offset: -2px;
	}

	.content :global(.bold) {
		font-weight: 600;
	}

	.content :global(.italic) {
		font-style: italic;
	}

	.content :global(.struck) {
		text-decoration: line-through;
	}

	.content :global(.inline-code),
	.content :global(.code) {
		font-family: var(--font-mono);
		background: var(--bg-muted);
	}

	.content :global(.code) {
		display: block;
		padding: var(--space-2);
		white-space: pre-wrap;
	}

	.content :global(.quote) {
		margin: var(--space-2) 0;
		padding-left: var(--space-3);
		border-left: 3px solid var(--border);
		color: var(--text-muted);
	}

	.content :global(.link) {
		color: var(--accent);
		text-decoration: underline;
	}

	.content :global(.table) {
		border-collapse: collapse;
	}

	.content :global(.cell) {
		min-width: 4rem;
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border);
	}

	.content :global(.header) {
		font-weight: 600;
		background: var(--bg-muted);
	}

	.content :global(.footnote-reference) {
		font-size: var(--text-sm);
		vertical-align: super;
		color: var(--accent);
	}

	.content :global(.footnote-definition) {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.content :global(.footnote-definition)::before {
		content: '[^' attr(data-label) ']: ';
		font-family: var(--font-mono);
	}

	.content :global(.checked) {
		text-decoration: line-through;
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	form label {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.hint {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}
</style>
