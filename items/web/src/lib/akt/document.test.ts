import { describe, expect, it } from 'vitest';
import {
	availableTransitions,
	containedTypes,
	draftedFields,
	outcomeRefusal,
	requiredFields
} from './document';

describe('akt type documents', () => {
	// r[verify app.akt-types]
	it('says what an akt type contains and drafts, whatever it is', () => {
		const document = {
			contains: [{ type: 'backlog' }, { type: 'release_candidate', while: ['planning'] }],
			fields: [{ key: 'text', draft: { types: ['epic'] } }, { key: 'summary' }]
		};
		expect(containedTypes(document)).toEqual(['backlog', 'release_candidate']);
		expect(draftedFields(document)).toEqual(['text']);
		expect(containedTypes(null)).toEqual([]);
		expect(draftedFields({})).toEqual([]);
	});

	// r[verify app.transitions]
	it('offers the transitions a state allows, with the fields they require', () => {
		const transitions = [
			{ key: 'test', from: ['planning'], requires: [] },
			{ key: 'release', from: ['testing'], requires: ['version', 'go'] }
		];
		expect(
			availableTransitions(transitions, { key: 'testing', final: false }).map((t) => t.key)
		).toEqual(['release']);
		expect(availableTransitions(transitions, { key: 'testing', final: true })).toEqual([]);
		const fields = [{ key: 'title' }, { key: 'go' }, { key: 'version' }];
		expect(requiredFields(fields, transitions[1]).map((f) => f.key)).toEqual(['go', 'version']);
	});
});

// r[verify app.transitions]
describe('outcomeRefusal', () => {
	const finish = { outcomes: ['PASS', 'FAIL', 'SKIP'] };
	it('says what a transition allows while the outcome is another', () => {
		expect(outcomeRefusal(finish, 'PENDING')).toBe('Only when the outcome is pass, fail or skip');
		expect(outcomeRefusal({ outcomes: ['PASS'] }, null)).toBe('Only when the outcome is pass');
	});
	it('lets the outcomes it allows, and any for a transition naming none, through', () => {
		expect(outcomeRefusal(finish, 'FAIL')).toBeNull();
		expect(outcomeRefusal({ outcomes: [] }, null)).toBeNull();
	});
});
