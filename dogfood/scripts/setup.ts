// Seeds the dogfooding Konklav once, over its control plane: Kuria's
// actions, the tenant `kuria`, a planner role with every action, you as a
// person who signs in with a password, an agent (a service principal with
// a secret key), and Kuria's OIDC clients. Writes .state/kuria.env,
// .state/agent.env and .state/seeded.json. Does nothing if it has run.
import { execFileSync } from 'node:child_process';
import { randomBytes, randomUUID } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { KONKLAV_CONTROL, KONKLAV_RELYING, KONKLAV_SUBJECT, KURIA, STATE, seeded, type Seeded } from './stack.ts';

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

const ACTIONS = {
	'kuria:akt_types:define': 'Define akt types',
	'kuria:akter:read': "See an akt type's akter",
	'kuria:akter:create': "Create an akt type's akter",
	'kuria:akter:edit': "Edit an akt type's akter",
	'kuria:akter:transition': "Perform an akt type's transitions",
	'kuria:contacts:read': 'List contacts',
	'kuria:contacts:manage': 'Add, edit and erase contacts',
	'kuria:tenant:export': "Export the tenant's akt types and akter as bundles",
	'kuria:tenant:import': 'Import bundles into the tenant'
};
await control('PUT', '/catalog', {
	actions: Object.entries(ACTIONS).map(([name, description]) => ({ name, description })),
	scope_kinds: [{ name: 'kuria_akt_type', description: 'An akt type, by its key' }]
});
const tenant = randomUUID();
await control('PUT', `/tenants/${tenant}`, { metadata: { name: 'kuria' } });
await control('PUT', `/tenants/${tenant}/roles/planner`, {
	policies: Object.keys(ACTIONS).map((action) => ({ action, effect: 'allow', scope_bound: false }))
});

async function member(subject: string) {
	const membership = randomUUID();
	await control('PUT', `/memberships/${membership}`, { subject, tenant });
	await control('POST', `/memberships/${membership}/assignments`, { role: 'planner' });
}

// You: by DOGFOOD_EMAIL, else Git's email. Local only.
const email =
	process.env.DOGFOOD_EMAIL ||
	execFileSync('git', ['config', 'user.email'], { encoding: 'utf8' }).trim() ||
	'you@kuria.localhost';
const password = randomBytes(12).toString('base64url');
const subject = randomUUID();
await control('PUT', `/subjects/${subject}`, { kind: 'human' });
await control('POST', `/subjects/${subject}/credentials`, { kind: 'password', identifier: email, password });
await member(subject);

async function servicePrincipal(): Promise<{ subject: string; key: string }> {
	const id = randomUUID();
	await control('PUT', `/subjects/${id}`, { kind: 'service' });
	const created = await control('POST', `/subjects/${id}/credentials`, { kind: 'secret_key' });
	return { subject: id, key: created.secret_keys.a };
}
const agent = await servicePrincipal();
await member(agent.subject);
const kuria = await servicePrincipal();

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
await control('PUT', '/oidc/clients/kuria-cli', {
	kind: 'public',
	redirect_uris: [`${KURIA}/cli/authorized`],
	audience: 'kuria',
	scopes: ['openid', 'email', 'tenant'],
	refresh: true,
	require_pkce: true,
	require_dpop: true
});

const lines = (env: Record<string, string>) =>
	Object.entries(env)
		.map(([key, value]) => `${key}=${value}`)
		.join('\n') + '\n';
writeFileSync(
	join(STATE, 'kuria.env'),
	lines({
		KURIA_LISTEN: '127.0.0.1:18190',
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
	}),
	{ mode: 0o600 }
);
writeFileSync(
	join(STATE, 'agent.env'),
	lines({ KURIA_URL: KURIA, KURIA_TENANT: tenant, KURIA_KEY: agent.key }),
	{ mode: 0o600 }
);
const result: Seeded = { tenant, person: { email, password, subject }, agent };
writeFileSync(join(STATE, 'seeded.json'), JSON.stringify(result, null, 2), { mode: 0o600 });
console.log(`seeded the tenant kuria (${tenant}); see ${STATE}`);
