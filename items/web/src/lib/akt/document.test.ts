import { describe, expect, it } from 'vitest';
import { availableTransitions, containedTypes, draftedFields, requiredFields } from './document';

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
