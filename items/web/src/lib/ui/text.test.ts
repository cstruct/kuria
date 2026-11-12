import { describe, expect, it } from 'vitest';
import { truncateMiddle } from './text';

describe('truncateMiddle', () => {
	it('keeps short values and shortens long ones in the middle', () => {
		expect(truncateMiddle('v4.2.0', 12)).toBe('v4.2.0');
		const digest = `sha256:${'e'.repeat(64)}`;
		const short = truncateMiddle(digest, 20);
		expect(short).toHaveLength(20);
		expect(short.startsWith('sha256:eee')).toBe(true);
		expect(short.endsWith('eee')).toBe(true);
		expect(short).toContain('…');
	});
});
