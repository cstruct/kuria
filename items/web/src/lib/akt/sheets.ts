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

/**
 * How tall a sheet is whose text runs from `start` to `end`: a whole
 * number of pages `page` tall, at least one, counted from `start`.
 */
export function wholePages(start: number, end: number, page: number): number {
	if (!(page > 0)) return end;
	// A pixel's rounding is not another page.
	const pages = Math.max(1, Math.ceil((end - start - 1) / page));
	return start + pages * page;
}

/** Where a sheet's text ends, its bottom margin included. */
function textEnd(sheet: HTMLElement): number {
	const padding = parseFloat(getComputedStyle(sheet).paddingBottom) || 0;
	const last = sheet.lastElementChild;
	if (!(last instanceof HTMLElement)) return padding * 2;
	return last.offsetTop + last.offsetHeight + padding;
}

/**
 * An action keeping a sheet a whole number of A4 pages tall, as its text
 * or width changes; `start` is where its last page break leaves off, for
 * the editor, whose page breaks are gaps in one sheet.
 */
export function pagesOf(sheet: HTMLElement, start: () => number = () => 0) {
	const fit = () => {
		const height = `${wholePages(start(), textEnd(sheet), sheet.offsetWidth * A4)}px`;
		if (sheet.style.minHeight !== height) sheet.style.minHeight = height;
	};
	const resized = new ResizeObserver(fit);
	resized.observe(sheet);
	const changed = new MutationObserver(fit);
	changed.observe(sheet, { childList: true, subtree: true, characterData: true });
	// Images and fonts change how tall the text is without changing it.
	sheet.addEventListener('load', fit, true);
	document.fonts?.ready.then(fit);
	fit();
	return {
		destroy() {
			resized.disconnect();
			changed.disconnect();
			sheet.removeEventListener('load', fit, true);
		}
	};
}
