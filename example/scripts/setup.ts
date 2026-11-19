// Seeds the example stack's Konklav once, over its control plane: Kuria's
// actions and the examples' own, one tenant with the release-management
// example's roles, the examples' people as subjects with the very ids
// their bundles name (so a bundle's people are the tenant's users, with
// no subjects mapping), an admin service principal, and Kuria's OIDC
// clients. Writes .state/kuria.env and .state/seeded.json, which records
// the seed's fingerprint. Does nothing if it has run (up.ts throws
// Konklav's data away when what seed.ts holds has changed since).
import { randomBytes, randomUUID } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ACTIONS, FINGERPRINT, GRANTS, PEOPLE, ROLES, type Grant } from './seed.ts';
import {
	APP,
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

const people: Person[] = [];
for (const { subject, name, email } of PEOPLE) {
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
const result: Seeded = { tenant, people, admin: { key: admin.key }, fingerprint: FINGERPRINT };
writeFileSync(join(STATE, 'seeded.json'), JSON.stringify(result, null, 2), { mode: 0o600 });
console.log(`seeded the tenant shop (${tenant}); see ${STATE}`);
