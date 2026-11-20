import { describe as group, expect, it } from 'vitest';
import { ago, describe, merged, namesOf, valueText } from './history';

const by = { name: 'rm@shop.example' };

group('history', () => {
	// r[verify app.history]
	it('says a file is a file, not someone', () => {
		const file = '01a1e91d-5c9f-7675-b09e-2fd4f89598e8';
		const entry = describe(
			{
				revision: 2,
				transition: null,
				reason: null,
				changes: { pdf: file, owner: file },
				createdAt: '2026-04-01T10:00:00Z',
				by
			},
			{ pdf: 'PDF', owner: 'Owner' },
			{},
			{ pdf: 'ATTACHMENT', owner: 'USER' }
		);
		expect(entry.changes).toEqual([
			{ field: 'PDF', value: 'a new file' },
			{ field: 'Owner', value: 'a user' }
		]);
	});

	// r[verify app.history]
	it('says who did what, what it changed and which memberships', () => {
		const names = { version: 'Version', go: 'Go for release' };
		const transitions = { release: 'Release' };
		expect(
			describe(
				{
					revision: 6,
					transition: 'release',
					reason: null,
					changes: {
						version: '4.2.0',
						go: { checked_by: 'x', checked_at: 't', for: { contact: 'c' } },
						gone: null
					},
					createdAt: '2026-04-01T10:00:00Z',
					by
				},
				names,
				transitions
			)
		).toMatchObject({
			who: 'rm@shop.example',
			what: 'performed Release',
			changes: [
				{ field: 'Version', value: '4.2.0' },
				{ field: 'Go for release', value: 'checked for a contact by a user' },
				{ field: 'gone', value: null }
			]
		});
		const moved = describe(
			{
				revision: 3,
				changes: {},
				membership: {
					added: [{ akt: 'a', number: 'RC-2' }],
					removed: [{ akt: 'b', number: 'RC-1' }]
				},
				createdAt: '2026-04-01T10:00:00Z',
				by
			},
			{},
			{}
		);
		expect(moved.what).toBe('changed what it holds or where it is');
		expect(moved.memberships).toEqual(['Added to RC-2', 'Removed from RC-1']);
		expect(
			describe(
				{ revision: 1, changes: { title: 'T' }, createdAt: '2026-04-01T10:00:00Z', by },
				{},
				{}
			).what
		).toBe('created it');
	});

	it('shows values and times briefly', () => {
		expect(valueText({ url: 'https://x.example', label: 'Build 812' })).toBe('Build 812');
		expect(valueText({ repository: 'r/backend', digest: 'sha256:1', tag: '4.2.1' })).toBe(
			'r/backend:4.2.1'
		);
		expect(valueText(['a', 'b'])).toBe('a, b');
		expect(
			valueText([{ title: 'Setup', collapsible: true, items: ['T-1', { items: [] }] }, 'T-2'])
		).toBe('Setup (T-1, a section (empty)), T-2');
		const named = new Map([['00000000-0000-0000-0000-000000000001', 'Alice']]);
		expect(
			valueText(
				[{ assignee: '00000000-0000-0000-0000-000000000001', verdict: 'pass', by: 'x', at: 'y' }],
				named
			)
		).toBe('Alice: pass');
		expect(valueText('866b9e77-6aa2-4778-bb2f-77f6c8d0c816')).toBe('a user');
		const long = valueText(`## Epic\n- ${'x'.repeat(300)}`)!;
		expect(long).toHaveLength(140);
		expect(long.startsWith('## Epic - xxx')).toBe(true);
		expect(long.endsWith('…')).toBe(true);
		const now = new Date('2026-04-01T12:00:00Z');
		expect(ago('2026-04-01T11:59:58Z', now)).toBe('just now');
		expect(ago('2026-04-01T11:55:00Z', now)).toBe('5 minutes ago');
		expect(ago('2026-03-31T12:00:00Z', now)).toBe('yesterday');
	});

	it('names users and contacts by the names the revision gives', () => {
		const ann = '866b9e77-6aa2-4778-bb2f-77f6c8d0c816';
		const bob = '01a1d402-87e4-77b0-bd6a-fc70a528df15';
		const names = namesOf([
			{ __typename: 'User', id: ann, name: 'Ann' },
			{ __typename: 'Contact', id: bob, displayName: 'Bob' }
		]);
		expect(valueText(ann, names)).toBe('Ann');
		expect(valueText({ contact: bob }, names)).toBe('Bob');
		expect(valueText({ checked_by: ann, checked_at: 'x', for: { contact: bob } }, names)).toBe(
			'checked for Bob by Ann'
		);
		expect(valueText({ checked_by: ann, checked_at: 'x', for: ann }, names)).toBe('checked by Ann');
		// Unnamed, they are still said to be a user or a contact.
		expect(valueText({ contact: 'other' }, names)).toBe('a contact');
		const described = describe(
			{
				revision: 2,
				changes: { owner: ann },
				createdAt: '2026-04-01T10:00:00Z',
				by: { name: 'Cy' },
				named: [{ __typename: 'User', id: ann, name: 'Ann' }]
			},
			{ owner: 'Owner' },
			{}
		);
		expect(described.changes).toEqual([{ field: 'Owner', value: 'Ann' }]);
	});

	// r[verify app.links]
	it('says link changes with the name this akt reads them by', () => {
		const linked = describe(
			{
				revision: 3,
				changes: {},
				membership: { linked_from: [{ number: 'TASK-1', name: 'Blocked by', kind: 'blocks' }] },
				createdAt: '2026-04-01T10:00:00Z',
				by: { name: 'Ann' }
			},
			{},
			{}
		);
		expect(linked.what).toBe('changed its links');
		expect(linked.memberships).toEqual(['Linked: Blocked by TASK-1']);
		expect(
			describe(
				{
					revision: 4,
					changes: {},
					membership: { unlinked: [{ number: 'TASK-2', name: 'Blocks' }] },
					createdAt: '2026-04-01T10:00:00Z',
					by: { name: 'Ann' }
				},
				{},
				{}
			).memberships
		).toEqual(['Unlinked: Blocks TASK-2']);
	});

	// r[verify app.archive]
	it('says an archive and an unarchive, a rule’s by Kuria', () => {
		const archived = describe(
			{
				revision: 5,
				transition: 'archive',
				reason: 'by the rule `newer`',
				changes: {},
				createdAt: '2026-04-01T10:00:00Z',
				by: { id: '00000000-0000-0000-0000-000000000000', name: '00000000' }
			},
			{},
			{}
		);
		expect(archived).toMatchObject({ who: 'Kuria', what: 'archived it', changes: [] });
		expect(
			describe(
				{
					revision: 6,
					transition: 'unarchive',
					changes: {},
					createdAt: '2026-04-01T10:00:00Z',
					by: { id: 'u1', name: 'Ann' }
				},
				{},
				{}
			)
		).toMatchObject({ who: 'Ann', what: 'unarchived it' });
	});
});

// r[verify app.history]
group('a readable history', () => {
	const by = { id: 'u1', name: 'Devi' };
	const added = (revision: number, member: string, at: string) =>
		describe(
			{
				revision,
				changes: {},
				membership: { added_member: [{ number: member }] },
				createdAt: at,
				by
			},
			{},
			{}
		);

	it('says choices by their names, in the order the fields are shown', () => {
		const entry = describe(
			{
				revision: 2,
				changes: { kind: 'feature', title: 'T' },
				createdAt: '2026-11-19T10:00:00Z',
				by
			},
			{ title: 'Title', kind: 'Kind' },
			{},
			{},
			{ kind: { feature: 'Feature' } }
		);
		expect(entry.changes).toEqual([
			{ field: 'Title', value: 'T' },
			{ field: 'Kind', value: 'Feature' }
		]);
	});

	it('says one person adding several within a minute once', () => {
		const entries = merged([
			added(4, 'T-3', '2026-11-19T10:00:40Z'),
			added(3, 'T-2', '2026-11-19T10:00:20Z'),
			added(2, 'T-1', '2026-11-19T10:00:00Z'),
			added(1, 'T-0', '2026-11-19T09:00:00Z')
		]);
		expect(entries.map((e) => [e.what, e.memberships.length])).toEqual([
			['added 3 items', 3],
			['created it', 1]
		]);
	});
});
