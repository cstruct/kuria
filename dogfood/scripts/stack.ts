/** Where the dogfooding stack runs, and what was made for it. */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export const KURIA = 'http://localhost:18190';
export const KONKLAV_SUBJECT = 'http://localhost:18180';
export const KONKLAV_RELYING = 'http://localhost:18181';
export const KONKLAV_CONTROL = 'http://localhost:18182';

export const ROOT = join(import.meta.dirname, '..', '..');
export const STATE = join(import.meta.dirname, '..', '.state');

/** What setup seeded, once. */
export interface Seeded {
	tenant: string;
	person: { email: string; password: string; subject: string };
	/** The agent: a service principal's secret key, `<key id>.<secret>`. */
	agent: { key: string; subject: string };
}

export function seeded(): Seeded | null {
	const path = join(STATE, 'seeded.json');
	return existsSync(path) ? (JSON.parse(readFileSync(path, 'utf8')) as Seeded) : null;
}

/** `.state/<name>` as `KEY=value` lines. */
export function envFile(name: string): Record<string, string> {
	return Object.fromEntries(
		readFileSync(join(STATE, name), 'utf8')
			.split('\n')
			.filter(Boolean)
			.map((line) => [line.slice(0, line.indexOf('=')), line.slice(line.indexOf('=') + 1)])
	);
}
