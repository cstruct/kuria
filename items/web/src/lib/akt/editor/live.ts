/**
 * The editor bound to a live draft's text (`app.live`, PLAN-016-D03): a
 * `markdown` field's `Y.Text` holds its Markdown, not Lexical's tree, so
 * the server can read, check and merge it.
 *
 * - A local change becomes the one span of the exported Markdown that
 *   differs, deleted and inserted in the `Y.Text` in a transaction of the
 *   binding's origin.
 * - A remote change re-reads the top-level blocks it touched from the new
 *   Markdown, the others kept as they are, and keeps the selection where it
 *   was: mapped to Markdown offsets before, moved by the change, and back
 *   after. When the editor's blocks don't line up with the Markdown's, the
 *   whole text is read again.
 *
 * Offsets are in UTF-16 code units, as `Y.Text` counts them.
 */
// r[impl app.live]
import {
	$createRangeSelection,
	$getNodeByKey,
	$getRoot,
	$getSelection,
	$isElementNode,
	$isParagraphNode,
	$isRangeSelection,
	$isTextNode,
	$setSelection,
	type LexicalEditor,
	type LexicalNode,
	type PointType,
	type TextNode
} from 'lexical';
import { Lexer } from 'marked';
import * as Y from 'yjs';
import { $blocksFrom, $finish, $fromMarkdown, $toMarkdown } from './markdown';

/** The tag of the editor's updates that bring in a remote change. */
export const REMOTE = 'kuria-live-remote';

/** One span of a text replaced: `remove` code units at `at`, then
 * `insert` there. */
export interface Edit {
	at: number;
	remove: number;
	insert: string;
}

const isHigh = (code: number) => code >= 0xd800 && code <= 0xdbff;
const isLow = (code: number) => code >= 0xdc00 && code <= 0xdfff;

/** The one span that turns `before` into `after`, without splitting a
 * surrogate pair; `null` if they are the same. */
export function minimalEdit(before: string, after: string): Edit | null {
	if (before === after) return null;
	const most = Math.min(before.length, after.length);
	let start = 0;
	while (start < most && before.charCodeAt(start) === after.charCodeAt(start)) start++;
	if (start > 0 && isHigh(before.charCodeAt(start - 1))) start--;
	let end = 0;
	while (
		end < most - start &&
		before.charCodeAt(before.length - 1 - end) === after.charCodeAt(after.length - 1 - end)
	)
		end++;
	if (end > 0 && isLow(before.charCodeAt(before.length - end))) end--;
	return {
		at: start,
		remove: before.length - start - end,
		insert: after.slice(start, after.length - end)
	};
}

/** Applies the edit that turns `text` into `after`, in a transaction of
 * `origin`, and says whether there was one. */
export function writeText(text: Y.Text, after: string, origin: unknown): boolean {
	const edit = minimalEdit(text.toString(), after);
	if (!edit) return false;
	const apply = () => {
		if (edit.remove > 0) text.delete(edit.at, edit.remove);
		if (edit.insert) text.insert(edit.at, edit.insert);
	};
	if (text.doc) text.doc.transact(apply, origin);
	else apply();
	return true;
}

/** A Yjs text delta. */
export type Delta = { insert?: unknown; delete?: number; retain?: number }[];

/** Where `offset` in a text is once `delta` has changed it. Text inserted
 * where it is goes after it. */
export function moveOffset(offset: number, delta: Delta): number {
	// Where in the text as it was each operation starts.
	let at = 0;
	let moved = offset;
	for (const op of delta) {
		if (at >= offset) break;
		if (op.retain !== undefined) at += op.retain;
		else if (op.delete !== undefined) {
			moved -= Math.min(op.delete, offset - at);
			at += op.delete;
		} else if (op.insert !== undefined) {
			moved += typeof op.insert === 'string' ? op.insert.length : 1;
		}
	}
	return moved;
}

/** A top-level block of Markdown: where it starts and ends (without the
 * line breaks after it, or a heading's id), and what it is. */
export interface Block {
	start: number;
	end: number;
	raw: string;
}

const HEADING_ID = /\s*\{#[a-z][a-z0-9_-]*\}\s*$/;

/** The top-level blocks of `markdown`, as Markdown reads them. */
export function blocksOf(markdown: string): Block[] {
	const blocks: Block[] = [];
	let at = 0;
	for (const token of Lexer.lex(markdown)) {
		const raw = token.raw;
		if (token.type !== 'space' && raw.trim() !== '') {
			let text = raw.replace(/\s+$/, '');
			if (token.type === 'heading') text = text.replace(HEADING_ID, '');
			blocks.push({ start: at, end: at + text.length, raw: raw.replace(/\s+$/, '') });
		}
		at += raw.length;
	}
	return blocks;
}

/** The editor's top-level blocks that hold something: an empty paragraph
 * isn't in its Markdown. */
export function $blocks(): LexicalNode[] {
	return $getRoot()
		.getChildren()
		.filter((node) => !($isParagraphNode(node) && node.getTextContentSize() === 0));
}

/** The text nodes of `node`, in order. */
function textsOf(node: LexicalNode): TextNode[] {
	if ($isTextNode(node)) return [node];
	return $isElementNode(node) ? node.getChildren().flatMap(textsOf) : [];
}

/** How far into `block`'s text a point is, counting its text nodes only. */
function $offsetIn(block: LexicalNode, point: PointType): number {
	const texts = textsOf(block);
	const node = point.getNode();
	if (point.type === 'text') {
		let count = 0;
		for (const text of texts) {
			if (text.is(node)) return count + point.offset;
			count += text.getTextContentSize();
		}
		return count;
	}
	// An element point: the text of the children before it.
	const before = $isElementNode(node) ? node.getChildren().slice(0, point.offset) : [];
	const inBefore = new Set(before.flatMap(textsOf).map((t) => t.getKey()));
	let count = 0;
	for (const text of texts) {
		if (inBefore.has(text.getKey())) count += text.getTextContentSize();
		else if (count > 0 || inBefore.size > 0) break;
	}
	return count;
}

/** Where the `wanted`th character of `plain` is in `markdown[start, end)`,
 * matching `plain`'s characters in order and skipping what isn't one, such
 * as markup: just after it, or, for 0, at the first. */
function markdownOffset(
	markdown: string,
	start: number,
	end: number,
	plain: string,
	wanted: number
): number {
	if (wanted <= 0) {
		if (plain.length === 0) return end;
		for (let k = start; k < end; k++) if (markdown[k] === plain[0]) return k;
		return start;
	}
	let matched = 0;
	for (let k = start; k < end; k++) {
		if (matched < plain.length && markdown[k] === plain[matched]) {
			matched++;
			if (matched === wanted) return k + 1;
		}
	}
	return end;
}

/** How many of `plain`'s characters `markdown[start, at)` holds, matched as
 * markdownOffset matches them. */
function plainOffset(markdown: string, start: number, at: number, plain: string): number {
	let matched = 0;
	for (let k = start; k < at; k++) {
		if (matched < plain.length && markdown[k] === plain[matched]) matched++;
	}
	return matched;
}

/** The editor's point `point` as an offset in `markdown`, which the editor
 * holds; `null` if it can't be placed. */
export function $toMarkdownOffset(point: PointType, markdown: string): number | null {
	const blocks = blocksOf(markdown);
	const lexical = $blocks();
	const top = point.getNode().getTopLevelElement() ?? point.getNode();
	if (lexical.length !== blocks.length) {
		// Not lined up: the whole text, matched as one.
		const all = lexical.flatMap(textsOf);
		const plain = all.map((t) => t.getTextContent()).join('');
		let before = 0;
		for (const block of lexical) {
			if (block.is(top))
				return markdownOffset(
					markdown,
					0,
					markdown.length,
					plain,
					before + $offsetIn(block, point)
				);
			before += textsOf(block).reduce((n, t) => n + t.getTextContentSize(), 0);
		}
		return null;
	}
	const index = lexical.findIndex((block) => block.is(top));
	if (index < 0) {
		// An empty paragraph: just after the block before it.
		const previous = lexical.filter((b) => b.isBefore(top)).length - 1;
		return previous >= 0 ? blocks[previous].end : 0;
	}
	const block = blocks[index];
	const plain = textsOf(lexical[index])
		.map((t) => t.getTextContent())
		.join('');
	return markdownOffset(markdown, block.start, block.end, plain, $offsetIn(lexical[index], point));
}

/** The editor's point at `offset` in `markdown`, which it holds, as a key,
 * offset and type; `null` if the editor is empty. */
export function $fromMarkdownOffset(
	offset: number,
	markdown: string
): { key: string; offset: number; type: 'text' | 'element' } | null {
	const blocks = blocksOf(markdown);
	const lexical = $blocks();
	if (lexical.length === 0) {
		const first = $getRoot().getFirstChild();
		return first ? { key: first.getKey(), offset: 0, type: 'element' } : null;
	}
	let index: number;
	let wanted: number;
	if (lexical.length === blocks.length) {
		index = blocks.findIndex((b) => offset <= b.end);
		if (index < 0) index = blocks.length - 1;
		const block = blocks[index];
		const plain = textsOf(lexical[index])
			.map((t) => t.getTextContent())
			.join('');
		wanted = plainOffset(
			markdown,
			block.start,
			Math.max(block.start, Math.min(offset, block.end)),
			plain
		);
	} else {
		const all = lexical.flatMap(textsOf);
		const plain = all.map((t) => t.getTextContent()).join('');
		let count = plainOffset(markdown, 0, Math.min(offset, markdown.length), plain);
		index = lexical.length - 1;
		for (let i = 0; i < lexical.length; i++) {
			const size = textsOf(lexical[i]).reduce((n, t) => n + t.getTextContentSize(), 0);
			if (count <= size) {
				index = i;
				break;
			}
			count -= size;
		}
		wanted = count;
	}
	const texts = textsOf(lexical[index]);
	let count = 0;
	for (const text of texts) {
		const size = text.getTextContentSize();
		if (wanted <= count + size) return { key: text.getKey(), offset: wanted - count, type: 'text' };
		count += size;
	}
	const last = texts.at(-1);
	if (last) return { key: last.getKey(), offset: last.getTextContentSize(), type: 'text' };
	return { key: lexical[index].getKey(), offset: 0, type: 'element' };
}

/** How the binding reads Markdown into the editor. */
export interface Reading {
	pictures?: ReadonlyMap<string, string>;
	diagrams?: boolean;
}

/** Replaces the editor's blocks that differ between `before`, which it
 * holds, and `after` with those `after` makes, keeping the others; or reads
 * `after` whole if its blocks don't line up with `before`'s. Answers
 * whether it read it whole. */
export function $replaceBlocks(before: string, after: string, reading: Reading = {}): boolean {
	const old = blocksOf(before);
	const now = blocksOf(after);
	const lexical = $blocks();
	if (lexical.length !== old.length) {
		$fromMarkdown(after, reading.pictures, { diagrams: reading.diagrams });
		return true;
	}
	const most = Math.min(old.length, now.length);
	let same = 0;
	while (same < most && old[same].raw === now[same].raw) same++;
	let sameEnd = 0;
	while (
		sameEnd < most - same &&
		old[old.length - 1 - sameEnd].raw === now[now.length - 1 - sameEnd].raw
	)
		sameEnd++;
	const replaced = lexical.slice(same, lexical.length - sameEnd);
	const added = now.slice(same, now.length - sameEnd);
	const nodes =
		added.length > 0
			? $blocksFrom(
					after.slice(
						added[0].start,
						added[added.length - 1].start + added[added.length - 1].raw.length
					)
				)
			: [];
	if (replaced.length > 0) {
		for (const node of nodes) replaced[0].insertBefore(node);
		for (const node of replaced) node.remove();
	} else if (same > 0) {
		let at = lexical[same - 1];
		for (const node of nodes) {
			at.insertAfter(node);
			at = node;
		}
	} else if (lexical.length > 0) {
		for (const node of nodes) lexical[0].insertBefore(node);
	} else {
		$getRoot().append(...nodes);
	}
	$finish(nodes, reading.pictures, { diagrams: reading.diagrams });
	if ($getRoot().getChildrenSize() === 0) $fromMarkdown('', reading.pictures);
	return false;
}

/** Brings a remote change, `before` to `after` by `delta`, into the editor,
 * in an update, keeping its selection where it was. */
export function $applyRemote(
	before: string,
	after: string,
	delta: Delta,
	reading: Reading = {}
): void {
	const selection = $getSelection();
	let kept: { anchor: number | null; focus: number | null } | null = null;
	if ($isRangeSelection(selection)) {
		kept = {
			anchor: $toMarkdownOffset(selection.anchor, before),
			focus: $toMarkdownOffset(selection.focus, before)
		};
	}
	const anchorKey = $isRangeSelection(selection) ? selection.anchor.key : null;
	const focusKey = $isRangeSelection(selection) ? selection.focus.key : null;
	const whole = $replaceBlocks(before, after, reading);
	if (!kept || kept.anchor === null || kept.focus === null) return;
	// Untouched blocks keep their nodes, and the selection in them.
	const attached = (key: string | null) => key !== null && !!$getNodeByKey(key)?.isAttached();
	if (!whole && attached(anchorKey) && attached(focusKey)) return;
	const anchor = $fromMarkdownOffset(moveOffset(kept.anchor, delta), after);
	const focus = $fromMarkdownOffset(moveOffset(kept.focus, delta), after);
	if (!anchor || !focus) return;
	const moved = $createRangeSelection();
	moved.anchor.set(anchor.key, anchor.offset, anchor.type);
	moved.focus.set(focus.key, focus.offset, focus.type);
	$setSelection(moved);
}

/** What a binding needs to know. */
export interface Binding extends Reading {
	/** The origin of the binding's own transactions. */
	origin: unknown;
	/** Called after a remote change is in the editor. */
	onRemote?: () => void;
}

/**
 * Binds `editor` to `text` both ways: fills it from the text, sends each
 * local change to the text as its minimal edit, and brings each remote
 * change in. Answers what unbinds them.
 */
export function bindText(editor: LexicalEditor, text: Y.Text, binding: Binding): () => void {
	let known = text.toString();
	editor.update(() => $fromMarkdown(known, binding.pictures, { diagrams: binding.diagrams }), {
		discrete: true,
		tag: REMOTE
	});
	const stopLocal = editor.registerUpdateListener(
		({ editorState, dirtyElements, dirtyLeaves, tags }) => {
			if (tags.has(REMOTE)) return;
			if (dirtyElements.size === 0 && dirtyLeaves.size === 0) return;
			const markdown = editorState.read(() => $toMarkdown());
			// Read and not changed, the text stays as it was written.
			if (markdown === known) return;
			writeText(text, markdown, binding.origin);
			known = text.toString();
		}
	);
	const observer = (event: Y.YTextEvent, transaction: Y.Transaction) => {
		if (transaction.origin === binding.origin) return;
		const before = known;
		const after = text.toString();
		known = after;
		editor.update(() => $applyRemote(before, after, event.delta as Delta, binding), {
			discrete: true,
			tag: REMOTE
		});
		binding.onRemote?.();
	};
	text.observe(observer);
	return () => {
		text.unobserve(observer);
		stopLocal();
	};
}

/** The editor's selection as offsets in its Markdown, which `text` holds,
 * as relative positions in it: what awareness says of a cursor. */
export function selectionIn(
	editor: LexicalEditor,
	text: Y.Text
): { anchor: unknown; head: unknown } | null {
	const markdown = text.toString();
	return editor.getEditorState().read(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) return null;
		const anchor = $toMarkdownOffset(selection.anchor, markdown);
		const head = $toMarkdownOffset(selection.focus, markdown);
		if (anchor === null || head === null) return null;
		return {
			anchor: Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(text, anchor)),
			head: Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(text, head))
		};
	});
}

/** Where a cursor awareness gave, a relative position in `text`, is in the
 * editor: its text node's key and the offset in it. */
export function cursorIn(
	editor: LexicalEditor,
	text: Y.Text,
	position: unknown
): { key: string; offset: number; type: 'text' | 'element' } | null {
	if (!text.doc || !position) return null;
	const absolute = Y.createAbsolutePositionFromRelativePosition(
		Y.createRelativePositionFromJSON(position),
		text.doc
	);
	if (!absolute || absolute.type !== text) return null;
	const markdown = text.toString();
	return editor.getEditorState().read(() => $fromMarkdownOffset(absolute.index, markdown));
}

/**
 * Binds a plain text area to `text` both ways, keeping its selection
 * across remote changes. Answers what unbinds them.
 */
export function bindTextarea(
	area: HTMLTextAreaElement | HTMLInputElement,
	text: Y.Text,
	origin: unknown
): () => void {
	area.value = text.toString();
	const input = () => writeText(text, area.value, origin);
	const observer = (event: Y.YTextEvent, transaction: Y.Transaction) => {
		if (transaction.origin === origin) return;
		const delta = event.delta as Delta;
		const start = moveOffset(area.selectionStart ?? 0, delta);
		const end = moveOffset(area.selectionEnd ?? 0, delta);
		area.value = text.toString();
		if (document.activeElement === area) area.setSelectionRange(start, end);
	};
	area.addEventListener('input', input);
	text.observe(observer);
	return () => {
		area.removeEventListener('input', input);
		text.unobserve(observer);
	};
}
