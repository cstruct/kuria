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
/** A footnote and the reference to it, as `app.references` numbers them. */
const NOTE_ID = /^fn(?:ref)?:\d+$/;
const NOTE_HREF = /^#fn(?:ref)?:\d+$/;
/** An attachment's download, the only thing an image may show (`app.images`). */
const PICTURE = /^\/attachments\/[0-9a-f-]{36}$/;

let hooked = false;

function purifier() {
	if (!hooked) {
		DOMPurify.addHook('afterSanitizeAttributes', (node) => {
			if (node.tagName === 'A') {
				const href = node.getAttribute('href') ?? '';
				if (IN_APP.test(href) || NOTE_HREF.test(href)) {
					// A page of the app, or a footnote, opened in place.
				} else if (!LINK.test(href)) {
					node.removeAttribute('href');
				} else {
					node.setAttribute('rel', 'noopener noreferrer');
					node.setAttribute('target', '_blank');
				}
			}
			// An image only of an attachment's download, else nothing.
			if (node.tagName === 'IMG' && !PICTURE.test(node.getAttribute('src') ?? '')) {
				node.remove();
				return;
			}
			// Ids only on headings, where a document's heading ids go, and on
			// footnotes and the references to them.
			if (
				node.hasAttribute?.('id') &&
				!HEADING.test(node.tagName) &&
				!NOTE_ID.test(node.getAttribute('id') ?? '')
			) {
				node.removeAttribute('id');
			}
		});
		hooked = true;
	}
	return DOMPurify;
}

/** `html` sanitized: no scripts, styles, frames, forms or event handlers,
 * links only to http, https and mailto addresses, or with `headingIds` to
 * the app's akt pages and footnotes, with ids on headings and footnotes,
 * and images of attachments' downloads. */
export function sanitized(html: string, { headingIds = false } = {}): string {
	return purifier().sanitize(html, {
		FORBID_TAGS: [
			...(headingIds ? [] : ['img']),
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
			'svg',
			'math'
		],
		FORBID_ATTR: headingIds ? ['style', 'class'] : ['style', 'class', 'id'],
		ALLOW_DATA_ATTR: false,
		ALLOWED_URI_REGEXP: headingIds
			? /^(?:https?:|mailto:|\/akt\/|\/attachments\/|#fn(?:ref)?:\d+$)/i
			: LINK
	});
}

/** `text` as sanitized HTML. */
export function renderMarkdown(text: string): string {
	return sanitized(marked.parse(text, { async: false, gfm: true, breaks: true }) as string);
}
