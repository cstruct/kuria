/** A `sections` field in the app (`akt.field.sections`): its value as a
 * tree, read and given as the JSON `editAkt` takes, each section's outcome
 * from its members', and the outline editor's moves. Nothing here knows
 * any akt type. */
// r[impl app.sections]
import { ofAll, type Outcome } from './outcomes';

export interface Section {
	title: string;
	/** Markdown. */
	text: string;
	collapsible: boolean;
	items: Item[];
}

/** A member, by number, or a section. */
export type Item = { akt: string } | { section: Section };

/** Where an item is: its index in each list from the top. */
export type Path = readonly number[];

/** Reads a sections value as `SectionsValue.value` gives it. */
export function readSections(value: unknown): Item[] {
	if (!Array.isArray(value)) return [];
	return value.flatMap((item): Item[] => {
		if (typeof item === 'string') return [{ akt: item }];
		if (typeof item !== 'object' || item === null) return [];
		const s = item as Record<string, unknown>;
		return [
			{
				section: {
					title: typeof s.title === 'string' ? s.title : '',
					text: typeof s.text === 'string' ? s.text : '',
					collapsible: s.collapsible === true,
					items: readSections(s.items)
				}
			}
		];
	});
}

/** `items` as `editAkt` takes them: an empty title or text left out. */
export function inputOfSections(items: readonly Item[]): unknown[] {
	return items.map((item) => {
		if ('akt' in item) return item.akt;
		const { title, text, collapsible, items: inner } = item.section;
		return {
			...(title.trim() ? { title: title.trim() } : {}),
			...(text.trim() ? { text: text.trim() } : {}),
			...(collapsible ? { collapsible: true } : {}),
			items: inputOfSections(inner)
		};
	});
}

/** Every member `items` place, in order, nested ones too. */
export function numbersIn(items: readonly Item[]): string[] {
	return items.flatMap((item) => ('akt' in item ? [item.akt] : numbersIn(item.section.items)));
}

/** A section's outcome, from those of the members it places, as
 * `outcomeOf` gives them (`undefined` for one without outcomes), and of its
 * nested sections. */
// r[impl akt.outcome.section]
export function sectionOutcome(
	section: Section,
	outcomeOf: (number: string) => Outcome | null | undefined
): Outcome {
	return ofAll(
		section.items.flatMap((item) => {
			if ('section' in item) return [sectionOutcome(item.section, outcomeOf)];
			const outcome = outcomeOf(item.akt);
			return outcome ? [outcome] : [];
		})
	);
}

/** The list holding the item at `path`, within `items`, and its index. */
function listOf(items: Item[], path: Path): [Item[], number] {
	let list = items;
	for (const i of path.slice(0, -1)) {
		const item = list[i];
		if (!item || !('section' in item)) throw new Error(`no section at ${path.join('.')}`);
		list = item.section.items;
	}
	return [list, path[path.length - 1]];
}

/** A plain copy of `items`: through JSON, which reads Svelte's state
 * proxies as `structuredClone` can't. */
function copyOf(items: readonly Item[]): Item[] {
	return JSON.parse(JSON.stringify(items)) as Item[];
}

function edited(items: readonly Item[], change: (copy: Item[]) => void): Item[] {
	const copy = copyOf(items);
	change(copy);
	return copy;
}

/** `items` with the item at `path` moved one place up (`-1`) or down
 * (`1`) within its list. */
export function moveItem(items: readonly Item[], path: Path, by: -1 | 1): Item[] {
	return edited(items, (copy) => {
		const [list, i] = listOf(copy, path);
		const j = i + by;
		if (j < 0 || j >= list.length) return;
		[list[i], list[j]] = [list[j], list[i]];
	});
}

/** Whether the item at `path` can go into the section just before it. */
export function canIndent(items: readonly Item[], path: Path): boolean {
	const [list, i] = listOf(items as Item[], path);
	return i > 0 && 'section' in list[i - 1];
}

/** `items` with the item at `path` moved to the end of the section just
 * before it. */
export function indentItem(items: readonly Item[], path: Path): Item[] {
	if (!canIndent(items, path)) return [...items];
	return edited(items, (copy) => {
		const [list, i] = listOf(copy, path);
		const [item] = list.splice(i, 1);
		const before = list[i - 1];
		if ('section' in before) before.section.items.push(item);
	});
}

/** `items` with the item at `path` moved out of its section, to just
 * after it. */
export function outdentItem(items: readonly Item[], path: Path): Item[] {
	if (path.length < 2) return [...items];
	return edited(items, (copy) => {
		const [list, i] = listOf(copy, path);
		const [item] = list.splice(i, 1);
		const [outer, at] = listOf(copy, path.slice(0, -1));
		outer.splice(at + 1, 0, item);
	});
}

/** `items` without the item at `path`: a member taken out of the
 * sections, or a section whose items take its place. */
export function removeItem(items: readonly Item[], path: Path): Item[] {
	return edited(items, (copy) => {
		const [list, i] = listOf(copy, path);
		const [item] = list.splice(i, 1);
		if ('section' in item) list.splice(i, 0, ...item.section.items);
	});
}

/** `items` with `item` added at the end. */
export function addItem(items: readonly Item[], item: Item): Item[] {
	return [...copyOf(items), item];
}

/** `items` with the section at `path` changed by `change`. */
export function editSection(
	items: readonly Item[],
	path: Path,
	change: Partial<Omit<Section, 'items'>>
): Item[] {
	return edited(items, (copy) => {
		const [list, i] = listOf(copy, path);
		const item = list[i];
		if ('section' in item) Object.assign(item.section, change);
	});
}

/** What `copyAkt` keeps, given the members chosen of all of them: every
 * one (`null`) if all are chosen, else those chosen. */
// r[impl app.copy]
export function onlyOf(all: readonly string[], chosen: ReadonlySet<string>): string[] | null {
	return all.every((n) => chosen.has(n)) ? null : all.filter((n) => chosen.has(n));
}
