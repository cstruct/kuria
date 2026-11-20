import { describe, expect, it } from 'vitest';
import { changes, draftOf, inputOf, textOf, type FieldDef, type Value } from './values';

function field(kind: FieldDef['kind'], extra: Partial<FieldDef> = {}): FieldDef {
	return {
		key: 'f',
		name: 'F',
		kind,
		description: null,
		required: false,
		diagrams: false,
		locked: [],
		multiline: false,
		multiple: false,
		of: [],
		assignees: null,
		references: null,
		options: [
			{ key: 'a', name: 'A' },
			{ key: 'b', name: 'B' }
		],
		format: null,
		...extra
	};
}

const ALICE = { __typename: 'User' as const, id: 'alice', name: 'Alice' };
const BOB = { __typename: 'Contact' as const, id: 'c-bob', displayName: 'Bob' };

describe('field values', () => {
	// r[verify app.fields]
	it('reads every kind into its editor and back into the input the server takes', () => {
		const cases: [FieldDef, Value, unknown][] = [
			[field('TEXT'), { __typename: 'TextValue', field: 'f', text: 'hi' }, 'hi'],
			[
				field('CHOICE'),
				{ __typename: 'ChoiceValue', field: 'f', option: { key: 'b', name: 'B' } },
				'b'
			],
			[
				field('CHOICES'),
				{ __typename: 'ChoicesValue', field: 'f', options: [{ key: 'a', name: 'A' }] },
				['a']
			],
			[field('DATE'), { __typename: 'DateValue', field: 'f', date: '2026-04-01' }, '2026-04-01'],
			[field('USER'), { __typename: 'UserValue', field: 'f', user: ALICE }, 'alice'],
			[field('USER'), { __typename: 'UserValue', field: 'f', user: BOB }, { contact: 'c-bob' }],
			[
				field('REFERENCE'),
				{ __typename: 'ReferenceValue', field: 'f', url: 'https://x.example', label: null },
				{ url: 'https://x.example', label: undefined }
			],
			[
				field('GIT'),
				{
					__typename: 'GitValue',
					field: 'f',
					repository: 'https://g.example/a.git',
					sha: null,
					tag: 'v1'
				},
				{ repository: 'https://g.example/a.git', sha: undefined, tag: 'v1' }
			],
			[
				field('DOCKER'),
				{ __typename: 'DockerValue', field: 'f', repository: 'r/a', digest: 'sha256:1', tag: null },
				{ repository: 'r/a', digest: 'sha256:1', tag: undefined }
			],
			[
				field('USER', { multiple: true }),
				{ __typename: 'UsersValue', field: 'f', users: [ALICE, BOB] },
				['alice', { contact: 'c-bob' }]
			],
			[
				field('SECTIONS'),
				{
					__typename: 'SectionsValue',
					field: 'f',
					value: [{ title: 'Setup', collapsible: true, items: ['T-1'] }, 'T-2']
				},
				[{ title: 'Setup', collapsible: true, items: ['T-1'] }, 'T-2']
			]
		];
		for (const [def, value, input] of cases) {
			expect(inputOf(draftOf(def, value)), def.kind).toEqual(input);
		}
	});

	// r[verify app.fields]
	it('checks a box for someone else, and clears fields with null', () => {
		const box = field('CHECKBOX');
		const forBob: Value = {
			__typename: 'CheckboxValue',
			field: 'f',
			checked: true,
			at: '2026-04-01T00:00:00Z',
			by: ALICE,
			for: BOB
		};
		expect(draftOf(box, forBob)).toEqual({
			kind: 'CHECKBOX',
			checked: true,
			for: { contact: 'c-bob' }
		});
		expect(inputOf(draftOf(box, forBob))).toEqual({ for: { contact: 'c-bob' } });
		expect(inputOf(draftOf(box, { ...forBob, for: ALICE }))).toBe(true);
		expect(inputOf(draftOf(box, undefined))).toBe(false);
		expect(inputOf(draftOf(field('TEXT'), undefined))).toBeNull();
		expect(textOf(forBob)).toBe('Yes, for Bob (checked by Alice)');
	});

	it('sends only the fields that changed', () => {
		const fields = [field('TEXT', { key: 'title' }), field('CHOICE', { key: 'kind' })];
		const before = {
			title: draftOf(fields[0], { __typename: 'TextValue', field: 'title', text: 'Old' }),
			kind: draftOf(fields[1], undefined)
		};
		const after = { ...before, title: { kind: 'TEXT' as const, text: 'New' } };
		expect(changes(fields, before, after)).toEqual({ title: 'New' });
		expect(changes(fields, before, before)).toEqual({});
	});
});

// r[verify app.fields]
describe('markdown fields', () => {
	it('are edited as their Markdown, and empty clears them', () => {
		const value: Value = {
			__typename: 'MarkdownValue',
			field: 'f',
			markdown: '# Scope {#scope}\n',
			headings: [{ id: 'scope', level: 1, text: 'Scope' }],
			references: [],
			images: [],
			diagrams: []
		};
		const draft = draftOf(field('MARKDOWN'), value);
		expect(draft).toEqual({
			kind: 'MARKDOWN',
			text: '# Scope {#scope}\n',
			pictures: [],
			drawings: []
		});
		expect(inputOf(draft)).toBe('# Scope {#scope}\n');
		expect(inputOf({ kind: 'MARKDOWN', text: '  \n', pictures: [], drawings: [] })).toBeNull();
		expect(textOf(value)).toBe('# Scope {#scope}\n');
	});
});
