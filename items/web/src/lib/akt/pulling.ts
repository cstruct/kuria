/** Pulling members from a container's parent board (`board.parent`) into
 * it, several at once (`app.pull`): what the parent's board offers, column
 * by column, and which are here already or in another container like this
 * one. Nothing here knows any akt type. */
// r[impl app.pull]

interface ColumnShape {
	key: string;
	name: string;
	akter: readonly {
		number: string;
		title?: string | null;
		state: { name: string; final?: boolean | null };
		type: { key: string };
		containers: readonly { number: string; type: { key: string } }[];
	}[];
}

/** One akt the parent's board offers. */
export interface Offered {
	number: string;
	title: string;
	status: string;
	/** Here already: not offered again. */
	here: boolean;
	/** Another container of this one's type it is in, such as another
	 * candidate, if any. */
	elsewhere: string | null;
	/** Whether it is finished, in a final state. */
	finished: boolean;
}

/** The parent's board as offered to the container `here`, of the type
 * `type`, which holds the types `contained`: each column with the akter of
 * those types in it, and none without. */
export function offeredFrom(
	columns: readonly ColumnShape[],
	here: string,
	type: string,
	contained: readonly string[]
): { key: string; name: string; akter: Offered[] }[] {
	return columns
		.map((column) => ({
			key: column.key,
			name: column.name,
			akter: column.akter
				.filter((m) => contained.includes(m.type.key))
				.map((m) => ({
					number: m.number,
					title: m.title ?? m.number,
					status: m.state.name,
					here: m.containers.some((c) => c.number === here),
					elsewhere:
						m.containers.find((c) => c.type.key === type && c.number !== here)?.number ?? null,
					finished: Boolean(m.state.final)
				}))
		}))
		.filter((column) => column.akter.length > 0);
}

/** The number of the container's parent board, if it links to one: the
 * first akt it links to by the kind its type document names as
 * `board.parent`. */
export function parentOf(
	document: unknown,
	links: readonly {
		kind: string;
		outgoing: boolean;
		akt: { number: string; title?: string | null };
	}[]
): { number: string; title: string } | null {
	const kind = (document as { board?: { parent?: string } } | null)?.board?.parent;
	const link = kind ? links.find((l) => l.outgoing && l.kind === kind) : undefined;
	return link ? { number: link.akt.number, title: link.akt.title ?? link.akt.number } : null;
}
