/** What the app reads from an akt type: beyond the fields, states and
 * transitions GraphQL lists, what it contains and which fields are drafted
 * from its document; and which transitions an akt may take. Nothing here
 * knows any akt type. */
// r[impl app.akt-types]

interface Document {
	contains?: { type: string }[];
	fields?: { key: string; draft?: unknown }[];
	links?: LinkKind[];
}

/** A kind of link an akt type declares (`link.kinds`). */
export interface LinkKind {
	key: string;
	name: string;
	inverse: string;
	to: string[];
}

function documentOf(document: unknown): Document {
	return typeof document === 'object' && document !== null ? (document as Document) : {};
}

/** The akt types an akt of this type contains, by key. */
export function containedTypes(document: unknown): string[] {
	return (documentOf(document).contains ?? []).map((c) => c.type);
}

/** The kinds of link an akt of this type may have to others. */
// r[impl app.links]
export function linkKinds(document: unknown): LinkKind[] {
	return documentOf(document).links ?? [];
}

/** The keys of the fields that have a draft recipe. */
export function draftedFields(document: unknown): string[] {
	return (documentOf(document).fields ?? []).filter((f) => f.draft).map((f) => f.key);
}

interface TransitionShape {
	from: readonly string[];
	requires: readonly string[];
}

/** The transitions an akt in `state` may take: none in a final state. */
// r[impl app.transitions]
export function availableTransitions<T extends TransitionShape>(
	transitions: readonly T[],
	state: { key: string; final: boolean }
): T[] {
	return state.final ? [] : transitions.filter((t) => t.from.includes(state.key));
}

/** The fields a transition asks for, in the akt type's order. */
export function requiredFields<F extends { key: string }>(
	fields: readonly F[],
	transition: TransitionShape
): F[] {
	return fields.filter((f) => transition.requires.includes(f.key));
}
