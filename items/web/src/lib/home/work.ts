/** "My work" on Home: the akter whose user fields or checkboxes name the
 * viewer, found by one filtered list per such field (`akt.filter`), and
 * shown by akt type. */
// r[impl app.my-work]

interface TypeShape {
	key: string;
	fields: readonly { key: string; kind: string; multiple?: boolean }[];
}

/** The lists to ask for: each user field naming one person, and each
 * checkbox, of each type. A field naming several can't be filtered by
 * (`akt.filter`). */
export function workQueries(types: readonly TypeShape[]): { type: string; field: string }[] {
	return types.flatMap((t) =>
		t.fields
			.filter((f) => (f.kind === 'USER' && !f.multiple) || f.kind === 'CHECKBOX')
			.map((f) => ({ type: t.key, field: f.key }))
	);
}

/** The lists' akter by type, in the order the types come, each akt once,
 * the most recently changed first. Types with none are left out. */
export function workByType<A extends { id: string; updatedAt: string }>(
	types: readonly string[],
	answers: readonly { type: string; nodes: readonly A[] }[]
): { type: string; akter: A[] }[] {
	return types.flatMap((type) => {
		const seen = new Map<string, A>();
		for (const answer of answers.filter((a) => a.type === type)) {
			for (const akt of answer.nodes) seen.set(akt.id, akt);
		}
		const akter = [...seen.values()].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
		return akter.length > 0 ? [{ type, akter }] : [];
	});
}
