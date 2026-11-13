import { describe, expect, it } from 'vitest';
import { groupMembers, mappedTypes, placeFor, shownFields, sortAkter } from './members';
import type { Value } from './values';

const member = (number: string, type: string) => ({
	number,
	type: { key: type, name: type.toUpperCase() }
});

const document = {
	board: {
		columns: [
			{ key: 'todo', name: 'To do', states: { story: ['open'] } },
			{ key: 'done', name: 'Done', states: { story: ['done'], bug: ['fixed'] } }
		]
	}
};

// r[verify app.members]
describe('groupMembers', () => {
	it('knows which akt types the columns name', () => {
		expect([...mappedTypes(document)].sort()).toEqual(['bug', 'story']);
		expect(mappedTypes({}).size).toBe(0);
		expect(mappedTypes(null).size).toBe(0);
	});

	it('lists the akt types no column names by type, in contained order', () => {
		const columns = [
			{ key: 'todo', name: 'To do', akter: [member('S-1', 'story')] },
			{ key: 'done', name: 'Done', akter: [] },
			{
				key: 'unmapped',
				name: 'Unmapped',
				akter: [
					member('N-1', 'note'),
					member('B-1', 'bug'),
					member('A-1', 'artifact'),
					member('N-2', 'note')
				]
			}
		];
		const grouped = groupMembers(columns, mappedTypes(document), [
			'story',
			'bug',
			'artifact',
			'note'
		]);
		expect(grouped.columns.map((c) => [c.key, c.akter.map((m) => m.number)])).toEqual([
			['todo', ['S-1']],
			['done', []],
			['unmapped', ['B-1']]
		]);
		expect(grouped.lists.map((l) => [l.name, l.akter.map((m) => m.number)])).toEqual([
			['ARTIFACT', ['A-1']],
			['NOTE', ['N-1', 'N-2']]
		]);
	});

	it('keeps no unmapped column when every mapped member is placed', () => {
		const columns = [
			{ key: 'todo', name: 'To do', akter: [member('S-1', 'story')] },
			{ key: 'unmapped', name: 'Unmapped', akter: [member('N-1', 'note')] }
		];
		const grouped = groupMembers(columns, mappedTypes(document), ['story', 'note']);
		expect(grouped.columns.map((c) => c.key)).toEqual(['todo']);
		expect(grouped.lists.map((l) => l.type)).toEqual(['note']);
	});

	it('lists every member by type without a board', () => {
		const columns = [
			{ key: 'unmapped', name: 'Unmapped', akter: [member('S-1', 'story'), member('B-1', 'bug')] }
		];
		const grouped = groupMembers(columns, new Set(), ['bug', 'story']);
		expect(grouped.columns).toEqual([]);
		expect(grouped.lists.map((l) => l.type)).toEqual(['bug', 'story']);
	});
});

describe('shownFields', () => {
	const akt = {
		fields: [{ __typename: 'TextValue', field: 'kind', text: 'Feature' }] as unknown as Value[],
		type: {
			fields: [
				{ key: 'kind', name: 'Kind' },
				{ key: 'owner', name: 'Owner' }
			],
			presentation: { card: ['kind', 'gone'], list: ['owner', 'kind'] }
		}
	};

	it('names the fields the presentation picks, with their values', () => {
		expect(shownFields(akt, 'card')).toEqual([{ name: 'Kind', text: 'Feature' }]);
		expect(shownFields(akt, 'list')).toEqual([
			{ name: 'Owner', text: '' },
			{ name: 'Kind', text: 'Feature' }
		]);
	});
});

describe('placeFor', () => {
	const order = ['A', 'B', 'C', 'D'];

	it('places a member before the one it is dropped on', () => {
		expect(placeFor(order, 'D', 1)).toEqual({ before: 'B' });
		expect(placeFor(order, 'A', 3)).toEqual({ before: 'D' });
	});

	it('places a member after the last when dropped at the end', () => {
		expect(placeFor(order, 'A', 4)).toEqual({ after: 'D' });
	});

	it('moves nothing when dropped where it is', () => {
		expect(placeFor(order, 'B', 1)).toBeNull();
		expect(placeFor(order, 'B', 2)).toBeNull();
		expect(placeFor(order, 'X', 0)).toBeNull();
	});
});

describe('sortAkter', () => {
	const akter = [
		{ number: 'A', rank: 'b', updatedAt: '2026-11-01T10:00:00Z' },
		{ number: 'B', rank: 'a', updatedAt: '2026-11-03T10:00:00Z' },
		{ number: 'C', rank: 'c', updatedAt: '2026-11-02T10:00:00Z' }
	];

	it('sorts by rank or by when they last changed', () => {
		expect(sortAkter(akter, 'rank').map((a) => a.number)).toEqual(['B', 'A', 'C']);
		expect(sortAkter(akter, 'updated').map((a) => a.number)).toEqual(['B', 'C', 'A']);
	});
});
