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
