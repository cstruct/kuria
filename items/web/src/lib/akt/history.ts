/** An akt's history as people read it: who did what, when, and what it
 * changed, memberships named. */
// r[impl app.history]

export interface RevisionData {
	revision: number;
	transition?: string | null;
	reason?: string | null;
	changes: unknown;
	membership?: unknown;
	createdAt: string;
	by: { name: string };
	/** The users and contacts its values name, with their names. */
	named?: readonly Named[];
}

/** A user or contact as the server names it. */
export type Named =
	| { __typename: 'User'; id: string; name: string }
	| { __typename: 'Contact'; id: string; displayName: string };

/** Names by user or contact identifier. */
export function namesOf(named: readonly Named[] = []): Map<string, string> {
	return new Map(named.map((n) => [n.id, n.__typename === 'User' ? n.name : n.displayName]));
}

export interface Described {
	revision: number;
	who: string;
	at: string;
	/** "performed Release", "created it", "edited it" or "changed its containers". */
	what: string;
	reason: string | null;
	/** Field name and the value it was set to, as text; `null` when cleared. */
	changes: { field: string; value: string | null }[];
	memberships: string[];
}

const MEMBERSHIP: Record<string, string> = {
	added: 'Added to',
	removed: 'Removed from',
	added_member: 'Added',
	removed_member: 'Removed'
};

/** Link changes (`link.revisions`), each said with the link's name as this
 * akt reads it, such as "Linked: Blocked by TASK-1". */
const LINKS: Record<string, string> = {
	linked: 'Linked:',
	linked_from: 'Linked:',
	unlinked: 'Unlinked:',
	unlinked_from: 'Unlinked:'
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The longest text a change shows before it is shortened. */
const LONG = 140;

/** A stored value as short text. Revisions hold users' and contacts'
 * identifiers; `names` shows them by name, else as "a user" or "a contact". */
export function valueText(value: unknown, names: Map<string, string> = new Map()): string | null {
	if (value === null || value === undefined) return null;
	const someone = (who: unknown): string => {
		if (typeof who === 'string') return names.get(who) ?? 'a user';
		const contact = (who as { contact?: string } | null)?.contact;
		return (contact && names.get(contact)) ?? 'a contact';
	};
	if (typeof value === 'string') {
		if (UUID.test(value)) return someone(value);
		const flat = value.replace(/\s+/g, ' ').trim();
		return flat.length > LONG ? `${flat.slice(0, LONG - 1)}…` : flat;
	}
	if (typeof value === 'boolean' || typeof value === 'number') return String(value);
	if (Array.isArray(value)) return value.map((v) => valueText(v, names) ?? '').join(', ');
	if (typeof value === 'object') {
		const o = value as Record<string, unknown>;
		if ('checked_by' in o)
			return o.for && o.for !== o.checked_by
				? `checked for ${someone(o.for)} by ${someone(o.checked_by)}`
				: `checked by ${someone(o.checked_by)}`;
		if ('contact' in o) return someone(o);
		if ('url' in o) return String(o.label ?? o.url);
		if ('digest' in o) return `${o.repository}${o.tag ? `:${o.tag}` : ''}`;
		if ('repository' in o) return `${o.repository}${o.tag ? `@${o.tag}` : ''}`;
		if ('verdict' in o && 'assignee' in o) return `${someone(o.assignee)}: ${o.verdict}`;
		if (Array.isArray(o.items)) {
			const title = typeof o.title === 'string' ? o.title : 'a section';
			return `${title} (${valueText(o.items, names) || 'empty'})`;
		}
	}
	return JSON.stringify(value);
}

/** One revision, described with the akt type's field and transition names. */
export function describe(
	r: RevisionData,
	fieldNames: Record<string, string>,
	transitionNames: Record<string, string>
): Described {
	const names = namesOf(r.named);
	const changes = Object.entries((r.changes as Record<string, unknown>) ?? {}).map(
		([key, value]) => ({
			field: fieldNames[key] ?? key,
			value: valueText(value, names)
		})
	);
	const membership = Object.entries(
		(r.membership as Record<string, { number: string; name?: string }[]>) ?? {}
	);
	const memberships = membership.flatMap(([kind, entries]) =>
		entries.map((e) =>
			kind in LINKS
				? `${LINKS[kind]} ${e.name ?? ''} ${e.number}`.replace(/\s+/g, ' ')
				: `${MEMBERSHIP[kind] ?? kind} ${e.number}`
		)
	);
	const onlyLinks = membership.length > 0 && membership.every(([kind]) => kind in LINKS);
	let what: string;
	if (r.transition) what = `performed ${transitionNames[r.transition] ?? r.transition}`;
	else if (r.revision === 1) what = 'created it';
	else if (onlyLinks && changes.length === 0) what = 'changed its links';
	else if (memberships.length > 0 && changes.length === 0)
		what = 'changed its containers or members';
	else what = 'edited it';
	return {
		revision: r.revision,
		who: r.by.name,
		at: r.createdAt,
		what,
		reason: r.reason ?? null,
		changes,
		memberships
	};
}

/** How long ago `at` was, from `now`, in words. */
export function ago(at: string, now: Date = new Date()): string {
	const seconds = Math.round((now.getTime() - new Date(at).getTime()) / 1000);
	const format = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
	const steps: [number, Intl.RelativeTimeFormatUnit][] = [
		[60, 'second'],
		[60, 'minute'],
		[24, 'hour'],
		[30, 'day'],
		[12, 'month'],
		[Infinity, 'year']
	];
	let value = seconds;
	for (const [size, unit] of steps) {
		if (Math.abs(value) < size)
			return unit === 'second' && Math.abs(value) < 10 ? 'just now' : format.format(-value, unit);
		value = Math.round(value / size);
	}
	return format.format(-value, 'year');
}
