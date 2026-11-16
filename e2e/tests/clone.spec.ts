// A document index of the documents example cloned with the `kuria`
// binary, its live document edited as a file while someone types in it in
// the app: the push behind the live edit is refused, a pull merges it, and
// the push lands in the browser as it is typed in.
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { api } from '../support/api.ts';
import { open } from '../support/app.ts';
import { Cli } from '../support/cli.ts';
import { defineDocuments } from '../support/example.ts';
import { signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify clone.clone]
// r[verify clone.status]
// r[verify clone.add]
// r[verify clone.push]
// r[verify clone.pull]
test('a clone pushes and pulls while a document is edited live', async ({ browser }) => {
	test.setTimeout(180_000);
	const stamp = Date.now();
	await defineDocuments();
	const admin = await api(seeded().clients.admin);
	const create = async (type: string, fields: Record<string, unknown>, inside: string[] = []) =>
		(
			await admin.graphql<{ createAkt: { number: string } }>(
				'mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }',
				{ t: type, f: fields, in: inside }
			)
		).createAkt.number;
	const index = await create('document_index', { title: `Clone handbook ${stamp}` });
	const guide = await create('document', { title: 'Guide', body: 'Install it.\n\nUse it.\n' }, [index]);

	const cli = new Cli({ KURIA_KEY: seeded().clients.admin.key });
	const ok = async (args: string[], cwd: string) => {
		const ran = await cli.run(args, {}, cwd);
		expect(ran.code, `kuria ${args.join(' ')}: ${ran.stderr}`).toBe(0);
		return ran.stdout;
	};
	const here = mkdtempSync(join(tmpdir(), 'kuria-clone-'));
	await ok(['clone', index, 'handbook'], here);
	const root = join(here, 'handbook');
	const file = readdirSync(root).find((f) => f.startsWith(`${guide}-`))!;
	expect(file).toBe(`${guide}-guide.md`);
	expect(readFileSync(join(root, file), 'utf8')).toContain('Install it.');

	// Someone types at its end in the app meanwhile.
	const dev = await signIn(browser, seeded().people.developer);
	await open(dev, guide);
	await dev.getByRole('button', { name: 'Edit Body' }).click();
	const editor = dev.getByRole('textbox', { name: 'Body' });
	await editor.click();
	await dev.keyboard.press('Control+End');
	await dev.keyboard.type(' Typed live.');
	await expect(dev.getByTestId('live-status')).toHaveText('Saved');

	// The clone changes its start, staged and pushed: refused, behind.
	const path = join(root, file);
	writeFileSync(path, readFileSync(path, 'utf8').replace('Install it.', 'Install it first.'));
	expect(JSON.parse(await ok(['status', '--format', 'json'], root)).unstaged).toEqual([file]);
	await ok(['add', file], root);
	const behind = await cli.run(['push', '-m', 'Install first', '--format', 'json'], {}, root);
	expect(behind.code, behind.stderr).toBe(3);
	expect(JSON.parse(behind.stderr).error.code).toBe('STALE');

	// Pulled, merged, and pushed: the browser has both.
	await ok(['pull'], root);
	const merged = readFileSync(path, 'utf8');
	expect(merged).toContain('Install it first.');
	expect(merged).toContain('Use it. Typed live.');
	await ok(['push', '-m', 'Install first'], root);
	await expect(editor).toContainText('Install it first.');
	await expect(editor).toContainText('Use it. Typed live.');
	const status = JSON.parse(await ok(['status', '--format', 'json'], root));
	expect(status).toMatchObject({ staged: [], unstaged: [], conflicted: [] });
	const changes = await admin.graphql<{ akt: { draftChanges: { message: string | null }[] } }>(
		'query($n: String!) { akt(number: $n) { draftChanges(all: true) { message } } }',
		{ n: guide }
	);
	expect(changes.akt.draftChanges.map((c) => c.message)).toContain('Install first');
});
