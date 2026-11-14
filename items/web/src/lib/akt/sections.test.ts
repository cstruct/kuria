import { describe, expect, it } from 'vitest';
import { ofAll, shownOutcome } from './outcomes';
import {
	addItem,
	canIndent,
	editSection,
	indentItem,
	inputOfSections,
	moveItem,
	numbersIn,
	onlyOf,
	outdentItem,
	readSections,
	removeItem,
	sectionOutcome,
	type Item
} from './sections';

const stored = [
	{
		title: 'Setup',
		collapsible: true,
		items: ['T1', { title: 'Network', text: 'Plug it in.', items: ['T2'] }]
	},
	{ items: ['T3'] },
	'N1'
];

describe('sections', () => {
	// r[verify app.sections]
	it('reads a value and gives it back as editAkt takes it', () => {
		const items = readSections(stored);
		expect(numbersIn(items)).toEqual(['T1', 'T2', 'T3', 'N1']);
		expect(inputOfSections(items)).toEqual(stored);
		expect(readSections(null)).toEqual([]);
	});

	// r[verify akt.outcome.section]
	it('adds up outcomes as the engine does', () => {
		expect(ofAll([])).toBe('PENDING');
		expect(ofAll(['PASS', 'SKIP'])).toBe('PASS');
		expect(ofAll(['SKIP', 'SKIP'])).toBe('SKIP');
		expect(ofAll(['PASS', 'PENDING'])).toBe('PENDING');
		expect(ofAll(['PENDING', 'FAIL'])).toBe('FAIL');
		const [setup] = readSections(stored) as [Extract<Item, { section: unknown }>];
		const outcomes: Record<string, 'PASS' | 'SKIP' | 'FAIL'> = { T1: 'PASS', T2: 'SKIP' };
		expect(sectionOutcome(setup.section, (n) => outcomes[n])).toBe('PASS');
		outcomes.T2 = 'FAIL';
		expect(sectionOutcome(setup.section, (n) => outcomes[n])).toBe('FAIL');
		// A member without outcomes doesn't count; an empty section is pending.
		expect(
			sectionOutcome(
				{ title: '', text: '', collapsible: false, items: [{ akt: 'N1' }] },
				() => undefined
			)
		).toBe('PENDING');
		expect(shownOutcome('FAIL')).toEqual({ tone: 'DANGER', text: 'Failed' });
	});

	// r[verify app.sections]
	it('moves, nests and removes items in the outline', () => {
		const items = readSections(stored);
		expect(numbersIn(moveItem(items, [2], -1))).toEqual(['T1', 'T2', 'N1', 'T3']);
		expect(moveItem(items, [0], -1)).toEqual(items);
		expect(canIndent(items, [2])).toBe(true);
		expect(canIndent(items, [0])).toBe(false);
		const indented = indentItem(items, [2]);
		expect(inputOfSections(indented)[1]).toEqual({ items: ['T3', 'N1'] });
		expect(inputOfSections(outdentItem(indented, [1, 1]))).toEqual(stored);
		// A section removed leaves its items in its place.
		expect(inputOfSections(removeItem(items, [0, 1]))[0]).toEqual({
			title: 'Setup',
			collapsible: true,
			items: ['T1', 'T2']
		});
		expect(numbersIn(removeItem(items, [2]))).toEqual(['T1', 'T2', 'T3']);
		const added = addItem(items, {
			section: { title: 'New', text: '', collapsible: false, items: [] }
		});
		expect(
			inputOfSections(editSection(added, [3], { title: ' Later ', collapsible: true }))[3]
		).toEqual({
			title: 'Later',
			collapsible: true,
			items: []
		});
		expect(numbersIn(items)).toEqual(['T1', 'T2', 'T3', 'N1']);
	});

	// r[verify app.copy]
	it('keeps every member, or those chosen, in a copy', () => {
		expect(onlyOf(['A', 'B'], new Set(['A', 'B']))).toBeNull();
		expect(onlyOf(['A', 'B'], new Set(['B']))).toEqual(['B']);
		expect(onlyOf(['A', 'B'], new Set())).toEqual([]);
	});
});

describe('sections in state', () => {
	it('edits items held in a proxy, as Svelte state holds them', () => {
		const held = new Proxy(readSections(stored), {});
		expect(numbersIn(addItem(held, { akt: 'T9' })).at(-1)).toBe('T9');
		expect(numbersIn(moveItem(held, [2], -1))).toEqual(['T1', 'T2', 'N1', 'T3']);
	});
});
