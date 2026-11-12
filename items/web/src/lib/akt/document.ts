/** What the app reads from an akt type's document beyond the fields,
 * states and transitions GraphQL lists: what it contains, and which fields
 * are drafted. */

interface Document {
	contains?: { type: string }[];
	fields?: { key: string; draft?: unknown }[];
}

function documentOf(document: unknown): Document {
	return typeof document === 'object' && document !== null ? (document as Document) : {};
}

/** The akt types an akt of this type contains, by key. */
export function containedTypes(document: unknown): string[] {
	return (documentOf(document).contains ?? []).map((c) => c.type);
}

/** The keys of the fields that have a draft recipe. */
export function draftedFields(document: unknown): string[] {
	return (documentOf(document).fields ?? []).filter((f) => f.draft).map((f) => f.key);
}
