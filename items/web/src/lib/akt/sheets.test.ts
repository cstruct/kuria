// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { renderReferenced } from './references';
import { guideOffsets, sheetsOf } from './sheets';

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
