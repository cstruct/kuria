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
	const third = await create('backlog_item', { title: 'Third' }, [backlog]);
	await admin.graphql(
		'mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: "start" }) { number } }',
		{ n: second }
	);

	const page = await signIn(browser, seeded().people.developer);

	// The release's board maps candidates and hotfixes; its backlog is
	// listed by type, not in an "Unmapped" column.
	await open(page, release);
	const members = page.getByRole('region', { name: 'Members', exact: true });
	await expect(members.getByRole('region', { name: 'Testing', exact: true })).toBeVisible();
	await expect(members.getByRole('region', { name: 'Unmapped' })).toHaveCount(0);
	expect(await column(page, 'Backlog')).toEqual([backlog]);
	await expect(members.getByRole('table').getByRole('cell', { name: 'Autumn work' })).toBeVisible();

	// The backlog's board: cards titled, by column.
	await open(page, backlog);
	expect(await column(page, 'To do')).toEqual([first, third]);
	expect(await column(page, 'In progress')).toEqual([second]);
	await expect(members.getByRole('link', { name: 'Third' })).toBeVisible();

	// Dragged onto the first card, the third goes before it.
	await drag(page, third, first);
	await expect.poll(() => column(page, 'To do')).toEqual([third, first]);
	// The menu does the same from the keyboard.
	await page.getByRole('button', { name: `Actions for ${third}` }).focus();
	await page.keyboard.press('Enter');
	await page.getByRole('menuitem', { name: `Move ${third} down` }).press('Enter');
	await expect.poll(() => column(page, 'To do')).toEqual([first, third]);
	// Dropped on another column, it stays where it is: a transition moves it.
	await drag(page, first, second);
	await page.waitForTimeout(300);
	expect(await column(page, 'To do')).toEqual([first, third]);
	expect(await column(page, 'In progress')).toEqual([second]);
	await memberAction(page, first, 'remove');
	await expect.poll(() => column(page, 'To do')).toEqual([third]);
});

// r[verify app.akt-types]
test('an akt type’s akter are listed, filtered by state and sorted', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (title: string) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON) { createAkt(input: { type: "backlog_item", fields: $f }) { number } }',
				{ f: { title } }
			)
		).createAkt.number;
	const older = await create('Older listed item');
	const newer = await create('Newer listed item');
	await admin.graphql(
		'mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: "start" }) { number } }',
		{ n: older }
	);

	const page = await signIn(browser, seeded().people.developer);
	await page.goto('/types/backlog_item');
	const table = page.getByRole('table', { name: 'Backlog item' });
	const numbers = () =>
		table
			.locator('tbody tr')
			.evaluateAll((rows) => rows.map((r) => r.querySelector('a')?.textContent?.trim() ?? ''));
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
});
