/** How a container's page lays out its members: on the board, the akt
 * types its columns name; every other akt type as a list of its own. The
 * server's board keeps everything else in `unmapped` (`board.unmapped`);
 * this regroups it. Nothing here knows any akt type. */
// r[impl app.members]
// r[impl app.akt-types]
import { textOf, type Value } from './values';

const UNMAPPED = 'unmapped';

interface Document {
	board?: { columns?: { states?: Record<string, unknown> }[] };
}

/** The akt types, by key, that a column of the board names. */
export function mappedTypes(document: unknown): Set<string> {
	const doc = typeof document === 'object' && document !== null ? (document as Document) : {};
	return new Set((doc.board?.columns ?? []).flatMap((c) => Object.keys(c.states ?? {})));
}

interface ColumnShape {
	key: string;
	name: string;
	akter: readonly { type: { key: string; name: string } }[];
}

type MemberOf<C extends ColumnShape> = C['akter'][number];

export interface MemberList<M> {
	type: string;
	name: string;
	akter: M[];
}

export interface Members<C extends ColumnShape> {
	/** The configured columns, and `unmapped` when members of a type the
	 * columns name are in a state none does. Empty with no columns. */
	columns: C[];
	/** Members of each other akt type, in the order the type is contained
	 * and then the order they come in, each in rank order. */
	lists: MemberList<MemberOf<C>>[];
}

/** Splits a board into the columns for the akt types its columns name
 * and a list per other akt type. `contained` orders the lists. */
export function groupMembers<C extends ColumnShape>(
	columns: readonly C[],
	mapped: Set<string>,
	contained: readonly string[]
): Members<C> {
	type M = MemberOf<C>;
	const configured = columns.filter((c) => c.key !== UNMAPPED);
	const unmapped = columns.find((c) => c.key === UNMAPPED)?.akter ?? [];
	const stray = unmapped.filter((m) => mapped.has(m.type.key));
	const shown: C[] =
		configured.length === 0
			? []
			: stray.length > 0
				? [...configured, { ...columns.find((c) => c.key === UNMAPPED)!, akter: stray }]
				: configured;
	const lists = new Map<string, MemberList<M>>();
	for (const key of contained) {
		if (!mapped.has(key) || configured.length === 0)
			lists.set(key, { type: key, name: key, akter: [] });
	}
	for (const member of unmapped) {
		if (configured.length > 0 && mapped.has(member.type.key)) continue;
		const list = lists.get(member.type.key) ?? { type: member.type.key, name: '', akter: [] };
		list.name = member.type.name;
		list.akter.push(member);
		lists.set(member.type.key, list);
	}
	return { columns: shown, lists: [...lists.values()].filter((l) => l.akter.length > 0) };
}

interface Shown {
	fields: readonly Value[];
	type: {
		fields: readonly { key: string; name: string }[];
		presentation: { card: readonly string[]; list: readonly string[] };
	};
}

/** The fields a presentation names for cards or lists, by name, with their
 * value as text, or `''` when the akt holds none. */
export function shownFields(akt: Shown, where: 'card' | 'list'): { name: string; text: string }[] {
	return akt.type.presentation[where].flatMap((key) => {
		const def = akt.type.fields.find((f) => f.key === key);
		if (!def) return [];
		const value = akt.fields.find((v) => 'field' in v && v.field === key);
		return [{ name: def.name, text: value ? textOf(value) : '' }];
	});
}

/** The place to move a member to so that it lands at `to` among `numbers`
 * (its list or column, in order), or null if it is already there. */
export function placeFor(
	numbers: readonly string[],
	number: string,
	to: number
): { before: string } | { after: string } | null {
	const from = numbers.indexOf(number);
	if (from < 0) return null;
	const others = numbers.filter((n) => n !== number);
	const at = Math.max(0, Math.min(to > from ? to - 1 : to, others.length));
	if (at === from) return null;
	return at < others.length ? { before: others[at] } : { after: others[others.length - 1] };
}

export type SortBy = 'rank' | 'updated';

/** Akter in rank order (as boards place them) or most recently changed
 * first. */
export function sortAkter<A extends { rank: string; updatedAt: string }>(
	akter: readonly A[],
	by: SortBy
): A[] {
	return [...akter].sort((a, b) =>
		by === 'rank'
			? a.rank < b.rank
				? -1
				: a.rank > b.rank
					? 1
					: 0
			: b.updatedAt.localeCompare(a.updatedAt)
	);
}
