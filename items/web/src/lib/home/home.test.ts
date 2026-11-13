import { describe, expect, it } from 'vitest';
import { counted } from './home';

// r[verify app.home]
describe('counted', () => {
	it('says how many akter, in the singular for one', () => {
		expect(counted(1)).toBe('1 akt');
		expect(counted(0)).toBe('0 akter');
		expect(counted(3)).toBe('3 akter');
		expect(counted(null)).toBe('0 akter');
	});
});
