// Links between akter: added from an akt's page by a kind its type
// declares, shown both ways, recorded in the history, and removed.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { history, open, pick } from '../support/app.ts';
import { defineExample, exampleDocument } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.links]
// r[verify link.read]
test('a task is linked to the task it blocks, and unlinked', async ({ browser }) => {
	// A version of the example's backlog item that declares a link kind; the
	// developer may edit backlog items. Other tests define the example anew.
	await defineExample();
	const admin = await api(seeded().clients.admin);
	await admin.graphql('mutation($d: JSON!) { defineAktType(document: $d) { key } }', {
		d: {
			...exampleDocument('backlog_item'),
			links: [{ key: 'blocks', name: 'Blocks', inverse: 'Blocked by', to: ['backlog_item'] }]
		}
	});
	const create = async (title: string) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON) { createAkt(input: { type: "backlog_item", fields: $f }) { number } }',
				{ f: { title } }
			)
		).createAkt.number;
	const first = await create('Schema first');
	const second = await create('Then the API');

	const page = await signIn(browser, seeded().people.developer);
	await open(page, first);
	await page.getByRole('button', { name: 'Add link' }).click();
	await pick(page, 'Blocks…', second);
	const blocks = page.getByRole('list', { name: 'Blocks', exact: true });
	await expect(blocks).toContainText(second);
	await expect((await history(page)).locator(':scope > li').first()).toContainText(
		`Linked: Blocks ${second}`
	);

	// The other side reads it by the inverse name.
	await open(page, second);
	await expect(page.getByRole('list', { name: 'Blocked by', exact: true })).toContainText(first);
	await page.getByRole('button', { name: `Remove link Blocked by ${first}` }).click();
	// None shown when there are none, only a way to add one.
	await expect(page.getByRole('heading', { name: 'Links' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Add link' })).toBeVisible();

	await open(page, first);
	await expect(page.getByRole('list', { name: 'Blocks', exact: true })).toHaveCount(0);
});
