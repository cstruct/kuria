/** Where the end-to-end stack runs, and what setup wrote about it. */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const KURIA = 'http://localhost:18090';
export const KONKLAV_SUBJECT = 'http://localhost:18080';
export const KONKLAV_RELYING = 'http://localhost:18081';
export const KONKLAV_CONTROL = 'http://localhost:18082';
export const BOOTSTRAP_TOKEN = 'kuria-e2e-bootstrap-token-0123456789abcdef';

export const STATE = join(import.meta.dirname, '..', '.state');

/** A person who signs in through the login page. Test-only passwords. */
export interface Person {
	email: string;
	password: string;
	subject: string;
}

/** What setup seeded. */
export interface Seeded {
	tenant: string;
	/** A second tenant, where the admin is a member too. */
	lab: string;
	people: { developer: Person; releaseManager: Person; headOfProduct: Person };
	/** Software with secret keys: the admin, who defines akt types, and the
	 * build pipeline. */
	clients: { admin: ServiceClient; pipeline: ServiceClient };
}

export interface ServiceClient {
	/** `<key id>.<secret>`. */
	key: string;
}

export function seeded(): Seeded {
	return JSON.parse(readFileSync(join(STATE, 'seeded.json'), 'utf8')) as Seeded;
}
