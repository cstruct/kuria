// Makes, once, what Konklav needs before it starts: its signing keys and
// its secrets (the control plane's bootstrap token, the TOTP key). Kept in
// .state, never committed, and never made again unless `example:reset`
// removes them.
import { generateKeyPairSync, randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { STATE } from './stack.ts';

const keys = join(STATE, 'keys');
mkdirSync(keys, { recursive: true });

function write(path: string, text: string, mode: number) {
	if (existsSync(path)) return;
	writeFileSync(path, text, { mode });
	console.log(`wrote ${path}`);
}

const pkcs8 = { type: 'pkcs8', format: 'pem' } as const;
// Readable by Konklav's non-root user in the container.
write(join(keys, 'signing-key.pem'), generateKeyPairSync('ed25519', { privateKeyEncoding: pkcs8 }).privateKey, 0o644);
write(
	join(keys, 'id-token-key.pem'),
	generateKeyPairSync('ec', { namedCurve: 'P-256', privateKeyEncoding: pkcs8 }).privateKey,
	0o644
);
write(
	join(STATE, 'secrets.styx'),
	`// Made by scripts/prepare.ts for this machine's example stack.
database {
    password konklav
}

control {
    bootstrap_token ${randomBytes(24).toString('hex')}
}

mfa {
    totp_keys ({id example, key "${randomBytes(32).toString('base64')}"})
}
`,
	0o644
);
