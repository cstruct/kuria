// Runs the Kuria server for the end-to-end tests: against the seeded
// Konklav (.state/kuria.env), a fresh database of its own on the
// development Postgres, the development Valkey and S3 gateway, and serving
// the built app. Playwright starts this and waits for /healthz.
import { execFileSync, spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { STATE } from '../support/stack.ts';

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

const port = process.env.KURIA_DEV_PG_PORT ?? '15442';
const server = spawn('cargo', ['run', '-q', '-p', 'kuria-server'], {
	cwd: root,
	stdio: 'inherit',
	env: {
		...process.env,
		...env,
		DATABASE_URL: `postgres://kuria:kuria@localhost:${port}/${database}`,
		KURIA_WEB_DIR: join(root, 'items', 'web', 'build'),
		RUST_LOG: process.env.RUST_LOG ?? 'info,kuria_server=debug'
	}
});
const stop = () => server.kill('SIGINT');
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
server.on('exit', (code) => process.exit(code ?? 0));
