// A test protocol in the app: its tests in nested sections, one
// collapsible; verdicts given, changed and taken back where the sections
// and the members' list show them; outcomes on the protocol, its sections
// and its tests; the sections edited as an outline; a copy of part of it;
// and each of those views checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map(
		(v) => `${v.id} (${v.impact}): ${v.help}\n  ${v.nodes.map((n) => n.target.join(' ')).join('\n  ')}`
	);
}

const states = [{ key: 'open', name: 'Open', initial: true }];

// r[verify app.sections]
// r[verify app.verdicts]
// r[verify app.copy]
// r[verify app.fields]
// r[verify app.accessible]
test('a protocol lays out its tests in sections, takes verdicts and is copied in part', async ({ browser }) => {
	test.setTimeout(120_000);
	const stamp = Date.now();
	const admin = await api(seeded().clients.admin);
	for (const document of [
		{
			key: 'test_protocol',
			name: 'Test protocol',
			number: `TP${stamp}-{seq}`,
			states,
			fields: [
				{ key: 'title', name: 'Title', kind: 'text', required: true },
				{ key: 'layout', name: 'Layout', kind: 'sections', of: ['test_case'] }
			],
			contains: [{ type: 'test_case' }]
		},
		{
			key: 'test_case',
			name: 'Test case',
			number: '{parent}-T{seq}',
			created_in: ['test_protocol'],
			states,
			fields: [
				{ key: 'title', name: 'Title', kind: 'text', required: true },
				{ key: 'testers', name: 'Testers', kind: 'user', multiple: true },
				{
					key: 'verdict',
					name: 'Verdict',
					kind: 'verdict',
					assignees: 'testers'
				}
			],
			presentation: { list: ['testers'] }
		}
	]) {
		await admin.graphql('mutation($d: JSON!) { defineAktType(document: $d) { key } }', { d: document });
	}
	const { developer, releaseManager } = seeded().people;
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const protocol = await create('test_protocol', { title: 'Release check' });
	const signIn_ = await create(
		'test_case',
		{ title: 'Sign in', testers: [developer.subject, releaseManager.subject] },
		[protocol]
	);
	const signOut = await create('test_case', { title: 'Sign out', testers: [developer.subject] }, [protocol]);
	const reset = await create('test_case', { title: 'Reset', testers: [developer.subject] }, [protocol]);
	await admin.graphql('mutation($i: EditAktInput!) { editAkt(input: $i) { revision } }', {
		i: {
			number: protocol,
			revision: 4,
			fields: {
				layout: [
					{
						title: 'Session',
						text: 'Each way **in** and out.',
						items: [{ title: 'Signing in', collapsible: true, items: [signIn_] }, signOut]
					}
				]
			}
		}
	});

	const page = await signIn(browser, developer);
	await open(page, protocol);
	await expect(page.getByTestId('outcome')).toHaveText('Pending');
	const sections = page.getByRole('region', { name: 'Sections' });
	await expect(sections.getByRole('heading', { name: /Session/ })).toBeVisible();
	await expect(sections.locator('strong')).toHaveText('in');

	// The collapsible section is closed until opened.
	await expect(sections.getByRole('link', { name: 'Sign in' })).toBeHidden();
	await sections.getByText('Signing in').click();
	await expect(sections.getByRole('link', { name: 'Sign in' })).toBeVisible();
	expect(await violations(page), 'a protocol in its sections').toEqual([]);

	// My verdicts, where the sections show the tests.
	const mine = (number: string) => page.getByRole('group', { name: `Your verdict on ${number}` });
	await mine(signIn_).getByRole('button', { name: 'Pass' }).click();
	await expect(mine(signIn_).getByRole('button', { name: 'Pass' })).toHaveAttribute('aria-pressed', 'true');
	const row = (number: string) => sections.locator(`[data-number="${number}"]`);
	// The release manager hasn't answered.
	await expect(row(signIn_).locator('[data-outcome]')).toHaveText('Pending');
	await expect(page.getByRole('list', { name: `Verdicts on ${signIn_}` })).toContainText('Not yet');
	await mine(signOut).getByRole('button', { name: 'Fail' }).click();
	await expect(row(signOut).locator('[data-outcome]')).toHaveText('Failed');
	await expect(page.getByTestId('outcome')).toHaveText('Failed');
	await mine(signOut).getByRole('button', { name: 'Take back' }).click();
	await expect(row(signOut).locator('[data-outcome]')).toHaveText('Pending');
	await mine(signOut).getByRole('button', { name: 'Skip' }).click();
	await expect(row(signOut).locator('[data-outcome]')).toHaveText('Skipped');

	// A test no section names follows, listed, with its verdicts there.
	await expect(page.getByRole('heading', { name: 'Not in a section' })).toBeVisible();
	await mine(reset).getByRole('button', { name: 'Pass' }).click();
	await expect(
		page.getByRole('region', { name: 'Test case' }).locator(`[data-number="${reset}"] [data-outcome]`)
	).toHaveText('Passed');

	// The sections edited as an outline: the test placed, and out of the
	// section again.
	await page.getByRole('button', { name: 'Edit Layout' }).click();
	const outline = page.getByRole('form', { name: 'Edit Layout' });
	expect(await violations(page), 'the sections outline').toEqual([]);
	await outline.getByRole('button', { name: `Place ${reset}` }).click();
	await outline.getByRole('button', { name: `Move ${reset} into the section above` }).click();
	await page.getByRole('button', { name: 'Save' }).click();
	await expect(page.getByRole('heading', { name: 'Not in a section' })).toHaveCount(0);
	await expect(sections.getByRole('link', { name: 'Reset' })).toBeVisible();

	// The test's own page: its testers and verdicts, and mine.
	await open(page, signOut);
	await expect(page.locator('dd[data-field="testers"]')).not.toHaveText('—');
	await expect(mine(signOut).getByRole('button', { name: 'Skip' })).toHaveAttribute('aria-pressed', 'true');
	await expect(page.getByTestId('outcome')).toHaveText('Skipped');

	// A copy with the sign-in tests only, all chosen at first.
	await open(page, protocol);
	await page.getByRole('button', { name: 'Make a copy' }).click();
	const dialog = page.getByRole('dialog', { name: `Copy ${protocol}` });
	await expect(dialog.getByRole('checkbox', { name: 'Sign out' })).toBeChecked();
	expect(await violations(page), 'the copy dialog').toEqual([]);
	await dialog.getByRole('checkbox', { name: 'Sign out' }).uncheck();
	await dialog.getByRole('checkbox', { name: 'Reset' }).uncheck();
	await expect(dialog.getByRole('checkbox', { name: 'Session' })).not.toBeChecked();
	await dialog.getByRole('button', { name: 'Copy', exact: true }).click();
	const copy = `TP${stamp}-2`;
	await expect(page.getByTestId('number')).toHaveText(copy);
	await expect(page.getByText(`Copied from Release check`)).toBeVisible();
	const copied = page.getByRole('region', { name: 'Sections' });
	await expect(copied.getByRole('link', { name: 'Sign out' })).toHaveCount(0);
	await copied.getByText('Signing in').click();
	await expect(copied.getByRole('link', { name: 'Sign in' })).toBeVisible();
	// Its tests have their testers, and no verdicts yet.
	await expect(page.getByTestId('outcome')).toHaveText('Pending');
});
