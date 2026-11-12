/** The trail the top bar shows: where the page is, set by the page. */

export interface Crumb {
	label: string;
	href?: string;
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
