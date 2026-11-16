// Archiving in the app: a release candidate archived from its page with a
// reason, gone from its type's list, from search and from its release's
// board, which says how many members are archived; Show archived bringing
// it back, marked; its page saying it is archived, by whom and why, and
// offering no edit; a developer refused; Unarchive making it live again.
// The archived page and a list showing archived akter are checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { column, open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map((v) => `${v.id} (${v.impact}): ${v.help}`);
}

// r[verify app.archive]
// r[verify app.accessible]
test('an akt is archived with a reason, left out unless asked for, and unarchived', async ({ browser }) => {
	test.setTimeout(120_000);
	const stamp = Date.now();
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const release = await create('release', { title: `Archiving ${stamp}` });
	const kept = await create('release_candidate', {}, [release]);
	const gone = await create('release_candidate', {}, [release]);

	// The release manager archives a candidate from its menu, saying why.
	const rm = await signIn(browser, seeded().people.releaseManager);
	await open(rm, gone);
	await rm.getByRole('button', { name: `More actions for ${gone}` }).click();
	await rm.getByRole('menuitem', { name: 'Archive…' }).click();
	const archive = rm.getByRole('dialog', { name: `Archive ${gone}` });
	await archive.getByLabel('Reason').fill('Superseded by the next build');
	await archive.getByRole('button', { name: 'Archive', exact: true }).click();
	await expect(archive).toBeHidden();

	// Its page says it is archived, when, by whom and why, and offers no edit.
	const banner = rm.getByRole('region', { name: 'Archived' });
	await expect(banner.getByTestId('archived')).toHaveText(/^Archived .+ by .+/);
	await expect(banner).toContainText('Superseded by the next build');
	await expect(banner.getByRole('button', { name: 'Unarchive' })).toBeVisible();
	const main = rm.getByRole('main');
	await expect(main.getByRole('button', { name: /^Edit / })).toHaveCount(0);
	await expect(main.getByRole('button', { name: 'Reject', exact: true })).toHaveCount(0);
	await expect(main.getByRole('button', { name: 'Accept', exact: true })).toHaveCount(0);
	await expect(main.getByRole('button', { name: `More actions for ${gone}` })).toHaveCount(0);
	await expect(main.getByRole('combobox', { name: 'Add to a container' })).toHaveCount(0);
	await expect(main.getByRole('button', { name: `Remove from ${release}` })).toHaveCount(0);
	await expect(rm.getByRole('list', { name: 'Revisions' })).toContainText('archived it');
	expect(await violations(rm), 'an archived akt’s page').toEqual([]);

	// Its release's board shows the live candidate only, and says one is archived.
	await open(rm, release);
	const members = rm.getByRole('region', { name: 'Members', exact: true });
	await expect(members.getByTestId('archived-members')).toHaveText('1 archived');
	expect(await column(rm, 'Testing')).toEqual([kept]);
	// Show archived adds it, marked, and is kept in the address.
	await members.getByLabel('Show archived').check();
	await expect(rm).toHaveURL(/[?&]archived=all/);
	await expect.poll(() => column(rm, 'Testing')).toEqual(expect.arrayContaining([kept, gone]));
	await expect(members.locator(`[data-number="${gone}"]`)).toContainText('Archived');
	await expect(members.locator(`[data-number="${kept}"]`)).not.toContainText('Archived');

	// Its type's list leaves it out.
	await rm.goto('/types/release_candidate');
	const list = rm.getByRole('table', { name: 'Release candidate' });
	await expect(list).toBeVisible();
	await expect(list.getByRole('link', { name: gone })).toHaveCount(0);

	// So does search, until it is asked for archived akter too.
	await rm.keyboard.press('/');
	const search = rm.getByRole('dialog', { name: 'Search' });
	await search.getByLabel('Search by number or title').fill(gone);
	await expect(search).toContainText(`Nothing matches “${gone}”`);
	await search.getByLabel('Show archived').check();
	await expect(search.getByRole('option', { name: new RegExp(`${gone}.*Archived`) })).toBeVisible();
	await rm.keyboard.press('Escape');
	await expect(search).toBeHidden();

	// Show archived brings it back to the list, marked.
	await rm.getByLabel('Show archived').check();
	await expect(rm).toHaveURL(/[?&]archived=all/);
	const row = list.getByRole('row').filter({ has: rm.getByRole('link', { name: gone }) });
	await expect(row).toContainText('Archived');
	expect(await violations(rm), 'a list showing archived akter').toEqual([]);

	// A developer, who may read it but not archive, is told so.
	const dev = await signIn(browser, seeded().people.developer);
	await open(dev, kept);
	await dev.getByRole('button', { name: `More actions for ${kept}` }).click();
	await dev.getByRole('menuitem', { name: 'Archive…' }).click();
	const refused = dev.getByRole('dialog', { name: `Archive ${kept}` });
	await refused.getByRole('button', { name: 'Archive', exact: true }).click();
	await expect(refused.getByRole('alert')).toContainText('aren’t allowed');

	// Unarchived, it is live again: editable, in the list and on the board.
	await open(rm, gone);
	await rm.getByRole('region', { name: 'Archived' }).getByRole('button', { name: 'Unarchive' }).click();
	const unarchive = rm.getByRole('dialog', { name: `Unarchive ${gone}` });
	await unarchive.getByRole('button', { name: 'Unarchive', exact: true }).click();
	await expect(unarchive).toBeHidden();
	await expect(rm.getByRole('region', { name: 'Archived' })).toHaveCount(0);
	await expect(rm.getByRole('main').getByRole('button', { name: 'Reject', exact: true })).toBeVisible();
	await expect(rm.getByRole('list', { name: 'Revisions' })).toContainText('unarchived it');
	await rm.goto('/types/release_candidate');
	await expect(rm.getByRole('table', { name: 'Release candidate' }).getByRole('link', { name: gone })).toBeVisible();
	await open(rm, release);
	await expect(rm.getByTestId('archived-members')).toHaveCount(0);
	await expect.poll(() => column(rm, 'Testing')).toEqual(expect.arrayContaining([kept, gone]));
});
