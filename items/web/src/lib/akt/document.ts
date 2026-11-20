/** What the app reads from an akt type: beyond the fields, states and
 * transitions GraphQL lists, what it contains and which fields are drafted
 * from its document; and which transitions an akt may take. Nothing here
 * knows any akt type. */
// r[impl app.akt-types]
import { shownOutcome, type Outcome } from './outcomes';

interface Document {
	contains?: { type: string }[];
	fields?: { key: string; draft?: unknown; references?: string }[];
	links?: LinkKind[];
	versions?: { file?: string };
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

/** The field each version is released with a PDF in, if this type's
 * versions carry one (`version.file`): only a release sets it. */
// r[impl app.external]
export function releasedFile(document: unknown): string | null {
	return documentOf(document).versions?.file ?? null;
}

/** The kinds of link an akt of this type may have to others. */
// r[impl app.links]
export function linkKinds(document: unknown): LinkKind[] {
	const doc = documentOf(document);
	// A kind a markdown field keeps its references as is changed in the
	// text, never by hand (`link.referenced`).
	const kept = new Set((doc.fields ?? []).flatMap((f) => (f.references ? [f.references] : [])));
	return (doc.links ?? []).filter((kind) => !kept.has(kind.key));
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

/** Why `transition` is unavailable to an akt with `outcome`, if it is: it
 * names the outcomes it allows, and the akt's isn't one of them. */
// r[impl app.transitions]
export function outcomeRefusal(
	transition: { outcomes?: readonly string[] | null },
	outcome: string | null | undefined
): string | null {
	const allowed = transition.outcomes ?? [];
	if (allowed.length === 0 || (outcome && allowed.includes(outcome))) return null;
	const named = (o: string) => shownOutcome(o as Outcome).text.toLowerCase();
	const words = allowed.map(named);
	const list = words.length > 1 ? `${words.slice(0, -1).join(', ')} or ${words.at(-1)}` : words[0];
	return `Needs the result to be ${list}; it is ${named(outcome ?? 'PENDING')}`;
}

/** What a transition the akt's state allows waits for
 * (`akt.transition.waits`): the akter not yet final, by number, and the
 * links it lacks, by name. */
export interface Waiting {
	transition: string;
	open: readonly string[];
	lacking: readonly string[];
}

/** What `transition` waits for, of the akt's `waits`, if anything. */
// r[impl app.transitions]
export function waitingFor(
	transition: { key: string },
	waits: readonly Waiting[] | null | undefined
): Waiting | null {
	const wait = waits?.find((w) => w.transition === transition.key);
	return wait && (wait.open.length > 0 || wait.lacking.length > 0) ? wait : null;
}

/** `words` as "a, b and c". */
export function allOf(words: readonly string[]): string {
	return words.length > 1
		? `${words.slice(0, -1).join(', ')} and ${words.at(-1)}`
		: (words[0] ?? '');
}

/** The links a waiting transition lacks, in words: "a Verified by link". */
export function lackingText(lacking: readonly string[]): string {
	return allOf(lacking.map((name) => `a ${name} link`));
}

/** Whether the field `key` of the type `document` is given in the state
 * `state`: in every state, unless it names its `states`
 * (`akt.field.states`). */
// r[impl akt.field.states]
export function givenIn(document: unknown, key: string, state: string): boolean {
	const fields = (document as { fields?: { key?: string; states?: string[] }[] } | null)?.fields;
	const states = fields?.find((f) => f.key === key)?.states;
	return !Array.isArray(states) || states.length === 0 || states.includes(state);
}
