/** What a refused change leaves the akt page with. */
import type { CombinedError } from '@urql/core';
import { refusalOf, type Refusal } from './errors';
import { inputOf, type Draft } from './values';

export interface Settled {
	refusal: Refusal | null;
	/** What the editors are compared with from now on, if it changes. */
	base?: Record<string, Draft>;
	/** What the editors hold from now on, if it changes. */
	drafts?: Record<string, Draft>;
}

/**
 * After a change: on success, the editors start again from the akt as it
 * is; on `STALE`, they keep what the person entered, compared with the akt
 * as it now is, so applying it again sends their input; on any other
 * refusal, nothing moves.
 */
// r[impl app.stale]
export function settle(
	error: CombinedError | undefined,
	fresh: Record<string, Draft> | undefined,
	fieldNames: Record<string, string>
): Settled {
	if (!error)
		return fresh
			? { refusal: null, base: fresh, drafts: structuredClone(fresh) }
			: { refusal: null };
	const refusal = refusalOf(error, fieldNames);
	if (refusal.code === 'STALE' && fresh) return { refusal, base: fresh };
	return { refusal };
}

/** The input that edits one field: its new value, if the editor changed
 * it, and nothing else. */
// r[impl app.inline-edit]
export function oneField(
	key: string,
	before: Draft | undefined,
	after: Draft | undefined
): Record<string, unknown> {
	if (!after || JSON.stringify(before) === JSON.stringify(after)) return {};
	return { [key]: inputOf(after) };
}
