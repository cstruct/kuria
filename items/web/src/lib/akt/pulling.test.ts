import { describe, expect, it } from 'vitest';
import { offeredFrom, parentOf } from './pulling';

const member = (
	number: string,
	containers: { number: string; type: { key: string } }[],
	type = 'task',
	final = false
) => ({
	number,
	title: `Title ${number}`,
	state: { name: final ? 'Done' : 'To do', final },
	type: { key: type },
	containers
});

// r[verify app.pull]
describe('pulling from a parent board', () => {
	it('offers what the container holds, marking what is here or elsewhere', () => {
		const backlog = { number: 'B-1', type: { key: 'backlog' } };
		const columns = [
			{
				key: 'todo',
				name: 'To do',
				akter: [
					member('T-1', [backlog]),
					member('T-2', [backlog, { number: 'RC-1', type: { key: 'candidate' } }]),
					member('T-3', [backlog, { number: 'RC-2', type: { key: 'candidate' } }]),
					member('E-1', [backlog], 'epic')
				]
			},
			{ key: 'done', name: 'Done', akter: [member('T-4', [backlog], 'task', true)] },
			{ key: 'empty', name: 'Empty', akter: [member('E-2', [backlog], 'epic')] }
		];
		const offered = offeredFrom(columns, 'RC-1', 'candidate', ['task']);
		expect(offered.map((c) => c.name)).toEqual(['To do', 'Done']);
		expect(offered[0].akter).toEqual([
			{
				number: 'T-1',
				title: 'Title T-1',
				status: 'To do',
				here: false,
				elsewhere: null,
				finished: false
			},
			{
				number: 'T-2',
				title: 'Title T-2',
				status: 'To do',
				here: true,
				elsewhere: null,
				finished: false
			},
			{
				number: 'T-3',
				title: 'Title T-3',
				status: 'To do',
				here: false,
				elsewhere: 'RC-2',
				finished: false
			}
		]);
		expect(offered[1].akter[0].finished).toBe(true);
	});

	it('finds the parent board by the kind its type names', () => {
		const document = { board: { parent: 'parent_board' } };
		const links = [
			{ kind: 'blocks', outgoing: true, akt: { number: 'X-1' } },
			{ kind: 'parent_board', outgoing: false, akt: { number: 'B-0' } },
			{ kind: 'parent_board', outgoing: true, akt: { number: 'B-1', title: 'Autumn backlog' } }
		];
		expect(parentOf(document, links)).toEqual({ number: 'B-1', title: 'Autumn backlog' });
		expect(parentOf({}, links)).toBeNull();
	});
});
