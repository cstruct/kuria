import { describe, expect, it } from 'vitest';
import { readiness } from './outcomes';

// r[verify app.header]
describe('readiness', () => {
	it('counts the members with outcomes that passed', () => {
		expect(
			readiness([
				{ outcome: 'PASS' },
				{ outcome: 'PENDING' },
				{ outcome: null },
				{ outcome: 'FAIL' }
			])
		).toEqual({ passed: 1, of: 3 });
		expect(readiness([{ outcome: null }])).toBeNull();
	});
});
