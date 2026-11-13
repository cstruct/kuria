import { createHash, randomBytes } from 'node:crypto';
import { expect, test } from '@playwright/test';
import { DpopKey } from '../support/dpop.ts';
import { newPage } from '../support/people.ts';
import { KURIA, seeded } from '../support/stack.ts';

const VIEWER = '{ viewer { subject tenant } }';
const REDIRECT_URI = `${KURIA}/cli/authorized`;

async function discovery(): Promise<{ authorization_endpoint: string; token_endpoint: string }> {
	return (await fetch(`${KURIA}/.well-known/openid-configuration`)).json();
}

/** `query` against Kuria with a DPoP-bound `token`. */
async function graphql(key: DpopKey, token: string, query: string) {
	return key.fetch(
		`${KURIA}/graphql`,
		{ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query }) },
		token
	);
}

async function collect(state: string): Promise<Response> {
	return fetch(`${KURIA}/cli/authorized/collect`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ state })
	});
}

// r[verify cli.relay]
// r[verify authz.sender-constrained]
// r[verify auth.dpop]
test('the CLI signs in by a link, with tokens bound to its key', async ({ browser }) => {
	const { tenant, people } = seeded();
	const person = people.developer;
	const { authorization_endpoint, token_endpoint } = await discovery();
	const key = await DpopKey.generate();
	const verifier = randomBytes(32).toString('base64url');
	const state = randomBytes(32).toString('base64url');
	const link = new URL(authorization_endpoint);
	link.search = new URLSearchParams({
		response_type: 'code',
		client_id: 'kuria-cli',
		redirect_uri: REDIRECT_URI,
		scope: `openid tenant:${tenant}`,
		state,
		code_challenge: createHash('sha256').update(verifier).digest('base64url'),
		code_challenge_method: 'S256',
		dpop_jkt: key.thumbprint()
	}).toString();
	expect((await collect(state)).status).toBe(204);

	// The link, opened in a browser of its own, as on another machine.
	const page = await newPage(browser);
	await page.goto(link.toString());
	await page.getByLabel('Email').fill(person.email);
	await page.getByLabel('Password').fill(person.password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'The CLI is signed in' })).toBeVisible();
	await expect(page.getByText('You can return to your terminal.')).toBeVisible();

	const collected = await collect(state);
	expect(collected.status).toBe(200);
	const { code } = (await collected.json()) as { code: string };
	expect((await collect(state)).status).toBe(204);

	const exchanged = await key.fetch(token_endpoint, {
		method: 'POST',
		headers: { 'content-type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			grant_type: 'authorization_code',
			client_id: 'kuria-cli',
			code,
			redirect_uri: REDIRECT_URI,
			code_verifier: verifier
		})
	});
	expect(exchanged.status, await exchanged.clone().text()).toBe(200);
	const tokens = (await exchanged.json()) as { access_token: string; refresh_token: string; token_type: string };
	expect(tokens.token_type).toBe('DPoP');

	const viewer = await graphql(key, tokens.access_token, VIEWER);
	expect(viewer.status).toBe(200);
	expect((await viewer.json()).data.viewer).toEqual({ subject: person.subject, tenant });
	// Without its proof, the token is nothing.
	const bearer = await fetch(`${KURIA}/graphql`, {
		method: 'POST',
		headers: { authorization: `Bearer ${tokens.access_token}`, 'content-type': 'application/json' },
		body: JSON.stringify({ query: VIEWER })
	});
	expect(bearer.status).toBe(401);

	const refreshed = await key.fetch(token_endpoint, {
		method: 'POST',
		headers: { 'content-type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			grant_type: 'refresh_token',
			client_id: 'kuria-cli',
			refresh_token: tokens.refresh_token
		})
	});
	expect(refreshed.status, await refreshed.clone().text()).toBe(200);
	const fresh = (await refreshed.json()) as { access_token: string; token_type: string };
	expect(fresh.token_type).toBe('DPoP');
	expect((await graphql(key, fresh.access_token, VIEWER)).status).toBe(200);
});

// r[verify authz.sender-constrained]
test('software signs in with a secret key, bound to its DPoP key', async () => {
	const { tenant, clients } = seeded();
	const key = await DpopKey.generate();
	const dot = clients.admin.key.indexOf('.');
	const signedIn = await key.fetch(`${KURIA}/v1/login/secret-key`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ key_id: clients.admin.key.slice(0, dot), secret: clients.admin.key.slice(dot + 1) })
	});
	expect(signedIn.status, await signedIn.clone().text()).toBe(200);
	const { access_token } = (await signedIn.json()) as { access_token: string };
	const moved = await key.fetch(
		`${KURIA}/v1/login/tenant`,
		{ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ tenant }) },
		access_token
	);
	expect(moved.status, await moved.clone().text()).toBe(200);
	const token = ((await moved.json()) as { access_token: string }).access_token;

	const viewer = await graphql(key, token, VIEWER);
	expect(viewer.status).toBe(200);
	expect((await viewer.json()).data.viewer.tenant).toBe(tenant);
	// Another key's proof isn't this token's.
	const other = await DpopKey.generate();
	expect((await graphql(other, token, VIEWER)).status).toBe(401);
});
