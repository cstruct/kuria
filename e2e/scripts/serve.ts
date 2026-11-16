// Runs the Kuria server for the end-to-end tests: against the seeded
// Konklav (.state/kuria.env), a fresh database of its own on the
// development Postgres, the development Valkey and S3 gateway, and serving
// the built app; and builds the `kuria` CLI. Playwright starts this and
// waits for /healthz. With KURIA_E2E_EXAMPLE naming an example in
// docs/examples, the server starts with its bundle bootstrapped into the
// seeded tenant, its people mapped by email to the seeded ones.
import { execFileSync, spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { STATE, seeded } from '../support/stack.ts';

const root = join(import.meta.dirname, '..', '..');
const env: Record<string, string> = Object.fromEntries(
	readFileSync(join(STATE, 'kuria.env'), 'utf8')
		.split('\n')
		.filter(Boolean)
		.map((line) => [line.slice(0, line.indexOf('=')), line.slice(line.indexOf('=') + 1)])
);

const database = 'kuria_e2e';
const psql = (sql: string) =>
	execFileSync(
		'docker',
		[
			'compose',
			'-f',
			join(root, 'dev', 'compose.yaml'),
			'exec',
			'-T',
			'postgres',
			'psql',
			'-U',
			'kuria',
			'-d',
			'kuria',
			'-c',
			sql
		],
		{ stdio: 'inherit' }
	);
psql(`drop database if exists ${database} with (force)`);
psql(`create database ${database}`);

// The CLI, which tests/cli.spec.ts runs against this server.
execFileSync('cargo', ['build', '-q', '-p', 'kuria-cli'], { cwd: root, stdio: 'inherit' });

const example = process.env.KURIA_E2E_EXAMPLE;
const bootstrap: Record<string, string> = example ? exampleBootstrap(example) : {};

/** The variables that bootstrap `example`'s bundle (`bundle.bootstrap.*`). */
function exampleBootstrap(example: string): Record<string, string> {
	const bundle = join(root, 'docs', 'examples', example, 'bundle');
	const people: { subject: string; email?: string | null }[] = JSON.parse(
		readFileSync(join(bundle, 'bundle.json'), 'utf8')
	).people;
	const { tenant, people: seededPeople } = seeded();
	const byEmail = new Map(Object.values(seededPeople).map((p) => [p.email.toLowerCase(), p.subject]));
	const subjects = Object.fromEntries(
		people.flatMap((p) => {
			const user = p.email ? byEmail.get(p.email.toLowerCase()) : undefined;
			return user ? [[p.subject, user]] : [];
		})
	);
	const mapping = join(STATE, 'example-subjects.json');
	writeFileSync(mapping, JSON.stringify(subjects, null, 2));
	return {
		KURIA_BOOTSTRAP_BUNDLE: bundle,
		KURIA_BOOTSTRAP_TENANT: tenant,
		KURIA_BOOTSTRAP_SUBJECTS: mapping
	};
}

const port = process.env.KURIA_DEV_PG_PORT ?? '15442';
const server = spawn('cargo', ['run', '-q', '-p', 'kuria-server'], {
	cwd: root,
	stdio: 'inherit',
	env: {
		...process.env,
		...env,
		...bootstrap,
		DATABASE_URL: `postgres://kuria:kuria@localhost:${port}/${database}`,
		KURIA_WEB_DIR: join(root, 'items', 'web', 'build'),
		RUST_LOG: process.env.RUST_LOG ?? 'info,kuria_server=debug'
	}
});
const stop = () => server.kill('SIGINT');
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
server.on('exit', (code) => process.exit(code ?? 0));
