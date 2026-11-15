// A `markdown` field in the rich-text editor: text written and formatted,
// a reference to another akt's heading inserted through the picker, saved
// as Markdown, and rendered as a link to that heading. Checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open, pick } from '../support/app.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.editor]
// r[verify app.references]
// r[verify app.accessible]
test('a markdown field is written in the editor, with a reference to a heading', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Handbook ${stamp}` } }
		)
	).createAkt.number;
	const create = async (title: string, body: string) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
				{ f: { title, body }, in: [index] }
			)
		).createAkt.number;
	const setup = await create(`Setup ${stamp}`, '# Installing\n\nRun the installer.\n');
	const guide = await create(`Guide ${stamp}`, 'First steps.\n');

	const page = await signIn(browser, seeded().people.developer);
	await open(page, guide);
	await page.getByRole('button', { name: 'Edit Body' }).click();
	const editor = page.getByRole('textbox', { name: 'Body' });
	await expect(editor).toContainText('First steps.');
	await editor.click();
	await page.keyboard.press('End');
	await page.keyboard.type(' See ');
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Reference' }).click();
	const dialog = page.getByRole('dialog', { name: 'Reference' });
	await pick(page, 'Akt', setup);
	await dialog.getByLabel('Heading').selectOption({ label: 'Installing' });
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the reference picker'
	).toEqual([]);
	await dialog.getByRole('button', { name: 'Insert' }).click();
	await expect(editor.getByRole('link', { name: 'Installing' })).toBeVisible();
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Bold' }).click();
	await page.keyboard.type('Now.');
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the editor'
	).toEqual([]);
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Save' }).click();

	// Saved as Markdown, rendered as a link to the heading.
	const shown = page.locator('dd[data-field="body"]');
	const link = shown.getByRole('link', { name: 'Installing' });
	await expect(link).toHaveAttribute('href', `/akt/${setup}#installing`);
	await expect(shown.locator('strong')).toHaveText('Now.');
	const saved = await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
		'query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	expect(saved.akt.fields.find((f) => f?.markdown)?.markdown).toBe(
		`First steps. See [Installing](kuria:${setup}#installing)**Now.**`
	);
	// Following it opens the setup at its heading.
	await link.click();
	await expect(page.getByTestId('number')).toHaveText(setup);
	await expect(page.locator('h1#installing')).toBeInViewport();

	// r[verify app.pdf]: its body, downloaded as a PDF.
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('link', { name: 'Download PDF' }).click()
	]);
	expect(download.suggestedFilename()).toBe(`${setup}.pdf`);
	const bytes = await (await download.createReadStream()).toArray();
	expect(Buffer.concat(bytes).subarray(0, 4).toString()).toBe('%PDF');
});
