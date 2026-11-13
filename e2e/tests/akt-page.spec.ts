// The akt page: the title first, one field edited in place, and a history
// that says what changed.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.header]
// r[verify app.inline-edit]
// r[verify app.history]
test('a field is edited in place, and the history says so', async ({ browser }) => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const { createAkt } = await admin.graphql<{ createAkt: { number: string } }>(
		'mutation { createAkt(input: { type: "backlog", fields: { title: "Autumn backlog" } }) { number } }'
	);
	const page = await signIn(browser, seeded().people.developer);
	await open(page, createAkt.number);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('Autumn backlog');
	await expect(page.getByTestId('state')).toHaveText('Open');
	// Its one transition is offered first.
	await expect(page.getByRole('main').getByRole('button', { name: 'Close', exact: true })).toBeVisible();

	await page.getByRole('button', { name: 'Edit Title' }).click();
	const editor = page.getByRole('form', { name: 'Edit Title' });
	await editor.getByLabel('Title', { exact: true }).fill('Winter backlog');
	await editor.getByLabel(/Reason/).fill('Renamed for the season');
	await editor.getByRole('button', { name: 'Save' }).click();
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('Winter backlog');
	await expect(editor).toBeHidden();

	const latest = page.getByRole('list', { name: 'Revisions' }).getByRole('listitem').first();
	await expect(latest).toContainText('edited it');
	await expect(latest).toContainText('Title');
	await expect(latest).toContainText('Winter backlog');
	await expect(latest).toContainText('Renamed for the season');
});
