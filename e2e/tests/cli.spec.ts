import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { Cli, finished, waitForLine } from '../support/cli.ts';
import { defineExample } from '../support/example.ts';
import { newPage } from '../support/people.ts';
import { KURIA, seeded } from '../support/stack.ts';

// r[verify cli.login]
// r[verify cli.secret]
// r[verify cli.dpop]
// r[verify cli.discovery]
// r[verify cli.profile]
test('kuria login signs in by a link, refreshes, and logs out', async ({ browser }) => {
	const { tenant, people } = seeded();
	const person = people.developer;
	const cli = new Cli();

	// The tenant is Kuria's default one: only the URL is named.
	const login = cli.start(['login', '--no-browser']);
	const done = finished(login);
	const link = await waitForLine(login, /^https?:\/\//);
	expect(link).toContain('client_id=kuria-cli');
	expect(link).toContain('dpop_jkt=');

	const page = await newPage(browser);
	await page.goto(link);
	await page.getByLabel('Email').fill(person.email);
	await page.getByLabel('Password').fill(person.password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'The CLI is signed in' })).toBeVisible();

	const signedIn = await done;
	expect(signedIn.code, signedIn.stderr).toBe(0);
	expect(signedIn.stdout).toContain(`Signed in to ${KURIA}`);

	// The profile names Kuria and the tenant; its secrets are in the store.
	const config = readFileSync(join(cli.dir, 'config.toml'), 'utf8');
	expect(config).toContain(`url = "${KURIA}"`);
	expect(config).toContain(`tenant = "${tenant}"`);
	const secretsFile = join(cli.dir, 'secrets', 'default.json');
	const secrets = JSON.parse(readFileSync(secretsFile, 'utf8'));
	expect(signedIn.stdout + signedIn.stderr).not.toContain(secrets.refresh_token);
	expect(signedIn.stdout + signedIn.stderr).not.toContain(secrets.access_token);

	// Without KURIA_URL, the profile says where.
	const whoami = await cli.run(['whoami', '--format', 'json'], { KURIA_URL: '' });
	expect(whoami.code, whoami.stderr).toBe(0);
	expect(JSON.parse(whoami.stdout)).toMatchObject({
		url: KURIA,
		tenant,
		subject: person.subject,
		signedInWith: 'link',
		profile: 'default'
	});

	// An access token about to expire is refreshed, and the refresh token
	// rotated.
	writeFileSync(secretsFile, JSON.stringify({ ...secrets, expires_at: 0 }));
	const refreshed = await cli.run(['whoami']);
	expect(refreshed.code, refreshed.stderr).toBe(0);
	const rotated = JSON.parse(readFileSync(secretsFile, 'utf8'));
	expect(rotated.expires_at).toBeGreaterThan(Date.now() / 1000);
	expect(rotated.access_token).not.toBe(secrets.access_token);
	expect(rotated.dpop_key).toBe(secrets.dpop_key);

	const logout = await cli.run(['logout']);
	expect(logout.code, logout.stderr).toBe(0);
	expect(existsSync(secretsFile)).toBe(false);
	const after = await cli.run(['whoami', '--format', 'json']);
	expect(after.code).toBe(4);
	expect(JSON.parse(after.stderr).error.code).toBe('NOT_SIGNED_IN');
	// The session ended at Konklav too: its refresh token is refused.
	writeFileSync(secretsFile, JSON.stringify({ ...rotated, expires_at: 0 }));
	const revoked = await cli.run(['whoami', '--format', 'json']);
	expect(revoked.code).toBe(4);
});

// r[verify cli.secret-key]
// r[verify cli.errors]
test('with KURIA_KEY, kuria signs in as software and saves nothing', async () => {
	const { tenant, clients } = seeded();
	const cli = new Cli({ KURIA_KEY: clients.admin.key });
	const whoami = await cli.run(['whoami', '--format', 'json']);
	expect(whoami.code, whoami.stderr).toBe(0);
	expect(JSON.parse(whoami.stdout)).toMatchObject({ url: KURIA, tenant, signedInWith: 'secret-key', profile: null });
	expect(existsSync(join(cli.dir, 'secrets'))).toBe(false);
	expect(existsSync(join(cli.dir, 'config.toml'))).toBe(false);

	const wrong = await cli.run(['whoami', '--format', 'json'], { KURIA_KEY: `${clients.admin.key}x` });
	expect(wrong.code).toBe(4);
	expect(JSON.parse(wrong.stderr).error).toMatchObject({ code: expect.any(String), message: expect.any(String) });

	const nowhere = await cli.run(['whoami'], { KURIA_URL: 'http://127.0.0.1:9' });
	expect(nowhere.code).toBe(5);
});

// r[verify cli.read]
// r[verify cli.file]
// r[verify cli.format]
// r[verify cli.values]
test('kuria reads akt types and akter like files', async () => {
	await defineExample();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const word = `cli${Date.now()}`;
	const release = await create('release', { title: `Release ${word}` });
	const backlog = await create('backlog', { title: `Backlog ${word}` }, [release]);
	const fix = await create(
		'backlog_item',
		{ title: `Fix ${word}`, kind: 'fix', description: 'First line\n\nSecond: line' },
		[backlog]
	);
	const feature = await create('backlog_item', { title: `Feature ${word}`, kind: 'feature' }, [backlog]);
	await admin.graphql(
		'mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: "start", reason: "Picked up" }) { number } }',
		{ n: fix }
	);

	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const json = async (args: string[]) => {
		const ran = await cli.run([...args, '--format', 'json']);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const md = async (args: string[]) => {
		const ran = await cli.run(args);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};

	// What the tenant has.
	const { types } = await json(['types']);
	expect(types.find((t: { key: string }) => t.key === 'backlog_item').akterCount).toBeGreaterThanOrEqual(2);
	const { type } = await json(['schema', 'backlog_item']);
	expect(type.fields.find((f: { key: string }) => f.key === 'kind').options.map((o: { key: string }) => o.key)).toEqual(
		['feature', 'fix', 'chore']
	);
	expect(type.transitions.find((t: { key: string }) => t.key === 'review').requires).toEqual(['change']);
	const board = await md(['schema', 'backlog']);
	expect(board).toContain('## Board');
	expect(board).toContain('`backlog_item`, exclusive');

	// Lists, filtered and paged.
	const fixes = await json(['ls', 'backlog_item', '--in', backlog, '--where', 'kind=Fix']);
	expect(fixes.akter.map((a: { number: string }) => a.number)).toEqual([fix]);
	expect(fixes.akter[0]).toMatchObject({ state: 'in_progress', fields: { kind: 'fix' } });
	const table = await md(['ls', 'backlog_item', '--in', backlog]);
	expect(table).toContain('| Number | Title | State | Kind | Assignee |');
	expect(table).toContain(`| ${fix} | Fix ${word} | In progress | Fix |  |`);
	const first = await json(['ls', 'backlog_item', '--in', backlog, '--limit', '1']);
	expect(first.akter).toHaveLength(1);
	expect(first.hasNextPage).toBe(true);
	const next = await json(['ls', 'backlog_item', '--in', backlog, '--limit', '1', '--after', first.endCursor]);
	expect(next.akter.map((a: { number: string }) => a.number)).not.toContain(first.akter[0].number);
	expect((await json(['ls', 'backlog_item', '--in', backlog, '--limit', '1', '--all'])).akter).toHaveLength(2);
	expect((await json(['ls', 'backlog_item', '--state', 'In progress', '--search', word])).akter).toHaveLength(1);
	const found = await json(['find', word]);
	expect(found.akter.map((a: { number: string }) => a.number).sort()).toEqual([release, backlog, fix, feature].sort());

	// One akt, as a file, shown, and its history.
	const file = await md(['cat', fix]);
	expect(file).toContain(
		`---\nnumber: ${fix}\ntype: backlog_item\nrevision: 2\nstate: in_progress\nin: [${backlog}]\ntitle: Fix ${word}\nkind: fix\n---\n`
	);
	expect(file).toContain('<!-- field: description -->\nFirst line\n\nSecond: line\n');
	const shown = await md(['show', fix, '--history']);
	expect(shown).toContain(`# ${fix} · Fix ${word}`);
	expect(shown).toContain('`review` Send to review → In review, requires Pull request');
	expect(shown).toContain('performed Start');
	expect(shown).toContain('Picked up');
	const container = await json(['show', backlog]);
	const todo = container.akt.members.columns.find((c: { key: string }) => c.key === 'todo');
	expect(todo.akter.map((a: { number: string }) => a.number)).toEqual([feature]);
	const history = await json(['history', fix]);
	expect(history.revisions.map((r: { what: string }) => r.what)).toEqual(['created it', 'performed Start']);
	await md(['users']);
	await md(['contacts']);

	// Refusals say why, with their exit codes.
	const unknown = await cli.run(['cat', 'NOPE-404', '--format', 'json']);
	expect(unknown.code).toBe(2);
	expect(JSON.parse(unknown.stderr).error.code).toBe('NOT_FOUND');
	const option = await cli.run(['ls', 'backlog_item', '--where', 'kind=nope', '--format', 'json']);
	expect(option.code).toBe(1);
	expect(JSON.parse(option.stderr).error).toMatchObject({ code: 'BAD_REQUEST', field: 'kind' });
	expect((await cli.run(['schema', 'nope'])).code).toBe(2);
});
