// Home: what changed most recently, and every akt type with its count and
// a way to create one.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.home]
// r[verify app.my-work]
// r[verify app.type-identity]
test('home shows what changed recently and every akt type', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const { createAkt } = await admin.graphql<{ createAkt: { number: string } }>(
		'mutation { createAkt(input: { type: "backlog", fields: { title: "Home-page backlog" } }) { number } }'
	);

	const page = await signIn(browser, seeded().people.developer);
	await page.goto('/');
	const recent = page.getByRole('list', { name: 'Recently changed' });
	await expect(recent.getByRole('listitem').first()).toContainText('Home-page backlog');
	await expect(recent.getByRole('listitem').first()).toContainText('Open');

	const types = page.getByRole('region', { name: 'Work' });
	// By its address: the planning example's tasks are named Task too.
	await expect(types.locator('a[href="/types/backlog_item"]')).toBeVisible();
	// Each type with the icon its presentation gives it.
	await expect(types.getByRole('link', { name: 'Release', exact: true }).locator('svg')).toBeVisible();
	const backlogs = types
		.getByRole('listitem')
		.filter({ has: page.getByRole('link', { name: 'Backlog', exact: true }) });
	// r[verify app.words]: counted in the type's own words.
	await expect(backlogs).toContainText(/\d+ backlogs?\b/);

	// My work: what names the viewer.
	await admin.graphql(
		'mutation($f: JSON) { createAkt(input: { type: "backlog_item", fields: $f }) { number } }',
		{ f: { title: 'Assigned to the developer', assignee: seeded().people.developer.subject } }
	);
	await page.reload();
	await expect(
		page.getByRole('list', { name: 'My work: Task' }).filter({ hasText: 'Assigned to the developer' })
	).toBeVisible();

	await recent.getByRole('link', { name: /Home-page backlog/ }).click();
	await expect(page.getByTestId('number')).toContainText(createAkt.number);

	await page.goto('/');
	await types.getByRole('link', { name: 'New Backlog', exact: true }).click();
	await expect(page).toHaveURL(/\/types\/backlog\/new$/);
});

// r[verify app.permitted]
test('a reader is offered nothing to create, edit or move', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const { createAkt } = await admin.graphql<{ createAkt: { number: string } }>(
		'mutation { createAkt(input: { type: "backlog", fields: { title: "Read-only backlog" } }) { number } }'
	);

	const page = await signIn(browser, seeded().people.headOfProduct);
	await page.goto('/');
	const types = page.getByRole('region', { name: 'Work' });
	await expect(types.locator('a[href="/types/backlog"]')).toBeVisible();
	await expect(types.getByRole('link', { name: /^New / })).toHaveCount(0);

	await page.goto('/types/backlog');
	await expect(page.getByRole('heading', { level: 1 })).toContainText('Backlog');
	await expect(page.getByRole('link', { name: 'New Backlog' })).toHaveCount(0);

	await page.goto(`/akt/${createAkt.number}`);
	await expect(page.getByTestId('number')).toHaveText(createAkt.number);
	const main = page.getByRole('main');
	await expect(main.getByRole('button', { name: 'Close', exact: true })).toHaveCount(0);
	await expect(main.getByRole('button', { name: /^Edit / })).toHaveCount(0);
	await expect(main.getByRole('button', { name: 'Make a copy' })).toHaveCount(0);
	await expect(main.getByRole('button', { name: /^New .* in / })).toHaveCount(0);

	// The developer, who may, is offered them.
	const developer = await signIn(browser, seeded().people.developer);
	await developer.goto(`/akt/${createAkt.number}`);
	await expect(developer.getByRole('main').getByRole('button', { name: 'Close', exact: true })).toBeVisible();
});
