/** Local mode's parity with a server: one sequence of commands, run against
 * the end-to-end Kuria and against a new akt directory, answers and akter
 * compared step by step (`cli.local.rules`, `cli.local.revision`). */
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { Cli, type Ran } from '../support/cli.ts';
import { seeded } from '../support/stack.ts';

/** What a run answered, as comparable across a server and a directory. */
interface Said {
	code: number | null;
	error?: string;
	markdown?: string;
}

/** The answer without what only local mode adds: the reason to commit with. */
function said(ran: Ran): Said {
	if (ran.code !== 0) {
		// A refusal ends with its code; a usage error is said as it is.
		const code = ran.stderr.trim().match(/\(([A-Z_]+)\)$/);
		return { code: ran.code, error: code ? code[1] : ran.stderr.trim() };
	}
	return { code: 0, markdown: ran.stdout.replace(/\n\nCommit with: .*$/s, '').trim() };
}

// r[verify cli.local.rules]
// r[verify cli.local.revision]
test('local mode answers a sequence of commands as the server does', async () => {
	test.setTimeout(300_000);
	const stamp = Date.now();
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const dir = join(cli.dir, 'plans');
	const box = `parity_box_${stamp}`;
	const item = `parity_item_${stamp}`;
	writeFileSync(
		join(cli.dir, 'types.json'),
		JSON.stringify([
			{
				key: box,
				name: 'Parity box',
				number: `PB${stamp}-{seq}`,
				fields: [{ key: 'title', name: 'Title', kind: 'text', required: true }],
				states: [
					{ key: 'open', name: 'Open', initial: true },
					{ key: 'paused', name: 'Paused' },
					{ key: 'shut', name: 'Shut', final: true }
				],
				transitions: [
					{ key: 'pause', name: 'Pause', from: ['open'], to: 'paused' },
					{ key: 'shut', name: 'Shut', from: ['open', 'paused'], to: 'shut' }
				],
				contains: [{ type: item, exclusive: true, while: ['open'] }]
			},
			{
				key: item,
				name: 'Parity item',
				number: '{parent}-I{seq}',
				created_in: [box],
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
					{ key: 'due', name: 'Due', kind: 'date' },
					{ key: 'checked', name: 'Checked', kind: 'checkbox' }
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
				links: [{ key: 'blocks', name: 'Blocks', inverse: 'Blocked by', to: [item] }]
			}
		])
	);
	writeFileSync(join(cli.dir, 'notes.md'), 'Why it matters.\n\nWhat done is.\n');
	expect((await cli.run(['init', '--dir', dir])).code).toBe(0);

	/** Runs `args` against the server and the directory, and compares. */
	const both = async (args: string[]): Promise<Said> => {
		const server = said(await cli.run(args));
		const local = said(await cli.run(['--dir', dir, ...args]));
		expect(local, args.join(' ')).toEqual(server);
		return server;
	};
	const number = (s: Said) => s.markdown!.match(/[A-Z0-9]+-\d+(?:-I\d+)?/)![0];

	await both(['types', 'define', join(cli.dir, 'types.json')]);
	const a = number(await both(['new', box, '--set', 'title=Box A', '--reason', 'Boxing']));
	const b = number(await both(['new', box, '--set', 'title=Box B']));
	const one = number(
		await both([
			'new',
			item,
			'--in',
			a,
			'--set',
			'title=One',
			'--set',
			'size=Small',
			'--set',
			`notes=@${join(cli.dir, 'notes.md')}`
		])
	);
	const two = number(await both(['new', item, '--in', a, '--set', 'title=Two']));
	const three = number(await both(['new', item, '--in', a, '--set', 'title=Three']));
	expect([one, two, three]).toEqual([`${a}-I1`, `${a}-I2`, `${a}-I3`]);

	const steps: [string[], number][] = [
		[['new', item, '--set', 'title=Nowhere'], 1],
		[['new', item, '--in', a, '--set', 'size=Huge', '--set', 'title=Bad'], 1],
		[['set', one, 'size=Large', 'due=2026-12-01'], 0],
		[['set', one, 'title='], 1],
		[['do', one, 'start', '--reason', 'Begun'], 0],
		[['do', one, 'finish'], 1],
		[['set', one, 'checked=true'], 0],
		[['do', one, 'finish'], 0],
		[['do', one, 'start'], 1],
		[['link', two, 'blocks', one, '--pin'], 0],
		[['link', two, 'blocks', one], 1],
		[['link', three, 'blocks', three], 1],
		[['link', three, 'blocks', two], 0],
		[['unlink', two, 'blocks', one], 0],
		[['unlink', two, 'blocks', one], 2],
		[['members', 'place', three, '--before', one], 0],
		[['members', 'place', three, '--after', three], 1],
		[['members', 'add', b, three], 1],
		[['members', 'transfer', two, a, b], 0],
		[['members', 'transfer', two, a, b], 1],
		[['members', 'remove', a, three], 0],
		[['members', 'remove', a, three], 1],
		[['members', 'add', b, three], 0],
		[['do', b, 'pause'], 0],
		[['members', 'add', b, one], 1],
		[['do', a, 'shut'], 0],
		[['members', 'remove', a, one], 1],
		[['set', 'NOPE-1', 'title=x'], 2]
	];
	for (const [args, code] of steps) {
		expect((await both(args)).code, args.join(' ')).toBe(code);
	}

	// The akt file, edited and applied, on both.
	for (const [where, prefix] of [
		['server', []],
		['local', ['--dir', dir]]
	] as const) {
		const out = join(cli.dir, `out-${where}`);
		mkdirSync(out);
		expect((await cli.run([...prefix, 'cat', two, '-o', out])).code).toBe(0);
		const file = join(out, `${two}.md`);
		writeFileSync(file, readFileSync(file, 'utf8').replace('title: Two', 'title: Two, applied'));
		const applied = await cli.run([...prefix, 'apply', file, '--reason', 'Applied']);
		expect(applied.code, applied.stderr).toBe(0);
	}

	// A box with its members, read, edited and applied as one file.
	// r[verify cli.members]
	// r[verify cli.file.multi]
	await both(['cat', a, '--all-members']);
	await both(['cat', b, '--members', item]);
	await both(['cat', b, '--members', 'nope']);
	for (const [where, prefix] of [
		['server', []],
		['local', ['--dir', dir]]
	] as const) {
		const read = await cli.run([...prefix, 'cat', b, '--all-members']);
		expect(read.code, read.stderr).toBe(0);
		const file = join(cli.dir, `box-${where}.md`);
		const edited = read.stdout.replace('title: Three\n', 'title: Three edited\n');
		expect(edited).not.toBe(read.stdout);

		// A part gone stale changes nothing, its other edits neither.
		writeFileSync(file, edited.replace('title: Box B\n', 'title: Box B edited\n'));
		expect((await cli.run([...prefix, 'set', two, 'notes=Changed meanwhile'])).code).toBe(0);
		const stale = await cli.run([...prefix, 'apply', file, '--format', 'json']);
		expect(JSON.parse(stale.stderr).error.code, where).toBe('STALE');
		const box = await cli.run([...prefix, 'cat', b]);
		expect(box.stdout, where).toContain('title: Box B\n');

		// Read again, it applies each part.
		const fresh = await cli.run([...prefix, 'cat', b, '--all-members']);
		writeFileSync(file, fresh.stdout.replace('title: Three\n', 'title: Three edited\n'));
		const applied = await cli.run([...prefix, 'apply', file, '--format', 'json', '--reason', 'Edited']);
		expect(applied.code, applied.stderr).toBe(0);
		expect(
			Object.fromEntries(
				JSON.parse(applied.stdout).applied.map((x: { number: string; changed: string[] }) => [x.number, x.changed])
			)
		).toEqual({ [b]: [], [two]: [], [three]: ['title'] });
	}
	await both(['cat', b, '--all-members']);

	// A refusal only the edit itself finds, of a task already done, comes
	// after another's edit in the same file: neither changes, on both.
	// r[verify cli.apply]
	for (const [where, prefix] of [
		['server', []],
		['local', ['--dir', dir]]
	] as const) {
		const read = await cli.run([...prefix, 'cat', two, one]);
		const file = join(cli.dir, `done-${where}.md`);
		// Both titles rewritten, whatever they are by now.
		let seen = 0;
		const edited = read.stdout.replace(/^title: .*$/gm, () => `title: ${++seen === 1 ? 'Two again' : 'One again'}`);
		expect(seen, where).toBe(2);
		writeFileSync(file, edited);
		const refused = await cli.run([...prefix, 'apply', file, '--format', 'json']);
		expect(refused.code, `${where}: ${refused.stdout}`).toBe(1);
		expect(JSON.parse(refused.stderr).error.code, where).toBe('REFUSED');
		if (where === 'server') {
			expect(JSON.parse(refused.stderr).error.message).toContain(`${one}: `);
		}
		expect((await cli.run([...prefix, 'cat', two])).stdout, where).not.toContain('Two again');
	}

	// Every akt reads the same, and each box's members are in the same
	// order. (Outside git, akter in one file share its time, so the most
	// recently changed first isn't compared.)
	for (const n of [a, b, one, two, three]) {
		await both(['cat', n]);
	}
	const order = async (prefix: string[], container: string) =>
		JSON.parse((await cli.run([...prefix, 'ls', item, '--in', container, '--format', 'json'])).stdout).akter.map(
			(x: { number: string }) => x.number
		);
	for (const container of [a, b]) {
		expect(await order(['--dir', dir], container)).toEqual(await order([], container));
	}
	const all = async (prefix: string[]) =>
		JSON.parse((await cli.run([...prefix, 'ls', item, '--format', 'json'])).stdout)
			.akter.map((x: { number: string }) => x.number)
			.sort();
	expect(await all(['--dir', dir])).toEqual(await all([]));
	expect((await cli.run(['--dir', dir, 'check'])).code).toBe(0);
});

// r[verify cli.file.kinds]
// r[verify cli.local.rules]
// r[verify akt.copy]
// r[verify akt.outcome]
// r[verify akt.transition.outcomes]
// r[verify akt.filter.outcome]
// r[verify akt.filter]
test('local mode lays out tests, takes verdicts, filters, copies and finishes as the server does', async () => {
	test.setTimeout(300_000);
	const stamp = Date.now();
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const dir = join(cli.dir, 'protocols');
	const protocol = `parity_protocol_${stamp}`;
	const check = `parity_test_${stamp}`;
	const states = [{ key: 'open', name: 'Open', initial: true }];
	writeFileSync(
		join(cli.dir, 'types.json'),
		JSON.stringify([
			{
				key: protocol,
				name: 'Parity protocol',
				number: `PP${stamp}-{seq}`,
				states: [...states, { key: 'done', name: 'Done', final: true }],
				transitions: [
					{ key: 'finish', name: 'Finish', from: ['open'], to: 'done', outcomes: ['pass', 'fail', 'skip'] }
				],
				fields: [
					{ key: 'title', name: 'Title', kind: 'text', required: true },
					{ key: 'layout', name: 'Layout', kind: 'sections', of: [check] }
				],
				contains: [{ type: check }]
			},
			{
				key: check,
				name: 'Parity test',
				number: '{parent}-T{seq}',
				created_in: [protocol],
				states,
				fields: [
					{ key: 'title', name: 'Title', kind: 'text', required: true },
					{ key: 'testers', name: 'Testers', kind: 'user', multiple: true },
					{ key: 'verdict', name: 'Verdict', kind: 'verdict', assignees: 'testers' }
				]
			}
		])
	);
	expect((await cli.run(['init', '--dir', dir])).code).toBe(0);
	// `@me` in the directory is whom the server names the client by.
	const viewer = JSON.parse((await cli.run(['whoami', '--format', 'json'])).stdout);
	const me = { KURIA_ME: viewer.email ?? viewer.name };

	const both = async (args: string[]): Promise<Said> => {
		const server = said(await cli.run(args));
		const local = said(await cli.run(['--dir', dir, ...args], me));
		expect(local, args.join(' ')).toEqual(server);
		return server;
	};
	const number = (s: Said) => s.markdown!.match(/PP\d+-\d+(?:-T\d+)?/)![0];

	await both(['types', 'define', join(cli.dir, 'types.json')]);
	const p = number(await both(['new', protocol, '--set', 'title=Template']));
	const tests: string[] = [];
	for (const title of ['One', 'Two', 'Three']) {
		tests.push(number(await both(['new', check, '--in', p, '--set', `title=${title}`, '--set', 'testers=@me'])));
	}
	const [one, two, three] = tests;
	const layout = join(cli.dir, 'layout.yaml');
	const bad = join(cli.dir, 'bad.yaml');
	writeFileSync(
		layout,
		`- title: Session\n  collapsible: true\n  text: Each way in and out.\n  items: [${one}, ${two}]\n- ${three}\n`
	);
	writeFileSync(bad, `- ${p}\n`);

	const steps: [string[], number][] = [
		[['set', p, `layout=@${bad}`], 1],
		[['set', p, `layout=@${layout}`], 0],
		[['cat', p], 0],
		[['cat', one], 0],
		[['types', 'schema', protocol], 0],
		[['do', p, 'finish'], 1],
		[['verdict', one, 'pass', '--reason', 'Ran it'], 0],
		[['verdict', one, 'maybe'], 1],
		[['verdict', p, 'pass'], 1],
		[['set', one, 'verdict=pass'], 1],
		[['verdict', two, 'fail'], 0],
		[['ls', check, '--in', p, '--where', 'testers=@me'], 0],
		[['ls', check, '--outcome', 'fail'], 0],
		[['ls', check, '--in', p, '--outcome', 'pending'], 0],
		[['ls', protocol, '--outcome', 'fail'], 1],
		[['cat', p], 0],
		[['verdict', two, 'clear'], 0],
		[['verdict', two, 'skip'], 0],
		[['ls', check, '--in', p], 0],
		[['verdict', three, 'pass'], 0],
		[['cat', p, '--all-members'], 0],
		[['copy', p, '--only', 'NOPE-1'], 1],
		[['copy', p, '--only', one, '--reason', 'Run it'], 0],
		[['copy', p], 0],
		[['members', 'remove', p, three], 0],
		[['do', p, 'finish'], 0],
		[['cat', p], 0]
	];
	for (const [args, code] of steps) {
		expect((await both(args)).code, args.join(' ')).toBe(code);
	}
	for (const copy of [`PP${stamp}-2`, `PP${stamp}-3`]) {
		await both(['cat', copy, '--all-members']);
		await both(['ls', check, '--in', copy]);
	}

	// Read and applied unchanged, nothing changes; a verdict changed in the
	// file is refused.
	for (const [where, prefix] of [
		['server', []],
		['local', ['--dir', dir]]
	] as const) {
		const read = await cli.run([...prefix, 'cat', p, '--all-members'], me);
		const file = join(cli.dir, `protocol-${where}.md`);
		writeFileSync(file, read.stdout);
		const applied = await cli.run([...prefix, 'apply', file], me);
		expect(applied.code, `${where}: ${applied.stderr}`).toBe(0);
		expect(applied.stdout, where).not.toContain('changed ');
		writeFileSync(file, read.stdout.replace(': pass}', ': fail}'));
		const refused = await cli.run([...prefix, 'apply', file, '--format', 'json'], me);
		expect(JSON.parse(refused.stderr).error.code, where).toBe('READ_ONLY');
	}
	expect((await cli.run(['--dir', dir, 'check'])).code).toBe(0);
});

// r[verify cli.local.rules]
// r[verify cli.local.versions]
// r[verify cli.local.references]
// r[verify akt.field.markdown]
// r[verify version.release]
// r[verify version.references]
// r[verify version.read]
// r[verify version.diff]
test('local mode writes documents, releases and reads versions as the server does', async () => {
	test.setTimeout(300_000);
	const stamp = Date.now();
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const dir = join(cli.dir, 'documents');
	const index = `parity_index_${stamp}`;
	const doc = `parity_doc_${stamp}`;
	writeFileSync(
		join(cli.dir, 'types.json'),
		JSON.stringify([
			{
				key: index,
				name: 'Parity index',
				number: `PI${stamp}-{seq}`,
				fields: [
					{ key: 'title', name: 'Title', kind: 'text', required: true },
					{ key: 'folders', name: 'Folders', kind: 'sections', of: [doc] }
				],
				states: [{ key: 'open', name: 'Open', initial: true }],
				contains: [{ type: doc, exclusive: true }]
			},
			{
				key: doc,
				name: 'Parity document',
				number: '{parent}-D{seq:2}',
				created_in: [index],
				fields: [
					{ key: 'title', name: 'Title', kind: 'text', required: true },
					{ key: 'body', name: 'Body', kind: 'markdown', references: 'refers_to' }
				],
				states: [{ key: 'draft', name: 'Draft', initial: true }],
				links: [{ key: 'refers_to', name: 'Refers to', inverse: 'Referred to by', to: [doc] }],
				versions: {}
			}
		])
	);
	expect((await cli.run(['init', '--dir', dir])).code).toBe(0);
	const git = (...args: string[]) =>
		execFileSync('git', ['-C', dir, '-c', 'commit.gpgsign=false', ...args], {
			env: {
				...process.env,
				GIT_AUTHOR_NAME: 'Ada',
				GIT_AUTHOR_EMAIL: 'ada@shop.example',
				GIT_COMMITTER_NAME: 'Ada',
				GIT_COMMITTER_EMAIL: 'ada@shop.example'
			}
		});
	git('init', '-q');
	const commit = () => {
		git('add', '-A');
		git('commit', '-qm', 'Saved', '--allow-empty');
	};
	const viewer = JSON.parse((await cli.run(['whoami', '--format', 'json'])).stdout);
	const me = { KURIA_ME: viewer.email ?? viewer.name };
	const both = async (args: string[]): Promise<Said> => {
		const server = said(await cli.run(args));
		const local = said(await cli.run(['--dir', dir, ...args], me));
		expect(local, args.join(' ')).toEqual(server);
		return server;
	};
	const body = (name: string, text: string) => {
		const path = join(cli.dir, `${name}.md`);
		writeFileSync(path, text);
		return `body=@${path}`;
	};
	const p = `PI${stamp}-1`;
	const [setup, guide] = [`${p}-D01`, `${p}-D02`];

	await both(['types', 'define', join(cli.dir, 'types.json')]);
	await both(['new', index, '--set', 'title=Handbook']);
	await both(['new', doc, '--in', p, '--set', 'title=Setup', '--set', body('setup', '# Install\n\nRun it.\n')]);
	await both([
		'new',
		doc,
		'--in',
		p,
		'--set',
		'title=Guide',
		'--set',
		body('guide', `[install](kuria:${setup}#install) and [gone](kuria:${setup}#gone)\n`)
	]);
	commit();
	const steps: [string[], number][] = [
		[['cat', setup], 0],
		[['cat', guide], 0],
		[['release', guide], 1],
		[['link', guide, 'refers_to', setup], 1],
		[['set', guide, body('fixed', `[install](kuria:${setup}#install)\n`)], 0]
	];
	for (const [args, code] of steps) {
		expect((await both(args)).code, args.join(' ')).toBe(code);
	}
	commit();
	// The setup has no version: the guide is released together with it.
	expect((await both(['release', guide, '--label', '1.0'])).code).toBe(1);
	expect((await both(['release', guide, setup, '--label', '1.0'])).code).toBe(0);
	commit();
	await both(['set', guide, body('more', `[install](kuria:${setup}#install)\n\nMore.\n`)]);
	commit();
	for (const args of [
		['cat', guide, '--version', '1'],
		['diff', guide, '1'],
		['cat', guide, '--version', '2']
	]) {
		await both(args);
	}
});
