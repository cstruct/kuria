import { describe, expect, it } from 'vitest';
import { boardLabels, legendOf } from './labels';

describe('accessible names', () => {
	// r[verify app.accessible]
	it('names field groups and a board’s buttons by what they act on', () => {
		expect(legendOf({ name: 'Title', required: true })).toBe('Title (required)');
		expect(legendOf({ name: 'Kind', required: false })).toBe('Kind');
		const labels = boardLabels('ITEM-0003');
		expect(labels.menu).toBe('Actions for ITEM-0003');
		expect([labels.up, labels.down, labels.remove]).toEqual([
			'Move up',
			'Move down',
			'Remove from here'
		]);
		expect(labels.into('In progress')).toBe('Move to In progress');
	});
});
