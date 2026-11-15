import { describe, expect, it } from 'vitest';
import { conditionsOf, filterable, filtersByOutcome, outcomeOf, withCondition } from './filters';

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

// r[verify app.filters]
describe('the outcome filter', () => {
	const tests = [{ key: 'verdict', name: 'Verdict', kind: 'VERDICT' }];
	it('is read from the address for a type with a verdict field only', () => {
		const params = new URLSearchParams('outcome=pending');
		expect(filtersByOutcome(tests)).toBe(true);
		expect(outcomeOf(params, tests)).toBe('PENDING');
		expect(outcomeOf(params, fields)).toBeNull();
		expect(outcomeOf(new URLSearchParams('outcome=maybe'), tests)).toBeNull();
	});
});
