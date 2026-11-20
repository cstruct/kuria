// The release-management example's problem reports and CAPAs
// (docs/examples/release-management), walked through in the app: a
// developer reports a problem and classifies it, the risk record asked for
// once the screening says it is needed; QA/RA escalates it to a CAPA with
// New CAPA, carrying its title, confirms it, plans it, its root cause asked
// for once it is corrective, and starts implementing it, the effectiveness
// check locked from then on; an action due in the past shows as overdue,
// and verifying waits for it, saying so, until the task implementing it is
// done and the action finished; then QA/RA closes the CAPA.
import { expect, test, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { create, moved, open, pick } from '../support/app.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

/** The open akt's field `key`, as its page shows it. */
const field = (page: Page, key: string) => page.locator(`dd[data-field="${key}"]`);

// r[verify app.transitions]
// r[verify app.fields]
// r[verify akt.transition.requires-when]
// r[verify akt.transition.members]
// r[verify akt.transition.links]
// r[verify akt.transition.waits]
// r[verify akt.field.locked]
// r[verify akt.field.due]
// r[verify link.create.carry]
test('a problem report is escalated to a CAPA, worked through its action, and closed', async ({ browser }) => {
	test.setTimeout(180_000);
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const { developer, quality } = seeded().people;
	const backlog = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation { createAkt(input: { type: "backlog", fields: { title: "Shop backlog" } }) { number } }'
		)
	).createAkt.number;

	// The developer reports it, and classifies it: uncertain, so the risk
	// record is asked for as soon as that is chosen.
	const dev = await signIn(browser, developer);
	const report = await create(
		dev,
		'problem_report',
		{ Title: 'Released images lack their SBOM' },
		backlog
	);
	await dev.getByRole('main').getByRole('button', { name: 'Classify', exact: true }).click();
	const classify = dev.getByRole('dialog', { name: 'Classify' });
	await expect(classify.getByRole('group', { name: /^Risk record/ })).toHaveCount(0);
	await classify
		.getByLabel('Could it lead to a hazardous situation?', { exact: true })
		.selectOption({ label: 'Uncertain' });
	await classify.getByLabel('Severity', { exact: true }).selectOption({ label: 'Major' });
	await classify.getByLabel('Priority', { exact: true }).selectOption({ label: 'P1' });
	const risk = classify.getByRole('group', { name: /^Risk record/ });
	await expect(risk).toBeVisible();
	await risk.getByLabel('URL').fill('https://qms.shop.example/risks/RA-31');
	await classify.getByRole('button', { name: 'Classify', exact: true }).click();
	await expect(dev.getByTestId('state')).toHaveText('Classified');

	// QA/RA escalates it: New CAPA starts with its title, linked to it.
	const qa = await signIn(browser, quality);
	await open(qa, report);
	await qa.getByRole('link', { name: /New CAPA/ }).click();
	await expect(qa.getByTestId('linking')).toContainText(report);
	await expect(qa.getByLabel('Title', { exact: true })).toHaveValue('Released images lack their SBOM');
	await qa.getByLabel('Problem statement', { exact: true }).fill('Two released images have no SBOM.');
	await qa.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(qa).toHaveURL(/\/akt\/CAPA-/);
	const capa = (await qa.getByTestId('number').textContent())!.trim();
	await expect(qa.getByRole('list', { name: 'Originated from', exact: true })).toContainText(report);

	await moved(qa, 'Confirm CAPA', 'Investigating', {
		Severity: { choose: 'Major' },
		'Impact and risk': 'Releases cannot be shown to match their dependencies.',
		'Risk record': { url: 'https://qms.shop.example/risks/RA-31' }
	});
	// Corrective, so its root cause is asked for too.
	await qa.getByRole('main').getByRole('button', { name: 'Plan actions', exact: true }).click();
	const plan = qa.getByRole('dialog', { name: 'Plan actions' });
	await expect(plan.getByLabel('Root cause', { exact: true })).toHaveCount(0);
	await plan.getByRole('group', { name: /^Actions needed/ }).getByRole('checkbox', { name: 'Corrective action' }).check();
	await plan.getByLabel('Root cause', { exact: true }).fill('No one posts deployment records.');
	await plan.getByLabel('Affected users, systems and versions', { exact: true }).fill('Releases 2026.1 and 2026.2.');
	await plan.getByLabel('Vigilance reporting', { exact: true }).selectOption({ label: 'Not required' });
	await plan.getByLabel('Customer communication', { exact: true }).selectOption({ label: 'Not required' });
	await plan.getByRole('button', { name: 'Plan actions', exact: true }).click();
	await expect(qa.getByTestId('state')).toHaveText('Planning');
	await moved(qa, 'Start implementing', 'Implementing', {
		'Effectiveness check': 'The next two releases have their records.',
		'Acceptance criteria': 'No release without image digests and SBOMs.'
	});
	// Fixed from now on: changing it needs a reason.
	await expect(qa.getByText('Effectiveness check').getByLabel('locked')).toBeVisible();

	// An action due in the past reads as overdue on the CAPA's board.
	const action = await create(
		qa,
		'capa_action',
		{ Title: 'Post deployment records', Kind: { choose: 'Corrective action' }, Due: '2020-01-01' },
		capa
	);
	await open(qa, capa);
	await expect(qa.locator(`[data-number="${action}"]`)).toContainText('2020-01-01, overdue');
	await expect(qa.getByRole('main').getByRole('button', { name: 'Verify effectiveness', exact: true })).toBeDisabled();
	await expect(qa.getByTestId('waits-verify')).toContainText(`waits for ${action}`);

	// The action is implemented by a task, made from it.
	await open(qa, action);
	await moved(qa, 'Start', 'In progress');
	await expect(field(qa, 'due')).toContainText('overdue');
	// QA/RA may not create tasks, so it isn't offered New Task: a task the
	// developers made is linked as what implements it.
	await expect(qa.getByRole('link', { name: /New Task/ })).toHaveCount(0);
	const task = (
		await admin.graphql<{ createAkt: { number: string } }>(
			'mutation($in: [String!]!) { createAkt(input: { type: "backlog_item", in: $in, fields: { title: "Post records from the pipeline" } }) { number } }',
			{ in: [backlog] }
		)
	).createAkt.number;
	await qa.getByRole('button', { name: 'Add link' }).click();
	await pick(qa, 'Implemented by…', task);
	await expect(qa.getByRole('list', { name: 'Implemented by', exact: true })).toContainText(task);
	await open(qa, action);
	await qa.getByRole('main').getByRole('button', { name: 'Finish', exact: true }).waitFor();
	await expect(qa.getByTestId('waits-finish')).toContainText(`waits for ${task}`);
	for (const [transition, fields] of [
		['start', {}],
		['review', { change: { url: 'https://git.shop.example/shop/pull/42' } }],
		['finish', {}]
	] as const) {
		const { akt } = await admin.graphql<{ akt: { revision: number } }>(
			'query($n: String!) { akt(number: $n) { revision } }',
			{ n: task }
		);
		await admin.graphql('mutation($i: PerformTransitionInput!) { performTransition(input: $i) { number } }', {
			i: { number: task, revision: akt.revision, transition, fields }
		});
	}
	await qa.reload();
	await moved(qa, 'Finish', 'Done', { Evidence: { url: 'https://git.shop.example/shop/pull/42' } });

	// Nothing to wait for now: verified, and closed by QA/RA.
	await open(qa, capa);
	await moved(qa, 'Verify effectiveness', 'Verifying');
	await qa.getByRole('main').getByRole('button', { name: 'Close', exact: true }).click();
	const close = qa.getByRole('dialog', { name: 'Close' });
	await expect(close).toContainText('where it stays unless it is reopened');
	await close.getByLabel('Effectiveness results', { exact: true }).fill('Both releases have every record.');
	await close.getByLabel('Adverse impact check', { exact: true }).fill('None: the pipeline only adds records.');
	await close.getByLabel('Closure outcome', { exact: true }).selectOption({ label: 'Completed' });
	// The dialog's own close button is named Close too: the transition is its submit.
	await close.locator('button[type="submit"]').click();
	await expect(qa.getByTestId('state')).toHaveText('Closed');
	await expect(qa.getByRole('main').getByRole('button', { name: 'Reopen', exact: true })).toBeVisible();
});
