import { describe, expect, it } from 'vitest';
import { COLOURS, ICONS, markOf } from './identity';
import { PAIRS } from '#lib/design/theme.ts';

// r[verify app.type-identity]
describe('markOf', () => {
	it('gives a type its icon and colour, or none and neutral', () => {
		const marked = markOf({ presentation: { icon: 'BUG', colour: 'RED' } });
		expect(marked.icon).toBe(ICONS.BUG);
		expect(marked.colour).toBe('red');
		expect(markOf({ presentation: { icon: null, colour: 'NEUTRAL' } })).toEqual({
			icon: null,
			colour: 'neutral'
		});
	});

	it('has a contrast-checked token pair for every colour', () => {
		for (const colour of Object.values(COLOURS)) {
			expect(
				PAIRS.some(([fg, bg]) => fg === `type-${colour}-fg` && bg === `type-${colour}-bg`)
			).toBe(true);
		}
	});
});
