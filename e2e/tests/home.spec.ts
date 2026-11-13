// Home: what changed most recently, and every akt type with its count and
// a way to create one.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.home]
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

	const types = page.getByRole('region', { name: 'Akt types' });
	await expect(types.getByRole('link', { name: 'Backlog item', exact: true })).toBeVisible();
	// Each type with the icon its presentation gives it.
	await expect(types.getByRole('link', { name: 'Release', exact: true }).locator('svg')).toBeVisible();
	const backlogs = types
		.getByRole('listitem')
		.filter({ has: page.getByRole('link', { name: 'Backlog', exact: true }) });
	await expect(backlogs).toContainText(/\d+ akt(er)?/);

	await recent.getByRole('link', { name: /Home-page backlog/ }).click();
	await expect(page.getByTestId('number')).toContainText(createAkt.number);

	await page.goto('/');
	await types.getByRole('link', { name: 'New Backlog', exact: true }).click();
	await expect(page).toHaveURL(/\/types\/backlog\/new$/);
});
