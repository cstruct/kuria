// Every main page, checked by axe against WCAG 2.2 A and AA, in the light
// and the dark appearance: the akt types, Home, akter by type, a new akt's
// form, an akt with a board and lists, its transition dialog, a member's
// menu and a field edited in place, contacts, the design gallery, the
// login page, and at a phone's width an akt and the drawer.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { newPage, signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

/** What axe finds on the page as it is, by rule, with the elements. */
async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map(
		(v) => `${v.id} (${v.impact}): ${v.help}\n  ${v.nodes.map((n) => n.target.join(' ')).join('\n  ')}`
	);
}

for (const colorScheme of ['light', 'dark'] as const) {
	// r[verify app.accessible]
	// r[verify app.themes]
	test(`every main page passes axe in the ${colorScheme} appearance`, async ({ browser }) => {
		test.setTimeout(120_000);
		await defineExample();
		const admin = await api(seeded().clients.admin);
		const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
			(
				await admin.graphql<{ createAkt: { number: string } }>(
					'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
					{ t: type, f: fields, in: inside }
				)
			).createAkt.number;
		const release = await create('release', { title: 'Checked release' });
		const backlog = await create('backlog', { title: 'Checked backlog' }, [release]);
		const item = await create('backlog_item', { title: 'Checked item', kind: 'fix' }, [backlog]);

		// Signed out, the login page.
		const login = await newPage(browser, { colorScheme });
		await login.goto('/');
		await expect(login.getByRole('heading', { name: /Sign in/ })).toBeVisible();
		expect(await violations(login), 'the login page').toEqual([]);

		const page = await signIn(browser, seeded().people.releaseManager, {
			colorScheme
		});
		const check = async (name: string, path: string, ready: (page: Page) => Promise<void>) => {
			await page.goto(path);
			await ready(page);
			expect(await violations(page), name).toEqual([]);
		};
		const heading = (name: string | RegExp) => async (page: Page) =>
			expect(page.getByRole('heading', { level: 1, name })).toBeVisible();

		await check('Home', '/', async (p) => {
			await expect(p.getByRole('heading', { name: 'Recently changed' })).toBeVisible();
		});
		await check('akter by type', '/types/backlog_item', async (p) => {
			await expect(p.getByRole('table', { name: 'Backlog item' })).toBeVisible();
		});
		await check('a new akt', '/types/backlog_item/new', heading(/Backlog item/));
		await check('contacts', '/contacts', heading('Contacts'));
		const contact = `Checked contact ${colorScheme} ${Date.now()}`;
		await page.getByRole('form', { name: 'Add a contact' }).getByLabel('Name').fill(contact);
		await page.getByRole('button', { name: 'Add contact' }).click();
		await page.getByRole('button', { name: `Actions for ${contact}` }).click();
		await page.getByRole('menuitem', { name: `Link ${contact} to a user…` }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		expect(await violations(page), 'linking a contact').toEqual([]);
		await page.keyboard.press('Escape');
		await check('the design gallery', '/design', heading(/Design/));

		await open(page, release);
		expect(await violations(page), 'a release with a board and lists').toEqual([]);
		await open(page, backlog);
		expect(await violations(page), 'a backlog with cards').toEqual([]);

		await page.getByRole('button', { name: `Actions for ${item}` }).click();
		await expect(page.getByRole('menu')).toBeVisible();
		expect(await violations(page), 'a member’s menu').toEqual([]);
		await page.keyboard.press('Escape');

		await page.getByRole('main').getByRole('button', { name: 'Close', exact: true }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		expect(await violations(page), 'a transition dialog').toEqual([]);
		await page.keyboard.press('Escape');

		await page.getByRole('button', { name: 'Edit Title' }).click();
		await expect(page.getByRole('form', { name: 'Edit Title' })).toBeVisible();
		expect(await violations(page), 'a field edited in place').toEqual([]);

		// At a phone's width, the release and the drawer.
		await page.setViewportSize({ width: 390, height: 844 });
		await open(page, release);
		expect(await violations(page), 'a release at 390 pixels').toEqual([]);
		await page.getByRole('button', { name: 'Open the menu' }).click();
		await expect(page.getByRole('dialog', { name: 'Menu' })).toBeVisible();
		expect(await violations(page), 'the drawer').toEqual([]);
	});
}
