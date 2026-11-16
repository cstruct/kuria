// Runs Kuria against the seeded end-to-end Konklav for clicking around in
// by hand: starts the server (scripts/serve.ts, on a fresh database),
// defines the release-management, test-protocols and documents examples'
// akt types once it answers, and prints who can sign in. Runs until Ctrl-C.
//
// `explore.ts <example>` (`mise run e2e:example <example>`) starts the
// server with only that example loaded, from its bundle
// (docs/examples/<example>/bundle): its akt types and akter, and no other
// example's types. It checks the tenant holds just the bundle's types.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { api } from '../support/api.ts';
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
const bundle = example && join(import.meta.dirname, '..', '..', 'docs', 'examples', example, 'bundle');
if (bundle && !existsSync(bundle)) {
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
if (bundle) {
	// The example's types came with its bundle, and are all there is.
	const expected: string[] = JSON.parse(readFileSync(join(bundle, 'bundle.json'), 'utf8')).types;
	const admin = await api(seeded().clients.admin);
	const { aktTypes } = await admin.graphql<{ aktTypes: { key: string }[] }>('{ aktTypes { key } }');
	const held = aktTypes.map((t) => t.key).sort();
	if (JSON.stringify(held) !== JSON.stringify([...expected].sort())) {
		stop();
		throw new Error(`the tenant holds the akt types ${held.join(', ')}, not just the ${example} bundle's ${expected.join(', ')}`);
	}
} else {
	await defineExample();
	await defineTestProtocols();
	await defineDocuments();
}

const { people } = seeded();
const rows = Object.entries(people).map(([who, p]) => `  ${who.padEnd(15)} ${p.email.padEnd(20)} ${p.password}`);
console.log(`
Kuria is at ${KURIA} (use localhost, not 127.0.0.1), with ${
	example
		? `the ${example} example's akt types and akter`
		: "the release-management, test-protocols and documents examples' akt types"
}. Sign in as:

${rows.join('\n')}

To leave a whole release behind to explore, run the walkthrough against
this server from another terminal:  mise run e2e:walk
Ctrl-C stops the server; mise run e2e:down stops Konklav.
`);
