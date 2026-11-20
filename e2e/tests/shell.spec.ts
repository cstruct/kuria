// The app's shell: the sidebar, search from the keyboard, the drawer on a
// narrow screen, and signing out.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import AxeBuilder from '@axe-core/playwright';
import { defineDocuments, defineExample } from '../support/example.ts';
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
	await expect(
		page.getByRole('navigation', { name: 'Kuria' }).getByRole('link', { name: /^Backlog( \d+)?$/ })
	).toBeVisible();
	await page.keyboard.press('/');
	const search = page.getByRole('dialog', { name: 'Search' });
	await search.getByLabel('Search by number or title').fill('shell backlog');
	await search
		.getByRole('option', { name: /Shell backlog/ })
		.first()
		.click();
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

// r[verify app.navigation]
test('akt types that say so are listed under Show more, which stays as left', async ({ browser }) => {
	await defineDocuments();
	const page = await signIn(browser, seeded().people.developer);
	const sidebar = page.getByRole('navigation', { name: 'Kuria' });
	const more = sidebar.getByRole('button', { name: /^Show more/ });
	await expect(more).toHaveAttribute('aria-expanded', 'false');
	await expect(sidebar.getByRole('link', { name: /^Document( \d+)?$/ })).toBeVisible();
	await expect(sidebar.getByRole('link', { name: /^Image( \d+)?$/ })).toBeHidden();
	await more.click();
	await expect(more).toHaveAttribute('aria-expanded', 'true');
	const image = sidebar.getByRole('list', { name: 'More work' }).getByRole('link', { name: /^Image( \d+)?$/ });
	await expect(image).toBeVisible();
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the sidebar, opened'
	).toEqual([]);
	await image.click();
	await expect(page).toHaveURL(/\/types\/image$/);
	// Remembered in the browser.
	await page.reload();
	await expect(sidebar.getByRole('button', { name: /^Show more/ })).toHaveAttribute('aria-expanded', 'true');
	await sidebar.getByRole('button', { name: /^Show more/ }).click();
	await page.reload();
	await expect(sidebar.getByRole('button', { name: /^Show more/ })).toHaveAttribute('aria-expanded', 'false');
	// Home lists it as before.
	await page.getByRole('link', { name: 'Home' }).click();
	await expect(page.getByRole('main').getByText('Image', { exact: true }).first()).toBeVisible();
});

// r[verify app.icon]
test("Kuria's mark is the page's icon, before any script runs", async ({ request }) => {
	const page = await (await request.get('/')).text();
	for (const [href, type] of [
		['/favicon.ico', 'image/x-icon'],
		['/favicon.svg', 'image/svg+xml'],
		['/apple-touch-icon.png', 'image/png']
	]) {
		expect(page, href).toContain(`href="${href}"`);
		const icon = await request.get(href);
		expect(icon.status(), href).toBe(200);
		expect(icon.headers()['content-type'], href).toContain(type);
	}
});
