/** Where the example stack runs, and what was made for it. */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** The app, from Vite's dev server, which passes the API on to Kuria. */
export const APP = 'http://localhost:18290';
/** The Kuria server behind it. */
export const KURIA = 'http://127.0.0.1:18291';
export const KONKLAV_SUBJECT = 'http://localhost:18280';
export const KONKLAV_RELYING = 'http://localhost:18281';
export const KONKLAV_CONTROL = 'http://localhost:18282';

export const ROOT = join(import.meta.dirname, '..', '..');
export const STATE = join(import.meta.dirname, '..', '.state');
export const EXAMPLES = join(ROOT, 'docs', 'examples');

/** A person who signs in through the login page. Local-only passwords. */
export interface Person {
	name: string;
	email: string;
	password: string;
	subject: string;
}

/** What setup seeded, once. */
export interface Seeded {
	tenant: string;
	/** The examples' people, with the subjects their bundles name. */
	people: Person[];
	/** A service principal's secret key, `<key id>.<secret>`, allowed
	 * everything: for checking what a start loaded. */
	admin: { key: string };
	/** seed.ts's `FINGERPRINT` when it seeded; absent from older
	 * seedings. */
	fingerprint?: string;
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
