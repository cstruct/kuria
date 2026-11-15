// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { destinationOf, parseReference, renderReferenced } from './references';

const href = (r: { number: string; version: number | null; heading: string | null }) =>
	`/akt/${r.number}${r.version ? `?version=${r.version}` : ''}${r.heading ? `#${r.heading}` : ''}`;

// r[verify app.references]
describe('references', () => {
	it('are read as the engine reads them', () => {
		expect(parseReference('kuria:D-4@2#install')).toEqual({
			number: 'D-4',
			version: 2,
			heading: 'install'
		});
		expect(parseReference('kuria:D-4')).toEqual({ number: 'D-4', version: null, heading: null });
		for (const bad of ['kuria:', 'kuria:D-4@0', 'kuria:D-4@x', 'kuria:D-4#', 'https://x']) {
			expect(parseReference(bad), bad).toBeNull();
		}
		expect(destinationOf({ number: 'D-4', version: 2, heading: 'install' })).toBe(
			'kuria:D-4@2#install'
		);
	});

	it('render headings with their ids, and references as links or broken', () => {
		const html = renderReferenced(
			'# Installing {#install}\n\nSee [setup](kuria:D-4@2#run), [gone](kuria:D-9) and [web](https://example.com).\n',
			[
				{ destination: 'kuria:D-4@2#run', broken: null },
				{ destination: 'kuria:D-9', broken: 'there is no akt `D-9`' }
			],
			href
		);
		expect(html).toContain('<h1 id="install">Installing</h1>');
		expect(html).toContain('<a href="/akt/D-4?version=2#run">setup</a>');
		expect(html).toContain('gone <small>(broken reference: there is no akt `D-9`)</small>');
		expect(html).toContain('href="https://example.com"');
		expect(html).toContain('target="_blank"');
	});

	it('stay safe', () => {
		const html = renderReferenced(
			'<p id="x" onclick="alert(1)">hi</p>\n\n[x](javascript:alert(1))\n\n<script>alert(1)</script>\n',
			[],
			href
		);
		expect(html).not.toContain('onclick');
		expect(html).not.toContain('id="x"');
		expect(html).not.toContain('javascript:');
		expect(html).not.toContain('<script');
	});
});
