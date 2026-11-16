import { describe, expect, it } from 'vitest';
import { lineDiff } from './diff';

// r[verify app.versions]
// r[verify version.diff]
describe('lineDiff', () => {
	it('keeps, removes and adds lines in order', () => {
		expect(lineDiff('a\nb\nc\n', 'a\nx\nc\n')).toEqual([
			{ tag: ' ', text: 'a' },
			{ tag: '-', text: 'b' },
			{ tag: '+', text: 'x' },
			{ tag: ' ', text: 'c' }
		]);
		expect(lineDiff('', 'new\n')).toEqual([{ tag: '+', text: 'new' }]);
		expect(lineDiff('old', '')).toEqual([{ tag: '-', text: 'old' }]);
		expect(lineDiff('same\n', 'same\n')).toEqual([{ tag: ' ', text: 'same' }]);
	});
});
