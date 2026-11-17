// The documents example (docs/examples/documents), walked through: a
// document written in the editor with a deep link to another's heading,
// a picture uploaded as an image akt and a fletcher diagram toggled from
// code to its picture, its release refused while the link is broken and
// taken once it isn't,
// read at its version with the link pinned there, compared with the
// living version and downloaded as a PDF; then the index cloned with the
// `kuria` binary into a git repository of its own, part of its work pushed
// and released while the rest waits on a branch, merged back and pushed
// in turn. Each view is checked by axe.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open, pick } from '../support/app.ts';
import { Cli } from '../support/cli.ts';
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
	await page.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();
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
// r[verify version.together]
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
		// Other tests define image types too: the example's, if asked.
		const asNew = image.getByLabel('As a new');
		if ((await asNew.count()) > 0) await asNew.selectOption({ label: 'Image' });
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
	await expect(release.getByRole('alert')).toContainText('has no version yet');
	expect(await violations(rm), 'a refused release').toEqual([]);
	await release.getByRole('button', { name: 'Cancel' }).click();

	// The setup gains the heading in its live draft. Never released, it
	// still breaks the link alone: released together, it doesn't.
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
	await expect(release.getByRole('alert')).toContainText('has no version yet');
	const together = release.getByRole('group', { name: 'Release together with' });
	await expect(together.getByRole('checkbox', { name: setup })).toBeChecked();
	expect(await violations(rm), 'a release offered together').toEqual([]);
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(versions.getByRole('link', { name: 'Version 1 (1.0)' })).toBeVisible();
	await open(rm, setup);
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
	// Pinned to the setup's version 1, released with it.
	await expect(body.getByRole('link', { name: 'Install' })).toHaveAttribute('href', `/akt/${setup}?version=1#install`);
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

	// The index cloned into a git repository of its own.
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const kuria = async (args: string[], cwd: string) => {
		const ran = await cli.run(args, {}, cwd);
		expect(ran.code, `kuria ${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};
	const here = mkdtempSync(join(tmpdir(), 'kuria-walkthrough-'));
	await kuria(['docs', 'clone', index, 'handbook'], here);
	const root = join(here, 'handbook');
	const git = (...args: string[]) =>
		execFileSync('git', ['-c', 'commit.gpgsign=false', ...args], {
			cwd: root,
			env: {
				PATH: process.env.PATH ?? '',
				GIT_CONFIG_GLOBAL: '/dev/null',
				GIT_CONFIG_NOSYSTEM: '1',
				GIT_AUTHOR_NAME: 'Dev',
				GIT_AUTHOR_EMAIL: 'dev@example.com',
				GIT_COMMITTER_NAME: 'Dev',
				GIT_COMMITTER_EMAIL: 'dev@example.com'
			}
		}).toString();
	const fileOf = (number: string) => readdirSync(root).find((f) => f.startsWith(`${number}-`))!;
	const append = (number: string, text: string) => {
		const path = join(root, fileOf(number));
		writeFileSync(path, `${readFileSync(path, 'utf8')}\n${text}\n`);
	};
	// The guide's picture is beside it, linked relatively; .kuria/ isn't committed.
	expect(readFileSync(join(root, fileOf(guide)), 'utf8')).toMatch(/!\[The flow at a glance\]\(attachments\/IMG-\d+\.png\)/);
	git('init', '-q', '-b', 'main');
	git('add', '-A');
	git('commit', '-q', '-m', 'Clone');
	expect(git('ls-files')).not.toContain('.kuria');

	// Two commits: one ready, one not.
	append(setup, 'Ready to release.');
	git('commit', '-q', '-am', 'Ready');
	append(guide, 'Not yet.');
	git('commit', '-q', '-am', 'Unready');

	// The unready commit kept on a branch and out of the working copy; the
	// ready part pushed and released.
	git('branch', 'keep');
	git('reset', '-q', '--hard', 'HEAD~1');
	await kuria(['docs', 'add', '.'], root);
	await kuria(['docs', 'push', '-m', 'Ready part'], root);
	await open(rm, setup);
	await versions.getByRole('button', { name: 'Release a version' }).click();
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(versions.getByRole('link', { name: 'Version 2', exact: true })).toBeVisible();
	await expect(rm.locator('dd[data-field="body"]')).toContainText('Ready to release.');

	// The rest merged back and pushed: the guide has it, the release doesn't.
	git('merge', '-q', 'keep');
	await kuria(['docs', 'add', '.'], root);
	await kuria(['docs', 'push', '-m', 'The rest'], root);
	await open(dev, guide);
	await expect(body).toContainText('Not yet.');
	const released = await admin.graphql<{ akt: { fields: ({ markdown?: string } | null)[] } }>(
		'query($n: String!) { akt(number: $n, version: 1) { fields { ... on MarkdownValue { markdown } } } }',
		{ n: guide }
	);
	expect(released.akt.fields.map((f) => f?.markdown).join('')).not.toContain('Not yet.');
	expect(JSON.parse(await kuria(['docs', 'status', '--format', 'json'], root))).toMatchObject({
		staged: [],
		unstaged: [],
		conflicted: []
	});
});

/** The supplier's policy the documents example keeps, a one-page PDF. */
const PDF = readFileSync(
	join(import.meta.dirname, '..', '..', 'items', 'server', 'tests', 'examples', 'supplier-security-policy.pdf')
);

// r[verify app.external]
// r[verify app.versions]
// r[verify version.release-file]
test('an external document is released as its PDF, shown and downloaded', async ({ browser }) => {
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const index = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "document_index", fields: $f, in: [] }) { number } }',
			{ f: { title: `Suppliers ${stamp}` } }
		)
	).createAkt.number;
	const rm = await signIn(browser, seeded().people.releaseManager);

	// Created in the index, with no PDF to give yet.
	await rm.goto(`/types/external_document/new?in=${index}`);
	const form = rm.getByRole('form', { name: 'New External document' });
	await expect(form.getByLabel('PDF')).toHaveCount(0);
	await form.getByLabel('Title').fill(`Supplier policy ${stamp}`);
	await form.getByRole('button', { name: 'Create' }).click();
	await expect(rm.getByTestId('number')).toHaveText(`${index}-X01`);
	const number = `${index}-X01`;
	await expect(rm.locator('dd[data-field="released-pdf"]')).toContainText('No version has been released yet');
	await expect(rm.getByRole('link', { name: 'Download PDF' })).toHaveCount(0);

	// Released with the PDF uploaded.
	const versions = rm.getByRole('region', { name: 'Versions' });
	await versions.getByRole('button', { name: 'Release a version' }).click();
	const release = rm.getByRole('dialog', { name: 'Release a version' });
	await expect(release).toContainText('the PDF you upload');
	await release.getByLabel('PDF').setInputFiles({ name: 'policy.pdf', mimeType: 'application/pdf', buffer: PDF });
	await release.getByLabel('Label').fill('Issue 3');
	await release.getByLabel('Description').fill('As the supplier published it.\nReplaces issue 2.');
	expect(await violations(rm), 'the release dialog').toEqual([]);
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(release).toBeHidden();

	// Shown as the document, downloaded, and opened from the versions.
	const shown = rm.locator('dd[data-field="released-pdf"] iframe');
	await expect(shown).toHaveAttribute('src', `/akter/${number}/pdf?version=1`);
	await expect(shown).toHaveAttribute('title', `Supplier policy ${stamp}, version 1`);
	const served = await rm.request.get(`/akter/${number}/pdf?version=1`);
	expect(served.headers()['content-type']).toBe('application/pdf');
	expect(Buffer.from(await served.body()).equals(PDF)).toBe(true);
	await expect(rm.getByRole('link', { name: 'Download PDF' })).toHaveAttribute(
		'href',
		`/akter/${number}/pdf?version=1&download`
	);
	// r[verify app.versions]: the version's description, as written.
	await expect(versions.getByRole('listitem').first()).toContainText(
		'As the supplier published it. Replaces issue 2.'
	);
	await expect(versions.getByRole('link', { name: 'Open PDF of version 1' })).toHaveAttribute(
		'href',
		`/akter/${number}/pdf?version=1`
	);
	expect(await violations(rm), 'the released document').toEqual([]);
	// Its PDF field is the release's to set.
	await expect(rm.getByRole('button', { name: 'Edit PDF' })).toHaveCount(0);
});
