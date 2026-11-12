import { describe, expect, it } from 'vitest';
import { isSearchShortcut, shortcutLabel } from './shell';

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
