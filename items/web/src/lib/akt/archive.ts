/** Archived akter as the app shows them (`app.archive`): live akter by
 * default, **Show archived** kept in the page's address as
 * `archived=all`, and what an archived akt says of how it was archived.
 * Nothing here knows any akt type. */
// r[impl app.archive]

const PARAM = 'archived';

/** The subject an archive by a rule is authored by: Kuria itself. */
export const KURIA_SUBJECT = '00000000-0000-0000-0000-000000000000';

/** Whether an address asks for archived akter too (`archived=all`). */
export function showsArchived(params: { get(name: string): string | null }): boolean {
	return params.get(PARAM)?.toLowerCase() === 'all';
}

/** The `ArchivedFilter` a read asks for: `ALL` with **Show archived**,
 * else `null`, the server's default (live akter, or all of an archived
 * container's members). */
export function archivedFilterOf(params: { get(name: string): string | null }): 'ALL' | null {
	return showsArchived(params) ? 'ALL' : null;
}

/** The address's parameters with **Show archived** on or off. */
export function withArchived(params: { toString(): string }, on: boolean): URLSearchParams {
	const next = new URLSearchParams(params.toString());
	if (on) next.set(PARAM, 'all');
	else next.delete(PARAM);
	return next;
}

/** How many of a container's members are archived, in words. */
export function archivedCount(count: number): string {
	return `${count} archived`;
}

/** Whether `subject` is Kuria itself, as rules archive. */
export function isKuria(subject: string | null | undefined): boolean {
	return subject === KURIA_SUBJECT;
}

export interface ArchivedShape {
	at: string;
	by?: { id: string; name: string } | null;
	rule?: string | null;
	reason?: string | null;
}

/** Who archived an akt: a person by name, else Kuria by its rule. */
export function archivedBy(archived: ArchivedShape): string {
	if (archived.by && !isKuria(archived.by.id)) return archived.by.name;
	return archived.rule ? `Kuria, by the rule “${archived.rule}”` : 'Kuria';
}

/** What an archived akt's page says of it: when, by whom, and why. */
export function archivedSummary(
	archived: ArchivedShape,
	date: (at: string) => string = (at) => new Date(at).toLocaleDateString()
): { title: string; reason: string | null } {
	return {
		title: `Archived ${date(archived.at)} by ${archivedBy(archived)}`,
		reason: archived.reason?.trim() || null
	};
}
