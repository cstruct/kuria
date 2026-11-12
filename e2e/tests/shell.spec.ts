// The app's shell: the sidebar, search from the keyboard, the drawer on a
// narrow screen, and signing out.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

test.beforeAll(async () => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	await admin.graphql(
		'mutation { createAkt(input: { type: "backlog", fields: { title: "Shell backlog" } }) { number } }'
	);
});

// r[verify app.search]
test('search finds an akt by its title and opens it', async ({ browser }) => {
	const page = await signIn(browser, seeded().people.developer);
	await expect(page.getByRole('navigation', { name: 'Kuria' }).getByRole('link', { name: /^Backlog( \d+)?$/ })).toBeVisible();
	await page.keyboard.press('/');
	const search = page.getByRole('dialog', { name: 'Search' });
	await search.getByLabel('Search by number or title').fill('shell backlog');
	await search.getByRole('option', { name: /Shell backlog/ }).first().click();
	await expect(page).toHaveURL(/\/akt\/BACKLOG-/);
	// The top bar says where we are.
	await expect(page.getByRole('navigation', { name: 'You are here' })).toContainText('Backlog');
});

// r[verify app.narrow]
test('at 390 pixels the sidebar is a drawer, and nothing scrolls sideways', async ({ browser }) => {
	const page = await signIn(browser, seeded().people.developer);
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.getByRole('navigation', { name: 'Kuria' })).toBeHidden();
	await page.getByRole('button', { name: 'Open the menu' }).click();
	const drawer = page.getByRole('dialog', { name: 'Menu' });
	await drawer.getByRole('link', { name: 'Contacts' }).click();
	await expect(page).toHaveURL(/\/contacts$/);
	await expect(drawer).toBeHidden();
	for (const path of ['/', '/contacts', '/types/backlog']) {
		await page.goto(path);
		await page.waitForLoadState('networkidle');
		const sideways = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
		expect(sideways, path).toBeLessThanOrEqual(0);
	}
});

// r[verify login.sign-out]
test('signing out ends the session', async ({ browser }) => {
	const page = await signIn(browser, seeded().people.developer);
	await page.getByRole('button', { name: 'Sign out' }).click();
	await expect(page.getByText('You’re signed out.')).toBeVisible();
	// The login page's own Konklav session ended too: it asks again.
	await page.getByRole('link', { name: 'Sign in again' }).click();
	await expect(page.getByRole('heading', { name: /Sign in to continue/ })).toBeVisible();
});
