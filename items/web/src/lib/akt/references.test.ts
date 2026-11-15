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

// r[verify app.references]
describe('footnotes', () => {
	it('are numbered as first referred to, listed at the end and linked both ways', () => {
		const html = renderReferenced(
			'Second[^b], first[^a]... no: [^b] again, and [^none].\n\n[^a]: Note *a*.\n\n[^b]: See [setup](kuria:D-4#run).\n\n[^unused]: Never shown.\n',
			[{ destination: 'kuria:D-4#run', broken: null }],
			href
		);
		expect(html).toContain(
			'Second<sup><a href="#fn:1" id="fnref:1" aria-label="Footnote 1">1</a></sup>'
		);
		expect(html).toContain(
			'first<sup><a href="#fn:2" id="fnref:2" aria-label="Footnote 2">2</a></sup>'
		);
		expect(html).toContain('no: <sup><a href="#fn:1" aria-label="Footnote 1">1</a></sup> again');
		expect(html).toContain('[^none]');
		expect(html).not.toContain('Never shown');
		expect(html).toContain('<li id="fn:1"><p>See <a href="/akt/D-4#run">setup</a>.</p>');
		expect(html).toContain('<li id="fn:2"><p>Note <em>a</em>.</p>');
		expect(html).toContain('href="#fnref:2"');
		expect(html.indexOf('id="fn:1"')).toBeLessThan(html.indexOf('id="fn:2"'));
	});
});

// r[verify app.images]
describe('embedded images', () => {
	const id = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b';
	it('show their picture, alone as a captioned figure, or say why they are broken', () => {
		const html = renderReferenced(
			`![The chart](kuria:IMG-1)\n\nInline ![logo](kuria:IMG-1) and ![none](kuria:IMG-2) and ![web](https://x.example/a.png).\n`,
			[],
			href,
			[
				{ destination: 'kuria:IMG-1', broken: null, attachment: { url: `/attachments/${id}` } },
				{ destination: 'kuria:IMG-2', broken: '`IMG-2` has no picture', attachment: null }
			]
		);
		expect(html).toContain(
			`<figure><img src="/attachments/${id}" alt="The chart"><figcaption>The chart</figcaption></figure>`
		);
		expect(html).toContain(`Inline <img src="/attachments/${id}" alt="logo"> and`);
		expect(html).toContain('none <small>(broken image: `IMG-2` has no picture)</small>');
		expect(html).toContain('web <small>(broken image: only images in Kuria are shown)</small>');
		expect(html).not.toContain('x.example');
	});

	it('never show anything but an attachment', () => {
		const html = renderReferenced(
			'<img src="https://evil.example/x.png" onerror="alert(1)">\n\n<img src="/attachments/../../graphql">\n\n<svg><script>alert(1)</script></svg>\n',
			[],
			href
		);
		expect(html).not.toContain('<img');
		expect(html).not.toContain('<svg');
		expect(html).not.toContain('onerror');
	});
});
