import { describe, expect, it } from 'vitest';
import { creatableIn, insideQuery, parentBoards } from './creating';

const candidate = {
	contains: [{ type: 'epic' }, { type: 'task' }, { type: 'artifact' }],
	links: [
		{ key: 'parent_board', name: 'Parent board', inverse: 'Parent board of', to: ['backlog'] }
	],
	board: {
		parent: 'parent_board',
		columns: [
			{ key: 'todo', name: 'To do', states: { task: ['open'], epic: ['open'] } },
			{
				key: 'doing',
				name: 'In progress',
				states: { task: ['in_progress'], epic: ['in_progress'] }
			}
		]
	}
};

const types = [
	{ key: 'task', name: 'Task', states: [{ key: 'open', initial: true }], document: {} },
	{ key: 'epic', name: 'Epic', states: [{ key: 'open', initial: true }], document: {} },
	{ key: 'artifact', name: 'Artifact', states: [{ key: 'built', initial: true }], document: {} },
	{
		key: 'backlog',
		name: 'Backlog',
		states: [{ key: 'open', initial: true }],
		document: { contains: [{ type: 'task' }, { type: 'epic' }] }
	}
];

const link = (kind: string, number: string, type: string, outgoing = true) => ({
	kind,
	outgoing,
	akt: { number, type: { key: type } }
});

describe('creating from a board', () => {
	// r[verify app.board-create]
	it('offers the types whose initial state a column names, in the order contained', () => {
		expect(creatableIn(candidate, 'todo', types).map((t) => t.key)).toEqual(['epic', 'task']);
		expect(creatableIn(candidate, 'doing', types)).toEqual([]);
		expect(creatableIn(candidate, 'unmapped', types)).toEqual([]);
		expect(creatableIn(candidate, 'todo', types.slice(1)).map((t) => t.key)).toEqual(['epic']);
	});

	// r[verify app.board-create]
	it('creates in each parent board that contains the type, the container first', () => {
		const links = [
			link('parent_board', 'BACKLOG-001', 'backlog'),
			link('parent_board', 'BACKLOG-001', 'backlog'),
			link('blocks', 'BACKLOG-002', 'backlog'),
			link('parent_board', 'BACKLOG-003', 'backlog', false)
		];
		expect(parentBoards(candidate, links, 'task', types)).toEqual(['BACKLOG-001']);
		expect(parentBoards(candidate, links, 'artifact', types)).toEqual([]);
		expect(parentBoards({ ...candidate, board: { columns: [] } }, links, 'task', types)).toEqual(
			[]
		);
		expect(insideQuery('RELEASE-2026-0001-RC-0001', ['BACKLOG-001'])).toBe(
			'in=RELEASE-2026-0001-RC-0001&in=BACKLOG-001'
		);
	});
});
