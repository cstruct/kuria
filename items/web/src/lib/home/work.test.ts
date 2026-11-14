import { describe, expect, it } from 'vitest';
import { workByType, workQueries } from './work';

// r[verify app.my-work]
describe('my work', () => {
	it('asks one list per user field and checkbox', () => {
		expect(
			workQueries([
				{
					key: 'release',
					fields: [
						{ key: 'manager', kind: 'USER' },
						{ key: 'go', kind: 'CHECKBOX' },
						{ key: 'title', kind: 'TEXT' },
						// Several people can't be filtered by.
						{ key: 'testers', kind: 'USER', multiple: true }
					]
				},
				{ key: 'backlog', fields: [{ key: 'title', kind: 'TEXT' }] }
			])
		).toEqual([
			{ type: 'release', field: 'manager' },
			{ type: 'release', field: 'go' }
		]);
	});

	it('shows each akt once, by type, newest first', () => {
		const a = { id: 'a', updatedAt: '2026-11-01T00:00:00Z' };
		const b = { id: 'b', updatedAt: '2026-11-02T00:00:00Z' };
		expect(
			workByType(
				['release', 'task', 'backlog'],
				[
					{ type: 'task', nodes: [a] },
					{ type: 'release', nodes: [a, b] },
					{ type: 'release', nodes: [a] }
				]
			)
		).toEqual([
			{ type: 'release', akter: [b, a] },
			{ type: 'task', akter: [a] }
		]);
	});
});
