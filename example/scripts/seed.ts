// What setup seeds the example stack's Konklav with: the actions, the
// release-management example's roles, who holds them, and the examples'
// people. Its fingerprint goes into `.state/seeded.json`, so a start after
// any of it changes seeds Konklav again (up.ts).
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { EXAMPLES } from './stack.ts';

const examples = readdirSync(EXAMPLES, { withFileTypes: true })
	.filter((entry) => entry.isDirectory())
	.map((entry) => join(EXAMPLES, entry.name));

const KURIA_ACTIONS: Record<string, string> = {
	'kuria:akt_types:define': 'Define akt types',
	'kuria:akter:read': "See an akt type's akter",
	'kuria:akter:create': "Create an akt type's akter",
	'kuria:akter:edit': "Edit an akt type's akter",
	'kuria:akter:transition': "Perform an akt type's transitions",
	'kuria:akter:archive': "Archive and unarchive an akt type's akter",
	'kuria:contacts:read': 'List contacts',
	'kuria:contacts:manage': 'Add, edit and erase contacts',
	'kuria:tenant:export': "Export the tenant's akt types and akter as bundles",
	'kuria:tenant:import': 'Import bundles into the tenant'
};
/** The actions the examples' akt types require of transitions. */
const exampleActions = new Set(
	examples.flatMap((dir) =>
		readdirSync(dir)
			.filter((file) => file.endsWith('.json'))
			.flatMap((file) => [...readFileSync(join(dir, file), 'utf8').matchAll(/"action"\s*:\s*"([^"]+)"/g)])
			.map((match) => match[1])
	)
);
export const ACTIONS: Record<string, string> = {
	...KURIA_ACTIONS,
	...Object.fromEntries([...exampleActions].sort().map((action) => [action, `The examples' ${action}`]))
};

const allow = (actions: string[], scopeBound = false) =>
	actions.map((action) => ({ action, effect: 'allow', scope_bound: scopeBound }));

/** The release-management example's roles, as the end-to-end seed has
 * them: tenant-wide parts and parts granted at one akt type. */
export const ROLES: Record<string, ReturnType<typeof allow>> = {
	admin: allow(Object.keys(ACTIONS)),
	reader: allow(['kuria:akter:read']),
	'contacts-manager': allow(['kuria:contacts:manage']),
	'developer-work': allow(['kuria:akter:create', 'kuria:akter:edit', 'kuria:akter:transition'], true),
	'release-work': allow(
		['kuria:akter:create', 'kuria:akter:edit', 'kuria:akter:transition', 'kuria:akter:archive', ...exampleActions],
		true
	),
	'document-release': allow(
		['kuria:akter:create', 'kuria:akter:edit', 'kuria:akter:transition', ...exampleActions],
		true
	)
};

export type Grant = string | { role: string; at: string[] };

/** What each of the examples' people may do, by email; anyone else reads. */
export const GRANTS: Record<string, Grant[]> = {
	'dev@shop.example': [
		'reader',
		{
			role: 'developer-work',
			at: ['backlog', 'epic', 'backlog_item', 'test_protocol', 'test_case', 'document_index', 'document', 'image', 'plan', 'task', 'gap', 'decision']
		}
	],
	'rm@shop.example': [
		'reader',
		'contacts-manager',
		{ role: 'release-work', at: ['release', 'release_candidate', 'hotfix', 'release_notes', 'plan', 'task', 'gap', 'decision'] },
		{ role: 'developer-work', at: ['backlog', 'epic', 'backlog_item', 'test_protocol', 'test_case', 'document_index', 'image'] },
		{ role: 'document-release', at: ['document', 'external_document'] }
	],
	'hedda@shop.example': ['reader']
};

/** The examples' people who sign in: every bundle's, once each, as the
 * subjects the bundles name. An example without a bundle names none. */
export const PEOPLE: { subject: string; name: string; email: string }[] = (() => {
	const named = new Map<string, { subject: string; name: string; email: string }>();
	for (const dir of examples.filter((d) => existsSync(join(d, 'bundle', 'bundle.json')))) {
		const bundle: { people: { subject: string; name?: string | null; email?: string | null }[] } = JSON.parse(
			readFileSync(join(dir, 'bundle', 'bundle.json'), 'utf8')
		);
		for (const p of bundle.people) {
			if (p.email && !named.has(p.subject)) named.set(p.subject, { subject: p.subject, name: p.name ?? p.email, email: p.email });
		}
	}
	return [...named.values()];
})();

/** A digest of everything above: when it differs from the one
 * `.state/seeded.json` records, Konklav holds an older seeding. */
export const FINGERPRINT = createHash('sha256')
	.update(JSON.stringify({ ACTIONS, ROLES, GRANTS, PEOPLE }))
	.digest('hex');
