// Contacts linked by hand, for people whose email Konklav never verified
// (a password's), and an akt's history naming the people its values name.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify contact.link-by-hand]
// r[verify person.list]
test('a contact is linked to the user it is, by hand', async ({ browser }) => {
	const { people } = seeded();
	// Hedda signs in with a password: her email is unverified, so it links
	// nothing, but she is now one of the tenant's users.
	const hedda = await signIn(browser, people.headOfProduct);
	await hedda.context().close();

	const rm = await signIn(browser, people.releaseManager);
	await rm.goto('/contacts');
	const name = `Hedda's alias ${Date.now()}`;
	const add = rm.getByRole('form', { name: 'Add a contact' });
	await add.getByLabel('Name').fill(name);
	await add.getByLabel('Email').fill(`alias-${Date.now()}@shop.example`);
	await rm.getByRole('button', { name: 'Add contact' }).click();
	const row = rm.getByRole('row', { name: new RegExp(name) });
	await expect(row).toContainText('Not yet');

	await rm.getByRole('button', { name: `Actions for ${name}` }).click();
	await rm.getByRole('menuitem', { name: `Link ${name} to a user…` }).click();
	const dialog = rm.getByRole('dialog', { name: `Link ${name} to a user` });
	await dialog.getByRole('combobox', { name: 'User' }).fill('hedda');
	await rm.getByRole('option', { name: /hedda@shop\.example/ }).click();
	await expect(dialog).toBeHidden();

	// Shown as Hedda from now on, saying who linked it.
	const linked = rm.getByRole('row', {
		name: /hedda@shop\.example.*linked by rm@shop\.example/
	});
	await expect(linked).toBeVisible();
	await expect(rm.getByRole('row', { name: new RegExp(name) })).toHaveCount(0);
});

// r[verify app.history]
test('the history names the users and contacts values name', async ({ browser }) => {
	await defineExample();
	const { people } = seeded();
	const admin = await api(seeded().clients.admin);
	const { createAkt } = await admin.graphql<{ createAkt: { number: string } }>(
		'mutation($f: JSON) { createAkt(input: { type: "release", fields: $f }) { number } }',
		{
			f: {
				title: 'Named release',
				release_manager: people.releaseManager.subject
			}
		}
	);
	const page = await signIn(browser, people.releaseManager);
	await open(page, createAkt.number);
	// The first revision is the timeline's last entry.
	const created = page.getByRole('list', { name: 'Revisions' }).locator(':scope > li').last();
	await expect(created).toContainText('Release manager');
	await expect(created).toContainText('rm@shop.example');
	await expect(created).not.toContainText('a user');
});
