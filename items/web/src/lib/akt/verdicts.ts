/** Verdicts in the app (`app.verdicts`): a member's assignees, each one's
 * verdict and its description, and whether the viewer is one of them, from
 * its fields; and the input that reports or deletes one's own. Nothing
 * here knows any akt type. */
// r[impl app.verdicts]
import type { Verdict } from '#lib/graphql/generated/graphql.ts';
import { nameOf, type Value } from './values';

/** A verdict as it is reported: its status and what was seen, empty if
 * nothing was said. */
export interface Report {
	verdict: Verdict;
	description: string;
}

export interface Assignee {
	/** A user's subject or a contact's id. */
	id: string;
	name: string;
	verdict: Verdict | null;
	description: string | null;
}

export interface Judged {
	/** The verdict field's key. */
	field: string;
	assignees: Assignee[];
	/** The viewer's verdict if they are an assignee: `null` if they
	 * haven't reported one; `undefined` if they aren't one. */
	mine: Report | null | undefined;
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
			? people.users.map((someone) => {
					const theirs = verdicts.find(
						(v) => v.assignee.__typename === someone.__typename && v.assignee.id === someone.id
					);
					return {
						id: someone.id,
						name: nameOf(someone),
						verdict: theirs?.verdict ?? null,
						description: theirs?.description ?? null
					};
				})
			: [];
	const own = assignees.find((a) => a.id === me);
	const mine = own
		? own.verdict
			? { verdict: own.verdict, description: own.description ?? '' }
			: null
		: undefined;
	return { field: def.key, assignees, mine };
}

/** Whether `verdict` must say why (`akt.field.verdict.description`). */
export const needsDescription = (verdict: Verdict): boolean => verdict !== 'PASS';

/** Why `report` can't be saved, if it can't: nothing chosen, or a fail or
 * a skip that doesn't say why. */
export function reportError(report: {
	verdict: Verdict | null;
	description: string;
}): string | null {
	if (!report.verdict) return 'Choose a status.';
	if (needsDescription(report.verdict) && report.description.trim() === '')
		return `Say why it ${report.verdict === 'FAIL' ? 'failed' : 'was skipped'}.`;
	return null;
}

/** The input that reports `report` as one's own verdict, or deletes it. */
export function verdictInput(report: Report | null): unknown {
	if (!report) return { clear: true };
	const description = report.description.trim();
	const verdict = report.verdict.toLowerCase();
	return description ? { verdict, description } : verdict;
}

export const VERDICTS: readonly { verdict: Verdict; label: string }[] = [
	{ verdict: 'PASS', label: 'Pass' },
	{ verdict: 'FAIL', label: 'Fail' },
	{ verdict: 'SKIP', label: 'Skip' }
];

/** Whether verdicts on the members of a container of the type `type` may
 * be given while it is in `state`: while a transition that names the
 * outcomes it allows may leave it, such as a run's Finish, or always if
 * none names any (`app.verdicts`). A template or a run not yet started
 * takes none. */
export function judgingOpen(
	type: {
		transitions: readonly { from: readonly string[]; outcomes?: readonly string[] | null }[];
	},
	state: string
): boolean {
	const judging = type.transitions.filter((t) => (t.outcomes ?? []).length > 0);
	return judging.length === 0 || judging.some((t) => t.from.includes(state));
}
