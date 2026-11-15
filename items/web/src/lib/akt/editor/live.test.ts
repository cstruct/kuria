// @vitest-environment jsdom
import { createHeadlessEditor } from '@lexical/headless';
import {
	$createRangeSelection,
	$getRoot,
	$getSelection,
	$isRangeSelection,
	$isTextNode,
	$setSelection,
	type ElementNode,
	type LexicalEditor
} from 'lexical';
import { describe, expect, it } from 'vitest';
import * as Y from 'yjs';
import { bindText, blocksOf, minimalEdit, moveOffset, type Delta } from './live';
import { $toMarkdown, NODES } from './markdown';

function editorOf(): LexicalEditor {
	return createHeadlessEditor({
		nodes: NODES,
		onError: (e) => {
			throw e;
		}
	});
}

/** Two documents, as two clients have one live draft, each update of one
 * applied to the other. */
function pair(markdown: string) {
	const local = new Y.Doc();
	const remote = new Y.Doc();
	remote.getText('body').insert(0, markdown);
	Y.applyUpdate(local, Y.encodeStateAsUpdate(remote));
	local.on('update', (update: Uint8Array, origin: unknown) => {
		if (origin !== 'remote') Y.applyUpdate(remote, update, 'local');
	});
	remote.on('update', (update: Uint8Array, origin: unknown) => {
		if (origin !== 'local') Y.applyUpdate(local, update, 'remote');
	});
	return { local: local.getText('body'), remote: remote.getText('body') };
}

/** Puts the caret `offset` into the text of the `block`th block. */
function caretAt(editor: LexicalEditor, block: number, offset: number) {
	editor.update(
		() => {
			const text = ($getRoot().getChildren()[block] as ElementNode).getFirstDescendant();
			if (!$isTextNode(text)) throw new Error('no text');
			const selection = $createRangeSelection();
			selection.anchor.set(text.getKey(), offset, 'text');
			selection.focus.set(text.getKey(), offset, 'text');
			$setSelection(selection);
		},
		{ discrete: true }
	);
}

/** Where the caret is: its block's index, and the text before it there. */
function caret(editor: LexicalEditor): { block: number; before: string } {
	return editor.getEditorState().read(() => {
		const selection = $getSelection();
		if (!$isRangeSelection(selection)) throw new Error('no selection');
		const node = selection.anchor.getNode();
		const top = node.getTopLevelElementOrThrow();
		const block = $getRoot().getChildren().indexOf(top);
		let before = '';
		for (const text of top.getAllTextNodes()) {
			if (text.is(node)) {
				before += text.getTextContent().slice(0, selection.anchor.offset);
				break;
			}
			before += text.getTextContent();
		}
		return { block, before };
	});
}

const markdownOf = (editor: LexicalEditor) => editor.getEditorState().read(() => $toMarkdown());

// r[verify app.live]
describe('the editor bound to a live draft', () => {
	it('finds the one span that differs', () => {
		expect(minimalEdit('Hello world.', 'Hello, world.')).toEqual({ at: 5, remove: 0, insert: ',' });
		expect(minimalEdit('one two', 'one')).toEqual({ at: 3, remove: 4, insert: '' });
		expect(minimalEdit('same', 'same')).toBeNull();
		// A surrogate pair stays whole.
		expect(minimalEdit('a😀b', 'a😁b')).toEqual({ at: 1, remove: 2, insert: '😁' });
	});

	it('moves an offset as a change moves the text around it', () => {
		const delta: Delta = [{ retain: 2 }, { insert: 'xyz' }, { retain: 3 }, { delete: 2 }];
		expect(moveOffset(1, delta)).toBe(1);
		expect(moveOffset(4, delta)).toBe(7);
		// In a deleted span: where it was.
		expect(moveOffset(6, delta)).toBe(8);
		expect(moveOffset(8, delta)).toBe(9);
		// Text inserted where it is goes after it.
		expect(moveOffset(2, delta)).toBe(2);
	});

	it('reads Markdown’s top-level blocks, without a heading’s id', () => {
		const markdown = '# Intro {#intro}\n\nOne\ntwo.\n\n- a\n- b\n';
		expect(blocksOf(markdown).map((b) => markdown.slice(b.start, b.end))).toEqual([
			'# Intro',
			'One\ntwo.',
			'- a\n- b'
		]);
	});

	it('sends a local change as the minimal edit of its Markdown', () => {
		const { local, remote } = pair('# Intro {#intro}\n\nHello world.\n\nThe end.');
		const editor = editorOf();
		const origin = {};
		bindText(editor, local, { origin });
		const deltas: Delta[] = [];
		remote.observe((event) => deltas.push(event.delta as Delta));
		caretAt(editor, 1, 5);
		editor.update(
			() => {
				const selection = $getSelection();
				if ($isRangeSelection(selection)) selection.insertText(',');
			},
			{ discrete: true }
		);
		expect(remote.toString()).toBe('# Intro {#intro}\n\nHello, world.\n\nThe end.');
		expect(deltas).toEqual([[{ retain: 23 }, { insert: ',' }]]);
	});

	it('brings in a remote change, keeping the selection where it was', () => {
		const { local, remote } = pair('# Intro {#intro}\n\nHello world.\n\nThe end.');
		const editor = editorOf();
		bindText(editor, local, { origin: {} });
		caretAt(editor, 2, 4);
		// Another block changes: the caret's stays as it was.
		remote.insert(0, 'Preface.\n\n');
		expect(markdownOf(editor)).toBe('Preface.\n\n# Intro {#intro}\n\nHello world.\n\nThe end.');
		expect(caret(editor)).toEqual({ block: 3, before: 'The ' });
		// Its own block changes before it: it moves with its text.
		remote.insert(remote.toString().indexOf('The end.'), 'At ');
		expect(markdownOf(editor)).toBe('Preface.\n\n# Intro {#intro}\n\nHello world.\n\nAt The end.');
		expect(caret(editor)).toEqual({ block: 3, before: 'At The ' });
		// And after it: it stays.
		remote.insert(remote.toString().length - 1, ', really');
		expect(caret(editor)).toEqual({ block: 3, before: 'At The ' });
		expect(local.toString()).toBe(remote.toString());
	});

	it('keeps the selection through markup in its block', () => {
		const { local, remote } = pair('Some **bold** words.');
		const editor = editorOf();
		bindText(editor, local, { origin: {} });
		// After "bold", in its own text node.
		editor.update(
			() => {
				const bold = $getRoot().getAllTextNodes()[1];
				const selection = $createRangeSelection();
				selection.anchor.set(bold.getKey(), 4, 'text');
				selection.focus.set(bold.getKey(), 4, 'text');
				$setSelection(selection);
			},
			{ discrete: true }
		);
		remote.insert(0, 'Here: ');
		expect(markdownOf(editor)).toBe('Here: Some **bold** words.');
		expect(caret(editor)).toEqual({ block: 0, before: 'Here: Some bold' });
	});

	it('edits on after a remote change', () => {
		const { local, remote } = pair('One.\n\nTwo.');
		const editor = editorOf();
		bindText(editor, local, { origin: {} });
		remote.insert(4, ' More');
		caretAt(editor, 1, 3);
		editor.update(
			() => {
				const selection = $getSelection();
				if ($isRangeSelection(selection)) selection.insertText('!');
			},
			{ discrete: true }
		);
		expect(remote.toString()).toBe('One. More\n\nTwo!.');
	});
});
