/**
 * A `markdown` field's sheets (`app.sheets`): the page breaks that start a
 * new one, and where a sheet would otherwise end. Nothing here knows any
 * akt type.
 */
// r[impl app.sheets]

/** A line holding only this is a page break (`akt.field.markdown`). */
export const PAGE_BREAK = '<!-- pagebreak -->';

/** A page break as rendered HTML, until it is split into sheets. */
export const PAGE_BREAK_HTML = '<hr aria-label="Page break">';

/** An A4 sheet's height over its width. */
export const A4 = 297 / 210;

function isPageBreak(node: Node): boolean {
	return (
		node instanceof Element &&
		node.tagName === 'HR' &&
		node.getAttribute('aria-label') === 'Page break'
	);
}

/** `html`, a field rendered and sanitized, as its sheets' HTML: a new one
 * at each page break of the text itself. One in a list or a quote isn't
 * one, as in the PDF, and is dropped. */
export function sheetsOf(html: string): string[] {
	const template = document.createElement('template');
	template.innerHTML = html;
	const sheets: DocumentFragment[] = [document.createDocumentFragment()];
	for (const node of [...template.content.childNodes]) {
		if (isPageBreak(node)) {
			sheets.push(document.createDocumentFragment());
			continue;
		}
		if (node instanceof Element)
			node.querySelectorAll('hr[aria-label="Page break"]').forEach((hr) => hr.remove());
		sheets[sheets.length - 1].append(node);
	}
	return sheets.map((fragment) => {
		const holder = document.createElement('div');
		holder.append(fragment);
		return holder.innerHTML.trim();
	});
}

/**
 * Where faint guides go in a run of sheets: each `[start, end]` a sheet
 * that only a page break, or the text's end, ends, as offsets from the
 * top, with a guide every `height` from its start, short of its end.
 */
export function guideOffsets(sheets: readonly [number, number][], height: number): number[] {
	if (!(height > 0)) return [];
	const guides: number[] = [];
	for (const [start, end] of sheets) {
		for (let at = start + height; at < end - 1; at += height) guides.push(Math.round(at));
	}
	return guides;
}
