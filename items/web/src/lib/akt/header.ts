/** What an akt's page shows first: its title, and its next actions. */
// r[impl app.header]

/** The heading an akt is shown by: its title, else its number. */
export function headingOf(akt: { number: string; title?: string | null }): string {
	return akt.title?.trim() || akt.number;
}

/** The transitions as actions: the first is the primary one. */
export function actionsOf<T>(transitions: readonly T[]): { primary: T | undefined; others: T[] } {
	const [primary, ...others] = transitions;
	return { primary, others };
}
