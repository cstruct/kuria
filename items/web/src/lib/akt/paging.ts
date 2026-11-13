/** Loading a list of akter a page at a time (`akt.page`): the pages
 * loaded so far, as one list. */
// r[impl app.filters]

/** The first page and those loaded after it, each akt once: the first
 * page is refetched when akter change, so an akt can move from a later
 * page into it. */
export function merged<A extends { id: string }>(first: readonly A[], more: readonly A[]): A[] {
	const seen = new Set(first.map((a) => a.id));
	return [...first, ...more.filter((a) => !seen.has(a.id))];
}
