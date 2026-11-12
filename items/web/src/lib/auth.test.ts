import { describe, expect, it } from 'vitest';
import { signInUrl, signedOut } from './auth';

describe('signing in', () => {
	// r[verify app.sign-in]
	it('returns to the page it was on, on this site only', () => {
		expect(signInUrl('/akt/RELEASE-2026-0001?tab=board')).toBe(
			'/auth/login?return_to=%2Fakt%2FRELEASE-2026-0001%3Ftab%3Dboard'
		);
		expect(signInUrl('https://evil.example/')).toBe('/auth/login?return_to=%2F');
		expect(signInUrl('//evil.example/')).toBe('/auth/login?return_to=%2F');
	});

	it('takes a 401 for being signed out', () => {
		expect(signedOut(401)).toBe(true);
		expect(signedOut(403)).toBe(false);
		expect(signedOut(undefined)).toBe(false);
	});
});
