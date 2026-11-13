/** Linking contacts by hand: which users a contact may be linked to. */
// r[impl contact.link-by-hand]
import type { Option } from '#lib/ui/Combobox.svelte';

export interface UserShape {
	id: string;
	name: string;
}

/** The users whose name contains every word of `text`, ignoring case, as
 * a picker's options, leaving out those other contacts are linked to
 * already. */
export function userOptions(
	users: readonly UserShape[],
	text: string,
	linked: ReadonlySet<string> = new Set()
): Option[] {
	const words = text.toLowerCase().split(/\s+/).filter(Boolean);
	return users
		.filter((u) => !linked.has(u.id))
		.filter((u) => words.every((w) => u.name.toLowerCase().includes(w)))
		.map((u) => ({ value: u.id, label: u.name }));
}
