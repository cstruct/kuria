import { describe, expect, it } from 'vitest';
import { keysOf } from './keys';

describe('keysOf', () => {
	it('embeds types without an id, and keys akt types by key and version', () => {
		const keys = keysOf(
			[
				{ kind: 'OBJECT', name: 'Akt', fields: [{ name: 'id' }, { name: 'number' }] },
				{ kind: 'OBJECT', name: 'TextValue', fields: [{ name: 'field' }] },
				{ kind: 'OBJECT', name: 'AktType', fields: [{ name: 'key' }] },
				{ kind: 'OBJECT', name: 'Query', fields: [{ name: 'akt' }] },
				{ kind: 'UNION', name: 'FieldValue' }
			],
			['Query']
		);
		expect(Object.keys(keys).sort()).toEqual(['AktType', 'TextValue']);
		expect(keys.TextValue?.({ __typename: 'TextValue' })).toBeNull();
		expect(keys.AktType?.({ __typename: 'AktType', key: 'release', version: 3 })).toBe('release:3');
	});
});
