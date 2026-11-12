import { CombinedError } from '@urql/core';
import { GraphQLError } from 'graphql';
import { describe, expect, it } from 'vitest';
import { settle } from './editing';
import type { Draft } from './values';

const fresh: Record<string, Draft> = { title: { kind: 'TEXT', text: 'Changed elsewhere' } };
const refused = (code: string) =>
	new CombinedError({ graphQLErrors: [new GraphQLError('no', { extensions: { code } })] });

describe('after a change', () => {
	// r[verify app.stale]
	it('keeps what was entered on STALE, compared with the akt as it now is', () => {
		const settled = settle(refused('STALE'), fresh, {});
		expect(settled.refusal?.code).toBe('STALE');
		expect(settled.base).toEqual(fresh);
		expect(settled.drafts).toBeUndefined();
	});

	it('starts again from the akt on success, and moves nothing on other refusals', () => {
		expect(settle(undefined, fresh, {})).toEqual({ refusal: null, base: fresh, drafts: fresh });
		const forbidden = settle(refused('FORBIDDEN'), fresh, {});
		expect(forbidden.base).toBeUndefined();
		expect(forbidden.drafts).toBeUndefined();
	});
});
