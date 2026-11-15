// The test-protocols example (docs/examples/test-protocols), walked
// through in the app: a template of three sections copied with two of
// them, started, verdicts given, changed and taken back by two testers in
// its sections and in its list, a test case taken out of the sections in
// the outline, each outcome following, the run's test cases in a tester's
// work and in the list filtered by outcome, and the run finished once its
// outcome allows. Each view is checked by axe.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { moved, open } from '../support/app.ts';
import { defineTestProtocols } from '../support/example.ts';
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

/** The viewer's verdict buttons on `number`. */
const mine = (page: Page, number: string) => page.getByRole('group', { name: `Your verdict on ${number}` });

/** Gives the viewer's verdict on `number`, and waits for it to show. */
async function give(page: Page, number: string, verdict: 'Pass' | 'Fail' | 'Skip') {
	await mine(page, number).getByRole('button', { name: verdict }).click();
	await expect(mine(page, number).getByRole('button', { name: verdict })).toHaveAttribute('aria-pressed', 'true');
}

/** The outcome shown for the member `number`, wherever it is listed. */
const outcomeOf = (page: Page, number: string) =>
	page.getByRole('region', { name: 'Members' }).locator(`[data-number="${number}"] [data-outcome]`).first();

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
	await defineTestProtocols();
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
	const template = await create('test_protocol', { title: 'Checkout regression' });
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
				{ title, steps: 'Do it.', expected: 'It works.', testers: [developer.subject, releaseManager.subject] },
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
					{ title: 'Paying', text: 'Use the **test** card.', collapsible: true, items: [cases[2], cases[3]] },
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
	await give(dev, card, 'Fail');
	await give(dev, invoice, 'Skip');
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
	await expect(mine(dev, passkey).getByRole('button', { name: 'Pass' })).toHaveAttribute('aria-pressed', 'true');

	// The second tester finds the run's test cases in their work.
	const rm = await signIn(browser, releaseManager);
	await rm.goto('/');
	const work = rm.getByRole('list', { name: 'My work: Test case' });
	for (const number of [password, passkey, card, invoice]) await expect(work).toContainText(number);

	// The second tester, in the sections and in the list.
	await open(rm, run);
	await sectionsOf(rm).getByText('Paying').click();
	await give(rm, password, 'Pass');
	await give(rm, passkey, 'Skip');
	await give(rm, card, 'Pass');
	await give(rm, invoice, 'Skip');
	await expect(outcomeOf(rm, password)).toHaveText('Passed');
	await expect(outcomeOf(rm, passkey)).toHaveText('Passed');
	await expect(outcomeOf(rm, invoice)).toHaveText('Skipped');
	await expect(rm.getByRole('list', { name: `Verdicts on ${card}` })).toContainText('Fail');
	await expect(rm.getByTestId('outcome')).toHaveText('Failed');

	// The first tester runs the card case again: it passes, and so does all.
	await dev.reload();
	await sectionsOf(dev).getByText('Paying').click();
	await mine(dev, card).getByRole('button', { name: 'Take back' }).click();
	await expect(outcomeOf(dev, card)).toHaveText('Pending');
	await expect(dev.getByTestId('outcome')).toHaveText('Pending');
	// Pending, the run can't be finished.
	const finish = dev.getByRole('button', { name: 'Finish', exact: true });
	await expect(finish).toBeDisabled();
	await expect(finish).toHaveAccessibleDescription('Finish: only when the outcome is pass, fail or skip');
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
	await expect(dev.locator('dd[data-field="layout"]')).toHaveText(/5 members placed/);
});
