/** What the app shell decides outside its components. */

/** Below this width the sidebar becomes a drawer behind a menu button. */
// r[impl app.narrow]
export const NARROW = '(max-width: 899px)';

/** Whether `event` asks for search: `/` outside a text field, or
 * Cmd-K / Ctrl-K anywhere. */
// r[impl app.search]
export function isSearchShortcut(event: KeyboardEvent): boolean {
	if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') return true;
	if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return false;
	const target = event.target as HTMLElement | null;
	const typing =
		target?.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target?.tagName ?? '');
	return !typing;
}

/** The shortcut's name on this platform. */
export function shortcutLabel(platform: string): string {
	return /mac|iphone|ipad/i.test(platform) ? '⌘K' : 'Ctrl K';
}

/** An akt type as the sidebar lists it. */
export interface Listed {
	key: string;
	presentation: { navigation: 'MAIN' | 'MORE' };
}

/** The types the sidebar lists, split into those it shows and those under
 * "Show more", each in the order given (`app.navigation`). */
// r[impl app.navigation]
export function splitTypes<T extends Listed>(types: readonly T[]): { main: T[]; more: T[] } {
	return {
		main: types.filter((t) => t.presentation.navigation !== 'MORE'),
		more: types.filter((t) => t.presentation.navigation === 'MORE')
	};
}

const MORE_OPEN = 'kuria.sidebar.more';

/** Whether "Show more" was left open in this browser. */
export function moreWasOpen(storage: Pick<Storage, 'getItem'> | undefined): boolean {
	try {
		return storage?.getItem(MORE_OPEN) === 'open';
	} catch {
		return false;
	}
}

/** Remembers whether "Show more" is open. */
export function rememberMore(storage: Pick<Storage, 'setItem'> | undefined, open: boolean): void {
	try {
		storage?.setItem(MORE_OPEN, open ? 'open' : 'closed');
	} catch {
		// Storage can be refused; the disclosure then starts closed.
	}
}
