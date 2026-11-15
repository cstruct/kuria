/** The release-management example's akt types, defined as their
 * documents are (docs/examples/release-management). */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { api } from './api.ts';
import { seeded } from './stack.ts';

const EXAMPLE = join(import.meta.dirname, '..', '..', 'docs', 'examples', 'release-management');

export const TYPES = [
	'artifact',
	'backlog_item',
	'epic',
	'backlog',
	'release',
	'release_candidate',
	'hotfix',
	'release_notes'
];

/** Defines every akt type of the example, as the seeded admin. Defining
 * one again makes a new version of it. */
export async function defineExample(): Promise<void> {
	const admin = await api(seeded().clients.admin);
	for (const key of TYPES) {
		await admin.graphql('mutation($d: JSON!) { defineAktType(document: $d) { key } }', {
			d: JSON.parse(readFileSync(join(EXAMPLE, `${key}.json`), 'utf8'))
		});
	}
}

/** The example's document for `key`, as its file holds it. */
export function exampleDocument(key: string): Record<string, unknown> {
	return JSON.parse(readFileSync(join(EXAMPLE, `${key}.json`), 'utf8'));
}

const TEST_PROTOCOLS = join(import.meta.dirname, '..', '..', 'docs', 'examples', 'test-protocols');

/** Defines the test-protocols example's akt types (docs/examples/test-protocols),
 * test cases before the protocols that name them, as the seeded admin. */
export async function defineTestProtocols(): Promise<void> {
	const admin = await api(seeded().clients.admin);
	for (const key of ['test_case', 'test_protocol']) {
		await admin.graphql('mutation($d: JSON!) { defineAktType(document: $d) { key } }', {
			d: JSON.parse(readFileSync(join(TEST_PROTOCOLS, `${key}.json`), 'utf8'))
		});
	}
}

const DOCUMENTS = join(import.meta.dirname, '..', '..', 'docs', 'examples', 'documents');

/** Defines the documents example's akt types (docs/examples/documents). */
export async function defineDocuments(): Promise<void> {
	const admin = await api(seeded().clients.admin);
	for (const key of ['image', 'document', 'document_index']) {
		await admin.graphql('mutation($d: JSON!) { defineAktType(document: $d) { key } }', {
			d: JSON.parse(readFileSync(join(DOCUMENTS, `${key}.json`), 'utf8'))
		});
	}
}
