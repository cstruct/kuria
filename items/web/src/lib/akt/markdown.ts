/**
 * Multiline text as Markdown, rendered to HTML that is safe to put in the
 * page: no scripts, styles, frames, forms or event handlers, and links only
 * to http, https and mailto addresses, opened without the opener.
 */
// r[impl app.markdown]
import DOMPurify from 'dompurify';
import { marked } from 'marked';

const LINK = /^(?:https?:|mailto:)/i;

let hooked = false;

function purifier() {
	if (!hooked) {
		DOMPurify.addHook('afterSanitizeAttributes', (node) => {
			if (node.tagName === 'A') {
				const href = node.getAttribute('href') ?? '';
				if (!LINK.test(href)) {
					node.removeAttribute('href');
				} else {
					node.setAttribute('rel', 'noopener noreferrer');
					node.setAttribute('target', '_blank');
				}
			}
		});
		hooked = true;
	}
	return DOMPurify;
}

/** `text` as sanitized HTML. */
export function renderMarkdown(text: string): string {
	const html = marked.parse(text, { async: false, gfm: true, breaks: true }) as string;
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
		FORBID_ATTR: ['style', 'class', 'id'],
		ALLOW_DATA_ATTR: false,
		ALLOWED_URI_REGEXP: LINK
	});
}
