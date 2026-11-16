// A document as sheets: a page break inserted in the editor starting a
// second sheet, the sheets white with dark text in dark mode, and a
// version's PDF opened in a new tab from the versions list, shown where
// it opens rather than downloaded. Each view is checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map((v) => `${v.id} (${v.impact}): ${v.help}`);
}

// r[verify app.sheets]
// r[verify app.pdf]
// r[verify app.accessible]
// r[verify pdf.route]
test('a document is shown and edited as sheets, and a version’s PDF opens in a new tab', async ({ browser }) => {
	test.setTimeout(120_000);
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
	const index = await create('document_index', { title: `Sheets ${stamp}` });
	const guide = await create(
		'document',
		{ title: 'Guide', body: '# First\n\nOn page one, with a [link](https://example.com).\n' },
		[index]
	);

	// In dark mode, the body is a white sheet with dark text.
	const dev = await signIn(browser, seeded().people.developer);
	await dev.emulateMedia({ colorScheme: 'dark' });
	await open(dev, guide);
	const body = dev.locator('dd[data-field="body"]');
	const sheets = body.locator('.sheet');
	await expect(sheets).toHaveCount(1);
	await expect(sheets.first()).toHaveCSS('background-color', 'rgb(255, 255, 255)');
	await expect(sheets.first()).toHaveCSS('color', 'rgb(27, 31, 36)');
	expect(await violations(dev), 'sheets in dark mode').toEqual([]);

	// A page break inserted in the editor, white too, starts a second sheet.
	await dev.getByRole('button', { name: 'Edit Body' }).click();
	const editor = dev.getByRole('textbox', { name: 'Body' });
	await expect(editor).toHaveCSS('background-color', 'rgb(255, 255, 255)');
	await editor.locator('p').last().click();
	await dev.keyboard.press('End');
	await dev.getByRole('toolbar', { name: 'Format Body' }).getByRole('button', { name: 'Page break' }).click();
	await expect(editor.getByRole('separator', { name: 'Page break' })).toBeVisible();
	await dev.keyboard.type('On page two.');
	expect(await violations(dev), 'the editor with a page break').toEqual([]);
	await dev.getByRole('form', { name: 'Edit Body' }).getByRole('button', { name: 'Done' }).click();
	await expect(dev.getByRole('form', { name: 'Edit Body' })).toHaveCount(0);
	await expect(sheets).toHaveCount(2);
	await expect(sheets.nth(0)).toContainText('On page one');
	await expect(sheets.nth(0)).not.toContainText('On page two');
	await expect(sheets.nth(1)).toContainText('On page two.');
	await expect(sheets.nth(1)).toHaveCSS('background-color', 'rgb(255, 255, 255)');

	// Released, the version's PDF opens in a new tab, shown there.
	const rm = await signIn(browser, seeded().people.releaseManager);
	await open(rm, guide);
	const versions = rm.getByRole('region', { name: 'Versions' });
	await versions.getByRole('button', { name: 'Release a version' }).click();
	const release = rm.getByRole('dialog', { name: 'Release a version' });
	await release.getByRole('button', { name: 'Release' }).click();
	await expect(versions.getByRole('link', { name: 'Version 1', exact: true })).toBeVisible();
	const openPdf = versions.getByRole('link', { name: 'Open PDF of version 1' });
	await expect(openPdf).toHaveAttribute('target', '_blank');
	expect(await violations(rm), 'the versions list').toEqual([]);
	const [tab] = await Promise.all([rm.context().waitForEvent('page'), openPdf.click()]);
	// A new tab opened; Chromium shows the PDF in its own viewer there, so
	// what the route answers is checked directly.
	expect(tab).toBeTruthy();
	const shown = await rm.request.get(`/akter/${encodeURIComponent(guide)}/pdf?field=body&version=1`);
	expect(shown.status()).toBe(200);
	expect(shown.headers()['content-disposition']).toMatch(/^inline/);
	const saved = await rm.request.get(`/akter/${encodeURIComponent(guide)}/pdf?field=body&version=1&download`);
	expect(saved.headers()['content-disposition']).toMatch(/^attachment/);
	await tab.close();
});
