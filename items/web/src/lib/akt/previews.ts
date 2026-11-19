/**
 * What a reference's preview shows (`app.reference-preview`): the reference
 * a link into the app goes to, and the start of the text it names, as
 * plain text. Nothing here knows any akt type.
 */
// r[impl app.reference-preview]
import type { Reference } from './references';

/** The reference a link to `href` goes to, if it goes to an akt's page
 * under `page`, the app's `/akt/` path, as
 * the rendered text's links write it: relative. */
export function referenceOfHref(href: string, page: string): Reference | null {
	const here = 'http://app.invalid';
	let url: URL;
	try {
		url = new URL(href, here);
	} catch {
		return null;
	}
	if (url.origin !== here || !url.pathname.startsWith(page)) return null;
	const number = decodeURIComponent(url.pathname.slice(page.length));
	if (!number || number.includes('/')) return null;
	const version = url.searchParams.get('version');
	return {
		number,
		version: version && /^\d+$/.test(version) && Number(version) > 0 ? Number(version) : null,
		heading: url.hash ? decodeURIComponent(url.hash.slice(1)) : null
	};
}

const HEADING = /^(#{1,6})\s+(.*?)\s*$/;
const ID = /\s*\{#([a-z][a-z0-9_-]*)\}\s*$/;

/** The section under the heading `heading` of `markdown`, up to the next
 * heading as high or higher; the whole text without one, and nothing if
 * it isn't there. */
function sectionOf(markdown: string, heading: string | null): string[] {
	const lines = markdown.split('\n');
	if (!heading) return lines;
	let level = 0;
	const kept: string[] = [];
	for (const line of lines) {
		const found = line.match(HEADING);
		if (level) {
			if (found && found[1].length <= level) break;
			kept.push(line);
		} else if (found && found[2].match(ID)?.[1] === heading) {
			level = found[1].length;
		}
	}
	return kept;
}

/** The start of `markdown`, or of its section under `heading`, as plain
 * text of at most `length` characters, cut at a word. */
export function excerptOf(markdown: string, heading: string | null, length = 280): string {
	let fenced = false;
	const words: string[] = [];
	for (const raw of sectionOf(markdown, heading)) {
		if (/^\s*(```|~~~)/.test(raw)) {
			fenced = !fenced;
			continue;
		}
		// Code, footnote definitions and table rules aren't text to read.
		if (fenced || /^\[\^[^\]]+\]:/.test(raw) || /^\s*\|?\s*:?-{3,}/.test(raw)) continue;
		const line = raw
			.replace(/<!--.*?-->|<[^>]+>/g, ' ')
			.replace(ID, '')
			.replace(/^\s*#{1,6}\s+/, '')
			.replace(/^\s*>\s?/, '')
			.replace(/^\s*(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?/, '')
			.replace(/!\[([^\]]*)\]\([^)]*\)/g, '')
			.replace(/\[\^[^\]]+\]/g, '')
			.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
			.replace(/[*~`]/g, '')
			.replace(/(^|\W)_+|_+(?=\W|$)/g, '$1')
			.replace(/\|/g, ' ');
		words.push(...line.split(/\s+/).filter(Boolean));
	}
	const text = words.join(' ');
	if (text.length <= length) return text;
	const cut = text.slice(0, length + 1);
	const at = cut.lastIndexOf(' ');
	return `${(at > length / 2 ? cut.slice(0, at) : text.slice(0, length)).replace(/[\s,.;:]+$/, '')}…`;
}
