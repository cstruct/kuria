import { describe, expect, it } from 'vitest';
import { conditionsOf, filterable, withCondition } from './filters';

const fields = [
	{ key: 'kind', name: 'Kind', kind: 'CHOICE', options: [{ key: 'fix', name: 'Fix' }] },
	{ key: 'assignee', name: 'Assignee', kind: 'USER' },
	{ key: 'notes', name: 'Notes', kind: 'TEXT', multiline: true },
	{ key: 'title', name: 'Title', kind: 'TEXT', multiline: false },
	{ key: 'sbom', name: 'SBOM', kind: 'ATTACHMENT' }
];

// r[verify app.filters]
describe('filters', () => {
	it('filter only the kinds the server filters', () => {
		expect(fields.filter(filterable).map((f) => f.key)).toEqual(['kind', 'assignee', 'title']);
	});

	it('read conditions from the address, and write them back', () => {
		let params = new URLSearchParams('state=open');
		params = withCondition(params, 'kind', 'fix');
		params = withCondition(params, 'assignee', 'me');
		params = withCondition(params, 'notes', 'x');
		expect(params.toString()).toBe('state=open&where.kind=fix&where.assignee=me&where.notes=x');
		// A field that can't be filtered is left out.
		expect(conditionsOf(params, fields)).toEqual([
			{ field: 'kind', equals: 'fix' },
			{ field: 'assignee', equals: 'me' }
		]);
		expect(withCondition(params, 'kind', '').has('where.kind')).toBe(false);
	});
});
