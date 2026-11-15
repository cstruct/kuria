/** git with `git-remote-kuria` on its path, signed in as `cli`'s key, in
 * `cli`'s directory unless told otherwise (docs/spec/git.md). */
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { expect } from '@playwright/test';
import { type Cli, finished, type Ran } from './cli.ts';
import { KURIA } from './stack.ts';

const BINARIES = join(import.meta.dirname, '..', '..', 'target', 'debug');

/** Runs `command args…` in `cwd` with the built binaries first on the path. */
export function runIn(cli: Cli, key: string, command: string, args: string[], cwd = cli.dir): Promise<Ran> {
	return finished(
		spawn(command, args, {
			cwd,
			env: {
				PATH: `${BINARIES}:${process.env.PATH ?? ''}`,
				HOME: cli.dir,
				KURIA_CONFIG_DIR: cli.dir,
				KURIA_SECRET_STORE: 'file',
				KURIA_URL: KURIA,
				KURIA_KEY: key,
				GIT_AUTHOR_NAME: 'Ada',
				GIT_AUTHOR_EMAIL: 'ada@shop.example',
				GIT_COMMITTER_NAME: 'Ada',
				GIT_COMMITTER_EMAIL: 'ada@shop.example'
			},
			stdio: ['ignore', 'pipe', 'pipe']
		})
	);
}

/** `git args…` in `cwd`, which must succeed; its output, trimmed. */
export async function gitIn(cli: Cli, key: string, cwd: string, ...args: string[]): Promise<string> {
	const ran = await runIn(cli, key, 'git', args, cwd);
	expect(ran.code, `git ${args.join(' ')}: ${ran.stderr}`).toBe(0);
	return ran.stdout.trim();
}

/** The `kuria::` URL of the container `number` on the end-to-end Kuria. */
export const cloneUrl = (number: string) => `kuria::${KURIA}/${number}`;
