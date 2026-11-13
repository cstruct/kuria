import { describe, expect, it } from 'vitest';
import { userOptions } from './contacts';

// r[verify contact.link-by-hand]
describe('userOptions', () => {
	const users = [
		{ id: 'a', name: 'Ann Lee' },
		{ id: 'b', name: 'Bob Stone' },
		{ id: 'c', name: 'ann@x.example' }
	];

	it('offers the users whose name holds every word, but not those linked already', () => {
		expect(userOptions(users, 'ANN').map((o) => o.value)).toEqual(['a', 'c']);
		expect(userOptions(users, 'lee ann').map((o) => o.label)).toEqual(['Ann Lee']);
		expect(userOptions(users, '', new Set(['b'])).map((o) => o.value)).toEqual(['a', 'c']);
	});
});
