/** What an akt's page shows first: its title, and its next actions. */
// r[impl app.header]

/** The heading an akt is shown by: its title, else its number. */
export function headingOf(akt: { number: string; title?: string | null }): string {
	return akt.title?.trim() || akt.number;
}

/** The action an akt's page offers first. */
export type Primary<T> =
	| { kind: 'transition'; transition: T }
	| { kind: 'copy' }
	| { kind: 'create'; name: string; href: string };

/** What a type document's presentation names as the action first offered
 * in `state`, if anything: a transition's key, or `copy`. */
function namedPrimary(document: unknown, state: string): string | undefined {
	const presentation = (document as { presentation?: { primary?: Record<string, unknown> } } | null)
		?.presentation;
	const named = presentation?.primary?.[state];
	return typeof named === 'string' ? named : undefined;
}

/** The transitions as actions, and the one offered first: the one the
 * type's presentation names for `state` (`primary`), if it is offered,
 * else creating the first of `creatable`, the akter a container's board
 * offers to create, else none. No transition is primary otherwise: one
 * that ends or decides an akt is never offered first by default. */
export function actionsOf<T extends { key: string }>(
	transitions: readonly T[],
	options: {
		document?: unknown;
		state?: string;
		/** Whether the viewer may make a copy. */
		copyable?: boolean;
		creatable?: readonly { name: string; href: string }[];
	} = {}
): { primary: Primary<T> | undefined; others: T[] } {
	const named = options.state ? namedPrimary(options.document, options.state) : undefined;
	const transition = named ? transitions.find((t) => t.key === named) : undefined;
	if (transition) {
		return {
			primary: { kind: 'transition', transition },
			others: transitions.filter((t) => t !== transition)
		};
	}
	if (named === 'copy' && options.copyable) {
		return { primary: { kind: 'copy' }, others: [...transitions] };
	}
	const create = options.creatable?.[0];
	return {
		primary: create ? { kind: 'create', ...create } : undefined,
		others: [...transitions]
	};
}

/** What confirming a transition says: for one to a final state, that it
 * ends there, unless a transition `reopens` it from there
 * (`akt.transition.reopens`). */
export function confirmationOf(
	heading: string,
	to: { key?: string; name: string; final?: boolean | null } | undefined,
	transitions: readonly { from: readonly string[]; reopens?: boolean | null }[] = []
): string | undefined {
	if (!to?.final) return undefined;
	const reopens = transitions.some((t) => t.reopens && to.key && t.from.includes(to.key));
	return reopens
		? `${heading} moves to ${to.name}, where it stays unless it is reopened.`
		: `${heading} moves to ${to.name}, where it stays.`;
}
