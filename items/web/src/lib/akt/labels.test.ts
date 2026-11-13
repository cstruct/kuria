import { describe, expect, it } from 'vitest';
import { boardLabels, legendOf } from './labels';

describe('accessible names', () => {
	// r[verify app.accessible]
	it('names field groups and a board’s buttons by what they act on', () => {
		expect(legendOf({ name: 'Title', required: true })).toBe('Title (required)');
		expect(legendOf({ name: 'Kind', required: false })).toBe('Kind');
		expect(boardLabels('ITEM-0003')).toEqual({
			menu: 'Actions for ITEM-0003',
			up: 'Move ITEM-0003 up',
			down: 'Move ITEM-0003 down',
			remove: 'Remove ITEM-0003'
		});
	});
});
