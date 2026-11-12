import { CombinedError } from '@urql/core';
import { GraphQLError } from 'graphql';
import { describe, expect, it } from 'vitest';
import { refusalOf } from './errors';

function refused(message: string, code: string) {
	return new CombinedError({
		graphQLErrors: [new GraphQLError(message, { extensions: { code } })]
	});
}

describe('refusals', () => {
	// r[verify app.errors]
	it('has a message for each code, and names a refused value’s field', () => {
		expect(refusalOf(refused('not permitted', 'FORBIDDEN')).message).toContain('aren’t allowed');
		expect(refusalOf(refused('the akt is at revision 3, not 2', 'STALE')).code).toBe('STALE');
		expect(
			refusalOf(refused('`root_cause`: expected a string', 'REFUSED'), { root_cause: 'Root cause' })
		).toEqual({ code: 'REFUSED', field: 'root_cause', message: 'Root cause: expected a string' });
		expect(refusalOf(refused('`close` requires root_cause', 'REFUSED'))).toMatchObject({
			code: 'REFUSED',
			message: 'close: requires root_cause'
		});
		expect(refusalOf(new CombinedError({ networkError: new Error('down') })).code).toBe('NETWORK');
	});
});
