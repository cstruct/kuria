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

/** Everyone a picker offers: the viewer as "Me", the tenant's other users,
 * then its contacts, a linked one as the user it is. Each once, by whom
 * they are. Users and contacts are there only for those who may read
 * contacts; the server refuses them to anyone else. */
// r[impl app.fields]
export function peopleOf(
	me: string | undefined,
	users: readonly { id: string; name: string }[] = [],
	contacts: readonly { id: string; displayName: string; user?: { id: string } | null }[] = [],
	/** The viewer, as recorded: named "Me (their name)" when Kuria knows
	 * a name other than their email. */
	viewer?: { name: string; email?: string | null }
): Person[] {
	const myName = viewer && viewer.name !== viewer.email ? viewer.name : undefined;
	const people: Person[] = [
		...(me ? [{ who: { user: me }, name: myName ? `Me (${myName})` : 'Me' }] : []),
		...users.map((u) => ({ who: { user: u.id }, name: u.name })),
		...contacts.map((c) => ({
			who: c.user ? { user: c.user.id } : { contact: c.id },
			name: c.displayName
		}))
	];
	const seen = new Set<string>();
	return people.filter((p) => {
		const key = keyOf(p.who);
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}

/** The keys of a type document's `user` fields that default to the one
 * creating an akt (`akt.field.default`). */
// r[impl akt.field.default]
export function creatorDefaults(document: unknown): string[] {
	const fields = (document as { fields?: { key?: string; default?: string }[] } | null)?.fields;
	return (fields ?? []).flatMap((f) => (f.default === 'creator' && f.key ? [f.key] : []));
}

/** Initials for an avatar: the first letters of a name's first two words,
 * or of an email's name. */
export function initialsOf(name: string): string {
	const words = name
		.split('@')[0]
		.split(/[\s._-]+/)
		.filter(Boolean);
	return words
		.slice(0, 2)
		.map((w) => w[0].toUpperCase())
		.join('');
}
