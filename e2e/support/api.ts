/** Kuria's API as software uses it: a secret-key sign-in to Konklav, moved
 * into the tenant, then GraphQL and attachments with that token. */
import { KONKLAV_SUBJECT, KURIA, seeded, type ServiceClient } from './stack.ts';

/** A tenant token for `client` in the seeded tenant. */
export async function tenantToken(client: ServiceClient): Promise<string> {
	const dot = client.key.indexOf('.');
	const granted = await fetch(`${KONKLAV_SUBJECT}/v1/login/secret-key`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ key_id: client.key.slice(0, dot), secret: client.key.slice(dot + 1) })
	});
	if (!granted.ok) throw new Error(`signing in: ${granted.status} ${await granted.text()}`);
	const { access_token } = (await granted.json()) as { access_token: string };
	const selected = await fetch(`${KONKLAV_SUBJECT}/v1/login/tenant`, {
		method: 'POST',
		headers: { authorization: `Bearer ${access_token}`, 'content-type': 'application/json' },
		body: JSON.stringify({ tenant: seeded().tenant })
	});
	if (!selected.ok) {
		throw new Error(`moving into the tenant: ${selected.status} ${await selected.text()}`);
	}
	return ((await selected.json()) as { access_token: string }).access_token;
}

/** A GraphQL client acting as `client`. */
export async function api(client: ServiceClient) {
	const token = await tenantToken(client);
	return {
		async graphql<T = any>(query: string, variables: Record<string, unknown> = {}): Promise<T> {
			const response = await fetch(`${KURIA}/graphql`, {
				method: 'POST',
				headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
				body: JSON.stringify({ query, variables })
			});
			const body = (await response.json()) as { data?: T; errors?: unknown[] };
			if (!response.ok || body.errors) {
				throw new Error(`${response.status}: ${JSON.stringify(body.errors ?? body)}`);
			}
			return body.data as T;
		},
		async upload(name: string, mediaType: string, body: string): Promise<{ id: string }> {
			const response = await fetch(`${KURIA}/attachments`, {
				method: 'POST',
				headers: {
					authorization: `Bearer ${token}`,
					'x-attachment-name': encodeURIComponent(name),
					'content-type': mediaType
				},
				body
			});
			if (!response.ok) throw new Error(`upload: ${response.status} ${await response.text()}`);
			return (await response.json()) as { id: string };
		}
	};
}
