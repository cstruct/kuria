/** The `kuria` CLI, run against the end-to-end stack with a directory of
 * its own, keeping its secrets in a file there. */
import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { KURIA } from './stack.ts';

const BINARY = join(import.meta.dirname, '..', '..', 'target', 'debug', 'kuria');

/** What a finished run said. */
export interface Ran {
	code: number | null;
	stdout: string;
	stderr: string;
}

/** A CLI with a config directory of its own. */
export class Cli {
	readonly dir = mkdtempSync(join(tmpdir(), 'kuria-cli-'));

	constructor(private readonly env: Record<string, string> = {}) {}

	/** Starts `kuria args…`, in `cwd` if given. */
	start(args: string[], env: Record<string, string> = {}, cwd?: string): ChildProcess {
		return spawn(BINARY, args, {
			cwd,
			env: {
				PATH: process.env.PATH ?? '',
				HOME: this.dir,
				KURIA_CONFIG_DIR: this.dir,
				KURIA_SECRET_STORE: 'file',
				KURIA_URL: KURIA,
				...this.env,
				...env
			},
			stdio: ['ignore', 'pipe', 'pipe']
		});
	}

	/** Runs `kuria args…` to the end, in `cwd` if given. */
	run(args: string[], env: Record<string, string> = {}, cwd?: string): Promise<Ran> {
		return finished(this.start(args, env, cwd));
	}
}

/** What `child` said by the time it exited. */
export function finished(child: ChildProcess): Promise<Ran> {
	let stdout = '';
	let stderr = '';
	child.stdout!.on('data', (chunk) => (stdout += chunk));
	child.stderr!.on('data', (chunk) => (stderr += chunk));
	return new Promise((resolve) => child.on('close', (code) => resolve({ code, stdout, stderr })));
}

/** The first line on `child`'s standard error, trimmed, matching `pattern`. */
export function waitForLine(child: ChildProcess, pattern: RegExp): Promise<string> {
	return new Promise((resolve, reject) => {
		let seen = '';
		const listen = (chunk: Buffer) => {
			seen += chunk;
			const line = seen
				.split('\n')
				.map((l) => l.trim())
				.find((l) => pattern.test(l));
			if (line) {
				child.stderr!.off('data', listen);
				resolve(line);
			}
		};
		child.stderr!.on('data', listen);
		child.on('close', (code) => reject(new Error(`exited with ${code} before printing ${pattern}: ${seen}`)));
	});
}
