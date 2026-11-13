import { describe, expect, it } from 'vitest';
import { actionsOf, headingOf } from './header';

describe('the header', () => {
	// r[verify app.header]
	it('heads with the title, and offers the first transition first', () => {
		expect(headingOf({ number: 'EPIC-001', title: 'Sign-in' })).toBe('Sign-in');
		expect(headingOf({ number: 'RC-1', title: null })).toBe('RC-1');
		expect(actionsOf(['accept', 'reject'])).toEqual({ primary: 'accept', others: ['reject'] });
		expect(actionsOf([])).toEqual({ primary: undefined, others: [] });
	});
});
