/**
 * Multiline text as Markdown, rendered to HTML that is safe to put in the
 * page: no scripts, styles, frames, forms or event handlers, and links only
 * to http, https and mailto addresses, opened without the opener.
 */
// r[impl app.markdown]
import DOMPurify from 'dompurify';
import { marked } from 'marked';

const LINK = /^(?:https?:|mailto:)/i;
/** A page of the app, as a reference links to it (`app.references`). */
const IN_APP = /^\/akt\//;
const HEADING = /^H[1-6]$/;

let hooked = false;

function purifier() {
	if (!hooked) {
		DOMPurify.addHook('afterSanitizeAttributes', (node) => {
			if (node.tagName === 'A') {
				const href = node.getAttribute('href') ?? '';
				if (IN_APP.test(href)) {
					// A page of the app, opened in place.
				} else if (!LINK.test(href)) {
					node.removeAttribute('href');
				} else {
					node.setAttribute('rel', 'noopener noreferrer');
					node.setAttribute('target', '_blank');
				}
			}
			// Ids only on headings, where a document's heading ids go.
			if (node.hasAttribute?.('id') && !HEADING.test(node.tagName)) {
				node.removeAttribute('id');
			}
		});
		hooked = true;
	}
	return DOMPurify;
}

/** `html` sanitized: no scripts, styles, frames, forms or event handlers,
 * links only to http, https and mailto addresses, or with `headingIds` to
 * the app's akt pages and with ids on headings. */
export function sanitized(html: string, { headingIds = false } = {}): string {
	return purifier().sanitize(html, {
		FORBID_TAGS: [
			'style',
			'script',
			'iframe',
			'frame',
			'object',
			'embed',
			'form',
			'input',
			'button',
			'textarea',
			'select',
			'img',
			'svg',
			'math'
		],
		FORBID_ATTR: headingIds ? ['style', 'class'] : ['style', 'class', 'id'],
		ALLOW_DATA_ATTR: false,
		ALLOWED_URI_REGEXP: headingIds ? /^(?:https?:|mailto:|\/akt\/)/i : LINK
	});
}

/** `text` as sanitized HTML. */
export function renderMarkdown(text: string): string {
	return sanitized(marked.parse(text, { async: false, gfm: true, breaks: true }) as string);
}
