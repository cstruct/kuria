import { describe, expect, it } from 'vitest';
import { excerptOf, referenceOfHref } from './previews';

// r[verify app.reference-preview]
describe('reference previews', () => {
	it('know the reference a link into the app goes to', () => {
		expect(referenceOfHref('/akt/D-4?version=2#install', '/akt/')).toEqual({
			number: 'D-4',
			version: 2,
			heading: 'install'
		});
		expect(referenceOfHref('/akt/D-4', '/akt/')).toEqual({
			number: 'D-4',
			version: null,
			heading: null
		});
		expect(referenceOfHref('/base/akt/D-4#a', '/base/akt/')?.heading).toBe('a');
		for (const other of ['https://example.com/akt/D-4', '#fn:1', '/akt/', '/akt/D-4/history']) {
			expect(referenceOfHref(other, '/akt/'), other).toBeNull();
		}
	});

	it('show the start of a heading’s section as plain text', () => {
		const text = [
			'Intro with **bold** and a [link](https://x) to snake_case.',
			'',
			'## Installing {#install}',
			'',
			'Run the *installer*.[^1]',
			'',
			'- [ ] Check ![logo](kuria:IMG-1) it',
			'',
			'```sh',
			'make install',
			'```',
			'',
			'### Afterwards {#after}',
			'',
			'> Restart.',
			'',
			'## Using {#use}',
			'',
			'Elsewhere.',
			'',
			'[^1]: A note.'
		].join('\n');
		expect(excerptOf(text, 'install')).toBe('Run the installer. Check it Afterwards Restart.');
		expect(excerptOf(text, 'use')).toBe('Elsewhere.');
		expect(excerptOf(text, 'missing')).toBe('');
		expect(excerptOf(text, null, 20)).toBe('Intro with bold and…');
	});
});
