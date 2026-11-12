// Runs Kuria against the seeded end-to-end Konklav for clicking around in
// by hand: starts the server (scripts/serve.ts, on a fresh database),
// defines the release-management example's akt types once it answers, and
// prints who can sign in. Runs until Ctrl-C.
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { defineExample } from '../support/example.ts';
import { KURIA, seeded } from '../support/stack.ts';

const server = spawn('node', [join(import.meta.dirname, 'serve.ts')], { stdio: 'inherit' });
const stop = () => server.kill('SIGINT');
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
server.on('exit', (code) => process.exit(code ?? 0));

async function healthy(): Promise<boolean> {
	try {
		return (await fetch(`${KURIA}/healthz`)).ok;
	} catch {
		return false;
	}
}

// The first start compiles the server.
for (let i = 0; !(await healthy()); i++) {
	if (i > 900) throw new Error('Kuria never answered /healthz');
	await new Promise((resolve) => setTimeout(resolve, 1000));
}
await defineExample();

const { people } = seeded();
const rows = Object.entries(people).map(([who, p]) => `  ${who.padEnd(15)} ${p.email.padEnd(20)} ${p.password}`);
console.log(`
Kuria is at ${KURIA} (use localhost, not 127.0.0.1), with the
release-management example's akt types. Sign in as:

${rows.join('\n')}

To leave a whole release behind to explore, run the walkthrough against
this server from another terminal:  mise run e2e:walk
Ctrl-C stops the server; mise run e2e:down stops Konklav.
`);
