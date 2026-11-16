// Two people editing one document of the documents example at once, in
// two browsers: each sees the other's text as it is typed, who else is
// editing and where their cursor is, and that it is saved, without a Save;
// a reader sees it live without being able to edit; and a release takes
// the live draft as it is.
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

/** Opens the body's editor, live, and puts the cursor at its end. */
async function editBody(page: Page) {
	await page.getByRole('button', { name: 'Edit Body' }).click();
	const editor = page.getByRole('textbox', { name: 'Body' });
	await editor.click();
	await page.keyboard.press('Control+End');
	return editor;
}

// r[verify app.live]
// r[verify live.sync]
// r[verify live.presence]
// r[verify live.release]
test('two people edit one document together, and release it as it is', async ({ browser }) => {
	test.setTimeout(180_000);
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const index = await create('document_index', { title: `Live handbook ${stamp}` });
	const guide = await create('document', { title: 'Guide', body: 'Hello.\n' }, [index]);

	const dev = await signIn(browser, seeded().people.developer);
	const rm = await signIn(browser, seeded().people.releaseManager);
	await open(dev, guide);
	await open(rm, guide);
	for (const page of [dev, rm]) {
		await expect(page.getByTestId('live-status')).toHaveText('Saved');
		// Each sees the other there.
		await expect(page.getByRole('list', { name: 'Also editing' }).getByTestId('peer')).toHaveCount(1);
	}

	// The developer writes at the end; the release manager sees it as typed.
	const devEditor = await editBody(dev);
	const rmEditor = await editBody(rm);
	await dev.keyboard.type(' Written together.');
	await expect(rmEditor).toContainText('Hello. Written together.');
	// And where the developer's cursor is.
	await expect(rm.getByTestId('caret')).toHaveCount(1);

	// The release manager starts a new paragraph before it; both have both.
	// At the paragraph's very start: Home doesn't move the caret on macOS.
	await rmEditor
		.locator('p')
		.first()
		.click({ position: { x: 1, y: 4 } });
	await rm.keyboard.type('Intro.');
	await rm.keyboard.press('Enter');
	await expect(devEditor).toContainText('Intro.');
	await expect(devEditor).toContainText('Hello. Written together.');
	// The developer goes on typing where they were, after the release
	// manager's edit.
	await dev.keyboard.type(' Still here.');
	await expect(rmEditor).toContainText('Hello. Written together. Still here.');
	for (const page of [dev, rm]) {
		await expect(page.getByTestId('live-status')).toHaveText('Saved');
		await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();
		const body = page.locator('dd[data-field="body"]');
		await expect(body).toContainText('Intro.');
		await expect(body).toContainText('Hello. Written together. Still here.');
	}

	// A reader sees it live, and can't edit it.
	const reader = await signIn(browser, seeded().people.headOfProduct);
	await open(reader, guide);
	await expect(reader.getByTestId('live-status')).toHaveText('Live, read-only');
	await expect(reader.getByRole('button', { name: 'Edit Body' })).toHaveCount(0);
	await editBody(dev);
	await dev.keyboard.type(' Read live.');
	await expect(reader.locator('dd[data-field="body"]')).toContainText('Still here. Read live.');
	await dev.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();

	// The release manager releases it as it is: a version, without saving.
	const versions = rm.getByRole('region', { name: 'Versions' });
	await versions.getByRole('button', { name: 'Release a version' }).click();
	const release = rm.getByRole('dialog', { name: 'Release a version' });
	await release.getByLabel('Label').fill('1.0');
	await release.getByRole('button', { name: 'Release' }).click();
	await versions.getByRole('link', { name: 'Version 1 (1.0)' }).click();
	await expect(rm.locator('dd[data-field="body"]')).toContainText('Still here. Read live.');
	const released = await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
		'query($n: String!) { akt(number: $n, version: 1) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	expect(released.akt.fields.map((f) => f?.markdown).filter(Boolean)).toEqual([
		'Intro.\n\nHello. Written together. Still here. Read live.'
	]);
});
