/** Finding an akt to pick, by number or title, instead of typing a number
 * exactly. */
// r[impl app.pickers]
import type { Client } from '@urql/core';
import type { Option } from '#lib/ui/Combobox.svelte';
import { SearchAkter } from '#lib/graphql/operations.ts';

export interface Found {
	number: string;
	title?: string | null;
	type: { key: string; name: string };
}

/** The options a picker shows for `found`, leaving out `except` and, if
 * given, akt types other than `types`. */
export function optionsOf(
	found: readonly Found[],
	except: readonly string[],
	types?: readonly string[]
): Option[] {
	return found
		.filter((a) => !except.includes(a.number) && (!types || types.includes(a.type.key)))
		.map((a) => ({
			value: a.number,
			label: a.title?.trim() || a.number,
			hint: `${a.number} · ${a.type.name}`
		}));
}

/** Searches the server for akter to pick. */
export async function pickable(
	client: Client,
	text: string,
	except: readonly string[],
	types?: readonly string[]
): Promise<Option[]> {
	const result = await client
		.query(SearchAkter, { text }, { requestPolicy: 'network-only' })
		.toPromise();
	return optionsOf(result.data?.akter.nodes ?? [], except, types);
}
