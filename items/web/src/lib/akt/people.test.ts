import { describe, expect, it } from 'vitest';
import { creatorDefaults, initialsOf, peopleOf } from './people';

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

describe('people by name', () => {
	// r[verify app.fields]
	it('names the viewer by their name, when Kuria knows one', () => {
		expect(peopleOf('me', [], [], { name: 'Rami Moberg', email: 'rm@shop.example' })[0].name).toBe(
			'Me (Rami Moberg)'
		);
		expect(
			peopleOf('me', [], [], { name: 'rm@shop.example', email: 'rm@shop.example' })[0].name
		).toBe('Me');
	});

	// r[verify akt.field.default]
	it('reads the fields that default to their creator', () => {
		expect(
			creatorDefaults({
				fields: [
					{ key: 'title' },
					{ key: 'manager', default: 'creator' },
					{ key: 'owner', default: 'creator' }
				]
			})
		).toEqual(['manager', 'owner']);
		expect(creatorDefaults(null)).toEqual([]);
	});

	it('makes initials of a name or an email', () => {
		expect(initialsOf('Devi Larsen')).toBe('DL');
		expect(initialsOf('rm@shop.example')).toBe('R');
		expect(initialsOf('hedda.lund@shop.example')).toBe('HL');
	});
});
