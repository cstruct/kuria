/**
 * DPoP proofs (RFC 9449) signed with a WebCrypto P-256 key the page can use
 * but never read: it is created non-extractable and kept in this origin's
 * IndexedDB, so a reload keeps the key and the session bound to it. No
 * token is ever stored, only the key.
 */
// r[impl login.dpop]

const DATABASE = 'kuria-login';
const STORE = 'keys';
const KEY = 'dpop';

const encoder = new TextEncoder();

interface StoredKey {
	privateKey: CryptoKey;
	jwk: { kty: string; crv: string; x: string; y: string };
}

export function b64url(bytes: ArrayBuffer | Uint8Array): string {
	let text = '';
	for (const byte of new Uint8Array(bytes)) text += String.fromCharCode(byte);
	return btoa(text).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function done<T>(operation: IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		operation.onsuccess = () => resolve(operation.result);
		operation.onerror = () => reject(operation.error);
	});
}

async function store(mode: IDBTransactionMode): Promise<IDBObjectStore> {
	const opening = indexedDB.open(DATABASE, 1);
	opening.onupgradeneeded = () => opening.result.createObjectStore(STORE);
	const db = await done(opening);
	return db.transaction(STORE, mode).objectStore(STORE);
}

async function loadOrCreate(): Promise<StoredKey> {
	const stored = (await done((await store('readonly')).get(KEY))) as StoredKey | undefined;
	if (stored) return stored;
	const pair = await crypto.subtle.generateKey(
		{ name: 'ECDSA', namedCurve: 'P-256' },
		false, // non-extractable: scripts can sign, not export
		['sign', 'verify']
	);
	const { kty, crv, x, y } = await crypto.subtle.exportKey('jwk', pair.publicKey);
	const key: StoredKey = {
		privateKey: pair.privateKey,
		jwk: { kty: kty!, crv: crv!, x: x!, y: y! }
	};
	await done((await store('readwrite')).put(key, KEY));
	return key;
}

let pending: Promise<StoredKey> | null = null;
const key = () => (pending ??= loadOrCreate());

/** A proof for one request: `method` and `url` (without query or
 * fragment), with the access token's hash when one is presented. */
export async function proof(method: string, url: string, accessToken?: string): Promise<string> {
	const { privateKey, jwk } = await key();
	const htu = new URL(url, location.href);
	htu.search = '';
	htu.hash = '';
	const claims: Record<string, string | number> = {
		jti: crypto.randomUUID(),
		htm: method,
		htu: htu.href,
		iat: Math.floor(Date.now() / 1000)
	};
	if (accessToken) {
		claims.ath = b64url(await crypto.subtle.digest('SHA-256', encoder.encode(accessToken)));
	}
	const header = { typ: 'dpop+jwt', alg: 'ES256', jwk };
	const input = `${b64url(encoder.encode(JSON.stringify(header)))}.${b64url(encoder.encode(JSON.stringify(claims)))}`;
	// WebCrypto's ECDSA signature is already the raw r || s that ES256 uses.
	const signature = await crypto.subtle.sign(
		{ name: 'ECDSA', hash: 'SHA-256' },
		privateKey,
		encoder.encode(input)
	);
	return `${input}.${b64url(signature)}`;
}
