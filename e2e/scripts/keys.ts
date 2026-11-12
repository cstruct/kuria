// Creates Konklav's signing keys for the end-to-end stack, unless they
// exist: `signing-key.pem` (Ed25519, for access tokens) and
// `id-token-key.pem` (P-256, for ES256 ID tokens). Test-only keys.
import { generateKeyPairSync } from 'node:crypto';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const directory = join(import.meta.dirname, '..', '.state', 'keys');
mkdirSync(directory, { recursive: true });

function write(name: string, pem: string) {
	const path = join(directory, name);
	if (existsSync(path)) return;
	// Readable by Konklav's non-root user in the container.
	writeFileSync(path, pem, { mode: 0o644 });
	console.log(`wrote ${path}`);
}

const pkcs8 = { type: 'pkcs8', format: 'pem' } as const;
write('signing-key.pem', generateKeyPairSync('ed25519', { privateKeyEncoding: pkcs8 }).privateKey);
write(
	'id-token-key.pem',
	generateKeyPairSync('ec', { namedCurve: 'P-256', privateKeyEncoding: pkcs8 }).privateKey
);
