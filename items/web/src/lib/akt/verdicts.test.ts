import { describe, expect, it } from 'vitest';
import type { Value } from './values';
import { judgedOf, verdictInput } from './verdicts';

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
				verdicts: [{ verdict: 'FAIL', assignee: BOB }]
			}
		];
		const judged = judgedOf({ fields, type }, 'alice');
		expect(judged).toEqual({
			field: 'verdict',
			assignees: [
				{ id: 'alice', name: 'Alice', verdict: null },
				{ id: 'c-bob', name: 'Bob', verdict: 'FAIL' }
			],
			mine: null
		});
		expect(judgedOf({ fields, type }, 'eve')?.mine).toBeUndefined();
		expect(judgedOf({ fields, type: { fields: [] } }, 'alice')).toBeNull();
		expect(verdictInput('SKIP')).toBe('skip');
		expect(verdictInput(null)).toEqual({ clear: true });
	});
});
