/**
 * A `markdown` field in the app (`akt.field.markdown`): its headings with
 * their ids as anchors, and its references to akter as links into the app,
 * or said to be broken (`app.references`). Nothing here knows any akt
 * type.
 */
// r[impl app.references]
import { Lexer, Marked, type MarkedExtension, type Token, type Tokens } from 'marked';
import { sanitized } from './markdown';

/** A reference, as `kuria:<number>[@<version>][#<heading>]` names it. */
export interface Reference {
	number: string;
	version: number | null;
	heading: string | null;
}

/** A reference as the server answers it: its destination, and why it is
 * broken, if it is. */
export interface ShownReference {
	destination: string;
	broken?: string | null;
	/** The version a version read pins it to, if it names none. */
	pinnedVersion?: number | null;
}

const ID = /\s*\{#([a-z][a-z0-9_-]*)\}\s*$/;

/** Reads a link's destination, as the engine does; `null` if it isn't a
 * reference. */
export function parseReference(destination: string): Reference | null {
	if (!destination.startsWith('kuria:')) return null;
	let rest = destination.slice('kuria:'.length);
	let heading: string | null = null;
	const hash = rest.indexOf('#');
	if (hash >= 0) {
		heading = rest.slice(hash + 1);
		rest = rest.slice(0, hash);
		if (!heading) return null;
	}
	let version: number | null = null;
	const at = rest.indexOf('@');
	if (at >= 0) {
		const text = rest.slice(at + 1);
		if (!/^\d+$/.test(text) || Number(text) === 0) return null;
		version = Number(text);
		rest = rest.slice(0, at);
	}
	return rest ? { number: rest, version, heading } : null;
}

/** `kuria:<number>[@<version>][#<heading>]` for `reference`. */
export function destinationOf(reference: Reference): string {
	return `kuria:${reference.number}${reference.version ? `@${reference.version}` : ''}${
		reference.heading ? `#${reference.heading}` : ''
	}`;
}

function escaped(text: string): string {
	return text
		.replaceAll('&', '&amp;')
		.replaceAll('<', '&lt;')
		.replaceAll('>', '&gt;')
		.replaceAll('"', '&quot;');
}

/** A footnote's definition: `[^label]: text`, and lines indented under it. */
const DEFINITION = /^\[\^([^\]\n]+)\]:[ \t]?([^\n]*(?:\n(?:[ \t]*\n)*(?: {4}|\t)[^\n]*)*)(?:\n+|$)/;
/** A reference to a footnote, `[^label]`, unless it starts a definition. */
const NOTE = /^\[\^([^\]\n]+)\](?!:)/;

/**
 * GitHub's footnotes: each reference to a defined one a superscript number
 * linking to it, numbered in the order first referred to, and the notes
 * listed at the end, each linking back. A definition nothing refers to
 * isn't shown, and a reference to none is its text, as the engine reads
 * them (`akt.field.markdown`).
 */
function footnotes(): { extension: MarkedExtension; notes: () => string } {
	const defined = new Map<string, Token[]>();
	const numbers = new Map<string, number>();
	const rendered = new Map<number, string>();
	const extension: MarkedExtension = {
		extensions: [
			{
				name: 'footnoteDefinition',
				level: 'block',
				start: (src) => src.match(/^\[\^/m)?.index,
				tokenizer(src) {
					const match = DEFINITION.exec(src);
					if (!match) return undefined;
					const text = match[2].replace(/^(?: {4}|\t)/gm, '');
					const token = {
						type: 'footnoteDefinition',
						raw: match[0],
						label: match[1],
						tokens: this.lexer.blockTokens(text, [])
					};
					if (!defined.has(match[1])) defined.set(match[1], token.tokens);
					return token;
				},
				renderer: () => ''
			},
			{
				name: 'footnoteReference',
				level: 'inline',
				start: (src) => (src.indexOf('[^') >= 0 ? src.indexOf('[^') : undefined),
				tokenizer(src) {
					const match = NOTE.exec(src);
					if (!match || !defined.has(match[1])) return undefined;
					return { type: 'footnoteReference', raw: match[0], label: match[1] };
				},
				renderer(token) {
					const label = token.label as string;
					let n = numbers.get(label);
					const first = n === undefined;
					if (n === undefined) {
						n = numbers.size + 1;
						numbers.set(label, n);
						rendered.set(n, this.parser.parse(defined.get(label) ?? []));
					}
					const id = first ? ` id="fnref:${n}"` : '';
					return `<sup><a href="#fn:${n}"${id} aria-label="Footnote ${n}">${n}</a></sup>`;
				}
			}
		]
	};
	const notes = () => {
		if (rendered.size === 0) return '';
		const items = [...rendered.entries()]
			.sort(([a], [b]) => a - b)
			.map(
				([n, html]) =>
					`<li id="fn:${n}">${html.trim()} <a href="#fnref:${n}" aria-label="Back to the reference to footnote ${n}">↩</a></li>`
			);
		return `<section aria-label="Footnotes"><hr><ol>${items.join('')}</ol></section>`;
	};
	return { extension, notes };
}

/**
 * `text` as sanitized HTML: each heading with its id, each reference a
 * link to `hrefOf` it, unless `references` says it is broken, when it is
 * its text and why.
 */
export function renderReferenced(
	text: string,
	references: readonly ShownReference[],
	hrefOf: (reference: Reference) => string
): string {
	const shown = new Map(references.map((r) => [r.destination, r]));
	const notes = footnotes();
	const marked = new Marked(notes.extension, {
		gfm: true,
		breaks: true,
		walkTokens(token) {
			if (token.type !== 'heading') return;
			const heading = token as Tokens.Heading & { anchor?: string };
			const id = heading.text.match(ID);
			if (id) {
				heading.anchor = id[1];
				heading.text = heading.text.replace(ID, '');
				heading.tokens = Lexer.lexInline(heading.text);
			}
		},
		renderer: {
			heading(token) {
				const anchor = (token as Tokens.Heading & { anchor?: string }).anchor;
				const inner = this.parser.parseInline(token.tokens);
				return anchor
					? `<h${token.depth} id="${escaped(anchor)}">${inner}</h${token.depth}>\n`
					: `<h${token.depth}>${inner}</h${token.depth}>\n`;
			},
			link(token) {
				const reference = parseReference(token.href);
				if (!reference) return false;
				const inner = this.parser.parseInline(token.tokens);
				const known = shown.get(token.href);
				if (known?.broken) {
					return `<span>${inner} <small>(broken reference: ${escaped(known.broken)})</small></span>`;
				}
				// Read at a version, a reference naming none goes where its pin says.
				const to = { ...reference, version: reference.version ?? known?.pinnedVersion ?? null };
				return `<a href="${escaped(hrefOf(to))}">${inner}</a>`;
			}
		}
	});
	const html = marked.parse(text, { async: false }) as string;
	return sanitized(html + notes.notes(), { headingIds: true });
}
