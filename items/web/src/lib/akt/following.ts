/**
 * Following a reference in a shown `markdown` field onto the trail
 * (`app.trail`): the page of an akt says what its trail is, and a shown
 * field's reference, followed, goes on from it.
 */
// r[impl app.trail]
import { getContext, setContext } from 'svelte';
import { goto } from '$app/navigation';
import type { Step } from '#lib/shell/trail.ts';

const KEY = Symbol('following');

/** Sets the trail a reference followed from this page goes on from: the
 * steps before this akt, and this one. */
export function setFollowing(trail: () => Step[] | null): void {
	setContext(KEY, trail);
}

/** What a click on a link to `href` does, from a component of the page:
 * goes there in place, onto the page's trail, if the page has one. A new
 * tab or window gets none. */
export function following(): (event: MouseEvent, href: string) => void {
	const trail = getContext<(() => Step[] | null) | undefined>(KEY);
	return (event, href) => {
		const steps = trail?.();
		if (!steps || event.defaultPrevented || event.button !== 0) return;
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
		event.preventDefault();
		// Already resolved: the app's own link.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(href, { state: { trail: steps } });
	};
}
