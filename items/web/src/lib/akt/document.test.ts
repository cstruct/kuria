import { describe, expect, it } from 'vitest';
import {
	availableTransitions,
	containedTypes,
	draftedFields,
	givenIn,
	linkKinds,
	lackingText,
	outcomeRefusal,
	requiredFields,
	waitingFor
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
		expect(outcomeRefusal(finish, 'PENDING')).toBe(
			'Needs the result to be passed, failed or skipped; it is pending'
		);
		expect(outcomeRefusal({ outcomes: ['PASS'] }, null)).toBe(
			'Needs the result to be passed; it is pending'
		);
	});
	it('lets the outcomes it allows, and any for a transition naming none, through', () => {
		expect(outcomeRefusal(finish, 'FAIL')).toBeNull();
		expect(outcomeRefusal({ outcomes: [] }, null)).toBeNull();
	});
});

// r[verify link.referenced]
describe('linkKinds', () => {
	it('leaves out the kinds a markdown field keeps its references as', () => {
		const kind = (key: string) => ({ key, name: key, inverse: key, to: ['document'] });
		const document = {
			fields: [{ key: 'body', references: 'refers_to' }],
			links: [kind('refers_to'), kind('replaces')]
		};
		expect(linkKinds(document).map((k) => k.key)).toEqual(['replaces']);
	});
});

// r[verify akt.field.states]
describe('givenIn', () => {
	const document = {
		fields: [{ key: 'title' }, { key: 'released_on', states: ['released', 'closed'] }]
	};

	it('gives a field in its states only, or in all without any', () => {
		expect(givenIn(document, 'title', 'planning')).toBe(true);
		expect(givenIn(document, 'released_on', 'planning')).toBe(false);
		expect(givenIn(document, 'released_on', 'released')).toBe(true);
	});
});

// r[verify app.transitions]
describe('waitingFor', () => {
	const waits = [
		{ transition: 'close', open: ['CASE-1-A01'], lacking: ['Verified by', 'Reviewed by'] },
		{ transition: 'drop', open: [], lacking: [] }
	];
	it('finds what a transition waits for, if anything', () => {
		expect(waitingFor({ key: 'close' }, waits)?.open).toEqual(['CASE-1-A01']);
		expect(waitingFor({ key: 'drop' }, waits)).toBeNull();
		expect(waitingFor({ key: 'reopen' }, waits)).toBeNull();
		expect(waitingFor({ key: 'close' }, null)).toBeNull();
	});
	it('names the links it lacks', () => {
		expect(lackingText(['Verified by', 'Reviewed by'])).toBe(
			'a Verified by link and a Reviewed by link'
		);
	});
});

// r[verify app.transitions]
// r[verify akt.transition.requires-when]
describe('requiredFields, with conditions', () => {
	const fields = [{ key: 'kind' }, { key: 'root_cause' }, { key: 'hypothesis' }];
	const plan = {
		key: 'plan',
		from: ['open'],
		requires: [],
		requiresWhen: [
			{ field: 'root_cause', when: [{ field: 'kind', options: ['corrective'] }] },
			{ field: 'hypothesis', when: [{ field: 'kind', options: ['preventive'] }] }
		]
	};
	it('asks for a field while the akt meets its condition', () => {
		const keys = (held: string[]) => requiredFields(fields, plan, () => held).map((f) => f.key);
		expect(keys(['corrective'])).toEqual(['root_cause']);
		expect(keys(['corrective', 'preventive'])).toEqual(['root_cause', 'hypothesis']);
		expect(keys([])).toEqual([]);
		expect(requiredFields(fields, plan)).toEqual([]);
	});
});
