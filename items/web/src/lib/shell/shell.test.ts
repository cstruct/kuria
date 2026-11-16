import { describe, expect, it } from 'vitest';
import { isSearchShortcut, moreWasOpen, rememberMore, shortcutLabel, splitTypes } from './shell';

function key(init: Partial<KeyboardEvent> & { key: string }, tagName = 'BODY') {
	return {
		metaKey: false,
		ctrlKey: false,
		altKey: false,
		target: { tagName },
		...init
	} as unknown as KeyboardEvent;
}

describe('the search shortcut', () => {
	// r[verify app.search]
	it('is / outside text fields, and Cmd-K or Ctrl-K anywhere', () => {
		expect(isSearchShortcut(key({ key: '/' }))).toBe(true);
		expect(isSearchShortcut(key({ key: '/' }, 'INPUT'))).toBe(false);
		expect(isSearchShortcut(key({ key: '/' }, 'TEXTAREA'))).toBe(false);
		expect(isSearchShortcut(key({ key: 'k', metaKey: true }, 'INPUT'))).toBe(true);
		expect(isSearchShortcut(key({ key: 'K', ctrlKey: true }))).toBe(true);
		expect(isSearchShortcut(key({ key: 'k' }))).toBe(false);
		expect(shortcutLabel('MacIntel')).toBe('⌘K');
		expect(shortcutLabel('Linux x86_64')).toBe('Ctrl K');
	});
});

// r[verify app.navigation]
describe('the sidebar', () => {
	const type = (key: string, navigation: 'MAIN' | 'MORE') => ({
		key,
		presentation: { navigation }
	});

	it('lists types under "Show more" apart, in order', () => {
		const { main, more } = splitTypes([
			type('a', 'MAIN'),
			type('img', 'MORE'),
			type('b', 'MAIN'),
			type('c', 'MORE')
		]);
		expect(main.map((t) => t.key)).toEqual(['a', 'b']);
		expect(more.map((t) => t.key)).toEqual(['img', 'c']);
	});

	it('remembers whether "Show more" is open, closed if it can’t', () => {
		const kept = new Map<string, string>();
		const storage = {
			getItem: (k: string) => kept.get(k) ?? null,
			setItem: (k: string, v: string) => void kept.set(k, v)
		};
		expect(moreWasOpen(storage)).toBe(false);
		rememberMore(storage, true);
		expect(moreWasOpen(storage)).toBe(true);
		rememberMore(storage, false);
		expect(moreWasOpen(storage)).toBe(false);
		const refusing = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		rememberMore(refusing, true);
		expect(moreWasOpen(refusing)).toBe(false);
		expect(moreWasOpen(undefined)).toBe(false);
	});
});
