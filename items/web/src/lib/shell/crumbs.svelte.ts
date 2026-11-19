/** The trail the top bar shows: where the page is, set by the page. */

export interface Crumb {
	label: string;
	/** Shown under the label, smaller, such as an akt's title. */
	detail?: string;
	href?: string;
	/** The history state going to `href` gives its page (`app.trail`). */
	state?: App.PageState;
}

export const crumbs = $state<{ trail: Crumb[] }>({ trail: [] });

/** Sets the top bar's trail for as long as the calling component lives. */
export function setCrumbs(trail: () => Crumb[]): void {
	$effect(() => {
		crumbs.trail = trail();
		return () => {
			crumbs.trail = [];
		};
	});
}
