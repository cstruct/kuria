// The documents example (docs/examples/documents), walked through: a
// document written in the editor with a deep link to another's heading,
// a picture uploaded as an image akt and a fletcher diagram toggled from
// code to its picture, its release refused while the link is broken and
// taken once it isn't,
// read at its version with the link pinned there, compared with the
// living version and downloaded as a PDF. Each view is checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open, pick } from '../support/app.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

/** A one-pixel red PNG. */
const PNG = Buffer.from(
	'89504e470d0a1a0a0000000d4948445200000001000000010802000000907753de0000000c49444154789c63f8cfc0000003010100c9fe92ef0000000049454e44ae426082',
	'hex'
);

async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map((v) => `${v.id} (${v.impact}): ${v.help}`);
}

/** Saves the body in the editor, after `write` has written in it. */
async function editBody(page: Page, write: () => Promise<void>) {
	await page.getByRole('button', { name: 'Edit Body' }).click();
	await page.getByRole('textbox', { name: 'Body' }).click();
	await write();
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Save' }).click();
	await expect(page.getByRole('form', { name: 'Edit Body' })).toHaveCount(0);
}

// r[verify app.editor]
// r[verify app.images]
// r[verify app.diagrams]
// r[verify app.navigation]
// r[verify app.references]
// r[verify app.versions]
// r[verify app.pdf]
// r[verify app.accessible]
// r[verify version.release]
// r[verify version.references]
// r[verify version.read]
// r[verify version.pins]
test('a document is written and released at its version', async ({ browser }) => {
	test.setTimeout(240_000);
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
	const index = await create('document_index', { title: `Handbook ${stamp}` });
	const setup = await create('document', { title: 'Setup', body: '# Install\n\nRun the installer.\n' }, [index]);
	// The guide also names the setup's checks, a heading it hasn't got yet.
	const guide = await create(
		'document',
		{ title: 'Guide', body: `First steps. Mind the [checks](kuria:${setup}#checks).\n` },
		[index]
	);

	// The developer writes the guide, linking to the setup's install heading.
	const dev = await signIn(browser, seeded().people.developer);
	await open(dev, guide);
	await editBody(dev, async () => {
		await dev.keyboard.press('End');
		await dev.keyboard.type(' See ');
		await dev.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Reference' }).click();
		await pick(dev, 'Akt', setup);
		await dev.getByRole('dialog', { name: 'Reference' }).getByLabel('Heading').selectOption({ label: 'Install' });
		await dev.getByRole('dialog', { name: 'Reference' }).getByRole('button', { name: 'Insert' }).click();
		await dev.keyboard.type(' first.');
	});
	const body = dev.locator('dd[data-field="body"]');
	await expect(body.getByRole('link', { name: 'Install' })).toHaveAttribute('href', `/akt/${setup}#install`);

	// A picture, uploaded as an image akt, and a diagram, written as code and
	// shown as its picture.
	const toolbar = dev.getByRole('toolbar', { name: 'Format Body' });
	await editBody(dev, async () => {
		await dev.keyboard.press('Control+End');
		await dev.keyboard.press('Enter');
		await toolbar.getByRole('button', { name: 'Image' }).click();
		const image = dev.getByRole('dialog', { name: 'Image' });
		await image
			.getByLabel('Or upload a file')
			.setInputFiles({ name: `flow-${stamp}.png`, mimeType: 'image/png', buffer: PNG });
		await image.getByLabel('Alt text').fill('The flow at a glance');
		await image.getByRole('button', { name: 'Insert' }).click();
		await expect(image).toBeHidden();
		await toolbar.getByRole('button', { name: 'Diagram' }).click();
		const preview = dev.getByRole('region', { name: 'Diagram preview' });
		await expect(preview.getByRole('img')).toBeVisible();
		expect(await violations(dev), 'a diagram as code').toEqual([]);
		await preview.getByRole('button', { name: 'Show diagram' }).click();
		await expect(dev.getByRole('textbox', { name: 'Body' }).locator('.diagram').getByRole('img')).toBeVisible();
	});
	const figures = body.getByRole('figure');
	await expect(figures).toHaveCount(2);
	await expect(figures.first().getByRole('img', { name: 'The flow at a glance' })).toBeVisible();
	await expect(figures.nth(1).getByRole('img')).toBeVisible();
	expect(await violations(dev), 'a body with a picture and a diagram').toEqual([]);
	// The image akt sits under Show more.
	const sidebar = dev.getByRole('navigation', { name: 'Kuria' });
	await sidebar.getByRole('button', { name: /^Show more/ }).click();
	await expect(
		sidebar.getByRole('list', { name: 'More akt types' }).getByRole('link', { name: /^Image( \d+)?$/ })
	).toBeVisible();

	// The release manager may release documents; a broken link is refused.
	const rm = await signIn(browser, seeded().people.releaseManager);
	await open(rm, guide);
	const versions = rm.getByRole('region', { name: 'Versions' });
	await versions.getByRole('button', { name: 'Release a version' }).click();
	const release = rm.getByRole('dialog', { name: 'Release a version' });
	await release.getByLabel('Label').fill('1.0');
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(release.getByRole('alert')).toContainText(`has no heading`);
	expect(await violations(rm), 'a refused release').toEqual([]);
	await release.getByRole('button', { name: 'Cancel' }).click();

	// The setup gains the heading, and the release is taken.
	const revision = (
		await admin.graphql<{ akt: { revision: number } }>('query($n: String!) { akt(number: $n) { revision } }', {
			n: setup
		})
	).akt.revision;
	await admin.graphql('mutation($i: EditAktInput!) { editAkt(input: $i) { revision } }', {
		i: { number: setup, revision, fields: { body: '# Install\n\nRun the installer.\n\n## Checks\n\nRun them.\n' } }
	});
	await open(rm, guide);
	await versions.getByRole('button', { name: 'Release a version' }).click();
	await release.getByLabel('Label').fill('1.0');
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(versions.getByRole('link', { name: 'Version 1 (1.0)' })).toBeVisible();

	// The guide changes on; version 1 reads as released, its links pinned.
	await open(dev, guide);
	await editBody(dev, async () => {
		// After the diagram that ends the body.
		await dev.getByRole('textbox', { name: 'Body' }).locator('p').last().click();
		await dev.keyboard.type('More soon.');
	});
	await dev.getByRole('region', { name: 'Versions' }).getByRole('link', { name: 'Version 1 (1.0)' }).click();
	await expect(dev.getByRole('status').filter({ hasText: 'not the living version' })).toContainText('Version 1 (1.0)');
	await expect(dev.getByRole('button', { name: 'Edit Body' })).toHaveCount(0);
	await expect(body).not.toContainText('More soon.');
	await expect(body.getByRole('link', { name: 'Install' })).toHaveAttribute('href', `/akt/${setup}#install`);
	await expect(body.getByRole('figure')).toHaveCount(2);
	expect(await violations(dev), 'a version').toEqual([]);
	const [download] = await Promise.all([
		dev.waitForEvent('download'),
		dev.getByRole('link', { name: 'Download PDF' }).click()
	]);
	expect(download.suggestedFilename()).toBe(`${guide}-v1.pdf`);
	const pdf = Buffer.concat(await (await download.createReadStream()).toArray());
	expect(pdf.subarray(0, 4).toString()).toBe('%PDF');
	// The picture is in it, as an image.
	expect(pdf.includes(Buffer.from('/Subtype /Image')) || pdf.includes(Buffer.from('/Subtype/Image'))).toBe(true);

	// Compared with the living version.
	const compare = dev.getByRole('form', { name: 'Compare versions' });
	await compare.getByLabel('From').selectOption({ label: 'Version 1' });
	await compare.getByRole('button', { name: 'Compare' }).click();
	const changes = dev.getByRole('dialog', { name: 'Version 1 → now' });
	await expect(changes.getByLabel('Changes to Body')).toContainText('More soon.');
	expect(await violations(dev), 'the comparison').toEqual([]);
	await dev.keyboard.press('Escape');

	// The clone workflow is walked through with kuria clone/pull/push (PLAN-016-T13).
});
