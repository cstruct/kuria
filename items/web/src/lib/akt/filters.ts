/** Filtering a list of akter by field values (`akt.filter`), kept in the
 * page's address as `where.<field>=<value>`. Nothing here knows any akt
 * type. */
// r[impl app.filters]

export interface FilterField {
	key: string;
	name: string;
	kind: string;
	multiline?: boolean | null;
	options?: readonly { key: string; name: string }[] | null;
}

const PREFIX = 'where.';

/** Whether a list can be filtered by `field`, as the server allows. */
export function filterable(field: FilterField): boolean {
	switch (field.kind) {
		case 'CHOICE':
		case 'CHOICES':
		case 'USER':
		case 'CHECKBOX':
		case 'DATE':
			return true;
		case 'TEXT':
			return !field.multiline;
		default:
			return false;
	}
}

/** The conditions an address holds, for the fields that can be filtered. */
export function conditionsOf(
	params: { get(name: string): string | null },
	fields: readonly FilterField[]
): { field: string; equals: string }[] {
	return fields.filter(filterable).flatMap((f) => {
		const equals = params.get(`${PREFIX}${f.key}`);
		return equals ? [{ field: f.key, equals }] : [];
	});
}

/** The address's parameters with `field` set to `value`, or cleared. */
export function withCondition(
	params: { toString(): string },
	field: string,
	value: string
): URLSearchParams {
	const next = new URLSearchParams(params.toString());
	if (value) next.set(`${PREFIX}${field}`, value);
	else next.delete(`${PREFIX}${field}`);
	return next;
}

/** The outcomes a list filters by, as GraphQL names them. */
export const OUTCOMES = ['PASS', 'FAIL', 'SKIP', 'PENDING'] as const;
export type FilterOutcome = (typeof OUTCOMES)[number];

/** Whether a list of a type with `fields` filters by outcome: it has a
 * `verdict` field (`akt.filter.outcome`). */
export function filtersByOutcome(fields: readonly FilterField[]): boolean {
	return fields.some((f) => f.kind === 'VERDICT');
}

/** The outcome an address asks for as `outcome=pass`, if the list filters
 * by one. */
export function outcomeOf(
	params: { get(name: string): string | null },
	fields: readonly FilterField[]
): FilterOutcome | null {
	const asked = params.get('outcome')?.toUpperCase();
	return filtersByOutcome(fields) && OUTCOMES.some((o) => o === asked)
		? (asked as FilterOutcome)
		: null;
}
