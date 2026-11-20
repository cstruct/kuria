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
	/** Where the trail began, for its first step: the containers it was
	 * shown in, outermost first, if any. */
	places?: readonly Place[];
}

/** A container an akt is in, as the trail shows it. */
export interface Place {
	number: string;
	title: string | null;
	href: string;
}

/** The steps before `number`: the trail cut back to it if it is already
 * on it. */
export function before(trail: readonly Step[], number: string): Step[] {
	const at = trail.findIndex((s) => s.number === number);
	return at >= 0 ? trail.slice(0, at) : [...trail];
}

/** An akt as a crumb: by its title, its number under it, else by its
 * number. */
function named(akt: { number: string; title: string | null }): Pick<Crumb, 'label' | 'detail'> {
	return akt.title
		? { label: akt.title, detail: akt.number }
		: { label: akt.number, detail: undefined };
}

/** The top bar's crumbs for `here`. Reached along `trail`: the first akt's
 * type, then each akt, an earlier one a link back to it with the trail it
 * had. Otherwise where it is: `places`, the containers it is in, outermost
 * first, else its type. */
export function crumbsOf(
	trail: readonly Step[],
	here: Omit<Step, 'href'>,
	typeHref: (key: string) => string,
	places: readonly Place[] = []
): Crumb[] {
	const earlier = before(trail, here.number);
	// Begun where the first akt was: its containers, else its type.
	const start = earlier.length > 0 ? (earlier[0].places ?? []) : places;
	const first = earlier[0]?.type ?? here.type;
	return [
		...(start.length > 0
			? start.map((p) => ({ ...named(p), href: p.href }))
			: [{ label: first.name, href: typeHref(first.key) }]),
		...earlier.map((step, i) => ({
			...named(step),
			href: step.href,
			state: { trail: earlier.slice(0, i) }
		})),
		named(here)
	];
}
