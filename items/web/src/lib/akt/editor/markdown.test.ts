// @vitest-environment jsdom
import { createHeadlessEditor } from '@lexical/headless';
import { describe, expect, it } from 'vitest';
import { $getRoot, $isElementNode } from 'lexical';
import {
	$fromMarkdown,
	$isFootnoteDefinitionNode,
	$isFootnoteReferenceNode,
	$nextFootnoteLabel,
	$toMarkdown,
	atx,
	NODES
} from './markdown';

/** `markdown` through the editor and back. */
function roundTrip(markdown: string): string {
	const editor = createHeadlessEditor({
		nodes: NODES,
		onError: (e) => {
			throw e;
		}
	});
	editor.update(() => $fromMarkdown(markdown), { discrete: true });
	return editor.getEditorState().read(() => $toMarkdown());
}

// r[verify app.editor]
describe('the editor’s Markdown', () => {
	// Each construct the editor offers, as it writes it.
	const constructs: [string, string][] = [
		['headings with ids', '# Installing {#install}\n\n## Set up `kuria` {#set-up}'],
		['emphasis', 'Some **bold**, *italic* and ~~struck~~ text, and `code`.'],
		['lists', '- one\n- two\n\n1. first\n2. second'],
		['task lists', '- [ ] to do\n- [x] done'],
		['quotes', '> Quoted.'],
		['code', '```\nlet x = 1;\n```'],
		['links and references', '[web](https://example.com) and [setup](kuria:DOCS-1-D04@2#install)'],
		['tables', '| Name | Value |\n| --- | --- |\n| a | 1 |\n| b | 2 |'],
		['rules', 'Above\n\n---\n\nBelow'],
		[
			'footnotes',
			'Said[^1], and *done*[^note].\n\n[^1]: First, with a [link](kuria:D-4#run).\n\n[^note]: Second **note**.'
		]
	];
	for (const [name, markdown] of constructs) {
		it(`come back unchanged: ${name}`, () => {
			expect(roundTrip(markdown)).toBe(markdown);
		});
	}

	it('read setext headings as ATX ones', () => {
		expect(atx('Overview {#overview}\n========\n\nText.\n')).toBe(
			'# Overview {#overview}\n\nText.\n'
		);
		expect(roundTrip('Overview {#overview}\n========\n\nText.')).toBe(
			'# Overview {#overview}\n\nText.'
		);
	});

	it('hold footnotes as their own nodes, and number the next one', () => {
		const editor = createHeadlessEditor({
			nodes: NODES,
			onError: (e) => {
				throw e;
			}
		});
		editor.update(() => $fromMarkdown('Said[^1] and[^x].\n\n[^1]: One.\n\n[^x]: Ex.'), {
			discrete: true
		});
		editor.getEditorState().read(() => {
			const [paragraph, one, ex] = $getRoot().getChildren();
			const children = $isElementNode(paragraph) ? paragraph.getChildren() : [];
			const references = children.filter($isFootnoteReferenceNode);
			expect(references.length).toBe(2);
			expect($isFootnoteDefinitionNode(one) && one.getLabel()).toBe('1');
			expect($isFootnoteDefinitionNode(ex) && ex.getLabel()).toBe('x');
			expect($nextFootnoteLabel()).toBe('2');
		});
	});
});
