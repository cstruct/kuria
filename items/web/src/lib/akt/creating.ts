/** Creating akter from a container's board: which akt types each column
 * offers, and the parent boards (`board.parent`) a new akt goes into
 * beside the container. Nothing here knows any akt type. */
// r[impl app.board-create]
// r[impl app.akt-types]
import { isEmpty, type Draft, type FieldDef } from './values';

interface Document {
	contains?: { type?: string }[];
	board?: { parent?: string; columns?: { key?: string; states?: Record<string, unknown> }[] };
}

function documentOf(document: unknown): Document {
	return typeof document === 'object' && document !== null ? (document as Document) : {};
}

/** An akt type as the app reads it: its key, name, states and document. */
export interface TypeShape {
	key: string;
	name: string;
	states: readonly { key: string; initial: boolean }[];
	document: unknown;
}

/** The akt types, by key, that akter of the type `document` defines may
 * contain. */
function containsOf(document: unknown): string[] {
	return (documentOf(document).contains ?? []).flatMap((c) => (c.type ? [c.type] : []));
}

/** The akt types the container of type `document` contains whose initial
 * state the board column `column` names for them, in the order contained.
 * `types` are the tenant's akt types; a contained type it lacks isn't
 * offered. */
export function creatableIn<T extends TypeShape>(
	document: unknown,
	column: string,
	types: readonly T[]
): T[] {
	const def = (documentOf(document).board?.columns ?? []).find((c) => c.key === column);
	if (!def) return [];
	return containsOf(document).flatMap((key) => {
		const type = types.find((t) => t.key === key);
		const initial = type?.states.find((s) => s.initial)?.key;
		const states = def.states?.[key];
		return type && initial && Array.isArray(states) && states.includes(initial) ? [type] : [];
	});
}

interface LinkShape {
	kind: string;
	outgoing: boolean;
	akt: { number: string; type: { key: string } };
}

/** The numbers of the container's parent boards, the akter it links to by
 * the kind its type document names as `board.parent`, that can contain an
 * akt of the type `key`, by their akt type's latest version in `types`. */
export function parentBoards(
	document: unknown,
	links: readonly LinkShape[],
	key: string,
	types: readonly Pick<TypeShape, 'key' | 'document'>[]
): string[] {
	const kind = documentOf(document).board?.parent;
	if (!kind) return [];
	const numbers = links
		.filter((l) => l.outgoing && l.kind === kind)
		.filter((l) => containsOf(types.find((t) => t.key === l.akt.type.key)?.document).includes(key))
		.map((l) => l.akt.number);
	return [...new Set(numbers)];
}

/** The query of the new-akt page that creates an akt in `container`, its
 * parent, then in each of `parents`. */
export function insideQuery(container: string, parents: readonly string[]): string {
	return [container, ...parents.filter((p) => p !== container)]
		.map((n) => `in=${encodeURIComponent(n)}`)
		.join('&');
}

/** What the create page says on each required field left empty, by field
 * key, so that nothing is sent until they are given (`app.create`). Fields
 * without a draft aren't asked for there, and aren't named. */
// r[impl app.create]
export function missingRequired(
	fields: readonly Pick<FieldDef, 'key' | 'name' | 'required'>[],
	drafts: Readonly<Record<string, Draft | undefined>>
): Record<string, string> {
	return Object.fromEntries(
		fields.flatMap((f) => {
			const draft = drafts[f.key];
			return f.required && draft && isEmpty(draft) ? [[f.key, `${f.name} is required.`]] : [];
		})
	);
}
