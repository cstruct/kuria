// The release-management example in a browser: a developer and a release
// manager each signed in through the login page, and the build pipeline
// through the API, from planning to a closed release. It mirrors
// items/server/tests/release_flow.rs, which walks the same release through
// GraphQL alone.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { defineExample } from '../support/example.ts';
import {
	addMembers,
	addTo,
	column,
	memberAction,
	create,
	moved,
	open,
	rendered,
	shown,
	transfer,
	transition
} from '../support/app.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

const digest = (digit: string) => `sha256:${digit.repeat(64)}`;

// r[verify app.akt-types]
// r[verify app.fields]
// r[verify app.transitions]
// r[verify app.errors]
// r[verify app.live]
// r[verify app.accessible]
test('a release from planning to closed', async ({ browser }) => {
	test.setTimeout(600_000);
	const { people, clients } = seeded();

	// The admin defines the example's akt types, as their documents are.
	await defineExample();
	const pipeline = await api(clients.pipeline);
	/** The pipeline registers a build of `component`, and answers its number. */
	async function built(component: string, version: string, digit: string, sbom?: string) {
		const fields: Record<string, unknown> = {
			component,
			change: 'built',
			version,
			source: { repository: `https://git.example/shop/${component}.git`, tag: `v${version}` },
			image: { repository: `registry.example/shop/${component}`, digest: digest(digit), tag: version }
		};
		if (sbom) fields.sbom = sbom;
		const data = await pipeline.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "artifact", fields: $f }) { number } }',
			{ f: fields }
		);
		return data.createAkt.number;
	}
	async function removed(component: string) {
		const data = await pipeline.graphql<{ createAkt: { number: string } }>(
			'mutation($f: JSON) { createAkt(input: { type: "artifact", fields: $f }) { number } }',
			{ f: { component, change: 'removed' } }
		);
		return data.createAkt.number;
	}

	const dev = await signIn(browser, people.developer);
	const rm = await signIn(browser, people.releaseManager);

	/** The developer takes an item through review to done. */
	async function finish(item: string) {
		await open(dev, item);
		await moved(dev, 'Start', 'In progress');
		await moved(dev, 'Send to review', 'In review', {
			'Pull request': { url: `https://git.example/shop/pull/${item}` }
		});
		await moved(dev, 'Finish', 'Done');
	}

	// Planning.
	const release = await create(rm, 'release', {
		Title: 'Spring',
		'Release manager': { choose: 'Me' }
	});
	expect(release).toMatch(/^RELEASE-\d{4}-\d{4}$/);
	const backlog = await create(dev, 'backlog', { Title: 'Spring backlog' });
	await open(rm, backlog);
	await addTo(rm, release);
	const epic = await create(dev, 'epic', { Title: 'Sign-in' }, backlog);
	const item = async (title: string, kind: string, inEpic: boolean) => {
		const number = await create(dev, 'backlog_item', { Title: title, Kind: { choose: kind } }, backlog);
		if (inEpic) await addTo(dev, epic);
		return number;
	};
	const passkeys = await item('Passkeys', 'Feature', true);
	const crash = await item('Crash on save', 'Fix', false);
	const reset = await item('Password reset', 'Feature', true);
	const typo = await item('Typo in the menu', 'Fix', false);

	// Reprioritising the backlog reorders the epic too.
	await open(dev, backlog);
	// Up twice: past the crash fix, then past passkeys.
	await memberAction(dev, reset, 'up');
	await expect.poll(() => column(dev, 'To do')).toEqual([epic, passkeys, reset, crash, typo]);
	await memberAction(dev, reset, 'up');
	await expect.poll(() => column(dev, 'To do')).toEqual([epic, reset, passkeys, crash, typo]);
	await open(dev, epic);
	expect(await column(dev, 'To do')).toEqual([reset, passkeys]);

	// Testing: the developer can't cut a candidate; the release manager can.
	await open(rm, release);
	await moved(rm, 'Start testing', 'Testing');
	await dev.goto(`/types/release_candidate/new?in=${release}`);
	await dev.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(dev.getByRole('alert')).toContainText('aren’t allowed');

	const rc1 = await create(rm, 'release_candidate', {}, release);
	expect(rc1).toBe(`${release}-RC-0001`);
	const experiment = await item('Experimental search', 'Feature', false);
	const sbom = await pipeline.upload(
		'backend.cdx.json',
		'application/vnd.cyclonedx+json',
		JSON.stringify({
			bomFormat: 'CycloneDX',
			specVersion: '1.6',
			metadata: { component: { name: 'backend' } },
			components: [{ name: 'serde' }, { name: 'tokio' }, { name: 'axum' }]
		})
	);
	const backendRc1 = await built('backend', '4.2.0-rc.1', 'a', sbom.id);
	const frontend = await built('frontend', '4.2.0-rc.1', 'b');
	const search = await built('search', '0.1.0', 'c');
	await open(rm, rc1);
	await addMembers(rm, epic, crash, experiment, backendRc1, frontend, search);

	// Work moves across the candidate's board as it is done, live.
	await finish(crash);
	await expect.poll(() => column(rm, 'Done')).toEqual([crash]);

	// The first candidate fails; the second rebuilds the backend and drops
	// the search.
	await moved(rm, 'Reject', 'Rejected');
	const rc2 = await create(rm, 'release_candidate', {}, release);
	const backendRc2 = await built('backend', '4.2.0-rc.2', 'd');
	const noSearch = await removed('search');
	await open(rm, rc2);
	await addMembers(rm, epic, crash, typo, backendRc2, noSearch);
	for (const done of [passkeys, reset, typo]) await finish(done);
	await open(rm, rc2);
	await transition(rm, 'Accept');
	await expect(rm.getByRole('alert')).toContainText('requires');
	await rm.getByRole('button', { name: 'Cancel' }).click();
	await moved(rm, 'Accept', 'Accepted', {
		Build: { url: 'https://ci.example/builds/812', label: 'Build 812' },
		'Test record': { url: 'https://kuria.example/akt/TR-17' }
	});

	// The go comes from the head of product, who isn't a user yet: the
	// release manager names her as a contact and checks the go for her.
	await rm.goto('/contacts');
	await rm.getByRole('form', { name: 'Add a contact' }).getByLabel('Name').fill('Hedda Product');
	await rm.getByRole('form', { name: 'Add a contact' }).getByLabel('Email').fill(people.headOfProduct.email);
	await rm.getByRole('button', { name: 'Add contact' }).click();
	await expect(rm.getByRole('cell', { name: 'Hedda Product', exact: true })).toBeVisible();

	await open(rm, release);
	const releasing = { Version: '4.2.0', 'Released on': '2026-04-01' };
	await transition(rm, 'Release', releasing);
	await expect(rm.getByRole('alert')).toContainText('requires');
	await rm.getByRole('button', { name: 'Cancel' }).click();
	await moved(rm, 'Release', 'Released', {
		...releasing,
		'Go for release': { check: true, for: 'Hedda Product' }
	});
	await expect(rm.locator('dd[data-field="go"]')).toHaveText(
		`Yes, for Hedda Product (checked by ${people.releaseManager.email})`
	);

	// A hotfix after the release, with an item and a new backend.
	const hotfix = await create(rm, 'hotfix', { "Why it can't wait": 'Sessions are lost on restart' }, release);
	expect(hotfix).toBe(`${release}-HOTFIX-0001`);
	const sessions = await item('Lost sessions', 'Fix', false);
	const backendHotfix = await built('backend', '4.2.1', 'e');
	await open(rm, hotfix);
	await addMembers(rm, sessions, backendHotfix);
	await finish(sessions);
	await open(rm, hotfix);
	await moved(rm, 'Deploy', 'Deployed', { Build: { url: 'https://ci.example/builds/815' } });

	// The experiment is replanned into the next backlog in one step.
	const nextBacklog = await create(dev, 'backlog', { Title: 'Summer backlog' });
	await open(dev, experiment);
	await transfer(dev, backlog, nextBacklog);

	// Release notes, drafted from the candidates and the hotfix, then approved.
	const notes = await create(rm, 'release_notes', {}, release);
	await rm.getByRole('button', { name: 'Draft Text' }).click();
	// Shown as the Markdown it is: headings and lists.
	await expect
		.poll(() => rendered(rm, 'text'))
		.toEqual([
			'Epic',
			`${epic} Sign-in ()`,
			'Task',
			`${reset} Password reset (Feature)`,
			`${passkeys} Passkeys (Feature)`,
			`${crash} Crash on save (Fix)`,
			`${typo} Typo in the menu (Fix)`,
			`${sessions} Lost sessions (Fix)`
		]);
	// The artifacts that ship: each component's newest, without the removed
	// search; the first candidate's frontend still ships.
	await rm.getByRole('button', { name: 'Draft Artifacts' }).click();
	await expect
		.poll(() => rendered(rm, 'artifacts'))
		.toEqual([
			`frontend 4.2.0-rc.1: registry.example/shop/frontend:4.2.0-rc.1@${digest('b')}`,
			`backend 4.2.1: registry.example/shop/backend:4.2.1@${digest('e')}`
		]);
	await moved(rm, 'Approve', 'Approved');
	expect(notes).toBe(`${release}-NOTES-1`);

	// The backend's SBOM, summarised, downloads from its artifact.
	await open(rm, backendRc1);
	expect(await shown(rm, 'sbom')).toBe('backend.cdx.json (CycloneDX 1.6, 3 components)');
	const download = rm.waitForEvent('download');
	await rm.getByRole('link', { name: 'backend.cdx.json' }).click();
	expect((await download).suggestedFilename()).toBe('backend.cdx.json');

	// The release board shows its candidates and the hotfix by outcome.
	await open(rm, release);
	expect(await column(rm, 'Accepted')).toEqual([rc2, hotfix]);
	expect(await column(rm, 'Rejected')).toEqual([rc1]);

	// The head of product signs in with a password. Konklav reports a
	// password's email as unverified, so her contact stays unlinked and the
	// release still shows the contact (linking by a verified email is covered
	// by the server's tests).
	const hedda = await signIn(browser, people.headOfProduct);
	await open(hedda, release);
	await open(rm, release);
	await expect(rm.locator('dd[data-field="go"]')).toHaveText(
		`Yes, for Hedda Product (checked by ${people.releaseManager.email})`
	);

	// Closing the release freezes it.
	await moved(rm, 'End maintenance', 'Closed');
	await rm.goto(`/types/hotfix/new?in=${release}`);
	await rm.getByLabel("Why it can't wait", { exact: true }).fill('Too late');
	await rm.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(rm.getByRole('alert')).toContainText('final state');
});
