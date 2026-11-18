// Seeds the example stack's Konklav once, over its control plane: Kuria's
// actions and the examples' own, one tenant with the release-management
// example's roles, the examples' people as subjects with the very ids
// their bundles name (so a bundle's people are the tenant's users, with
// no subjects mapping), an admin service principal, and Kuria's OIDC
// clients. Writes .state/kuria.env and .state/seeded.json. Does nothing
// if it has run.
import { randomBytes, randomUUID } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
	APP,
	EXAMPLES,
	KONKLAV_CONTROL,
	KONKLAV_RELYING,
	KONKLAV_SUBJECT,
	STATE,
	seeded,
	type Person,
	type Seeded
} from './stack.ts';

if (seeded()) {
	console.log(`already seeded (${join(STATE, 'seeded.json')})`);
	process.exit(0);
}

const token = /bootstrap_token (\S+)/.exec(readFileSync(join(STATE, 'secrets.styx'), 'utf8'))![1];

async function control(method: string, path: string, body?: unknown): Promise<any> {
	const response = await fetch(`${KONKLAV_CONTROL}/v1${path}`, {
		method,
		headers: {
			authorization: `Bearer ${token}`,
			...(body === undefined ? {} : { 'content-type': 'application/json' })
		},
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	const text = await response.text();
	if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${text}`);
	return text ? JSON.parse(text) : null;
}

for (let i = 0; ; i++) {
	try {
		if ((await fetch(`${KONKLAV_SUBJECT}/readyz`)).ok) break;
	} catch {
		// Not listening yet.
	}
	if (i > 180) throw new Error('Konklav never became ready');
	await new Promise((resolve) => setTimeout(resolve, 1000));
}

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
const ACTIONS: Record<string, string> = {
	...KURIA_ACTIONS,
	...Object.fromEntries([...exampleActions].map((action) => [action, `The examples' ${action}`]))
};

const allow = (actions: string[], scopeBound = false) =>
	actions.map((action) => ({ action, effect: 'allow', scope_bound: scopeBound }));

/** The release-management example's roles, as the end-to-end seed has
 * them: tenant-wide parts and parts granted at one akt type. */
const ROLES: Record<string, ReturnType<typeof allow>> = {
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

type Grant = string | { role: string; at: string[] };

/** What each of the examples' people may do, by email; anyone else reads. */
const GRANTS: Record<string, Grant[]> = {
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

const tenant = randomUUID();
await control('PUT', '/catalog', {
	actions: Object.entries(ACTIONS).map(([name, description]) => ({ name, description })),
	scope_kinds: [{ name: 'kuria_akt_type', description: 'An akt type, by its key' }]
});
await control('PUT', `/tenants/${tenant}`, { metadata: { name: 'shop' } });
for (const [key, policies] of Object.entries(ROLES)) {
	await control('PUT', `/tenants/${tenant}/roles/${key}`, { policies });
}

async function member(subject: string, grants: Grant[]) {
	const membership = randomUUID();
	await control('PUT', `/memberships/${membership}`, { subject, tenant });
	for (const grant of grants) {
		if (typeof grant === 'string') {
			await control('POST', `/memberships/${membership}/assignments`, { role: grant });
		} else {
			for (const key of grant.at) {
				await control('POST', `/memberships/${membership}/assignments`, {
					role: grant.role,
					scope: { kind: 'kuria_akt_type', id: key }
				});
			}
		}
	}
}

// The examples' people who sign in: every bundle's, once each, as the
// subjects the bundles name. An example without a bundle names none.
const named = new Map<string, { subject: string; name: string; email: string }>();
for (const dir of examples.filter((d) => existsSync(join(d, 'bundle', 'bundle.json')))) {
	const bundle: { people: { subject: string; name?: string | null; email?: string | null }[] } = JSON.parse(
		readFileSync(join(dir, 'bundle', 'bundle.json'), 'utf8')
	);
	for (const p of bundle.people) {
		if (p.email && !named.has(p.subject)) named.set(p.subject, { subject: p.subject, name: p.name ?? p.email, email: p.email });
	}
}
const people: Person[] = [];
for (const { subject, name, email } of named.values()) {
	const password = `${email.split('@')[0]}'s example password`;
	await control('PUT', `/subjects/${subject}`, { kind: 'human' });
	await control('POST', `/subjects/${subject}/credentials`, { kind: 'password', identifier: email, password });
	await member(subject, GRANTS[email] ?? ['reader']);
	people.push({ name, email, password, subject });
}

async function servicePrincipal(): Promise<{ subject: string; key: string }> {
	const id = randomUUID();
	await control('PUT', `/subjects/${id}`, { kind: 'service' });
	const created = await control('POST', `/subjects/${id}/credentials`, { kind: 'secret_key' });
	return { subject: id, key: created.secret_keys.a };
}
const admin = await servicePrincipal();
await member(admin.subject, ['admin']);
const kuria = await servicePrincipal();

await control('PUT', '/oidc/clients/kuria', {
	kind: 'confidential',
	redirect_uris: [`${APP}/auth/callback`],
	post_logout_redirect_uris: [`${APP}/`],
	audience: 'kuria',
	scopes: ['openid', 'email', 'tenant'],
	refresh: true,
	require_pkce: true
});
const { client_secret: kuriaSecret } = await control('POST', '/oidc/clients/kuria/secret');
await control('PUT', '/oidc/clients/kuria-cli', {
	kind: 'public',
	redirect_uris: [`${APP}/cli/authorized`],
	audience: 'kuria',
	scopes: ['openid', 'email', 'tenant'],
	refresh: true,
	require_pkce: true,
	require_dpop: true
});

const env = {
	KURIA_LISTEN: '127.0.0.1:18291',
	KURIA_PUBLIC_URL: APP,
	KURIA_SECURE_COOKIES: 'false',
	KURIA_LOGIN_APP: 'true',
	KURIA_DEFAULT_TENANT: tenant,
	KONKLAV_ISSUER: APP,
	KONKLAV_AUDIENCE: 'kuria,konklav',
	KONKLAV_SUBJECT_URL: KONKLAV_SUBJECT,
	KONKLAV_RELYING_URL: KONKLAV_RELYING,
	KONKLAV_SERVICE_KEY: kuria.key,
	KURIA_OIDC_CLIENT_ID: 'kuria',
	KURIA_OIDC_CLIENT_SECRET: kuriaSecret,
	KURIA_SESSION_KEY: randomBytes(32).toString('base64'),
	KURIA_RATE_LIMIT_SECRET: randomBytes(32).toString('hex')
};
writeFileSync(
	join(STATE, 'kuria.env'),
	Object.entries(env)
		.map(([key, value]) => `${key}=${value}`)
		.join('\n') + '\n',
	{ mode: 0o600 }
);
const result: Seeded = { tenant, people, admin: { key: admin.key } };
writeFileSync(join(STATE, 'seeded.json'), JSON.stringify(result, null, 2), { mode: 0o600 });
console.log(`seeded the tenant shop (${tenant}); see ${STATE}`);
