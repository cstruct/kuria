/**
 * What the rich-text editor holds, as Markdown (`app.editor`): the nodes
 * it offers, which are only those a `markdown` field can hold, and the
 * transformers between them and the field's Markdown, heading ids and
 * references included (`akt.field.markdown`). Nothing here knows any akt
 * type.
 */
// r[impl app.editor]
import { CodeHighlightNode, CodeNode } from '@lexical/code';
import {
	$createHorizontalRuleNode,
	$isHorizontalRuleNode,
	HorizontalRuleNode
} from '@lexical/extension';
import { LinkNode } from '@lexical/link';
import { ListItemNode, ListNode } from '@lexical/list';
import {
	$convertFromMarkdownString,
	$convertToMarkdownString,
	BOLD_ITALIC_STAR,
	BOLD_STAR,
	CHECK_LIST,
	CODE,
	type ElementTransformer,
	HEADING,
	type TextMatchTransformer,
	INLINE_CODE,
	ITALIC_STAR,
	ITALIC_UNDERSCORE,
	LINK,
	ORDERED_LIST,
	QUOTE,
	STRIKETHROUGH,
	type Transformer,
	UNORDERED_LIST
} from '@lexical/markdown';
import { $isHeadingNode, HeadingNode, QuoteNode } from '@lexical/rich-text';
import {
	$createTableCellNode,
	$createTableNode,
	$createTableRowNode,
	$isTableCellNode,
	$isTableNode,
	$isTableRowNode,
	TableCellHeaderStates,
	TableCellNode,
	TableNode,
	TableRowNode
} from '@lexical/table';
import {
	$applyNodeReplacement,
	$createParagraphNode,
	$createTextNode,
	$getRoot,
	$getState,
	$isTextNode,
	$setState,
	createState,
	ElementNode,
	type EditorConfig,
	type Klass,
	type LexicalNode,
	type LexicalUpdateJSON,
	type NodeKey,
	type RangeSelection,
	type SerializedElementNode,
	type SerializedTextNode,
	TextNode
} from 'lexical';
import { Lexer } from 'marked';

type SerializedFootnoteReference = SerializedTextNode & { label: string };

/** A reference to a footnote, `[^label]`: one token, shown as it is
 * written. */
export class FootnoteReferenceNode extends TextNode {
	__label: string;

	static getType(): string {
		return 'kuria-footnote-reference';
	}

	static clone(node: FootnoteReferenceNode): FootnoteReferenceNode {
		return new FootnoteReferenceNode(node.__label, node.__key);
	}

	constructor(label: string, key?: NodeKey) {
		super(`[^${label}]`, key);
		this.__label = label;
	}

	static importJSON(json: SerializedFootnoteReference): FootnoteReferenceNode {
		return $createFootnoteReferenceNode(json.label).updateFromJSON(json);
	}

	exportJSON(): SerializedFootnoteReference {
		return { ...super.exportJSON(), label: this.getLabel() };
	}

	createDOM(config: EditorConfig): HTMLElement {
		const dom = super.createDOM(config);
		dom.classList.add('footnote-reference');
		return dom;
	}

	getLabel(): string {
		return this.getLatest().__label;
	}
}

export function $createFootnoteReferenceNode(label: string): FootnoteReferenceNode {
	return $applyNodeReplacement(new FootnoteReferenceNode(label)).setMode('token');
}

export function $isFootnoteReferenceNode(
	node: LexicalNode | null | undefined
): node is FootnoteReferenceNode {
	return node instanceof FootnoteReferenceNode;
}

type SerializedFootnoteDefinition = SerializedElementNode & { label: string };

/** A footnote's definition, `[^label]: text`: a paragraph that says its
 * label. */
export class FootnoteDefinitionNode extends ElementNode {
	__label: string;

	static getType(): string {
		return 'kuria-footnote-definition';
	}

	static clone(node: FootnoteDefinitionNode): FootnoteDefinitionNode {
		return new FootnoteDefinitionNode(node.__label, node.__key);
	}

	constructor(label: string, key?: NodeKey) {
		super(key);
		this.__label = label;
	}

	static importJSON(json: SerializedFootnoteDefinition): FootnoteDefinitionNode {
		return $createFootnoteDefinitionNode(json.label).updateFromJSON(
			json as LexicalUpdateJSON<SerializedFootnoteDefinition>
		);
	}

	exportJSON(): SerializedFootnoteDefinition {
		return { ...super.exportJSON(), label: this.getLabel() };
	}

	createDOM(): HTMLElement {
		const dom = document.createElement('p');
		dom.className = 'footnote-definition';
		dom.dataset.label = this.__label;
		return dom;
	}

	updateDOM(previous: FootnoteDefinitionNode, dom: HTMLElement): boolean {
		if (previous.__label !== this.__label) dom.dataset.label = this.__label;
		return false;
	}

	getLabel(): string {
		return this.getLatest().__label;
	}

	/** Enter at its end goes on in a paragraph. */
	insertNewAfter(_: RangeSelection, restoreSelection = true): ElementNode {
		const paragraph = $createParagraphNode();
		this.insertAfter(paragraph, restoreSelection);
		return paragraph;
	}

	/** Backspace at its start makes it a paragraph. */
	collapseAtStart(): boolean {
		const paragraph = $createParagraphNode();
		paragraph.append(...this.getChildren());
		this.replace(paragraph);
		return true;
	}
}

export function $createFootnoteDefinitionNode(label: string): FootnoteDefinitionNode {
	return $applyNodeReplacement(new FootnoteDefinitionNode(label));
}

export function $isFootnoteDefinitionNode(
	node: LexicalNode | null | undefined
): node is FootnoteDefinitionNode {
	return node instanceof FootnoteDefinitionNode;
}

/** The nodes the editor offers. */
export const NODES: Klass<LexicalNode>[] = [
	FootnoteReferenceNode,
	FootnoteDefinitionNode,
	HeadingNode,
	QuoteNode,
	ListNode,
	ListItemNode,
	CodeNode,
	CodeHighlightNode,
	LinkNode,
	TableNode,
	TableRowNode,
	TableCellNode,
	HorizontalRuleNode
];

/** A heading's id, kept on its node so the editor keeps it (`heading id`). */
export const headingId = createState('kuria-heading-id', {
	parse: (value) => (typeof value === 'string' ? value : '')
});

const ID = /\s*\{#([a-z][a-z0-9_-]*)\}\s*$/;

/** A heading with its id, as the field writes it: `## Text {#id}`. */
const HEADING_WITH_ID: ElementTransformer = {
	...HEADING,
	export: (node, exportChildren) => {
		const line = HEADING.export(node, exportChildren);
		if (line === null || !$isHeadingNode(node)) return line;
		const id = $getState(node, headingId);
		return id ? `${line} {#${id}}` : line;
	}
};

const HORIZONTAL_RULE: ElementTransformer = {
	dependencies: [HorizontalRuleNode],
	export: (node) => ($isHorizontalRuleNode(node) ? '---' : null),
	regExp: /^(?:---|\*\*\*|___)\s*$/,
	replace: (parent, _children, _match, isImport) => {
		const rule = $createHorizontalRuleNode();
		if (isImport || parent.getNextSibling() != null) {
			parent.replace(rule);
		} else {
			parent.insertBefore(rule);
		}
		rule.selectNext();
	},
	type: 'element'
};

/** The cells of a table row, `| a | b |`. */
function cellsOf(line: string): string[] {
	return line
		.trim()
		.replace(/^\|/, '')
		.replace(/\|$/, '')
		.split(/(?<!\\)\|/)
		.map((cell) => cell.trim());
}

const DIVIDER = /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/;

/** A GitHub table: its first row the header, then a divider, then rows. */
const TABLE: ElementTransformer = {
	dependencies: [TableNode, TableRowNode, TableCellNode],
	export: (node, exportChildren) => {
		if (!$isTableNode(node)) return null;
		const rows = node.getChildren().filter($isTableRowNode);
		const lines = rows.map(
			(row) =>
				`| ${row
					.getChildren()
					.filter($isTableCellNode)
					.map((cell) => exportChildren(cell).replace(/\n/g, ' ').replace(/\|/g, '\\|'))
					.join(' | ')} |`
		);
		const columns = rows[0]?.getChildren().length ?? 0;
		lines.splice(1, 0, `| ${Array.from({ length: columns }, () => '---').join(' | ')} |`);
		return lines.join('\n');
	},
	regExp: /^\|(.+)\|\s*$/,
	replace: (parent, _children, match) => {
		const line = match[0];
		const previous = parent.getPreviousSibling();
		if (DIVIDER.test(line)) {
			if ($isTableNode(previous)) parent.remove();
			return;
		}
		const row = $createTableRowNode();
		const header = !$isTableNode(previous);
		for (const text of cellsOf(line)) {
			const cell = $createTableCellNode(
				header ? TableCellHeaderStates.ROW : TableCellHeaderStates.NO_STATUS
			);
			const paragraph = $createParagraphNode();
			paragraph.append($createTextNode(text.replace(/\\\|/g, '|')));
			cell.append(paragraph);
			row.append(cell);
		}
		if ($isTableNode(previous)) {
			previous.append(row);
			parent.remove();
		} else {
			const table = $createTableNode();
			table.append(row);
			parent.replace(table);
		}
	},
	type: 'element'
};

const FOOTNOTE_DEFINITION: ElementTransformer = {
	dependencies: [FootnoteDefinitionNode],
	export: (node, exportChildren) =>
		$isFootnoteDefinitionNode(node) ? `[^${node.getLabel()}]: ${exportChildren(node)}` : null,
	regExp: /^\[\^([^\]\n]+)\]:\s?/,
	replace: (parent, children, match) => {
		const definition = $createFootnoteDefinitionNode(match[1]);
		definition.append(...children);
		parent.replace(definition);
	},
	type: 'element'
};

const FOOTNOTE_REFERENCE: TextMatchTransformer = {
	dependencies: [FootnoteReferenceNode],
	export: (node) => ($isFootnoteReferenceNode(node) ? `[^${node.getLabel()}]` : null),
	importRegExp: /\[\^([^\]\n]+)\](?!:)/,
	regExp: /\[\^([^\]\n]+)\]$/,
	replace: (node, match) => {
		node.replace($createFootnoteReferenceNode(match[1]));
	},
	trigger: ']',
	type: 'text-match'
};

/** The next footnote label, a number after any the editor holds. */
export function $nextFootnoteLabel(): string {
	const labels: string[] = [];
	const visit = (node: LexicalNode) => {
		if ($isFootnoteReferenceNode(node) || $isFootnoteDefinitionNode(node))
			labels.push(node.getLabel());
		if (node instanceof ElementNode) node.getChildren().forEach(visit);
	};
	visit($getRoot());
	const numbers = labels.filter((l) => /^\d+$/.test(l)).map(Number);
	return String(Math.max(0, ...numbers) + 1);
}

/** The transformers between the editor and Markdown, in the order they
 * are tried. */
export const TRANSFORMERS: Transformer[] = [
	FOOTNOTE_DEFINITION,
	TABLE,
	HORIZONTAL_RULE,
	HEADING_WITH_ID,
	QUOTE,
	CODE,
	CHECK_LIST,
	UNORDERED_LIST,
	ORDERED_LIST,
	INLINE_CODE,
	BOLD_ITALIC_STAR,
	BOLD_STAR,
	ITALIC_STAR,
	ITALIC_UNDERSCORE,
	STRIKETHROUGH,
	FOOTNOTE_REFERENCE,
	LINK
];

/** `text` with every setext heading (`Title` over `===`) as an ATX one,
 * which the editor reads. */
export function atx(text: string): string {
	return Lexer.lex(text)
		.map((token) =>
			token.type === 'heading' && /\n[=-]+\s*$/.test(token.raw.trimEnd())
				? `${'#'.repeat(token.depth)} ${token.text}${token.raw.match(/\n*$/)?.[0] ?? ''}`
				: token.raw
		)
		.join('');
}

/** Fills the editor's root, in an update, from a field's Markdown. */
export function $fromMarkdown(markdown: string): void {
	$convertFromMarkdownString(atx(markdown), TRANSFORMERS, undefined, false);
	// Each heading's id, from its last text, onto its node.
	const headings = (node: LexicalNode): LexicalNode[] =>
		$isHeadingNode(node)
			? [node]
			: 'getChildren' in node
				? (node as ElementNode).getChildren().flatMap(headings)
				: [];
	for (const heading of headings($getRoot())) {
		const last = (heading as ElementNode).getLastDescendant();
		if (!$isTextNode(last)) continue;
		const match = last.getTextContent().match(ID);
		if (!match) continue;
		$setState(heading, headingId, match[1]);
		const rest = last.getTextContent().replace(ID, '');
		if (rest) last.setTextContent(rest);
		else last.remove();
	}
}

/** The editor's root, in a read, as a field's Markdown. */
export function $toMarkdown(): string {
	return $convertToMarkdownString(TRANSFORMERS, undefined, false);
}
