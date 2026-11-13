/** DPoP (RFC 9449) as `kuria` and software use it: an ES256 key, and a
 * proof for each request sent with it. */
import { createHash, randomUUID, webcrypto } from 'node:crypto';

const { subtle } = webcrypto;

const base64url = (bytes: ArrayBuffer | Uint8Array | string) => Buffer.from(bytes as Uint8Array).toString('base64url');

/** Proofs and requests made with one P-256 key. */
export class DpopKey {
	private nonces = new Map<string, string>();

	private constructor(
		private readonly privateKey: webcrypto.CryptoKey,
		readonly jwk: { kty: string; crv: string; x: string; y: string }
	) {}

	static async generate(): Promise<DpopKey> {
		const pair = await subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
		const { kty, crv, x, y } = await subtle.exportKey('jwk', pair.publicKey);
		return new DpopKey(pair.privateKey, { kty: kty!, crv: crv!, x: x!, y: y! });
	}

	/** The key's JWK thumbprint (RFC 7638), as `dpop_jkt` names it. */
	thumbprint(): string {
		const { crv, kty, x, y } = this.jwk;
		return base64url(createHash('sha256').update(JSON.stringify({ crv, kty, x, y })).digest());
	}

	/** A proof for `method` at `url`, bound to `accessToken` if given. */
	async proof(method: string, url: string, accessToken?: string): Promise<string> {
		const target = new URL(url);
		const htu = `${target.origin}${target.pathname}`;
		const header = { typ: 'dpop+jwt', alg: 'ES256', jwk: this.jwk };
		const nonce = this.nonces.get(target.origin);
		const payload = {
			jti: randomUUID(),
			htm: method,
			htu,
			iat: Math.floor(Date.now() / 1000),
			...(nonce ? { nonce } : {}),
			...(accessToken ? { ath: base64url(createHash('sha256').update(accessToken).digest()) } : {})
		};
		const input = `${base64url(JSON.stringify(header))}.${base64url(JSON.stringify(payload))}`;
		// WebCrypto signs ECDSA as r || s, which is what JWS wants.
		const signature = await subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, this.privateKey, Buffer.from(input));
		return `${input}.${base64url(signature)}`;
	}

	/** `fetch` with a proof, and with `accessToken` under the `DPoP` scheme
	 * if given. A server asking for a nonce gets one more try with it. */
	async fetch(url: string, init: RequestInit = {}, accessToken?: string): Promise<Response> {
		const method = init.method ?? 'GET';
		for (let attempt = 0; ; attempt++) {
			const headers = new Headers(init.headers);
			headers.set('dpop', await this.proof(method, url, accessToken));
			if (accessToken) headers.set('authorization', `DPoP ${accessToken}`);
			const response = await fetch(url, { ...init, headers });
			const nonce = response.headers.get('dpop-nonce');
			if (nonce) this.nonces.set(new URL(url).origin, nonce);
			if (attempt === 0 && nonce && (response.status === 400 || response.status === 401)) continue;
			return response;
		}
	}
}
