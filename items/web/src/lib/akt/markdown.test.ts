// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

describe('Markdown', () => {
	// r[verify app.markdown]
	it('renders headings, lists and links', () => {
		const html = renderMarkdown('## Epic\n- EPIC-001 Sign-in\n\n[CI](https://ci.example/812)');
		expect(html).toContain('<h2>Epic</h2>');
		expect(html).toContain('<li>EPIC-001 Sign-in</li>');
		expect(html).toContain('href="https://ci.example/812"');
		expect(html).toContain('rel="noopener noreferrer"');
	});

	// r[verify app.markdown]
	it('drops scripts, styles, frames, forms, handlers and other links', () => {
		const html = renderMarkdown(
			[
				'<script>alert(1)</script>',
				'<style>body{display:none}</style>',
				'<iframe src="https://evil.example"></iframe>',
				'<form><input></form>',
				'<img src=x onerror="alert(1)">',
				'<p onclick="alert(1)" style="color:red">hi</p>',
				'[js](javascript:alert(1)) [data](data:text/html,x) [ok](mailto:a@b.example)'
			].join('\n\n')
		);
		for (const bad of [
			'<script',
			'<style',
			'<iframe',
			'<form',
			'<input',
			'<img',
			'onerror',
			'onclick',
			'style=',
			'javascript:',
			'data:'
		]) {
			expect(html, bad).not.toContain(bad);
		}
		expect(html).toContain('<p>hi</p>');
		expect(html).toContain('href="mailto:a@b.example"');
	});
});
