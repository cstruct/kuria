import { describe, expect, it } from 'vitest';
import {
	KURIA_SUBJECT,
	archivedBy,
	archivedCount,
	archivedFilterOf,
	archivedSummary,
	showsArchived,
	withArchived
} from './archive';

describe('archive', () => {
	// r[verify app.archive]
	it('keeps Show archived in the address as archived=all', () => {
		expect(showsArchived(new URLSearchParams('archived=all'))).toBe(true);
		expect(showsArchived(new URLSearchParams('archived=ALL'))).toBe(true);
		expect(showsArchived(new URLSearchParams('archived=no'))).toBe(false);
		expect(showsArchived(new URLSearchParams(''))).toBe(false);
		expect(archivedFilterOf(new URLSearchParams('state=open&archived=all'))).toBe('ALL');
		// Off, the server's default: live akter only.
		expect(archivedFilterOf(new URLSearchParams('state=open'))).toBeNull();
		const on = withArchived(new URLSearchParams('state=open'), true);
		expect(on.toString()).toBe('state=open&archived=all');
		expect(withArchived(on, false).toString()).toBe('state=open');
	});

	// r[verify app.archive]
	it('says how many members are archived', () => {
		expect(archivedCount(3)).toBe('3 archived');
		expect(archivedCount(1)).toBe('1 archived');
	});

	// r[verify app.archive]
	it('says who archived an akt, a person or Kuria by a rule, when and why', () => {
		const date = () => '3 Nov 2026';
		expect(
			archivedSummary(
				{ at: '2026-11-03T10:00:00Z', by: { id: 'u1', name: 'Rita' }, reason: ' Done with it ' },
				date
			)
		).toEqual({ title: 'Archived 3 Nov 2026 by Rita', reason: 'Done with it' });
		expect(archivedSummary({ at: '2026-11-03T10:00:00Z', by: null, rule: 'newer' }, date)).toEqual({
			title: 'Archived 3 Nov 2026 by Kuria, by the rule “newer”',
			reason: null
		});
		expect(archivedBy({ at: '', by: { id: KURIA_SUBJECT, name: KURIA_SUBJECT } })).toBe('Kuria');
		expect(archivedBy({ at: '', by: null, rule: null })).toBe('Kuria');
	});
});
