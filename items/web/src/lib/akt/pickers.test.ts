import { describe, expect, it } from 'vitest';
import { optionsOf } from './pickers';

describe('pickers', () => {
	// r[verify app.pickers]
	it('offer akter by title, with number and type, leaving out what doesn’t fit', () => {
		const found = [
			{ number: 'EPIC-001', title: 'Sign-in', type: { key: 'epic', name: 'Epic' } },
			{ number: 'ITEM-0001', title: null, type: { key: 'backlog_item', name: 'Backlog item' } },
			{ number: 'BACKLOG-001', title: 'Spring', type: { key: 'backlog', name: 'Backlog' } }
		];
		expect(optionsOf(found, ['BACKLOG-001'])).toEqual([
			{ value: 'EPIC-001', label: 'Sign-in', hint: 'EPIC-001 · Epic' },
			{ value: 'ITEM-0001', label: 'ITEM-0001', hint: 'ITEM-0001 · Backlog item' }
		]);
		expect(optionsOf(found, [], ['epic']).map((o) => o.value)).toEqual(['EPIC-001']);
	});
});
