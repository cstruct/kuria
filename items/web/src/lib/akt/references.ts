/**
 * A `markdown` field in the app (`akt.field.markdown`): its headings with
 * their ids as anchors, and its references to akter as links into the app,
 * or said to be broken (`app.references`). Nothing here knows any akt
 * type.
 */
// r[impl app.references]
import { Lexer, Marked, type Tokens } from 'marked';
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
	const broken = new Map(references.map((r) => [r.destination, r.broken ?? null]));
	const marked = new Marked({
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
				const why = broken.get(token.href);
				if (why) {
					return `<span>${inner} <small>(broken reference: ${escaped(why)})</small></span>`;
				}
				return `<a href="${escaped(hrefOf(reference))}">${inner}</a>`;
			}
		}
	});
	return sanitized(marked.parse(text, { async: false }) as string, { headingIds: true });
}
