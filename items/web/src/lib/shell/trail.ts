/**
 * The trail of akter a person reached by following references
 * (`app.trail`), kept in each history entry's state, so that going back and
 * forward shows each page with the trail it had.
 */
// r[impl app.trail]
import type { Crumb } from './crumbs.svelte';

/** An akt on the trail: where it was read, and how it is named. */
export interface Step {
	number: string;
	title: string | null;
	type: { key: string; name: string };
	/** Its page, at the version it was read at. */
	href: string;
}

/** The steps before `number`: the trail cut back to it if it is already
 * on it. */
export function before(trail: readonly Step[], number: string): Step[] {
	const at = trail.findIndex((s) => s.number === number);
	return at >= 0 ? trail.slice(0, at) : [...trail];
}

/** The top bar's crumbs for `here`, reached along `trail`: the first akt's
 * type, then each akt, its title under its number, an earlier one a link
 * back to it with the trail it had. */
export function crumbsOf(
	trail: readonly Step[],
	here: Omit<Step, 'href'>,
	typeHref: (key: string) => string
): Crumb[] {
	const earlier = before(trail, here.number);
	const first = earlier[0]?.type ?? here.type;
	return [
		{ label: first.name, href: typeHref(first.key) },
		...earlier.map((step, i) => ({
			label: step.number,
			detail: step.title ?? undefined,
			href: step.href,
			state: { trail: earlier.slice(0, i) }
		})),
		{ label: here.number, detail: here.title ?? undefined }
	];
}
