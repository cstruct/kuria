import { describe, expect, it } from 'vitest';
import type { Value } from './values';
import { judgedOf, reportError, verdictInput } from './verdicts';

const ALICE = { __typename: 'User' as const, id: 'alice', name: 'Alice' };
const BOB = { __typename: 'Contact' as const, id: 'c-bob', displayName: 'Bob' };
const type = {
	fields: [
		{ key: 'testers', kind: 'USER', assignees: null },
		{ key: 'verdict', kind: 'VERDICT', assignees: 'testers' }
	]
};

describe('verdicts', () => {
	// r[verify app.verdicts]
	it("says each assignee's verdict and the viewer's own", () => {
		const fields: Value[] = [
			{ __typename: 'UsersValue', field: 'testers', users: [ALICE, BOB] },
			{
				__typename: 'VerdictValue',
				field: 'verdict',
				verdicts: [{ verdict: 'FAIL', description: 'It broke', assignee: BOB }]
			}
		];
		const judged = judgedOf({ fields, type }, 'alice');
		expect(judged).toEqual({
			field: 'verdict',
			assignees: [
				{ id: 'alice', name: 'Alice', verdict: null, description: null },
				{ id: 'c-bob', name: 'Bob', verdict: 'FAIL', description: 'It broke' }
			],
			mine: null
		});
		expect(judgedOf({ fields, type }, 'eve')?.mine).toBeUndefined();
		expect(judgedOf({ fields, type: { fields: [] } }, 'alice')).toBeNull();
		expect(judgedOf({ fields, type }, 'c-bob')?.mine).toEqual({
			verdict: 'FAIL',
			description: 'It broke'
		});
	});

	// r[verify app.verdicts]
	it('reports a verdict with its description, which a fail or a skip must have', () => {
		expect(verdictInput({ verdict: 'PASS', description: ' ' })).toBe('pass');
		expect(verdictInput({ verdict: 'SKIP', description: ' No device ' })).toEqual({
			verdict: 'skip',
			description: 'No device'
		});
		expect(verdictInput(null)).toEqual({ clear: true });
		expect(reportError({ verdict: null, description: '' })).toBe('Choose a status.');
		expect(reportError({ verdict: 'PASS', description: '' })).toBeNull();
		expect(reportError({ verdict: 'FAIL', description: '  ' })).toBe('Say why it failed.');
		expect(reportError({ verdict: 'SKIP', description: '' })).toBe('Say why it was skipped.');
		expect(reportError({ verdict: 'SKIP', description: 'No device' })).toBeNull();
	});
});
