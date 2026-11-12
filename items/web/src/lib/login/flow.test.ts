import { describe, expect, it } from 'vitest';
import { Konklav, type LoginRequest } from './konklav';
import { cancel, signIn, start } from './flow';

const ORIGIN = 'https://kuria.example';
const TENANT = '01900000-0000-7000-8000-000000000001';

interface Call {
	method: string;
	path: string;
	headers: Record<string, string>;
	body: unknown;
}

/** Konklav's API as a test describes it: an answer per method and path. */
function konklav(
	answers: Record<string, (body: unknown) => [number, unknown]>,
	request: Partial<LoginRequest> = {}
) {
	const calls: Call[] = [];
	const routes: Record<string, (body: unknown) => [number, unknown]> = {
		'GET /v1/oauth2/login-requests/r1': () => [
			200,
			{
				client_id: 'kuria',
				scopes: ['openid'],
				tenant: null,
				prompt: null,
				login_hint: null,
				...request
			}
		],
		'POST /v1/refresh': () => [401, { error: { code: 'UNAUTHENTICATED', message: 'no session' } }],
		'POST /v1/oauth2/login-requests/r1/accept': () => [200, { redirect_to: `${ORIGIN}/resume` }],
		'POST /v1/oauth2/login-requests/r1/reject': (body) => [
			200,
			{ redirect_to: `${ORIGIN}/refused?error=${(body as { error: string }).error}` }
		],
		...answers
	};
	const fetcher = (async (url: string, init: RequestInit) => {
		const path = new URL(url).pathname;
		const method = init.method ?? 'GET';
		const body = init.body ? JSON.parse(init.body as string) : undefined;
		calls.push({ method, path, headers: init.headers as Record<string, string>, body });
		const route = routes[`${method} ${path}`];
		const [status, data] = route ? route(body) : [404, { error: { message: 'no route' } }];
		return new Response(JSON.stringify(data), { status });
	}) as typeof fetch;
	const client = new Konklav(ORIGIN, async (method) => `proof-for-${method}`, fetcher);
	return { client, calls };
}

const session = (token: string) => () =>
	[200, { access_token: token, token_kind: 'tenant' }] as [number, unknown];

describe('the login page', () => {
	// r[verify login.request]
	it('signs nobody in without a live request', async () => {
		const { client } = konklav({});
		expect(await start(client, null)).toMatchObject({ kind: 'message' });
		const expired = konklav({ 'GET /v1/oauth2/login-requests/r1': () => [404, {}] });
		expect(await start(expired.client, 'r1')).toMatchObject({
			kind: 'message',
			text: expect.stringContaining('expired')
		});
	});

	// r[verify login.password]
	// r[verify login.accept]
	// r[verify login.dpop]
	it('signs in with a password and accepts the request', async () => {
		const { client, calls } = konklav({ 'POST /v1/login/password': session('t1') });
		const form = await start(client, 'r1');
		expect(form).toMatchObject({ kind: 'form', request: { client_id: 'kuria' } });
		if (form.kind !== 'form') throw new Error('expected the form');
		const done = await signIn(client, 'r1', form.request, 'ada@shop.example', 'pw');
		expect(done).toEqual({ kind: 'redirect', to: `${ORIGIN}/resume` });
		const login = calls.find((c) => c.path === '/v1/login/password')!;
		expect(login.body).toEqual({
			identifier: 'ada@shop.example',
			password: 'pw',
			client: 'browser'
		});
		for (const call of calls) {
			expect(call.headers['x-konklav-client']).toBe('kuria-login');
			expect(call.headers.dpop).toBe(`proof-for-${call.method}`);
		}
		const accept = calls.find((c) => c.path.endsWith('/accept'))!;
		expect(accept.headers.authorization).toBe('DPoP t1');
	});

	// r[verify login.password]
	it('says only that the email or password is wrong, and refuses second factors', async () => {
		const wrong = konklav({
			'POST /v1/login/password': () => [
				401,
				{ error: { code: 'INVALID_CREDENTIALS', message: 'no' } }
			]
		});
		const form = await start(wrong.client, 'r1');
		if (form.kind !== 'form') throw new Error('expected the form');
		expect(await signIn(wrong.client, 'r1', form.request, 'a', 'b')).toMatchObject({
			kind: 'form',
			error: 'Wrong email or password.'
		});
		const mfa = konklav({
			'POST /v1/login/password': () => [200, { access_token: 'p', token_kind: 'partial' }]
		});
		expect(await signIn(mfa.client, 'r1', form.request, 'a', 'b')).toMatchObject({
			kind: 'form',
			error: expect.stringContaining('second factor')
		});
		expect(mfa.calls.some((c) => c.path.endsWith('/accept'))).toBe(false);
	});

	// r[verify login.single-sign-on]
	it('accepts at once with a restored session, unless asked to sign in again', async () => {
		const { client, calls } = konklav({ 'POST /v1/refresh': session('t2') });
		expect(await start(client, 'r1')).toEqual({ kind: 'redirect', to: `${ORIGIN}/resume` });
		expect(calls.some((c) => c.path === '/v1/login/password')).toBe(false);

		const again = konklav({ 'POST /v1/refresh': session('t2') }, { prompt: 'login' });
		expect(await start(again.client, 'r1')).toMatchObject({ kind: 'form' });
		expect(again.calls.some((c) => c.path === '/v1/refresh')).toBe(false);
	});

	// r[verify login.prompt-none]
	it('refuses with login_required when it may not ask', async () => {
		const { client } = konklav({}, { prompt: 'none' });
		expect(await start(client, 'r1')).toEqual({
			kind: 'redirect',
			to: `${ORIGIN}/refused?error=login_required`
		});
	});

	// r[verify login.tenant]
	it('moves the session into the requested tenant before accepting', async () => {
		const { client, calls } = konklav(
			{ 'POST /v1/refresh': session('t1'), 'POST /v1/login/tenant': session('t-tenant') },
			{ tenant: TENANT }
		);
		expect(await start(client, 'r1')).toEqual({ kind: 'redirect', to: `${ORIGIN}/resume` });
		const paths = calls.map((c) => c.path);
		expect(paths.indexOf('/v1/login/tenant')).toBeLessThan(
			paths.indexOf('/v1/oauth2/login-requests/r1/accept')
		);
		expect(calls.find((c) => c.path === '/v1/login/tenant')!.body).toEqual({ tenant: TENANT });
		expect(calls.find((c) => c.path.endsWith('/accept'))!.headers.authorization).toBe(
			'DPoP t-tenant'
		);

		const outsider = konklav(
			{
				'POST /v1/refresh': session('t1'),
				'POST /v1/login/tenant': () => [403, { error: { code: 'FORBIDDEN', message: 'no' } }]
			},
			{ tenant: TENANT }
		);
		expect(await start(outsider.client, 'r1')).toMatchObject({
			kind: 'form',
			error: expect.stringContaining("aren't a member")
		});
	});

	// r[verify login.accept]
	it('refuses with access_denied when the person cancels', async () => {
		const { client } = konklav({});
		expect(await cancel(client, 'r1')).toEqual({
			kind: 'redirect',
			to: `${ORIGIN}/refused?error=access_denied`
		});
	});
});
