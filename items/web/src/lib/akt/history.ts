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

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The longest text a change shows before it is shortened. */
const LONG = 140;

/** A stored value as short text. Users are named as such, since revisions
 * hold their identifiers. */
export function valueText(value: unknown): string | null {
	if (value === null || value === undefined) return null;
	if (typeof value === 'string') {
		if (UUID.test(value)) return 'a user';
		const flat = value.replace(/\s+/g, ' ').trim();
		return flat.length > LONG ? `${flat.slice(0, LONG - 1)}…` : flat;
	}
	if (typeof value === 'boolean' || typeof value === 'number') return String(value);
	if (Array.isArray(value)) return value.map((v) => valueText(v) ?? '').join(', ');
	if (typeof value === 'object') {
		const o = value as Record<string, unknown>;
		if ('checked_by' in o) return o.for ? 'checked for someone' : 'checked';
		if ('contact' in o) return 'a contact';
		if ('url' in o) return String(o.label ?? o.url);
		if ('digest' in o) return `${o.repository}${o.tag ? `:${o.tag}` : ''}`;
		if ('repository' in o) return `${o.repository}${o.tag ? `@${o.tag}` : ''}`;
	}
	return JSON.stringify(value);
}

/** One revision, described with the akt type's field and transition names. */
export function describe(
	r: RevisionData,
	fieldNames: Record<string, string>,
	transitionNames: Record<string, string>
): Described {
	const changes = Object.entries((r.changes as Record<string, unknown>) ?? {}).map(
		([key, value]) => ({
			field: fieldNames[key] ?? key,
			value: valueText(value)
		})
	);
	const memberships = Object.entries(
		(r.membership as Record<string, { number: string }[]>) ?? {}
	).flatMap(([kind, entries]) => entries.map((e) => `${MEMBERSHIP[kind] ?? kind} ${e.number}`));
	let what: string;
	if (r.transition) what = `performed ${transitionNames[r.transition] ?? r.transition}`;
	else if (r.revision === 1) what = 'created it';
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
