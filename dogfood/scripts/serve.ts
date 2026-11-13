// Runs Kuria for dogfooding on http://localhost:18190, against the
// dogfooding Konklav (.state/kuria.env), the database `kuria_dogfood` on
// the development Postgres (made if missing, never dropped), the
// development Valkey and S3 gateway, serving the built app. Builds the
// `kuria` CLI too. Runs until Ctrl-C.
import { execFileSync, spawn } from 'node:child_process';
import { join } from 'node:path';
import { KURIA, ROOT, envFile, seeded } from './stack.ts';

const database = 'kuria_dogfood';
const psql = (sql: string) =>
	execFileSync(
		'docker',
		['compose', '-f', join(ROOT, 'dev', 'compose.yaml'), 'exec', '-T', 'postgres', 'psql', '-U', 'kuria', '-d', 'kuria', '-tAc', sql],
		{ encoding: 'utf8' }
	).trim();
if (psql(`select 1 from pg_database where datname = '${database}'`) !== '1') {
	psql(`create database ${database}`);
	console.log(`created the database ${database}`);
}
execFileSync('cargo', ['build', '-q', '-p', 'kuria-cli'], { cwd: ROOT, stdio: 'inherit' });

const port = process.env.KURIA_DEV_PG_PORT ?? '15442';
const server = spawn('cargo', ['run', '-q', '-p', 'kuria-server'], {
	cwd: ROOT,
	stdio: 'inherit',
	env: {
		...process.env,
		...envFile('kuria.env'),
		DATABASE_URL: `postgres://kuria:kuria@localhost:${port}/${database}`,
		KURIA_WEB_DIR: join(ROOT, 'items', 'web', 'build'),
		RUST_LOG: process.env.RUST_LOG ?? 'info'
	}
});
const stop = () => server.kill('SIGINT');
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
server.on('exit', (code) => process.exit(code ?? 0));

for (let i = 0; ; i++) {
	try {
		if ((await fetch(`${KURIA}/healthz`)).ok) break;
	} catch {
		// Not listening yet; the first start compiles the server.
	}
	if (i > 900) throw new Error('Kuria never answered /healthz');
	await new Promise((resolve) => setTimeout(resolve, 1000));
}
const { person } = seeded()!;
console.log(`
Kuria is at ${KURIA} (use localhost). Sign in as ${person.email} with the
password in dogfood/.state/seeded.json, or from a terminal:

  kuria login --url ${KURIA}

Agents run the CLI as the seeded agent:  mise run dogfood:kuria -- guide
Ctrl-C stops Kuria; mise run dogfood:down stops Konklav, keeping its data.
`);
