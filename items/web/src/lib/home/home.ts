/** The home page: the akter the viewer can read that changed most
 * recently, and a card per akt type with its count and "New". */
// r[impl app.home]

/** How many akter an akt type has, in words: "1 akt", "3 akter". */
export function counted(count: number | null | undefined): string {
	return count === 1 ? '1 akt' : `${count ?? 0} akter`;
}
