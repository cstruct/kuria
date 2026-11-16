// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { renderReferenced } from './references';
import { guideOffsets, sheetsOf, wholePages } from './sheets';

const href = (r: { number: string }) => `/akt/${r.number}`;
const sheets = (markdown: string) => sheetsOf(renderReferenced(markdown, [], href));

// r[verify app.sheets]
describe('sheets', () => {
	it('are one for a field without page breaks', () => {
		expect(sheets('# One {#one}\n\nText.')).toEqual(['<h1 id="one">One</h1>\n<p>Text.</p>']);
	});

	it('start a new one at each page break', () => {
		expect(
			sheets(
				'One.\n\n<!-- pagebreak -->\n\nTwo.\n<!-- pagebreak -->\n  <!-- pagebreak -->  \n\nFour.'
			)
		).toEqual(['<p>One.</p>', '<p>Two.</p>', '', '<p>Four.</p>']);
	});

	it('keep footnotes at the end, numbered across sheets', () => {
		const [first, second] = sheets(
			'Said[^a].\n\n<!-- pagebreak -->\n\nAgain[^b].\n\n[^a]: One.\n\n[^b]: Two.'
		);
		expect(first).toContain('href="#fn:1"');
		expect(second).toContain('href="#fn:2"');
		expect(second).toContain('<section aria-label="Footnotes">');
	});

	it('aren’t started by a page break in a list, a quote, code or text', () => {
		const shown = sheets(
			'- one\n\n  <!-- pagebreak -->\n\n> <!-- pagebreak -->\n\n```\n<!-- pagebreak -->\n```\n\nText <!-- pagebreak --> here.'
		);
		expect(shown).toHaveLength(1);
		expect(shown[0]).not.toContain('Page break');
		expect(shown[0]).toContain('&lt;!-- pagebreak --&gt;');
	});

	it('mark where a sheet would otherwise end, from each sheet’s start', () => {
		expect(
			guideOffsets(
				[
					[0, 2500],
					[2600, 3000]
				],
				1000
			)
		).toEqual([1000, 2000]);
		expect(guideOffsets([[0, 1000]], 1000)).toEqual([]);
		expect(guideOffsets([[100, 3200]], 1000)).toEqual([1100, 2100, 3100]);
		expect(guideOffsets([[0, 3000]], 0)).toEqual([]);
	});
});

// r[verify app.sheets]
describe('wholePages', () => {
	it('is one page for a little text', () => {
		expect(wholePages(0, 200, 1000)).toBe(1000);
	});

	it('rounds a longer text up to whole pages', () => {
		expect(wholePages(0, 1001.5, 1000)).toBe(2000);
		expect(wholePages(0, 2400, 1000)).toBe(3000);
	});

	it('is not another page for a pixel’s rounding', () => {
		expect(wholePages(0, 1000.6, 1000)).toBe(1000);
	});

	it('counts from where the last page break leaves off', () => {
		expect(wholePages(1300, 1500, 1000)).toBe(2300);
	});

	it('leaves a sheet without a width as it is', () => {
		expect(wholePages(0, 300, 0)).toBe(300);
	});
});
