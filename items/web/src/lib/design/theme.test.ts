import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { PAIRS, colours, contrast } from './theme';

const css = readFileSync(join(import.meta.dirname, 'tokens.css'), 'utf8');
const light = colours(css.slice(css.indexOf(':root {'), css.indexOf('@media')));
const media = css.slice(css.indexOf('@media'), css.indexOf(":root[data-theme='dark']"));
const dark = colours(media);
const forced = colours(css.slice(css.indexOf(":root[data-theme='dark']")));

describe('the tokens', () => {
	// r[verify app.themes]
	it('meet WCAG 2.2 AA contrast in light and in dark', () => {
		const failures: string[] = [];
		for (const [theme, tokens] of [
			['light', light],
			['dark', dark]
		] as const) {
			for (const [fg, bg, least] of PAIRS) {
				expect(tokens[fg], `${theme} --${fg}`).toBeDefined();
				expect(tokens[bg], `${theme} --${bg}`).toBeDefined();
				const ratio = contrast(tokens[fg], tokens[bg]);
				if (ratio < least)
					failures.push(`${theme}: --${fg} on --${bg} is ${ratio.toFixed(2)}:1, not ${least}:1`);
			}
		}
		expect(failures).toEqual([]);
	});

	it('are the same dark for the system preference and for data-theme', () => {
		expect(forced).toEqual(dark);
	});

	it('computes contrast as WCAG does', () => {
		expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 5);
		expect(contrast('#777777', '#ffffff')).toBeCloseTo(4.48, 2);
	});
});
