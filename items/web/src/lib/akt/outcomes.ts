/** Outcomes as the app shows them: what verdicts, or what a section holds,
 * add up to (`akt.outcome`), by the same rules as `kuria-engine`'s. The
 * server derives an akt's own; a section's is derived here, from its
 * members', since sections nest deeper than a query can ask. */
import type { Outcome } from '#lib/graphql/generated/graphql.ts';

export type { Outcome };

/** What `outcomes` add up to: `FAIL` if any fails; otherwise `PENDING` if
 * any is pending, or there are none; otherwise `SKIP` if all skip;
 * otherwise `PASS`. */
// r[impl akt.outcome.section]
export function ofAll(outcomes: readonly Outcome[]): Outcome {
	if (outcomes.includes('FAIL')) return 'FAIL';
	if (outcomes.length === 0 || outcomes.includes('PENDING')) return 'PENDING';
	if (outcomes.every((o) => o === 'SKIP')) return 'SKIP';
	return 'PASS';
}

/** An outcome's badge tone and its word. */
export function shownOutcome(outcome: Outcome): {
	tone: 'NEUTRAL' | 'SUCCESS' | 'WARNING' | 'DANGER';
	text: string;
} {
	switch (outcome) {
		case 'PASS':
			return { tone: 'SUCCESS', text: 'Passed' };
		case 'FAIL':
			return { tone: 'DANGER', text: 'Failed' };
		case 'SKIP':
			return { tone: 'WARNING', text: 'Skipped' };
		default:
			return { tone: 'NEUTRAL', text: 'Pending' };
	}
}

/** How many of a container's members with outcomes have passed, of how
 * many: what it is ready for (`app.header`). None if it holds none. */
// r[impl app.header]
export function readiness(
	members: readonly { outcome?: Outcome | null }[]
): { passed: number; of: number } | null {
	const judged = members.filter((m) => m.outcome);
	if (judged.length === 0) return null;
	return { passed: judged.filter((m) => m.outcome === 'PASS').length, of: judged.length };
}
