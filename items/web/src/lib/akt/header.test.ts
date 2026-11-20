import { describe, expect, it } from 'vitest';
import { actionsOf, confirmationOf, headingOf } from './header';

const close = { key: 'close' };
const reopen = { key: 'reopen' };
const document = { presentation: { primary: { open: 'close', template: 'copy' } } };

describe('the header', () => {
	// r[verify app.header]
	it('heads with the title, else the number', () => {
		expect(headingOf({ number: 'EPIC-001', title: 'Sign-in' })).toBe('Sign-in');
		expect(headingOf({ number: 'RC-1', title: null })).toBe('RC-1');
	});

	// r[verify app.header]
	it('offers first what the presentation names for the state', () => {
		expect(actionsOf([reopen, close], { document, state: 'open' })).toEqual({
			primary: { kind: 'transition', transition: close },
			others: [reopen]
		});
		expect(actionsOf([close], { document, state: 'template', copyable: true })).toEqual({
			primary: { kind: 'copy' },
			others: [close]
		});
		// A copy the viewer may not make isn't offered.
		expect(actionsOf([close], { document, state: 'template' }).primary).toBeUndefined();
	});

	// r[verify app.header]
	it('else creating a member, and never a transition by default', () => {
		const creatable = [{ name: 'Task', href: '/types/task/new?in=B-1' }];
		expect(actionsOf([close], { state: 'open', creatable })).toEqual({
			primary: { kind: 'create', name: 'Task', href: '/types/task/new?in=B-1' },
			others: [close]
		});
		expect(actionsOf([close, reopen], { state: 'open' })).toEqual({
			primary: undefined,
			others: [close, reopen]
		});
		// A named transition the state doesn't offer falls through.
		expect(actionsOf([reopen], { document, state: 'open' }).primary).toBeUndefined();
	});

	// r[verify app.header]
	it('confirms a transition to a final state', () => {
		expect(confirmationOf('Autumn', { name: 'Closed', final: true })).toBe(
			'Autumn moves to Closed, where it stays.'
		);
		expect(confirmationOf('Autumn', { name: 'Testing', final: false })).toBeUndefined();
	});

	// r[verify app.header]
	// r[verify akt.transition.reopens]
	it('says a final state can be left by reopening, where it can', () => {
		const reopen = { from: ['closed'], reopens: true };
		expect(
			confirmationOf('CAPA-2026-0001', { key: 'closed', name: 'Closed', final: true }, [reopen])
		).toBe('CAPA-2026-0001 moves to Closed, where it stays unless it is reopened.');
	});
});
