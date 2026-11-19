// A `markdown` field in the rich-text editor: text written and formatted,
// a reference to another akt's heading inserted through the picker, and a
// footnote, saved as Markdown, and rendered as a link to that heading and a
// numbered note. Checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open, pick } from '../support/app.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.editor]
// r[verify app.references]
// r[verify akt.type.presentation]
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
	await pick(page, 'Item', setup);
	await dialog.getByLabel('Heading').selectOption({ label: 'Installing' });
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the reference picker'
	).toEqual([]);
	await dialog.getByRole('button', { name: 'Insert' }).click();
	await expect(editor.getByRole('link', { name: 'Installing' })).toBeVisible();
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Bold' }).click();
	await page.keyboard.type('Now.');
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Footnote' }).click();
	await page.keyboard.type('Ran it once.');
	await expect(editor).toContainText('[^1]');
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the editor'
	).toEqual([]);
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();

	// Saved as Markdown, rendered as a link to the heading.
	const shown = page.locator('dd[data-field="body"]');
	const link = shown.getByRole('link', { name: 'Installing' });
	await expect(link).toHaveAttribute('href', `/akt/${setup}#installing`);
	// A document's type numbers references to it: its number first.
	await expect(link).toHaveText(`${setup} Installing`);
	await expect(shown.locator('strong')).toHaveText('Now.');
	const saved = await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
		'query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	expect(saved.akt.fields.find((f) => f?.markdown)?.markdown).toBe(
		`First steps. See [Installing](kuria:${setup}#installing)**Now.**[^1]\n\n[^1]: Ran it once.`
	);
	// The footnote, numbered, linking to its note at the end and back.
	const note = shown.getByRole('link', { name: 'Footnote 1', exact: true });
	await expect(note).toHaveAttribute('href', '#fn:1');
	const notes = shown.getByRole('region', { name: 'Footnotes' });
	await expect(notes).toContainText('Ran it once.');
	await expect(notes.getByRole('link', { name: 'Back to the reference to footnote 1' })).toHaveAttribute(
		'href',
		'#fnref:1'
	);
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the rendered body'
	).toEqual([]);
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

/** A one-pixel red PNG. */
const PNG = Buffer.from(
	'89504e470d0a1a0a0000000d4948445200000001000000010802000000907753de0000000c4944415478' +
		'9c63f8cfc00000030101' +
		'00c9fe92ef0000000049454e44ae426082',
	'hex'
);

// r[verify app.editor]
// r[verify app.images]
test('an image is uploaded from the editor, embedded and shown', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Pictures ${stamp}` } }
		)
	).createAkt.number;
	const guide = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
			{ f: { title: `Illustrated ${stamp}`, body: 'Look:\n' }, in: [index] }
		)
	).createAkt.number;

	const page = await signIn(browser, seeded().people.developer);
	await open(page, guide);
	await page.getByRole('button', { name: 'Edit Body' }).click();
	const editor = page.getByRole('textbox', { name: 'Body' });
	await editor.click();
	await page.keyboard.press('End');
	await page.keyboard.press('Enter');
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Image' }).click();
	const dialog = page.getByRole('dialog', { name: 'Image' });
	await dialog
		.getByLabel('Or upload a file')
		.setInputFiles({ name: `chart-${stamp}.png`, mimeType: 'image/png', buffer: PNG });
	await dialog.getByLabel('Alt text').fill('The chart');
	// Other tests define image types too: the example's, if asked.
	const asNew = dialog.getByLabel('As a new');
	if ((await asNew.count()) > 0) await asNew.selectOption({ label: 'Image' });
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the image dialog'
	).toEqual([]);
	await dialog.getByRole('button', { name: 'Insert' }).click();
	await expect(dialog).toBeHidden();
	await expect(editor.getByRole('img', { name: 'The chart' })).toBeVisible();
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();

	// Saved as a reference to a new image akt, shown as a figure.
	const shown = page.locator('dd[data-field="body"]');
	const figure = shown.getByRole('figure');
	await expect(figure.getByRole('img', { name: 'The chart' })).toBeVisible();
	await expect(figure).toContainText('The chart');
	const saved = await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
		'query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	const markdown = saved.akt.fields.find((f) => f?.markdown)?.markdown ?? '';
	const image = markdown.match(/!\[The chart\]\(kuria:(IMG-\d+)\)/)?.[1];
	expect(image, markdown).toBeTruthy();
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the body with its figure'
	).toEqual([]);
	// The image akt's page shows its picture.
	await open(page, image!);
	await expect(page.getByRole('img', { name: `chart-${stamp}.png` })).toBeVisible();
});

// r[verify app.editor]
// r[verify app.diagrams]
// r[verify diagram.preview]
test('a diagram is written as code, previewed, toggled to its picture and saved', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Diagrams ${stamp}` } }
		)
	).createAkt.number;
	const guide = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
			{ f: { title: `Flow ${stamp}`, body: 'The flow:\n' }, in: [index] }
		)
	).createAkt.number;

	const page = await signIn(browser, seeded().people.developer);
	await open(page, guide);
	await page.getByRole('button', { name: 'Edit Body' }).click();
	const editor = page.getByRole('textbox', { name: 'Body' });
	await editor.click();
	await page.keyboard.press('End');
	await page.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Diagram' }).click();
	// As code, with a preview that follows typing.
	const preview = page.getByRole('region', { name: 'Diagram preview' });
	await expect(preview.getByRole('img')).toBeVisible();
	await page.keyboard.press('End');
	await page.keyboard.type('(');
	await expect(preview.getByRole('alert')).toContainText('The diagram doesn’t compile');
	await page.keyboard.press('Backspace');
	await expect(preview.getByRole('img')).toBeVisible();
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the diagram as code'
	).toEqual([]);
	// As its picture, and back.
	await preview.getByRole('button', { name: 'Show diagram' }).click();
	await expect(preview).toBeHidden();
	const picture = editor.locator('.diagram');
	await expect(picture.getByRole('img')).toBeVisible();
	await picture.getByRole('button', { name: 'Show code' }).click();
	await expect(preview).toBeVisible();
	await preview.getByRole('button', { name: 'Show diagram' }).click();
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();

	// Saved as a diagram block, shown as its rendering.
	const shown = page.locator('dd[data-field="body"]');
	await expect(shown.getByRole('figure').getByRole('img')).toBeVisible();
	const saved = await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
		'query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	expect(saved.akt.fields.find((f) => f?.markdown)?.markdown?.trimEnd()).toBe(
		'The flow:\n\n```diagram\n#diagram(node((0, 0), [Start]), edge("-|>"), node((1, 0), [End]))\n```'
	);
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the body with its diagram'
	).toEqual([]);
});

// r[verify app.editor]
test('a link is changed and removed from its context menu', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Links ${stamp}` } }
		)
	).createAkt.number;
	const guide = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
			{ f: { title: `Linked ${stamp}`, body: 'Read [the docs](https://example.com/old) first.\n' }, in: [index] }
		)
	).createAkt.number;
	const body = async () =>
		(
			await admin.graphql<{ akt: { fields: { markdown?: string }[] } }>(
				'query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { markdown } } } }',
				{ n: guide }
			)
		).akt.fields.find((f) => f?.markdown)?.markdown;

	const page = await signIn(browser, seeded().people.developer);
	await open(page, guide);
	await page.getByRole('button', { name: 'Edit Body' }).click();
	const editor = page.getByRole('textbox', { name: 'Body' });

	// Right-clicked, the link's menu changes its text and address.
	await editor.getByRole('link', { name: 'the docs' }).click({ button: 'right' });
	const menu = page.getByRole('menu', { name: 'Link' });
	await expect(menu.getByRole('menuitem', { name: 'Open in a new tab' })).toBeVisible();
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the link menu'
	).toEqual([]);
	await menu.getByRole('menuitem', { name: 'Edit link…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Edit link' });
	await expect(dialog.getByLabel('Text')).toHaveValue('the docs');
	await dialog.getByLabel('Text').fill('the guide');
	await dialog.getByLabel('Address').fill('https://example.com/new');
	await dialog.getByRole('button', { name: 'Save' }).click();
	await expect(editor.getByRole('link', { name: 'the guide' })).toBeVisible();
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();
	await expect.poll(body).toBe('Read [the guide](https://example.com/new) first.');

	// From the keyboard, at the cursor, it is removed, its text kept.
	await page.getByRole('button', { name: 'Edit Body' }).click();
	await editor.getByRole('link', { name: 'the guide' }).click();
	await page.keyboard.press('Shift+F10');
	await page.getByRole('menu', { name: 'Link' }).getByRole('menuitem', { name: 'Remove link' }).click();
	await expect(editor.getByRole('link')).toHaveCount(0);
	await expect(editor).toContainText('Read the guide first.');
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();
	await expect.poll(body).toBe('Read the guide first.');
});

// r[verify app.reference-preview]
// r[verify app.accessible]
test('a reference in a shown document previews its akt', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Previews ${stamp}` } }
		)
	).createAkt.number;
	const create = async (title: string, body: string) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
				{ f: { title, body }, in: [index] }
			)
		).createAkt.number;
	const setup = await create(
		`Setup ${stamp}`,
		'Before.\n\n## Installing {#install}\n\nRun the **installer** twice.\n\n## Using {#use}\n\nElsewhere.\n'
	);
	const guide = await create(`Guide ${stamp}`, `See [installing](kuria:${setup}#install) first.\n`);

	const page = await signIn(browser, seeded().people.developer);
	await open(page, guide);
	const link = page.locator('dd[data-field="body"]').getByRole('link', { name: 'installing' });

	// Pointed at, it previews the akt and the start of the heading's section.
	await link.hover();
	const preview = page.getByRole('tooltip');
	await expect(preview).toContainText(setup);
	await expect(preview).toContainText(`Setup ${stamp}`);
	await expect(preview).toContainText('Document');
	await expect(preview).toContainText('Run the installer twice.');
	await expect(preview).not.toContainText('Elsewhere.');
	await expect(link).toHaveAccessibleDescription(/Run the installer twice/);
	expect(
		(await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze()).violations,
		'the preview'
	).toEqual([]);
	// It stays while the pointer is over it, and goes when it leaves both.
	await preview.hover();
	await page.waitForTimeout(400);
	await expect(preview).toBeVisible();
	await page.mouse.move(0, 0);
	await expect(preview).toBeHidden();

	// Focused, it shows too, and Escape closes it.
	await link.focus();
	await expect(preview).toContainText('Run the installer twice.');
	await page.keyboard.press('Escape');
	await expect(preview).toBeHidden();
	await expect(page.getByTestId('number')).toHaveText(guide);
});

// r[verify app.trail]
test('following references adds each akt to the trail', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Trail ${stamp}` } }
		)
	).createAkt.number;
	const create = async (title: string, body: string) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($f: JSON, $in: [String!]!) { createAkt(input: { type: "document", fields: $f, in: $in }) { number } }',
				{ f: { title, body }, in: [index] }
			)
		).createAkt.number;
	const update = async (number: string, body: string) => {
		const { revision } = (
			await admin.graphql<{ akt: { revision: number } }>('query($n: String!) { akt(number: $n) { revision } }', {
				n: number
			})
		).akt;
		await admin.graphql('mutation($i: EditAktInput!) { editAkt(input: $i) { revision } }', {
			i: { number, revision, fields: { body } }
		});
	};
	const policy = await create(`Policy ${stamp}`, 'Start.\n');
	const process = await create(`Process ${stamp}`, 'Middle.\n');
	const form = await create(`Form ${stamp}`, 'End.\n');
	await update(policy, `See [the process](kuria:${process}).\n`);
	await update(process, `Fill in [the form](kuria:${form}), under [the policy](kuria:${policy}).\n`);

	const page = await signIn(browser, seeded().people.developer);
	const trail = page.getByRole('navigation', { name: 'You are here' }).getByRole('listitem');
	const body = page.locator('dd[data-field="body"]');
	await open(page, policy);
	// The akt's title under its number.
	await expect(trail).toHaveText(['Document', `Policy ${stamp}${policy}`]);

	// Each reference followed goes on along the trail.
	await body.getByRole('link', { name: 'the process' }).click();
	await expect(page.getByTestId('number')).toHaveText(process);
	await expect(trail).toHaveText(['Document', `Policy ${stamp}${policy}`, `Process ${stamp}${process}`]);
	await body.getByRole('link', { name: 'the form' }).click();
	await expect(page.getByTestId('number')).toHaveText(form);
	await expect(trail).toHaveText([
		'Document',
		`Policy ${stamp}${policy}`,
		`Process ${stamp}${process}`,
		`Form ${stamp}${form}`
	]);

	// An earlier akt goes back to it, with the trail it had.
	await trail.getByRole('link', { name: process }).click();
	await expect(page.getByTestId('number')).toHaveText(process);
	await expect(trail).toHaveText(['Document', `Policy ${stamp}${policy}`, `Process ${stamp}${process}`]);
	// The browser's back shows the page with the trail it had.
	await page.goBack();
	await expect(page.getByTestId('number')).toHaveText(form);
	await expect(trail).toHaveCount(4);
	await page.goForward();
	await expect(trail).toHaveCount(3);

	// Reaching an akt already on the trail cuts it back.
	await body.getByRole('link', { name: 'the policy' }).click();
	await expect(page.getByTestId('number')).toHaveText(policy);
	await expect(trail).toHaveText(['Document', `Policy ${stamp}${policy}`]);
});
