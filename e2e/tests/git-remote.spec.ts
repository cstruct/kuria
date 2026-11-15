// `git-remote-kuria` against the end-to-end Kuria (docs/spec/git.md): a
// document index cloned twice with the same commits, a commit pushed back
// as a revision, a push refused until `git pull --rebase`, one refused
// until `kuria fmt`, and a version as a tag.
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { Cli, finished, type Ran } from '../support/cli.ts';
import { defineDocuments } from '../support/example.ts';
import { KURIA, seeded } from '../support/stack.ts';

const BINARIES = join(import.meta.dirname, '..', '..', 'target', 'debug');

// r[verify git.remote]
// r[verify git.tree]
// r[verify git.history]
// r[verify git.identity]
// r[verify git.fetch]
// r[verify git.push]
// r[verify git.push.fast-forward]
// r[verify git.push.normal]
// r[verify git.fmt]
test('a document index is cloned, pushed to and rebased with git', async () => {
	test.setTimeout(240_000);
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const run = (command: string, args: string[], cwd = cli.dir): Promise<Ran> =>
		finished(
			spawn(command, args, {
				cwd,
				env: {
					PATH: `${BINARIES}:${process.env.PATH ?? ''}`,
					HOME: cli.dir,
					KURIA_CONFIG_DIR: cli.dir,
					KURIA_SECRET_STORE: 'file',
					KURIA_URL: KURIA,
					KURIA_KEY: seeded().clients.admin.key,
					GIT_AUTHOR_NAME: 'Ada',
					GIT_AUTHOR_EMAIL: 'ada@shop.example',
					GIT_COMMITTER_NAME: 'Ada',
					GIT_COMMITTER_EMAIL: 'ada@shop.example'
				},
				stdio: ['ignore', 'pipe', 'pipe']
			})
		);
	const git = async (cwd: string, ...args: string[]) => {
		const ran = await run('git', args, cwd);
		expect(ran.code, `git ${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout.trim();
	};
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const read = async (number: string) =>
		(
			await admin.graphql<{ akt: { revision: number; fields: ({ markdown?: string } | null)[] } }>(
				'query($n: String!) { akt(number: $n) { revision fields { ... on MarkdownValue { markdown } } } }',
				{ n: number }
			)
		).akt;
	const edit = async (number: string, fields: Record<string, unknown>) =>
		admin.graphql('mutation($i: EditAktInput!) { editAkt(input: $i) { revision } }', {
			i: { number, revision: (await read(number)).revision, fields }
		});

	const index = await create('document_index', { title: `Handbook ${stamp}` });
	const setup = await create('document', { title: 'Setup', body: '# Install\n\nRun it.\n' }, [index]);
	const guide = await create('document', { title: 'Guide', body: 'Start here.\n' }, [index]);
	await edit(index, { folders: [{ title: 'Guides', items: [setup] }, guide] });

	// Cloned twice: the same tree, and the same commits.
	const url = `kuria::${KURIA}/${index}`;
	await git(cli.dir, 'clone', url, 'a');
	await git(cli.dir, 'clone', url, 'b');
	const a = join(cli.dir, 'a');
	const b = join(cli.dir, 'b');
	const files = (await git(a, 'ls-files')).split('\n');
	expect(files).toEqual([`${guide}-guide.md`, `${index}-handbook-${stamp}.md`, `guides/${setup}-setup.md`]);
	const guideFile = join(a, `${guide}-guide.md`);
	const text = readFileSync(guideFile, 'utf8');
	expect(text).toContain(`number: ${guide}\ntype: document\nstate: draft\n`);
	expect(text).not.toContain('revision:');
	expect(await git(a, 'rev-parse', 'HEAD')).toBe(await git(b, 'rev-parse', 'HEAD'));

	// A commit pushed is a revision; the other clone fetches the very commit.
	writeFileSync(guideFile, text.replace('Start here.', 'Start here, then read the setup.'));
	await git(a, 'commit', '-qam', 'Point to the setup');
	await git(a, 'push', 'origin', 'main');
	expect((await read(guide)).fields.find((f) => f?.markdown)?.markdown).toContain('then read the setup');
	await git(b, 'pull', '-q', '--rebase');
	expect(await git(b, 'rev-parse', 'HEAD')).toBe(await git(a, 'rev-parse', 'HEAD'));
	expect(await git(b, 'log', '-1', '--format=%an %s')).toBe('Ada Point to the setup');

	// Changed in Kuria meanwhile: refused until rebased onto it.
	await edit(setup, { body: '# Install {#install}\n\nRun it twice.\n' });
	const bGuide = join(b, `${guide}-guide.md`);
	writeFileSync(bGuide, readFileSync(bGuide, 'utf8').replace('Start here,', 'Begin here,'));
	await git(b, 'commit', '-qam', 'Begin');
	const refused = await run('git', ['push', '-q', 'origin', 'main'], b);
	expect(refused.code).not.toBe(0);
	expect(refused.stderr).toContain('pull --rebase');
	await git(b, 'pull', '-q', '--rebase');
	expect(readFileSync(join(b, 'guides', `${setup}-setup.md`), 'utf8')).toContain('Run it twice.');
	await git(b, 'push', '-q', 'origin', 'main');
	expect((await read(guide)).fields.find((f) => f?.markdown)?.markdown).toContain('Begin here,');

	// A heading without an id isn't as Kuria writes it: refused, then fixed.
	writeFileSync(bGuide, readFileSync(bGuide, 'utf8').replace('Begin here,', '## Before you start\n\nBegin here,'));
	await git(b, 'commit', '-qam', 'A heading');
	const unformatted = await run('git', ['push', '-q', 'origin', 'main'], b);
	expect(unformatted.code).not.toBe(0);
	expect(unformatted.stderr).toContain('kuria fmt');
	const formatted = await run('kuria', ['fmt'], b);
	expect(formatted.code, formatted.stderr).toBe(0);
	expect(readFileSync(bGuide, 'utf8')).toContain('## Before you start {#before-you-start}');
	await git(b, 'commit', '-qa', '--amend', '--no-edit');
	await git(b, 'push', '-q', 'origin', 'main');

	// A version is a tag.
	await admin.graphql('mutation($i: ReleaseVersionInput!) { releaseVersion(input: $i) { number } }', {
		i: { number: guide, revision: (await read(guide)).revision, label: '1.0' }
	});
	await git(a, 'pull', '-q', '--rebase');
	expect(await git(a, 'tag', '--list')).toContain(`${guide}/v1`);
	expect(await git(a, 'rev-parse', 'HEAD')).toBe(await git(b, 'rev-parse', 'HEAD'));
	expect(existsSync(join(a, 'guides'))).toBe(true);
});
