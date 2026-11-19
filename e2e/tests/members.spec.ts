// A container's members: the akt types its board maps on the board, the
// others as lists by type; reordering by dragging and from the menu; and
// an akt type's list, filtered by state and sorted.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { column, drag, memberAction, open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.members]
// r[verify app.board-drop]
test('members are on the board or listed by type, and reorder', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const release = await create('release', { title: 'Autumn' });
	const backlog = await create('backlog', { title: 'Autumn work' }, [release]);
	const first = await create('backlog_item', { title: 'First' }, [backlog]);
	const second = await create('backlog_item', { title: 'Second' }, [backlog]);
	const third = await create('backlog_item', { title: 'Third', kind: 'fix' }, [backlog]);
	const done = await create('backlog_item', { title: 'Done already' }, [backlog]);
	const transition = (n: string, revision: number, t: string, fields: Record<string, unknown> = {}) =>
		admin.graphql(
			'mutation($n: String!, $r: Int!, $t: String!, $f: JSON) { performTransition(input: { number: $n, revision: $r, transition: $t, fields: $f }) { number } }',
			{ n, r: revision, t, f: fields }
		);
	await transition(second, 1, 'start');
	await transition(done, 1, 'start');
	await transition(done, 2, 'review', {
		change: { url: 'https://git.example/pr/1' }
	});
	await transition(done, 3, 'finish');

	const page = await signIn(browser, seeded().people.developer);

	// The release's board maps candidates and hotfixes; its backlog is
	// listed by type, not in an "Unmapped" column.
	await open(page, release);
	const members = page.getByRole('region', { name: 'Contents', exact: true });
	await expect(members.getByRole('region', { name: 'Testing', exact: true })).toBeVisible();
	await expect(members.getByRole('region', { name: 'Unmapped' })).toHaveCount(0);
	expect(await column(page, 'Backlog')).toEqual([backlog]);
	await expect(members.getByRole('table').getByRole('cell', { name: 'Autumn work' })).toBeVisible();

	// The backlog's board: cards titled, by column.
	await open(page, backlog);
	expect(await column(page, 'To do')).toEqual([first, third]);
	expect(await column(page, 'In progress')).toEqual([second]);
	// The card shows the fields the type's presentation picks for cards,
	// and the state in its tone.
	const card = members.locator(`[data-number="${third}"]`);
	await expect(card.getByRole('link', { name: 'Third' })).toBeVisible();
	await expect(card.locator('dl')).toHaveText(/Kind\s*Fix/);
	await expect(members.locator(`[data-number="${second}"] .badge`)).toHaveClass(/active/);

	// Dragged onto the first card, the third goes before it.
	await drag(page, third, first);
	await expect.poll(() => column(page, 'To do')).toEqual([third, first]);
	// The menu does the same from the keyboard.
	await page.getByRole('button', { name: `Actions for ${third}` }).focus();
	await page.keyboard.press('Enter');
	await page.getByRole('menuitem', { name: `Move ${third} down` }).press('Enter');
	await expect.poll(() => column(page, 'To do')).toEqual([first, third]);
	// Dropped on a column no transition leads to, it stays where it is.
	await drag(page, first, done);
	await page.waitForTimeout(300);
	expect(await column(page, 'To do')).toEqual([first, third]);
	await expect(page.getByRole('dialog')).toHaveCount(0);
	// Dropped on one a transition leads to, it asks, and moves.
	await drag(page, first, second);
	const move = page.getByRole('dialog', { name: 'Move First to In progress' });
	await move.getByRole('button', { name: 'Start' }).click();
	await expect(move).toBeHidden();
	await expect.poll(() => column(page, 'In progress')).toEqual([first, second]);
	expect(await column(page, 'To do')).toEqual([third]);
	await memberAction(page, third, 'remove');
	await expect.poll(() => column(page, 'To do')).toEqual([]);
});

// r[verify app.akt-types]
// r[verify app.filters]
test('an akt type’s akter are listed, filtered by state and sorted', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (title: string, fields: Record<string, unknown> = {}) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON) { createAkt(input: { type: "backlog_item", fields: $f }) { number } }',
				{ f: { title, ...fields } }
			)
		).createAkt.number;
	const older = await create('Older listed item', { kind: 'fix', assignee: seeded().people.developer.subject });
	const newer = await create('Newer listed item', { kind: 'feature' });
	await admin.graphql(
		'mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: "start" }) { number } }',
		{ n: older }
	);

	const page = await signIn(browser, seeded().people.developer);
	await page.goto('/types/backlog_item');
	const table = page.getByRole('table', { name: 'Task' });
	const numbers = () =>
		table.locator('tbody tr').evaluateAll((rows) => rows.map((r) => r.querySelector('a')?.textContent?.trim() ?? ''));
	const mine = async () => (await numbers()).filter((n) => n === older || n === newer);

	// Most recently changed first: the started one.
	await expect.poll(mine).toEqual([older, newer]);
	await expect(table.getByRole('columnheader', { name: 'Title' })).toBeVisible();
	await page.getByLabel('Sort by').selectOption('rank');
	await expect(page).toHaveURL(/sort=rank/);
	await expect.poll(mine).toEqual([older, newer]);

	await page.getByLabel('State').selectOption({ label: 'In progress' });
	await expect(page).toHaveURL(/state=in_progress/);
	await expect.poll(mine).toEqual([older]);
	await expect(table.getByRole('row', { name: /Older listed item/ })).toContainText('In progress');

	// Filtered by field values, kept in the address.
	await page.getByLabel('State').selectOption('');
	await page.getByLabel('Kind').selectOption({ label: 'Feature' });
	await expect(page).toHaveURL(/where\.kind=feature/);
	await expect.poll(mine).toEqual([newer]);
	await page.getByLabel('Kind').selectOption('');
	await page.getByLabel('Assignee').selectOption({ label: 'Me' });
	await expect(page).toHaveURL(/where\.assignee=me/);
	await expect.poll(mine).toEqual([older]);
	await page.reload();
	await expect.poll(mine).toEqual([older]);
});

// r[verify akt.page]
// r[verify app.filters]
test('an akt type’s list loads more a page at a time', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	for (let i = 0; i < 51; i++) {
		await admin.graphql('mutation($f: JSON) { createAkt(input: { type: "epic", fields: $f }) { number } }', {
			f: { title: `Paged epic ${i}` }
		});
	}
	const page = await signIn(browser, seeded().people.developer);
	await page.goto('/types/epic');
	const rows = page.getByRole('table', { name: 'Epic' }).locator('tbody tr');
	await expect(rows).toHaveCount(50);
	await page.getByRole('button', { name: 'Load more' }).click();
	await expect.poll(() => rows.count()).toBeGreaterThan(50);
	// Each akt once, across pages.
	const numbers = await rows.evaluateAll((all) => all.map((r) => r.querySelector('a')?.textContent?.trim()));
	expect(new Set(numbers).size).toBe(numbers.length);
});

// r[verify app.board-create]
// r[verify board.parent]
test('a column creates its types, in the board and its parent boards', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const release = await create('release', { title: 'Winter' });
	const backlog = await create('backlog', { title: 'Winter work' }, [release]);
	const candidate = await create('release_candidate', {}, [release]);
	await admin.graphql(
		'mutation($s: String!, $t: String!) { addLink(input: { source: $s, kind: "parent_board", target: $t }) { number } }',
		{ s: candidate, t: backlog }
	);

	const page = await signIn(browser, seeded().people.releaseManager);
	await open(page, candidate);
	const members = page.getByRole('region', { name: 'Contents', exact: true });
	// Only the column holding the initial states offers to create.
	await expect(members.getByRole('button', { name: 'New in In progress' })).toHaveCount(0);
	await members.getByRole('button', { name: 'New in To do' }).click();
	await expect(page.getByRole('menuitem', { name: 'New Epic' })).toBeVisible();
	await page.getByRole('menuitem', { name: 'New Task' }).click();
	await expect(page.getByText(`In ${candidate}, ${backlog}`)).toBeVisible();
	await page.getByRole('group', { name: /^Title/ }).getByLabel('Title', { exact: true }).fill('Winter tyres');
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(page).toHaveURL(/\/akt\//);
	const task = (await page.getByTestId('number').textContent())!.trim();
	expect(task).toMatch(/^TASK-\d{4}$/);

	await open(page, candidate);
	expect(await column(page, 'To do')).toEqual([task]);
	await open(page, backlog);
	expect(await column(page, 'To do')).toEqual([task]);
});
