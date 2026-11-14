/** Verdicts in the app (`app.verdicts`): a member's assignees, each one's
 * verdict, and whether the viewer is one of them, from its fields; and the
 * input that gives or takes back one's own. Nothing here knows any akt
 * type. */
// r[impl app.verdicts]
import type { Verdict } from '#lib/graphql/generated/graphql.ts';
import { nameOf, type Value } from './values';

export interface Assignee {
	/** A user's subject or a contact's id. */
	id: string;
	name: string;
	verdict: Verdict | null;
}

export interface Judged {
	/** The verdict field's key. */
	field: string;
	assignees: Assignee[];
	/** The viewer's verdict if they are an assignee: `null` if they
	 * haven't given one; `undefined` if they aren't one. */
	mine: Verdict | null | undefined;
}

interface Judgeable {
	fields: readonly Value[];
	type: { fields: readonly { key: string; kind?: string; assignees?: string | null }[] };
}

/** `akt`'s assignees and their verdicts, if its type has a verdict field;
 * `me` is the viewer's subject. */
export function judgedOf(akt: Judgeable, me: string | undefined): Judged | null {
	const def = akt.type.fields.find((f) => f.kind === 'VERDICT');
	if (!def) return null;
	const people = akt.fields.find((v) => v.__typename === 'UsersValue' && v.field === def.assignees);
	const given = akt.fields.find((v) => v.__typename === 'VerdictValue' && v.field === def.key);
	const verdicts = given?.__typename === 'VerdictValue' ? given.verdicts : [];
	const assignees: Assignee[] =
		people?.__typename === 'UsersValue'
			? people.users.map((someone) => ({
					id: someone.id,
					name: nameOf(someone),
					verdict:
						verdicts.find(
							(v) => v.assignee.__typename === someone.__typename && v.assignee.id === someone.id
						)?.verdict ?? null
				}))
			: [];
	const own = assignees.find((a) => a.id === me);
	return { field: def.key, assignees, mine: own ? own.verdict : undefined };
}

/** The input that gives `verdict` as one's own, or takes it back. */
export function verdictInput(verdict: Verdict | null): unknown {
	return verdict ? verdict.toLowerCase() : { clear: true };
}

export const VERDICTS: readonly { verdict: Verdict; label: string }[] = [
	{ verdict: 'PASS', label: 'Pass' },
	{ verdict: 'FAIL', label: 'Fail' },
	{ verdict: 'SKIP', label: 'Skip' }
];
