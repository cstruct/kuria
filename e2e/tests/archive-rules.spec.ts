// The release-management example archiving by its rules: three releases
// closed in turn, the first archived once two newer ones are closed, its
// candidate archived with it; both left out of lists until Show archived,
// each saying which rule archived it; and the release unarchived by hand
// bringing its candidate back and staying, though the rule still holds.
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { Cli } from '../support/cli.ts';
import { defineExample } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify archive.rules.on-change]
// r[verify archive.rules.container]
// r[verify archive.rules.kept]
// r[verify app.archive]
// r[verify cli.archive]
test('old releases archive themselves with their candidates', async ({ browser }) => {
	test.setTimeout(120_000);
	const stamp = Date.now();
	const { people } = seeded();
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const move = async (number: string, transition: string, fields: Record<string, unknown> = {}) => {
		const { akt } = await admin.graphql<{ akt: { revision: number } }>(
			'query($n: String!) { akt(number: $n) { revision } }',
			{ n: number }
		);
		await admin.graphql(
			'mutation($n: String!, $v: Int!, $t: String!, $f: JSON) { performTransition(input: { number: $n, revision: $v, transition: $t, fields: $f }) { revision } }',
			{ n: number, v: akt.revision, t: transition, f: fields }
		);
	};
	const archived = async (number: string) =>
		(
			await admin.graphql<{ akt: { archived: { rule: string | null } | null } }>(
				'query($n: String!) { akt(number: $n) { archived { rule } } }',
				{ n: number }
			)
		).akt.archived;

	// Three releases, each with a candidate, released and closed in turn.
	const releases: string[] = [];
	const candidates: string[] = [];
	for (const n of [1, 2, 3]) {
		const release = await create('release', { title: `Rules ${stamp} ${n}` });
		releases.push(release);
		candidates.push(await create('release_candidate', {}, [release]));
		await move(release, 'test');
		await move(release, 'release', {
			version: `9.${n}.0`,
			released_on: '2026-11-16',
			release_manager: people.releaseManager.subject,
			go: true
		});
		await move(release, 'close');
		if (n === 2) {
			// One newer closed release isn't enough.
			expect(await archived(releases[0])).toBeNull();
		}
	}
	// Two newer ones closed: the first is archived, its candidate with it.
	expect(await archived(releases[0])).toEqual({ rule: 'newer' });
	expect(await archived(candidates[0])).toEqual({ rule: 'with_container' });
	expect(await archived(releases[1])).toBeNull();
	expect(await archived(candidates[1])).toBeNull();

	// In the app: out of the release list until Show archived.
	const rm = await signIn(browser, people.releaseManager);
	await rm.goto('/types/release');
	const list = rm.getByRole('table', { name: 'Release' });
	await expect(list.getByRole('link', { name: releases[1] })).toBeVisible();
	await expect(list.getByRole('link', { name: releases[0] })).toHaveCount(0);
	await rm.getByLabel('Show archived').check();
	const row = list.getByRole('row').filter({ has: rm.getByRole('link', { name: releases[0] }) });
	await expect(row).toContainText('Archived');

	// Each says which rule archived it, and Kuria did.
	await open(rm, releases[0]);
	const banner = rm.getByRole('region', { name: 'Archived' });
	await expect(banner).toContainText('Kuria');
	await expect(banner).toContainText('newer');
	await open(rm, candidates[0]);
	await expect(rm.getByRole('region', { name: 'Archived' })).toContainText('with_container');

	// Unarchived by hand, the release brings its candidate back, and stays.
	await open(rm, releases[0]);
	await rm.getByRole('region', { name: 'Archived' }).getByRole('button', { name: 'Unarchive' }).click();
	const unarchive = rm.getByRole('dialog', { name: `Unarchive ${releases[0]}` });
	await unarchive.getByRole('button', { name: 'Unarchive', exact: true }).click();
	await expect(unarchive).toBeHidden();
	await expect(rm.getByRole('region', { name: 'Archived' })).toHaveCount(0);
	expect(await archived(releases[0])).toBeNull();
	expect(await archived(candidates[0])).toBeNull();
	// Another release closed: the rule would take it, but it is kept.
	const fourth = await create('release', { title: `Rules ${stamp} 4` });
	await move(fourth, 'test');
	await move(fourth, 'release', {
		version: '9.4.0',
		released_on: '2026-11-16',
		release_manager: people.releaseManager.subject,
		go: true
	});
	await move(fourth, 'close');
	expect(await archived(releases[0])).toBeNull();
	expect(await archived(releases[1])).toEqual({ rule: 'newer' });

	// The CLI lists live releases unless asked, and unarchives too.
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const kuria = async (args: string[]) => {
		const ran = await cli.run(args);
		expect(ran.code, `kuria ${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};
	const live = await kuria(['ls', 'release', '--all']);
	expect(live).toContain(fourth);
	expect(live).not.toContain(releases[1]);
	const old = await kuria(['ls', 'release', '--archived', '--all']);
	expect(old).toContain(releases[1]);
	expect(old).not.toContain(fourth);
	await kuria(['unarchive', releases[1], '--reason', 'Still read']);
	expect(await archived(releases[1])).toBeNull();
	expect(await archived(candidates[1])).toBeNull();
});
