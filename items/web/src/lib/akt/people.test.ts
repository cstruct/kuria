import { describe, expect, it } from 'vitest';
import { peopleOf } from './people';

// r[verify app.fields]
describe('peopleOf', () => {
	it('offers me, the tenant’s users, then contacts, each once', () => {
		const people = peopleOf(
			'me',
			[
				{ id: 'me', name: 'Me Myself' },
				{ id: 'ann', name: 'Ann' }
			],
			[
				{ id: 'c1', displayName: 'Ann', user: { id: 'ann' } },
				{ id: 'c2', displayName: 'Bob', user: null }
			]
		);
		expect(people).toEqual([
			{ who: { user: 'me' }, name: 'Me' },
			{ who: { user: 'ann' }, name: 'Ann' },
			{ who: { contact: 'c2' }, name: 'Bob' }
		]);
		expect(peopleOf(undefined)).toEqual([]);
	});
});
