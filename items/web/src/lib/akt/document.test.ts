import { describe, expect, it } from 'vitest';
import { containedTypes, draftedFields } from './document';

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
});
