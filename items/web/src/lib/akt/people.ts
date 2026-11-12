import type { Who } from './values';

/** Someone a user field or a checkbox can name, as a picker lists them. */
export interface Person {
	who: Who;
	name: string;
}

export function keyOf(who: Who | null): string {
	if (!who) return '';
	return 'user' in who ? `user:${who.user}` : `contact:${who.contact}`;
}

export function whoOfKey(key: string): Who | null {
	if (key.startsWith('user:')) return { user: key.slice(5) };
	if (key.startsWith('contact:')) return { contact: key.slice(8) };
	return null;
}
