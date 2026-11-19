// The release-management example's test protocols
// (docs/examples/release-management), walked through in the app: a template of three sections copied with two of
// them, started, verdicts reported, edited and deleted by two testers in
// its sections and in its list, each in a dialog that takes a status and a
// description, which a fail or a skip must have, a test case taken out of the sections in
// the outline, each outcome following, the run's test cases in a tester's
// work and in the list filtered by outcome, and the run finished once its
// outcome allows. Each view is checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { moved, open } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22a', 'wcag22aa'];

async function violations(page: Page): Promise<string[]> {
	const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
	return results.violations.map(
		(v) => `${v.id} (${v.impact}): ${v.help}\n  ${v.nodes.map((n) => n.target.join(' ')).join('\n  ')}`
	);
}

/** The open akt's sections. */
const sectionsOf = (page: Page) => page.getByRole('region', { name: 'Sections' });

/** The viewer's own verdict on `number`: Report, or Edit once reported. */
const mine = (page: Page, number: string) => page.getByRole('group', { name: `Your verdict on ${number}` }).first();

/** Everyone's verdicts on `number`, with their descriptions. */
const verdictsOn = (page: Page, number: string) => page.getByRole('list', { name: `Verdicts on ${number}` }).first();

/** The dialog that reports the viewer's verdict on `number`, opened. */
async function reporting(page: Page, number: string) {
	await mine(page, number).getByRole('button', { name: 'Report' }).click();
	return page.getByRole('dialog', { name: `Report on ${number}` });
}

/** The dialog that edits the viewer's verdict on `number`, opened. */
async function editing(page: Page, number: string) {
	await mine(page, number).getByRole('button', { name: 'Edit' }).click();
	return page.getByRole('dialog', { name: `Edit your verdict on ${number}` });
}

/** Reports the viewer's verdict on `number`, with its description if
 * given, and waits for it to show. */
async function give(page: Page, number: string, verdict: 'Pass' | 'Fail' | 'Skip', description?: string) {
	const dialog = await reporting(page, number);
	await dialog.getByRole('radio', { name: verdict }).check();
	if (description) await dialog.getByLabel('Description').fill(description);
	await dialog.getByRole('button', { name: 'Save' }).click();
	await expect(dialog).toBeHidden();
	await expect(mine(page, number).getByRole('button', { name: 'Edit' })).toBeVisible();
	if (description) await expect(verdictsOn(page, number)).toContainText(description);
}

/** The outcome shown for the member `number`, wherever it is listed. */
const outcomeOf = (page: Page, number: string) =>
	page.getByRole('region', { name: 'Contents' }).locator(`[data-number="${number}"] [data-outcome]`).first();

/** The outcome shown on a section's heading or summary. */
const sectionOutcome = (page: Page, title: string) =>
	sectionsOf(page).locator('summary, [role="heading"]').filter({ hasText: title }).locator('[data-outcome]');

// r[verify app.sections]
// r[verify app.verdicts]
// r[verify app.copy]
// r[verify app.fields]
// r[verify app.accessible]
// r[verify akt.copy]
// r[verify akt.outcome]
// r[verify app.transitions]
// r[verify app.filters]
// r[verify app.my-work]
// r[verify akt.transition.outcomes]
// r[verify akt.filter.outcome]
test('a test protocol is copied from its template, run by two testers, and finished', async ({ browser }) => {
	test.setTimeout(180_000);
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const { developer, releaseManager } = seeded().people;
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;

	// The template: three sections of test cases, each run by both testers.
	const template = await create('test_protocol', {
		title: 'Checkout regression'
	});
	const cases: string[] = [];
	for (const title of [
		'Sign in with a password',
		'Sign in with a passkey',
		'Pay by card',
		'Pay by invoice',
		'Screen reader reads the basket'
	]) {
		cases.push(
			await create(
				'test_case',
				{
					title,
					steps: 'Do it.',
					expected: 'It works.',
					testers: [developer.subject, releaseManager.subject]
				},
				[template]
			)
		);
	}
	await admin.graphql('mutation($i: EditAktInput!) { editAkt(input: $i) { revision } }', {
		i: {
			number: template,
			revision: 6,
			fields: {
				layout: [
					{ title: 'Signing in', items: [cases[0], cases[1]] },
					{
						title: 'Paying',
						text: 'Use the **test** card.',
						collapsible: true,
						items: [cases[2], cases[3]]
					},
					{ title: 'Accessibility', items: [cases[4]] }
				]
			}
		}
	});
	await admin.graphql(
		'mutation($n: String!) { performTransition(input: { number: $n, revision: 7, transition: "keep" }) { revision } }',
		{ n: template }
	);

	// A run: the template copied without its accessibility section.
	const dev = await signIn(browser, developer);
	await open(dev, template);
	await expect(dev.getByTestId('state')).toHaveText('Template');
	await dev.getByRole('button', { name: 'Make a copy' }).click();
	const dialog = dev.getByRole('dialog', { name: `Copy ${template}` });
	await expect(dialog.getByRole('checkbox', { name: 'Screen reader reads the basket' })).toBeChecked();
	await dialog.getByRole('checkbox', { name: 'Accessibility' }).uncheck();
	await expect(dialog.getByRole('checkbox', { name: 'Screen reader reads the basket' })).not.toBeChecked();
	expect(await violations(dev), 'the copy dialog').toEqual([]);
	await dialog.getByRole('button', { name: 'Copy', exact: true }).click();
	await expect(dev.getByTestId('number')).not.toHaveText(template);
	const run = (await dev.getByTestId('number').textContent())!.trim();
	const [password, passkey, card, invoice] = [1, 2, 3, 4].map((n) => `${run}-C0${n}`);
	await expect(dev.getByText('Copied from Checkout regression')).toBeVisible();
	await expect(dev.getByTestId('state')).toHaveText('Draft');
	await expect(sectionsOf(dev).getByText('Accessibility')).toHaveCount(0);
	await moved(dev, 'Start', 'Running');

	// The first tester's verdicts; the collapsible section opened first.
	await sectionsOf(dev).getByText('Paying').click();
	await expect(sectionsOf(dev).locator('strong')).toHaveText('test');
	await give(dev, password, 'Pass');
	await give(dev, passkey, 'Pass');
	// A failure says why: without a description it isn't saved.
	const failing = await reporting(dev, card);
	await failing.getByRole('radio', { name: 'Fail' }).check();
	await failing.getByRole('button', { name: 'Save' }).click();
	await expect(failing.getByRole('alert')).toHaveText('Say why it failed.');
	expect(await violations(dev), 'the report dialog').toEqual([]);
	await failing.getByLabel('Description').fill('The card form rejects the test card.');
	await failing.getByRole('button', { name: 'Save' }).click();
	await expect(failing).toBeHidden();
	await expect(verdictsOn(dev, card)).toContainText('The card form rejects the test card.');
	await give(dev, invoice, 'Skip', 'No invoice customer on staging.');
	// The second tester hasn't answered, but a failure fails it already.
	await expect(outcomeOf(dev, password)).toHaveText('Pending');
	await expect(outcomeOf(dev, card)).toHaveText('Failed');
	await expect(sectionOutcome(dev, 'Signing in')).toHaveText('Pending');
	await expect(sectionOutcome(dev, 'Paying')).toHaveText('Failed');
	await expect(dev.getByTestId('outcome')).toHaveText('Failed');
	expect(await violations(dev), 'a run in its sections').toEqual([]);

	// The passkey case taken out of the sections: it is listed after them.
	await dev.getByRole('button', { name: 'Edit Layout' }).click();
	const outline = dev.getByRole('form', { name: 'Edit Layout' });
	expect(await violations(dev), 'the sections outline').toEqual([]);
	await outline.getByRole('button', { name: `Take ${passkey} out of the sections` }).click();
	await expect(outline.getByRole('button', { name: `Place ${passkey}` })).toBeVisible();
	await outline.getByRole('button', { name: 'Save' }).click();
	await expect(dev.getByRole('heading', { name: 'Not in a section' })).toBeVisible();
	await expect(mine(dev, passkey).getByRole('button', { name: 'Edit' })).toBeVisible();
	await expect(verdictsOn(dev, passkey)).toContainText('Pass');

	// The second tester finds the run's test cases in their work.
	const rm = await signIn(browser, releaseManager);
	await rm.goto('/');
	const work = rm.getByRole('list', { name: 'My work: Test case' });
	for (const number of [password, passkey, card, invoice]) await expect(work).toContainText(number);

	// The second tester, in the sections and in the list.
	await open(rm, run);
	await sectionsOf(rm).getByText('Paying').click();
	await give(rm, password, 'Pass');
	await give(rm, passkey, 'Skip', 'No passkey on my phone.');
	await give(rm, card, 'Pass');
	await give(rm, invoice, 'Skip', 'No invoice customer on staging.');
	await expect(outcomeOf(rm, password)).toHaveText('Passed');
	await expect(outcomeOf(rm, passkey)).toHaveText('Passed');
	await expect(outcomeOf(rm, invoice)).toHaveText('Skipped');
	await expect(rm.getByRole('list', { name: `Verdicts on ${card}` })).toContainText('Fail');
	await expect(rm.getByTestId('outcome')).toHaveText('Failed');

	// The first tester runs the card case again: it passes, and so does all.
	await dev.reload();
	await sectionsOf(dev).getByText('Paying').click();
	// Its description edited, then the verdict deleted.
	const edited = await editing(dev, card);
	await expect(edited.getByRole('radio', { name: 'Fail' })).toBeChecked();
	await expect(edited.getByLabel('Description')).toHaveValue('The card form rejects the test card.');
	await edited.getByLabel('Description').fill('Fixed in build 813; running it again.');
	await edited.getByRole('button', { name: 'Save' }).click();
	await expect(verdictsOn(dev, card)).toContainText('Fixed in build 813; running it again.');
	await (await editing(dev, card)).getByRole('button', { name: 'Delete' }).click();
	await expect(mine(dev, card).getByRole('button', { name: 'Report' })).toBeVisible();
	await expect(outcomeOf(dev, card)).toHaveText('Pending');
	await expect(dev.getByTestId('outcome')).toHaveText('Pending');
	// Pending, the run can't be finished.
	const finish = dev.getByRole('button', { name: 'Finish', exact: true });
	await expect(finish).toBeDisabled();
	await expect(finish).toHaveAccessibleDescription(
		'Finish: needs the result to be passed, failed or skipped; it is pending'
	);
	expect(await violations(dev), 'a run that is pending').toEqual([]);

	// The test cases still pending, as the list of test cases filters them.
	await dev.goto('/types/test_case');
	await dev.getByLabel('Outcome').selectOption({ label: 'Pending' });
	await expect(dev).toHaveURL(/outcome=pending/);
	const pending = dev.getByRole('table', { name: 'Test case' });
	await expect(pending.getByRole('link', { name: card, exact: true })).toBeVisible();
	await expect(pending.getByRole('link', { name: password, exact: true })).toHaveCount(0);
	expect(await violations(dev), 'test cases filtered by outcome').toEqual([]);
	await open(dev, run);
	await sectionsOf(dev).getByText('Paying').click();
	await give(dev, card, 'Pass');
	await expect(sectionOutcome(dev, 'Paying')).toHaveText('Passed');
	await expect(dev.getByTestId('outcome')).toHaveText('Passed');

	await expect(finish).toBeEnabled();
	await moved(dev, 'Finish', 'Done', { Conclusion: 'Ready for 4.2.' });
	await expect(dev.getByTestId('outcome')).toHaveText('Passed');

	// The template is as it was: no verdicts, all five cases.
	await open(dev, template);
	await expect(dev.getByTestId('state')).toHaveText('Template');
	await expect(dev.getByTestId('outcome')).toHaveText('Pending');
	await expect(dev.locator('dd[data-field="layout"]')).toHaveText(/5 items placed/);
});
