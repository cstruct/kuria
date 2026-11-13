import { describe as group, expect, it } from 'vitest';
import { ago, describe, valueText } from './history';

const by = { name: 'rm@shop.example' };

group('history', () => {
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
				{ field: 'Go for release', value: 'checked for someone' },
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
		expect(moved.what).toBe('changed its containers or members');
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
});
