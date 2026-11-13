import { describe, expect, it } from 'vitest';
import { merged } from './paging';

// r[verify app.filters]
describe('merged', () => {
	it('lists each akt once, the first page first', () => {
		const a = { id: 'a' };
		const b = { id: 'b' };
		const c = { id: 'c' };
		expect(merged([a, b], [c])).toEqual([a, b, c]);
		// b changed and moved into the refetched first page.
		expect(merged([b, a], [b, c])).toEqual([b, a, c]);
		expect(merged([], [])).toEqual([]);
	});
});
