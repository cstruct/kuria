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
	const { type } = await json(['types', 'schema', 'backlog_item']);
	expect(type.fields.find((f: { key: string }) => f.key === 'kind').options.map((o: { key: string }) => o.key)).toEqual(
		['feature', 'fix', 'chore']
	);
	expect(type.transitions.find((t: { key: string }) => t.key === 'review').requires).toEqual(['change']);
	const board = await md(['types', 'schema', 'backlog']);
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
	expect((await cli.run(['types', 'schema', 'nope'])).code).toBe(2);
});

// r[verify cli.write]
// r[verify cli.apply]
// r[verify cli.values]
// r[verify cli.file]
test('kuria writes akter: from files and flags, with refusals', async () => {
	// Some forty commands, each signing in with the key.
	test.setTimeout(300_000);
	const admin = await api(seeded().clients.admin);
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const word = `w${Date.now()}`;
	const dir = cli.dir;
	const ok = async (args: string[]) => {
		const ran = await cli.run([...args, '--format', 'json']);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const refused = async (args: string[], code: number) => {
		const ran = await cli.run([...args, '--format', 'json']);
		expect(ran.code, `${args.join(' ')}: ${ran.stdout}`).toBe(code);
		return JSON.parse(ran.stderr).error;
	};

	// Akt types, checked offline and then defined.
	const task = {
		key: 'cli_task',
		name: 'CLI task',
		number: 'CT-{seq}',
		fields: [
			{ key: 'title', name: 'Title', kind: 'text', required: true },
			{ key: 'notes', name: 'Notes', kind: 'text', multiline: true },
			{
				key: 'size',
				name: 'Size',
				kind: 'choice',
				options: [
					{ key: 'small', name: 'Small' },
					{ key: 'large', name: 'Large' }
				]
			},
			{ key: 'owner', name: 'Owner', kind: 'user' },
			{ key: 'due', name: 'Due', kind: 'date' },
			{ key: 'checked', name: 'Checked', kind: 'checkbox' },
			{
				key: 'siblings',
				name: 'Siblings',
				kind: 'text',
				multiline: true,
				draft: { types: ['cli_task'], line: '- {number} {field:title}' }
			}
		],
		states: [
			{ key: 'todo', name: 'To do', initial: true },
			{ key: 'doing', name: 'Doing' },
			{ key: 'done', name: 'Done', final: true }
		],
		transitions: [
			{ key: 'start', name: 'Start', from: ['todo'], to: 'doing' },
			{ key: 'finish', name: 'Finish', from: ['doing'], to: 'done', requires: ['checked'] }
		],
		links: [{ key: 'blocks', name: 'Blocks', inverse: 'Blocked by', to: ['cli_task'] }]
	};
	const plan = {
		key: 'cli_plan',
		name: 'CLI plan',
		number: 'CP-{seq}',
		fields: [{ key: 'title', name: 'Title', kind: 'text', required: true }],
		states: [{ key: 'open', name: 'Open', initial: true }],
		contains: [{ type: 'cli_task' }]
	};
	writeFileSync(join(dir, 'types.json'), JSON.stringify([plan, task]));
	writeFileSync(join(dir, 'bad.json'), JSON.stringify({ ...task, key: 'Bad Key', number: 'X-{nope}' }));
	expect((await ok(['types', 'define', '--check', join(dir, 'types.json')])).valid).toEqual(['cli_plan', 'cli_task']);
	const invalid = await refused(['types', 'define', '--check', join(dir, 'bad.json')], 1);
	expect(invalid.code).toBe('INVALID_AKT_TYPE');
	expect(invalid.message).toContain('number');
	const defined = await ok(['types', 'define', join(dir, 'types.json')]);
	expect(defined.defined.map((d: { key: string }) => d.key)).toEqual(['cli_plan', 'cli_task']);
	expect((await ok(['types', 'export', 'cli_task'])).types[0].links[0].key).toBe('blocks');

	const auditor = `auditor-${word}@x.example`;
	await admin.graphql('mutation($e: String!) { addContact(input: { name: "Auditor", email: $e }) { id } }', {
		e: auditor
	});

	// Created from flags, a multiline text from a file.
	writeFileSync(join(dir, 'notes.md'), 'Why it matters.\n\nWhat done is.\n');
	const p = (await ok(['new', 'cli_plan', '--set', `title=Plan ${word}`, '--reason', 'Planning'])).number;
	const a = (
		await ok([
			'new',
			'cli_task',
			'--in',
			p,
			'--set',
			`title=A ${word}`,
			'--set',
			'size=Small',
			'--set',
			`owner=${auditor}`,
			'--set',
			`notes=@${join(dir, 'notes.md')}`
		])
	).number;
	const b = (await ok(['new', 'cli_task', '--in', p, '--set', `title=B ${word}`])).number;

	// The file round trip: unchanged changes nothing; an edit changes what
	// it edits, a field left out is cleared.
	await ok(['cat', a, '-o', dir]);
	const path = join(dir, `${a}.md`);
	const read = readFileSync(path, 'utf8');
	expect(read).toContain(`owner: ${auditor}`);
	expect(read).toContain('<!-- field: notes -->\nWhy it matters.\n\nWhat done is.\n');
	expect((await ok(['apply', path])).applied[0].changed).toEqual([]);
	const edited = read
		.replace(`title: A ${word}`, `title: A ${word} again\ndue: 2026-12-01\nchecked: true`)
		.replace('size: small\n', '')
		.replace('What done is.', 'What done is, now.');
	writeFileSync(path, edited);
	expect((await ok(['apply', path, '--dry-run'])).applied[0]).toMatchObject({ dryRun: true, revision: 1 });
	const applied = await ok(['apply', path, '--reason', 'Clarified']);
	expect(applied.applied[0].changed.sort()).toEqual(['checked', 'due', 'notes', 'size', 'title']);
	const shown = (await ok(['show', a, '--history'])).akt;
	expect(shown.fields).toMatchObject({ title: `A ${word} again`, due: '2026-12-01', checked: true, owner: auditor });
	expect(shown.fields.size).toBeUndefined();
	expect(shown.history[1]).toMatchObject({ what: 'edited it', reason: 'Clarified' });

	// A stale file is refused, saying what changed since.
	const stale = await refused(['apply', path], 3);
	expect(stale.code).toBe('STALE');
	expect(stale.message).toContain('Title');
	// Read-only keys and unknown fields are refused, saying which.
	await ok(['cat', a, '-o', dir]);
	writeFileSync(path, readFileSync(path, 'utf8').replace('state: todo', 'state: done'));
	expect((await refused(['apply', path], 1)).field).toBe('state');
	await ok(['cat', a, '-o', dir]);
	writeFileSync(path, readFileSync(path, 'utf8').replace('---\n\n', 'colour: red\n---\n\n'));
	expect(await refused(['apply', path], 1)).toMatchObject({ code: 'UNKNOWN_FIELD', field: 'colour' });

	// Edits, transitions, links, order and membership by verbs.
	expect((await ok(['set', b, 'size=large', `owner=${auditor}`])).changed.sort()).toEqual(['owner', 'size']);
	expect((await ok(['do', a, 'Start'])).state).toBe('doing');
	expect((await refused(['do', a, 'nope'], 1)).code).toBe('UNKNOWN_TRANSITION');
	expect((await ok(['do', a, 'finish', '--reason', 'Shipped'])).state).toBe('done');
	expect((await ok(['link', b, 'blocks', a, '--pin'])).pinnedRevision).toBeGreaterThan(1);
	const blocked = (await ok(['show', a])).akt.links;
	expect(blocked).toMatchObject([{ name: 'Blocked by', outgoing: false, akt: { number: b } }]);
	await ok(['unlink', b, 'Blocks', a]);
	expect((await ok(['show', a])).akt.links).toEqual([]);
	expect((await refused(['link', b, 'relates', a], 1)).code).toBe('INVALID_LINK');
	await ok(['members', 'place', b, '--before', a]);
	const order = (await ok(['ls', 'cli_task', '--in', p])).akter.map((x: { number: string }) => x.number);
	expect(order).toEqual([b, a]);
	const q = (await ok(['new', 'cli_plan', '--set', `title=Next ${word}`])).number;
	await ok(['members', 'transfer', b, p, q, '--reason', 'Later']);
	expect((await ok(['show', b])).akt.in).toEqual([q]);
	await ok(['members', 'remove', q, b]);
	await ok(['members', 'add', p, b]);
	expect((await ok(['show', b])).akt.in).toEqual([p]);

	// A field drafted from its recipe: the other tasks in its plan.
	await ok(['draft', b, 'siblings']);
	const siblings = (await ok(['show', b])).akt.fields.siblings;
	expect(siblings).toContain(`${a} A ${word} again`);
	expect(siblings).not.toContain(`${b} B`);
	// A final akt can't be edited, even by a draft.
	expect((await refused(['draft', a, 'siblings'], 1)).code).toBe('REFUSED');

	// A new akt from a file, as another akt's file is a template.
	await ok(['cat', b, '-o', dir]);
	const copy = (await ok(['new', 'cli_task', '--file', join(dir, `${b}.md`), '--set', `title=Copy ${word}`])).number;
	const copied = (await ok(['show', copy])).akt;
	expect(copied).toMatchObject({ state: 'todo', in: [p], fields: { size: 'large', title: `Copy ${word}` } });
});

// r[verify cli.write]
// r[verify bundle.import.history]
test('kuria exports a bundle and imports it into another tenant', async () => {
	test.setTimeout(240_000);
	const { lab, clients } = seeded();
	const cli = new Cli({ KURIA_KEY: clients.admin.key });
	const ok = async (args: string[], env: Record<string, string> = {}) => {
		const ran = await cli.run([...args, '--format', 'json'], env);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const inLab = { KURIA_TENANT: lab };
	const word = `b${Date.now()}`;
	const key = `bundle_note_${Date.now()}`;
	writeFileSync(
		join(cli.dir, 'note.json'),
		JSON.stringify({
			key,
			name: 'Bundle note',
			number: `BN${Date.now()}-{seq}`,
			fields: [{ key: 'title', name: 'Title', kind: 'text', required: true }],
			states: [
				{ key: 'open', name: 'Open', initial: true },
				{ key: 'closed', name: 'Closed', final: true }
			],
			transitions: [{ key: 'close', name: 'Close', from: ['open'], to: 'closed' }]
		})
	);
	await ok(['types', 'define', join(cli.dir, 'note.json')]);
	const first = (await ok(['new', key, '--set', `title=First ${word}`, '--reason', 'Noted'])).number;
	await ok(['new', key, '--set', `title=Second ${word}`]);
	await ok(['do', first, 'close', '--reason', 'Done with it']);

	const dir = join(cli.dir, 'bundle');
	const exported = await ok(['export', '--types', key, '--akter', '-o', dir]);
	expect(exported).toMatchObject({ scope: 'types', aktTypes: 1, akter: 2 });
	expect(existsSync(join(dir, 'bundle.json'))).toBe(true);
	const refusedDir = await cli.run(['export', '--types', key, '-o', dir, '--format', 'json']);
	expect(refusedDir.code).toBe(1);

	// Into the lab, with numbers and history as they were.
	expect(await ok(['import', dir], inLab)).toMatchObject({ scope: 'types', aktTypes: 1, akter: 2 });
	const there = (await ok(['history', first], inLab)).revisions;
	const here = (await ok(['history', first])).revisions;
	expect(there).toEqual(here);
	expect(there.map((r: { what: string }) => r.what)).toEqual(['created it', 'performed Close']);
	const next = (await ok(['new', key, '--set', 'title=Third'], inLab)).number;
	expect(next.endsWith('-3')).toBe(true);

	// Again, it clashes.
	const again = await cli.run(['import', dir, '--format', 'json'], inLab);
	expect(again.code).toBe(1);
	expect(JSON.parse(again.stderr).error.code).toBe('CONFLICT');

	// The type alone defines a new version.
	const typeOnly = join(cli.dir, 'type-only');
	await ok(['export', '--types', key, '-o', typeOnly]);
	await ok(['import', typeOnly], inLab);
	expect((await ok(['types', 'schema', key], inLab)).type.version).toBe(2);

	// The whole tenant exports, but goes only into an empty tenant.
	const whole = join(cli.dir, 'whole');
	expect((await ok(['export', '--whole-tenant', '-o', whole])).scope).toBe('tenant');
	const full = await cli.run(['import', whole, '--format', 'json'], inLab);
	expect(full.code).toBe(1);
	expect(JSON.parse(full.stderr).error.code).toBe('CONFLICT');
});

// r[verify cli.local.convert]
test('kuria converts selected akt types to an akt directory and back', async () => {
	test.setTimeout(240_000);
	const { lab, clients } = seeded();
	const cli = new Cli({ KURIA_KEY: clients.admin.key });
	const ok = async (args: string[], env: Record<string, string> = {}) => {
		const ran = await cli.run([...args, '--format', 'json'], env);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const text = async (args: string[], env: Record<string, string> = {}) => {
		const ran = await cli.run(args, env);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};
	const inLab = { KURIA_TENANT: lab };
	const key = `dir_note_${Date.now()}`;
	writeFileSync(
		join(cli.dir, 'note.json'),
		JSON.stringify({
			key,
			name: 'Directory note',
			number: `DN${Date.now()}-{seq}`,
			fields: [
				{ key: 'title', name: 'Title', kind: 'text', required: true },
				{ key: 'body', name: 'Body', kind: 'text', multiline: true }
			],
			states: [
				{ key: 'open', name: 'Open', initial: true },
				{ key: 'closed', name: 'Closed', final: true }
			],
			transitions: [{ key: 'close', name: 'Close', from: ['open'], to: 'closed' }],
			links: [{ key: 'follows', name: 'Follows', inverse: 'Followed by', to: [key] }]
		})
	);
	await ok(['types', 'define', join(cli.dir, 'note.json')]);
	const first = (await ok(['new', key, '--set', 'title=First'])).number;
	const second = (await ok(['new', key, '--set', 'title=Second'])).number;
	await ok(['do', first, 'close']);
	await ok(['link', second, 'follows', first, '--pin']);

	// To a directory: each akt reads as the server's.
	const dir = join(cli.dir, 'plans');
	expect(await ok(['export', '--types', key, '--to-dir', dir])).toMatchObject({ aktTypes: 1, akter: 2 });
	for (const number of [first, second]) {
		expect(await text(['--dir', dir, 'cat', number])).toBe(await text(['cat', number]));
	}
	expect((await ok(['--dir', dir, 'check'])).consistent).toBe(true);
	const refused = await cli.run(['export', '--types', key, '--to-dir', dir, '--format', 'json']);
	expect(JSON.parse(refused.stderr).error.code).toBe('NOT_EMPTY');

	// Changed locally, then into the lab with its numbers and revisions.
	await ok(['--dir', dir, 'set', second, 'title=Second, changed']);
	expect(await ok(['import', '--from-dir', dir], inLab)).toMatchObject({ scope: 'types', akter: 2 });
	for (const number of [first, second]) {
		expect(await text(['cat', number], inLab)).toBe(await text(['--dir', dir, 'cat', number]));
	}
	const revisions = (await ok(['history', second], inLab)).revisions;
	expect(revisions.length).toBe((await ok(['--dir', dir, 'cat', second])).akter[0].revision);
	const again = await cli.run(['import', '--from-dir', dir, '--format', 'json'], inLab);
	expect(JSON.parse(again.stderr).error.code).toBe('CONFLICT');
});

// r[verify cli.local.attachments]
// r[verify cli.local.convert]
test('an akt directory carries its attachments to another tenant', async () => {
	test.setTimeout(240_000);
	const { lab, clients } = seeded();
	const cli = new Cli({ KURIA_KEY: clients.admin.key });
	const ok = async (args: string[], env: Record<string, string> = {}) => {
		const ran = await cli.run([...args, '--format', 'json'], env);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const text = async (args: string[], env: Record<string, string> = {}) => {
		const ran = await cli.run(args, env);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};
	const inLab = { KURIA_TENANT: lab };
	const key = `dir_image_${Date.now()}`;
	writeFileSync(
		join(cli.dir, 'image.json'),
		JSON.stringify({
			key,
			name: 'Directory image',
			number: `DI${Date.now()}-{seq}`,
			fields: [
				{ key: 'title', name: 'Title', kind: 'text', required: true },
				{ key: 'file', name: 'File', kind: 'attachment', media_types: ['image/svg+xml'] }
			],
			states: [{ key: 'current', name: 'Current', initial: true }],
			presentation: { image: 'file' }
		})
	);
	await ok(['types', 'define', join(cli.dir, 'image.json')]);
	const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>';
	const { id } = await (await api(clients.admin)).upload('logo.svg', 'image/svg+xml', svg);
	const number = (await ok(['new', key, '--set', 'title=Logo', '--set', `file=${id}`])).number;

	// Its file in the directory, named by its path.
	const dir = join(cli.dir, 'pictures');
	expect(await ok(['export', '--types', key, '--to-dir', dir])).toMatchObject({
		akter: 1,
		attachments: 1
	});
	const path = `attachments/${number}/logo.svg`;
	expect(readFileSync(join(dir, path), 'utf8')).toBe(svg);
	expect(await text(['--dir', dir, 'cat', number])).toContain(`file: ${path}\n`);
	expect((await ok(['--dir', dir, 'check'])).consistent).toBe(true);

	// And into the lab, with its bytes.
	expect(await ok(['import', '--from-dir', dir], inLab)).toMatchObject({ akter: 1 });
	const there = await text(['cat', number], inLab);
	expect(there).toContain('name: logo.svg');
	expect(there).not.toContain(id);
});

// r[verify cli.guide]
test('an agent works a plan as the guide says, on the planning example', async () => {
	test.setTimeout(300_000);
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const ok = async (args: string[]) => {
		const ran = await cli.run([...args, '--format', 'json']);
		expect(ran.code, `${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return JSON.parse(ran.stdout);
	};
	const refused = async (args: string[]) => {
		const ran = await cli.run([...args, '--format', 'json']);
		expect(ran.code, `${args.join(' ')}: ${ran.stdout}`).toBe(1);
		return JSON.parse(ran.stderr).error;
	};
	expect((await ok(['guide'])).guide).toContain('## Exit codes');

	// The example's documents, as they are in docs/examples/planning.
	const example = join(import.meta.dirname, '..', '..', 'docs', 'examples', 'planning');
	const files = ['task', 'gap', 'decision', 'plan'].map((key) => join(example, `${key}.json`));
	await ok(['types', 'define', '--check', ...files]);
	expect((await ok(['types', 'define', ...files])).defined.map((d: { key: string }) => d.key)).toEqual([
		'task',
		'gap',
		'decision',
		'plan'
	]);

	const word = `p${Date.now()}`;
	writeFileSync(join(cli.dir, 'goal.md'), 'Plan in Kuria instead of Markdown.\n');
	const plan = (
		await ok(['new', 'plan', '--set', `title=Dogfood ${word}`, '--set', `goal=@${join(cli.dir, 'goal.md')}`])
	).number;
	await ok(['do', plan, 'activate']);
	const first = (await ok(['new', 'task', '--in', plan, '--set', 'title=Build the CLI'])).number;
	const second = (await ok(['new', 'task', '--in', plan, '--set', 'title=Migrate the plans'])).number;
	expect([first, second]).toEqual([`${plan}-T01`, `${plan}-T02`]);
	await ok(['link', first, 'blocks', second]);
	const gap = (await ok(['new', 'gap', '--in', plan, '--set', 'title=Where to deploy'])).number;
	const decision = (await ok(['new', 'decision', '--in', plan, '--set', 'title=Fly.io'])).number;
	expect(gap).toBe(`${plan}-G01`);
	expect(decision).toBe(`${plan}-D01`);

	// The plan's board, and what blocks what.
	const shown = (await ok(['show', plan])).akt;
	const todo = shown.members.columns.find((c: { key: string }) => c.key === 'todo');
	expect(todo.akter.map((a: { number: string }) => a.number)).toEqual([first, second]);
	expect((await ok(['show', second])).akt.links).toMatchObject([{ name: 'Blocked by', akt: { number: first } }]);

	// Work a task through to done, recording progress in its file.
	await ok(['do', first, 'start', '--reason', 'Picked up']);
	await ok(['cat', first, '-o', cli.dir]);
	const path = join(cli.dir, `${first}.md`);
	writeFileSync(path, readFileSync(path, 'utf8') + '\n<!-- field: description -->\nLogin, reads and writes.\n');
	expect((await ok(['apply', path, '--reason', 'Progress'])).applied[0].changed).toEqual(['description']);
	await ok(['do', first, 'Send to review', '--set', 'change=https://git.example/pr/1']);
	expect((await ok(['do', first, 'finish'])).state).toBe('done');

	// A gap closes only with its resolution; a decision needs its reasons.
	expect((await refused(['do', gap, 'close'])).code).toBe('REFUSED');
	await ok(['do', gap, 'close', '--set', 'resolution=Decided: Fly.io, postponed']);
	await ok(['link', gap, 'resolved_by', decision]);
	await refused(['do', decision, 'decide']);
	await ok([
		'do',
		decision,
		'decide',
		'--set',
		'context=Kuria needs a home',
		'--set',
		'decision=Fly.io',
		'--set',
		'why=Postgres, Valkey and storage nearby'
	]);
	const history = (await ok(['history', first])).revisions.map((r: { what: string }) => r.what);
	expect(history).toEqual([
		'created it',
		'changed its links',
		'performed Start',
		'edited it',
		'performed Send to review',
		'performed Finish'
	]);
});
