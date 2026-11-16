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
		$getNodeByKey as getNodeByKey,
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
	import {
		AktReferable,
		CreateAkt,
		ImagePicture,
		ImageTypes,
		RenderDiagram
	} from '#lib/graphql/operations.ts';
	import { $isCodeNode as isCodeNode } from '@lexical/code';
	import { upload } from '../upload';
	import { pickable } from '../pickers';
	import { destinationOf } from '../references';
	import { A4, guideOffsets, pagesOf } from '../sheets';
	import {
		$toDiagram as toDiagram,
		setDiagramRenderer,
		$createImageNode as createImage,
		$createPageBreakNode as createPageBreak,
		$createFootnoteDefinitionNode as createFootnoteDefinition,
		$createFootnoteReferenceNode as createFootnoteReference,
		$fromMarkdown as fromMarkdown,
		$nextFootnoteLabel as nextFootnoteLabel,
		$toMarkdown as toMarkdown,
		NODES,
		TRANSFORMERS
	} from './markdown';
	import { bindText, cursorIn, selectionIn } from './live';
	import type { LiveDraft } from '../live.svelte';

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
		/** Each embedded image's picture, by destination. */
		pictures?: [string, string][];
		/** Each diagram block's rendering, by source. */
		drawings?: [string, string][];
		/** Whether the field's `diagram` blocks are diagrams. */
		diagrams?: boolean;
		/** The live draft whose text `field` the editor edits, instead of
		 * `text` (`app.live`). */
		live?: { draft: LiveDraft; field: string } | null;
	}

	let {
		text = $bindable(),
		label,
		pictures = [],
		drawings = [],
		diagrams = false,
		live = null
	}: Props = $props();

	// r[impl app.live]: where everyone else's cursor is in the text.
	let carets = $state<
		{ client: number; name: string; color: string; top: number; left: number; height: number }[]
	>([]);
	function placeCarets() {
		if (!editor || !live || !paper) return;
		const field = live.field;
		const ytext = live.draft.text(field);
		const origin = paper.getBoundingClientRect();
		const placed: typeof carets = [];
		for (const peer of live.draft.peers) {
			if (peer.cursor?.field !== field) continue;
			const at = cursorIn(editor, ytext, peer.cursor.head);
			const element = at ? editor.getElementByKey(at.key) : null;
			if (!at || !element) continue;
			let rect: DOMRect;
			const node = element.firstChild;
			if (at.type === 'text' && node?.nodeType === Node.TEXT_NODE) {
				const range = document.createRange();
				const offset = Math.min(at.offset, node.textContent?.length ?? 0);
				range.setStart(node, offset);
				range.setEnd(node, offset);
				rect = range.getBoundingClientRect();
			} else {
				rect = element.getBoundingClientRect();
			}
			placed.push({
				client: peer.client,
				name: peer.name,
				color: peer.color,
				top: rect.top - origin.top,
				left: rect.left - origin.left,
				height: rect.height || 18
			});
		}
		carets = placed;
	}
	$effect(() => {
		// Again whenever someone moves.
		void live?.draft.peers;
		placeCarets();
	});

	// r[impl app.editor]: diagrams shown as their pictures, rendered without
	// being stored (`diagram.preview`), each source once.
	// A cache, not state: nothing is shown from it until a render resolves.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const rendered = new Map<string, Promise<{ src: string } | { error: string }>>();
	function renderDiagram(source: string): Promise<{ src: string } | { error: string }> {
		const known = drawings.find(([s]) => s === source.replace(/\n$/, ''));
		if (known) return Promise.resolve({ src: known[1] });
		let found = rendered.get(source);
		if (!found) {
			found = client
				.query(RenderDiagram, { source }, { requestPolicy: 'network-only' })
				.toPromise()
				.then((result) => {
					const drawn = result.data?.renderDiagram;
					if (drawn?.svg) {
						const bytes = new TextEncoder().encode(drawn.svg);
						let binary = '';
						for (const b of bytes) binary += String.fromCharCode(b);
						return { src: `data:image/svg+xml;base64,${btoa(binary)}` };
					}
					return { error: drawn?.error ?? result.error?.message ?? 'it couldn’t be rendered' };
				});
			rendered.set(source, found);
		}
		return found;
	}

	/** The diagram code block the cursor is in, if any, and its preview. */
	let diagramCode = $state<string | null>(null);
	let diagramSource = $state('');
	let diagramPreview = $state<{ src: string } | { error: string } | null>(null);
	let previewTimer: ReturnType<typeof setTimeout> | undefined;
	function followDiagramCode() {
		const found = editor?.getEditorState().read(() => {
			const selection = getSelection();
			// Losing focus, as to a button of the preview, keeps the block.
			if (!isRangeSelection(selection)) {
				const kept = diagramCode ? getNodeByKey(diagramCode) : null;
				return isCodeNode(kept) ? { key: kept.getKey(), source: kept.getTextContent() } : null;
			}
			const top = selection.anchor.getNode().getTopLevelElement();
			return isCodeNode(top) && top.getLanguage() === 'diagram'
				? { key: top.getKey(), source: top.getTextContent() }
				: null;
		});
		diagramCode = found?.key ?? null;
		if (!found || found.source === diagramSource) return;
		diagramSource = found.source;
		clearTimeout(previewTimer);
		// Typing re-renders after a pause.
		previewTimer = setTimeout(() => {
			void renderDiagram(found.source).then((drawn) => {
				if (diagramSource === found.source) diagramPreview = drawn;
			});
		}, 500);
	}

	function showDiagram() {
		const key = diagramCode;
		editor?.update(() => {
			const node = key ? getNodeByKey(key) : null;
			if (node) toDiagram(node);
		});
		diagramCode = null;
		diagramPreview = null;
		diagramSource = '';
	}

	// r[impl app.sheets]: a page break inserted at the cursor, typing going
	// on in the next sheet.
	function insertPageBreak() {
		editor?.update(() => {
			const pageBreak = createPageBreak();
			insertAtRoot(pageBreak);
			if (!pageBreak.getNextSibling()) pageBreak.insertAfter(createParagraph());
			pageBreak.selectNext();
		});
		editor?.focus();
	}

	function insertDiagram() {
		editor?.update(() => {
			const code = createCode('diagram');
			code.append(createText('#diagram(node((0, 0), [Start]), edge("-|>"), node((1, 0), [End]))'));
			insertAtRoot(code);
			code.selectEnd();
		});
		editor?.focus();
	}
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
	let paper: HTMLDivElement | undefined = $state();
	/** Where the guides are, from the top of the sheets, and how tall. */
	let guides = $state<{ tops: number[]; height: number }>({ tops: [], height: 0 });
	let editor: LexicalEditor | undefined;

	// r[impl app.sheets]: a faint guide wherever a sheet would end, counted
	// from the sheet's start: the top, or the page break before it.
	function placeGuides() {
		if (!root) return;
		const sheets: [number, number][] = [];
		let start = 0;
		for (const pageBreak of root.querySelectorAll<HTMLElement>(':scope > .page-break')) {
			sheets.push([start, pageBreak.offsetTop]);
			start = pageBreak.offsetTop + pageBreak.offsetHeight;
		}
		sheets.push([start, root.offsetHeight]);
		const tops = guideOffsets(sheets, root.offsetWidth * A4);
		const height = root.offsetHeight;
		if (height !== guides.height || tops.join() !== guides.tops.join()) {
			guides = { tops, height };
		}
	}
	/** Where the last sheet starts: after the last page break, or the top. */
	function afterLastBreak(): number {
		const breaks = root?.querySelectorAll<HTMLElement>(':scope > .page-break');
		const last = breaks?.[breaks.length - 1];
		return last ? last.offsetTop + last.offsetHeight : 0;
	}
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
		const resized = new ResizeObserver(placeGuides);
		resized.observe(root);
		setDiagramRenderer(instance, renderDiagram);
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
		if (live) {
			// r[impl app.live]: bound to the live draft's text both ways, and
			// saying where the cursor is.
			const { draft, field } = live;
			const ytext = draft.text(field);
			const unbind = bindText(instance, ytext, {
				origin: draft.origin,
				pictures: new Map(pictures),
				diagrams,
				onRemote: placeCarets
			});
			const stop = instance.registerUpdateListener(() => {
				const selection = instance.getRootElement()?.contains(document.activeElement)
					? selectionIn(instance, ytext)
					: null;
				draft.setCursor(selection ? { field, ...selection } : null);
				if (diagrams) followDiagramCode();
				placeGuides();
				placeCarets();
			});
			return () => {
				draft.setCursor(null);
				resized.disconnect();
				stop();
				unbind();
				unregister();
				instance.setRootElement(null);
			};
		}
		instance.update(() => fromMarkdown(original, new Map(pictures), { diagrams }), {
			discrete: true
		});
		// Opened and saved unchanged, the value is what it was, as written.
		const unchanged = instance.getEditorState().read(() => toMarkdown());
		const stop = instance.registerUpdateListener(({ editorState }) => {
			const now = editorState.read(() => toMarkdown());
			text = now === unchanged ? original : now;
			if (diagrams) followDiagramCode();
			placeGuides();
		});
		return () => {
			resized.disconnect();
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

	// r[impl app.editor]: an image akt found, or a file uploaded as a new
	// one, embedded at the cursor.
	interface ImageType {
		key: string;
		name: string;
		title: string | null;
		image: string;
	}
	let imageOpen = $state(false);
	let imageTypes = $state<ImageType[]>([]);
	let imageType = $state('');
	let picked = $state<{ number: string; title: string | null; url: string | null } | null>(null);
	let file = $state<File | null>(null);
	let alt = $state('');
	let imageError = $state('');
	let imageBusy = $state(false);

	async function openImage() {
		keepSelection();
		picked = null;
		file = null;
		alt = '';
		imageError = '';
		imageOpen = true;
		const result = await client.query(ImageTypes, {}).toPromise();
		imageTypes = (result.data?.aktTypes ?? []).flatMap((t) =>
			t.presentation.image
				? [
						{
							key: t.key,
							name: t.name,
							title: t.presentation.title ?? null,
							image: t.presentation.image
						}
					]
				: []
		);
		imageType = imageTypes[0]?.key ?? '';
	}

	/** Image akter to embed, found by number or title. */
	const searchImages = (typed: string) =>
		pickable(
			client,
			typed,
			[],
			imageTypes.map((t) => t.key)
		);

	async function pickImage(option: Option) {
		file = null;
		const result = await client
			.query(ImagePicture, { number: option.value }, { requestPolicy: 'network-only' })
			.toPromise();
		const akt = result.data?.akt;
		const key = akt?.type.presentation.image;
		const value = akt?.fields.find((f) => 'field' in f && f.field === key);
		picked = {
			number: option.value,
			title: akt?.title ?? null,
			url: value && 'attachment' in value ? value.attachment.url : null
		};
		alt = alt || (akt?.title ?? option.label);
	}

	function chooseFile(chosen: File | null) {
		file = chosen;
		if (chosen) {
			picked = null;
			alt = alt || chosen.name.replace(/\.[^.]+$/, '');
		}
	}

	/** The picked image, or a new image akt holding the uploaded file. */
	async function imageToEmbed(): Promise<{ number: string; url: string | null } | null> {
		if (picked) return picked;
		const type = imageTypes.find((t) => t.key === imageType);
		if (!file || !type) return null;
		const uploaded = await upload(file);
		const fields: Record<string, unknown> = { [type.image]: uploaded.id };
		if (type.title) fields[type.title] = file.name.replace(/\.[^.]+$/, '') || file.name;
		const created = await client
			.mutation(CreateAkt, { input: { type: type.key, in: [], fields } })
			.toPromise();
		if (created.error || !created.data)
			throw new Error(created.error?.message ?? 'The image wasn’t created.');
		return { number: created.data.createAkt.number, url: `/attachments/${uploaded.id}` };
	}

	async function insertImage() {
		imageBusy = true;
		imageError = '';
		try {
			const image = await imageToEmbed();
			if (!image) return;
			editor?.update(() => {
				if (saved) setSelection(saved.clone());
				const selection = getSelection();
				if (!isRangeSelection(selection)) return;
				selection.insertNodes([
					createImage(`kuria:${image.number}`, alt.trim() || image.number, image.url)
				]);
			});
			imageOpen = false;
		} catch (e) {
			imageError = e instanceof Error ? e.message : String(e);
		} finally {
			imageBusy = false;
		}
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
		<Button size="sm" variant="ghost" onclick={insertPageBreak}>Page break</Button>
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
		<Button size="sm" variant="ghost" onclick={openImage}>Image</Button>
		{#if diagrams}
			<Button size="sm" variant="ghost" onclick={insertDiagram}>Diagram</Button>
		{/if}
	</div>
	<div class="sheets">
		<div class="paper" bind:this={paper}>
			{#each carets as caret (caret.client)}
				<span
					class="caret"
					style:top="{caret.top}px"
					style:left="{caret.left}px"
					style:height="{caret.height}px"
					style:--caret={caret.color}
					data-testid="caret"
					aria-hidden="true"><span class="caret-name">{caret.name}</span></span
				>
			{/each}
			<div
				class="content sheet edited"
				bind:this={root}
				use:pagesOf={afterLastBreak}
				contenteditable="true"
				role="textbox"
				aria-multiline="true"
				aria-label={label}
				tabindex="0"
			></div>
			<svg class="guides" aria-hidden="true" width="100%" height={guides.height}>
				{#each guides.tops as top (top)}
					<line x1="0" x2="100%" y1={top + 0.5} y2={top + 0.5} />
				{/each}
			</svg>
		</div>
	</div>
	{#if diagramCode}
		<section class="diagram-preview" aria-label="Diagram preview">
			<div class="preview-head">
				<span>Diagram preview</span>
				<Button size="sm" variant="ghost" onclick={showDiagram}>Show diagram</Button>
			</div>
			{#if diagramPreview && 'src' in diagramPreview}
				<img src={diagramPreview.src} alt={diagramSource} />
			{:else if diagramPreview}
				<p role="alert">The diagram doesn’t compile: {diagramPreview.error}</p>
			{:else}
				<p>Rendering the diagram…</p>
			{/if}
		</section>
	{/if}
</div>

<Dialog
	bind:open={imageOpen}
	title="Image"
	onCloseAutoFocus={backToEditor}
	description="An image akt, found or made from a file you upload, shown here."
>
	<form
		id="image-form"
		aria-label="Image"
		onsubmit={(e) => {
			e.preventDefault();
			void insertImage();
		}}
	>
		{#if imageTypes.length === 0}
			<p>No akt type holds images.</p>
		{:else}
			<Combobox
				label="Find an image"
				placeholder="Number or title"
				search={searchImages}
				onPick={pickImage}
			/>
			{#if picked}
				<p>{picked.title ?? picked.number} <span class="hint">{picked.number}</span></p>
			{/if}
			<label>
				Or upload a file
				<input
					type="file"
					accept="image/png,image/jpeg,image/svg+xml,image/webp"
					onchange={(e) => chooseFile(e.currentTarget.files?.[0] ?? null)}
				/>
			</label>
			{#if file && imageTypes.length > 1}
				<label>
					As a new
					<select bind:value={imageType}>
						{#each imageTypes as t (t.key)}
							<option value={t.key}>{t.name}</option>
						{/each}
					</select>
				</label>
			{/if}
			<label>Alt text <input bind:value={alt} /></label>
			{#if imageError}<p role="alert">{imageError}</p>{/if}
		{/if}
	</form>
	{#snippet footer()}
		<Button variant="ghost" onclick={() => (imageOpen = false)}>Cancel</Button>
		<Button
			variant="primary"
			type="submit"
			form="image-form"
			disabled={imageBusy || (!picked && !file)}>Insert</Button
		>
	{/snippet}
</Dialog>

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

	.sheets {
		padding: var(--space-4) var(--space-3);
		background: var(--sheet-outside);
	}

	.paper {
		position: relative;
		width: min(100cqi, 210mm);
		margin-inline: auto;
	}

	.content {
		/* A whole page, as the sheets it is shown as (app.sheets). */
		min-height: var(--sheet-height);
		outline: none;
	}

	.guides {
		position: absolute;
		top: 0;
		left: 0;
		pointer-events: none;
	}

	.guides line {
		stroke: var(--sheet-guide);
		stroke-dasharray: 4 4;
	}

	/* Someone else's cursor, and their name above it. */
	.caret {
		position: absolute;
		z-index: 1;
		width: 2px;
		margin-left: -1px;
		background: var(--caret);
		pointer-events: none;
	}

	.caret-name {
		position: absolute;
		bottom: 100%;
		left: 0;
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		background: var(--caret);
		color: #fff;
		font-size: var(--text-xs, 0.75rem);
		line-height: 1.4;
		white-space: nowrap;
	}

	/* A page break: the end of one sheet, a gap, and the start of the next. */
	.content :global(.page-break) {
		height: var(--space-4);
		margin: var(--sheet-margin) calc(-1 * var(--sheet-margin));
		background: var(--sheet-outside);
		box-shadow:
			inset 0 1px 0 var(--sheet-edge),
			inset 0 -1px 0 var(--sheet-edge);
		cursor: default;
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

	.content :global(.diagram) {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-1);
		margin: var(--space-2) 0;
		padding: var(--space-2);
		border: 1px dashed var(--border);
		border-radius: var(--radius-sm);
	}

	.content :global(.diagram img) {
		max-width: 100%;
	}

	.content :global(.diagram-toggle) {
		font: inherit;
		font-size: var(--text-sm);
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm);
		background: var(--surface);
		color: var(--text);
		cursor: pointer;
	}

	.diagram-preview {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-top: 1px solid var(--border);
	}

	.diagram-preview img {
		max-width: 100%;
		align-self: flex-start;
	}

	.preview-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.content :global(.image img) {
		max-width: 100%;
		max-height: 16rem;
		vertical-align: middle;
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
