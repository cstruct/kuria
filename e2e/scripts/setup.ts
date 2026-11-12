// Seeds the end-to-end Konklav over its control plane, with the bootstrap
// token: the catalog with Kuria's actions, the tenant `shop`, the roles of
// the release-management example, people, Kuria's OIDC client and service
// principal, and service clients for the admin and the build pipeline.
//
// Writes .state/kuria.env (Kuria's configuration) and .state/seeded.json
// (who and what the tests use). Konklav starts empty every run.
import { randomBytes, randomUUID } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
	BOOTSTRAP_TOKEN,
	KONKLAV_CONTROL,
	KONKLAV_RELYING,
	KONKLAV_SUBJECT,
	KURIA,
	STATE,
	type Person,
	type Seeded
} from '../support/stack.ts';

async function control(method: string, path: string, body?: unknown): Promise<any> {
	const response = await fetch(`${KONKLAV_CONTROL}/v1${path}`, {
		method,
		headers: {
			authorization: `Bearer ${BOOTSTRAP_TOKEN}`,
			...(body === undefined ? {} : { 'content-type': 'application/json' })
		},
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	const text = await response.text();
	if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${text}`);
	return text ? JSON.parse(text) : null;
}

async function waitUntilReady() {
	for (let i = 0; i < 180; i++) {
		try {
			if ((await fetch(`${KONKLAV_SUBJECT}/readyz`)).ok) return;
		} catch {
			// Not listening yet.
		}
		await new Promise((resolve) => setTimeout(resolve, 1000));
	}
	throw new Error('Konklav never became ready');
}

const ACTIONS = {
	'kuria:akt_types:define': 'Define akt types',
	'kuria:akter:read': "See an akt type's akter",
	'kuria:akter:create': "Create an akt type's akter",
	'kuria:akter:edit': "Edit an akt type's akter",
	'kuria:akter:transition': "Perform an akt type's transitions",
	'kuria:contacts:read': 'List contacts',
	'kuria:contacts:manage': 'Add, edit and erase contacts',
	'release:approve': 'Release, deploy hotfixes and approve release notes'
};

const allow = (actions: string[], scopeBound = false) =>
	actions.map((action) => ({ action, effect: 'allow', scope_bound: scopeBound }));

/** The example's roles (docs/examples/release-management/README.md), as
 * tenant-wide parts and parts granted at one akt type. */
const ROLES: Record<string, ReturnType<typeof allow>> = {
	admin: allow(Object.keys(ACTIONS)),
	reader: allow(['kuria:akter:read']),
	'contacts-manager': allow(['kuria:contacts:manage']),
	'developer-work': allow(['kuria:akter:create', 'kuria:akter:edit', 'kuria:akter:transition'], true),
	'release-work': allow(
		['kuria:akter:create', 'kuria:akter:edit', 'kuria:akter:transition', 'release:approve'],
		true
	),
	'pipeline-work': allow(['kuria:akter:read', 'kuria:akter:create'], true)
};

type Grant = string | { role: string; at: string[] };

async function member(tenant: string, subject: string, grants: Grant[]) {
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

async function person(
	tenant: string,
	email: string,
	grants: Grant[]
): Promise<Person> {
	const subject = randomUUID();
	const password = `${email.split('@')[0]}'s end-to-end password`;
	await control('PUT', `/subjects/${subject}`, { kind: 'human' });
	await control('POST', `/subjects/${subject}/credentials`, {
		kind: 'password',
		identifier: email,
		password
	});
	await member(tenant, subject, grants);
	return { email, password, subject };
}

async function servicePrincipal(): Promise<{ subject: string; key: string }> {
	const subject = randomUUID();
	await control('PUT', `/subjects/${subject}`, { kind: 'service' });
	const created = await control('POST', `/subjects/${subject}/credentials`, { kind: 'secret_key' });
	return { subject, key: created.secret_keys.a };
}

/** Software calling Kuria: a service principal with a secret key and
 * `grants` in the tenant. It signs in to Konklav directly, so its tokens
 * carry Konklav's own audience, which Kuria accepts beside its client's. */
async function software(tenant: string, grants: Grant[]) {
	const principal = await servicePrincipal();
	await member(tenant, principal.subject, grants);
	return { key: principal.key };
}

await waitUntilReady();
await control('PUT', '/catalog', {
	actions: Object.entries(ACTIONS).map(([name, description]) => ({ name, description })),
	scope_kinds: [{ name: 'kuria_akt_type', description: 'An akt type, by its key' }]
});
const tenant = randomUUID();
await control('PUT', `/tenants/${tenant}`, { metadata: { name: 'shop' } });
for (const [key, policies] of Object.entries(ROLES)) {
	await control('PUT', `/tenants/${tenant}/roles/${key}`, { policies });
}

const people = {
	developer: await person(tenant, 'dev@shop.example', [
		'reader',
		{ role: 'developer-work', at: ['backlog', 'epic', 'backlog_item'] }
	]),
	releaseManager: await person(tenant, 'rm@shop.example', [
		'reader',
		'contacts-manager',
		{ role: 'release-work', at: ['release', 'release_candidate', 'hotfix', 'release_notes'] }
	]),
	headOfProduct: await person(tenant, 'hedda@shop.example', ['reader'])
};

// Kuria itself: its browser sign-in client and its relying client's
// service principal.
await control('PUT', '/oidc/clients/kuria', {
	kind: 'confidential',
	redirect_uris: [`${KURIA}/auth/callback`],
	post_logout_redirect_uris: [`${KURIA}/`],
	audience: 'kuria',
	scopes: ['openid', 'email', 'tenant'],
	refresh: true,
	require_pkce: true
});
const { client_secret: kuriaSecret } = await control('POST', '/oidc/clients/kuria/secret');
const kuria = await servicePrincipal();

const clients = {
	admin: await software(tenant, ['admin']),
	pipeline: await software(tenant, [{ role: 'pipeline-work', at: ['artifact'] }])
};

mkdirSync(STATE, { recursive: true });
const env = {
	KURIA_LISTEN: '127.0.0.1:18090',
	KURIA_PUBLIC_URL: KURIA,
	KURIA_SECURE_COOKIES: 'false',
	KURIA_LOGIN_APP: 'true',
	KURIA_DEFAULT_TENANT: tenant,
	KONKLAV_ISSUER: KURIA,
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
const result: Seeded = { tenant, people, clients };
writeFileSync(join(STATE, 'seeded.json'), JSON.stringify(result, null, 2), { mode: 0o600 });
console.log(`seeded tenant ${tenant}; wrote ${STATE}/kuria.env and seeded.json`);
