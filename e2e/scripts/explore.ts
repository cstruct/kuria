// Runs Kuria against the seeded end-to-end Konklav for clicking around in
// by hand: starts the server (scripts/serve.ts, on a fresh database),
// defines the release-management, test-protocols and documents examples'
// akt types once it answers, and prints who can sign in. Runs until Ctrl-C.
//
// `explore.ts <example>` (`mise run e2e:example -- <example>`) starts the
// server with that example's bundle loaded (docs/examples/<example>/bundle),
// and defines the other examples' types as before.
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { defineDocuments, defineExample, defineTestProtocols } from '../support/example.ts';
import { KURIA, seeded } from '../support/stack.ts';

async function healthy(): Promise<boolean> {
	try {
		return (await fetch(`${KURIA}/healthz`)).ok;
	} catch {
		return false;
	}
}

if (await healthy()) {
	throw new Error(`Something already answers on ${KURIA}: stop it first (an earlier Kuria server?).`);
}

const example = process.argv[2];
if (example && !existsSync(join(import.meta.dirname, '..', '..', 'docs', 'examples', example, 'bundle'))) {
	throw new Error(`docs/examples/${example} holds no bundle: name one of the examples there.`);
}

const server = spawn('node', [join(import.meta.dirname, 'serve.ts')], {
	stdio: 'inherit',
	env: example ? { ...process.env, KURIA_E2E_EXAMPLE: example } : process.env
});
const stop = () => server.kill('SIGINT');
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
server.on('exit', (code) => process.exit(code ?? 0));

// The first start compiles the server.
for (let i = 0; !(await healthy()); i++) {
	if (i > 900) throw new Error('Kuria never answered /healthz');
	await new Promise((resolve) => setTimeout(resolve, 1000));
}
// The loaded example's types came with its bundle.
if (example !== 'release-management') await defineExample();
if (example !== 'test-protocols') await defineTestProtocols();
if (example !== 'documents') await defineDocuments();

const { people } = seeded();
const rows = Object.entries(people).map(([who, p]) => `  ${who.padEnd(15)} ${p.email.padEnd(20)} ${p.password}`);
console.log(`
Kuria is at ${KURIA} (use localhost, not 127.0.0.1), with the
release-management, test-protocols and documents examples' akt types${
	example ? `, and the ${example} example's akter` : ''
}. Sign in as:

${rows.join('\n')}

To leave a whole release behind to explore, run the walkthrough against
this server from another terminal:  mise run e2e:walk
Ctrl-C stops the server; mise run e2e:down stops Konklav.
`);
