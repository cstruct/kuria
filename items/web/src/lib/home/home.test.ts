import { describe, expect, it } from 'vitest';
import { counted } from './home';

// r[verify app.home]
// r[verify app.words]
describe('counted', () => {
	it('says how many, in the type’s words', () => {
		const task = { name: 'Task' };
		expect(counted(1, task)).toBe('1 task');
		expect(counted(0, task)).toBe('0 tasks');
		expect(counted(3, task)).toBe('3 tasks');
		expect(counted(null, task)).toBe('0 tasks');
		const notes = { name: 'Release notes', presentation: { plural: 'Release notes' } };
		expect(counted(2, notes)).toBe('2 release notes');
	});
});
