/** The home page: the akter the viewer can read that changed most
 * recently, and a card per akt type with its count and "New". */
// r[impl app.home]
// r[impl app.words]

/** How many akter an akt type has, in its own words: "1 test case",
 * "3 test cases", by its presentation's plural, else its name and an
 * `s` (PLAN-025-D01). */
export function counted(
	count: number | null | undefined,
	type: { name: string; presentation?: { plural?: string | null } | null }
): string {
	const one = type.name.toLowerCase();
	const many = (type.presentation?.plural ?? `${type.name}s`).toLowerCase();
	return count === 1 ? `1 ${one}` : `${count ?? 0} ${many}`;
}
