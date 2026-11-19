import { describe, expect, it } from 'vitest';
import { before, crumbsOf, type Step } from './trail';

const doc = { key: 'document', name: 'Document' };
const step = (number: string, title: string | null = null): Step => ({
	number,
	title,
	type: doc,
	href: `/akt/${number}`
});
const typeHref = (key: string) => `/types/${key}`;

// r[verify app.trail]
describe('the trail', () => {
	it('is the type, then the akt, its number under its title', () => {
		expect(crumbsOf([], { number: 'D-1', title: 'Policy', type: doc }, typeHref)).toEqual([
			{ label: 'Document', href: '/types/document' },
			{ label: 'Policy', detail: 'D-1' }
		]);
	});

	it('is where it is, outermost first, when it is in containers', () => {
		const places = [
			{ number: 'REL-1', title: 'Autumn', href: '/akt/REL-1' },
			{ number: 'RC-1', title: null, href: '/akt/RC-1' }
		];
		expect(
			crumbsOf([], { number: 'T-1', title: 'Gift cards', type: doc }, typeHref, places)
		).toEqual([
			{ label: 'Autumn', detail: 'REL-1', href: '/akt/REL-1' },
			{ label: 'RC-1', detail: undefined, href: '/akt/RC-1' },
			{ label: 'Gift cards', detail: 'T-1' }
		]);
	});

	it('grows as references are followed, each step linking back with its trail', () => {
		const trail = [step('D-1', 'Policy'), { ...step('D-2'), href: '/akt/D-2?version=2' }];
		const index = { key: 'index', name: 'Index' };
		expect(crumbsOf(trail, { number: 'X-3', title: 'Setup', type: index }, typeHref)).toEqual([
			{ label: 'Document', href: '/types/document' },
			{ label: 'Policy', detail: 'D-1', href: '/akt/D-1', state: { trail: [] } },
			{ label: 'D-2', detail: undefined, href: '/akt/D-2?version=2', state: { trail: [trail[0]] } },
			{ label: 'Setup', detail: 'X-3' }
		]);
	});

	it('is cut back to an akt already on it', () => {
		const trail = [step('D-1'), step('D-2'), step('D-3')];
		expect(before(trail, 'D-2')).toEqual([trail[0]]);
		expect(before(trail, 'D-9')).toEqual(trail);
		expect(crumbsOf(trail, { number: 'D-1', title: null, type: doc }, typeHref)).toEqual([
			{ label: 'Document', href: '/types/document' },
			{ label: 'D-1', detail: undefined }
		]);
	});
});
