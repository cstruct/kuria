// Serves one example (`mise run example <example>`): the database
// `kuria_example` on the development Postgres made again, the Kuria server
// on 127.0.0.1:18291 with the example's bundle bootstrapped into the seeded
// tenant (`bundle.bootstrap.*`), or, for an example without one, its akt
// type documents defined there, and the app from Vite's dev server on
// http://localhost:18290, hot-reloading, passing the API to the server.
// Checks the tenant holds just the example's akt types, then prints who
// can sign in. Runs until Ctrl-C, which stops the server and Vite.
import { type ChildProcess, execFileSync, spawn } from 'node:child_process';
import { constants, copyFileSync, existsSync, readFileSync, readdirSync, statSync, utimesSync } from 'node:fs';
import { join } from 'node:path';
import { APP, EXAMPLES, KONKLAV_SUBJECT, KURIA, ROOT, STATE, envFile, seeded } from './stack.ts';

const started = Date.now();
const elapsed = () => `${((Date.now() - started) / 1000).toFixed(1)}s`;

const example = process.argv[2];
const dir = example ? join(EXAMPLES, example) : '';
/** The example's akt type documents, by key, from its own files. */
const documents = example ? typeDocuments(dir) : new Map<string, unknown>();
const bundle = join(dir, 'bundle');
const bundled = Boolean(example) && existsSync(join(bundle, 'bundle.json'));
if (!example || (!bundled && documents.size === 0)) {
	const names = readdirSync(EXAMPLES).filter((name) => typeDocuments(join(EXAMPLES, name)).size > 0);
	throw new Error(`name an example to serve: ${names.join(', ')}`);
}
const stack = seeded();
if (!stack) throw new Error('the example stack is not seeded: run it through `mise run example`');

// Signing in to Konklav takes a while, so it starts now, for the check
// once the server answers.
const token = tenantToken();
token.catch(() => {});

for (const url of [`${KURIA}/healthz`, `${APP}/healthz`]) {
	if (await answers(url)) throw new Error(`Something already answers on ${url}: stop it first (an earlier example?).`);
}

// A fresh tenant for every start: the bootstrap goes only into one.
const database = 'kuria_example';
execFileSync(
	'docker',
	[
		...['compose', '-f', join(ROOT, 'dev', 'compose.yaml'), 'exec', '-T', 'postgres'],
		...['psql', '-q', '-U', 'kuria', '-d', 'kuria'],
		...['-c', `drop database if exists ${database} with (force)`, '-c', `create database ${database}`]
	],
	{ stdio: 'inherit' }
);

const children: ChildProcess[] = [];
function run(command: string, args: string[], options: { cwd: string; env: NodeJS.ProcessEnv }) {
	const child = spawn(command, args, { ...options, stdio: 'inherit' });
	children.push(child);
	// Either stopping stops both.
	child.on('exit', (code) => {
		stop();
		process.exitCode ||= code ?? 0;
	});
	return child;
}
let stopping = false;
function stop() {
	if (stopping) return;
	stopping = true;
	for (const child of children) if (child.exitCode === null) child.kill('SIGINT');
}
process.on('SIGINT', stop);
process.on('SIGTERM', stop);

run(join(ROOT, 'items', 'web', 'node_modules', '.bin', 'vite'), ['dev', '--port', new URL(APP).port, '--strictPort'], {
	cwd: join(ROOT, 'items', 'web'),
	env: { ...process.env, KURIA_DEV_SERVER: KURIA }
});

// Built under mise's environment, as every stack builds it (another makes
// cargo rebuild everything), then run from a copy that changes only when
// the build does: on macOS, a binary cargo has just looked at, even
// without rebuilding it, takes seconds longer to start (`cargo run` too).
const built = spawn('cargo', ['build', '-q', '-p', 'kuria-server'], { cwd: ROOT, stdio: 'inherit' });
if ((await new Promise((resolve) => built.on('exit', resolve))) !== 0) {
	stop();
	throw new Error('building kuria-server failed');
}
const server = join(STATE, 'kuria-server');
const target = join(process.env.CARGO_TARGET_DIR ?? join(ROOT, 'target'), 'debug', 'kuria-server');
const now = statSync(target);
const copied = existsSync(server) ? statSync(server) : null;
if (copied?.mtimeMs !== now.mtimeMs || copied?.size !== now.size) {
	// A clone on APFS, so instant.
	copyFileSync(target, server, constants.COPYFILE_FICLONE);
	utimesSync(server, now.atime, now.mtime);
}
const port = process.env.KURIA_DEV_PG_PORT ?? '15442';
run(server, [], {
	cwd: ROOT,
	env: {
		...process.env,
		...envFile('kuria.env'),
		DATABASE_URL: `postgres://kuria:kuria@localhost:${port}/${database}`,
		...(bundled ? { KURIA_BOOTSTRAP_BUNDLE: bundle } : {}),
		RUST_LOG: process.env.RUST_LOG ?? 'info'
	}
});

// The server first; then through Vite, which would log every request it
// couldn't pass on.
for (const url of [`${KURIA}/healthz`, `${APP}/healthz`]) {
	for (let i = 0; !(await answers(url)); i++) {
		if (stopping) process.exit();
		if (i > 9000) throw new Error(`nothing answered ${url}`);
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
}

// Without a bundle, its akt types defined, each once those it names are.
if (!bundled) await define(await token, documents);

// Only this example: its types, and nothing else.
const expected: string[] = bundled
	? JSON.parse(readFileSync(join(bundle, 'bundle.json'), 'utf8')).types
	: [...documents.keys()];
const { aktTypes } = await graphql<{ aktTypes: { key: string }[] }>(await token, '{ aktTypes { key } }');
const held = aktTypes.map((t) => t.key).sort();
if (JSON.stringify(held) !== JSON.stringify([...expected].sort())) {
	stop();
	throw new Error(`the tenant holds the akt types ${held.join(', ')}, not just the ${example} example's ${expected.join(', ')}`);
}

const rows = stack.people.map((p) => `  ${p.name.padEnd(13)} ${p.email.padEnd(20)} ${p.password}`);
console.log(`
The ${example} example is at ${APP} (use localhost), ready in ${elapsed()},
with the app from Vite's dev server: edits under items/web show at once.
Sign in as:

${rows.join('\n')}

Ctrl-C stops Kuria and Vite; mise run example:down stops Konklav, keeping it.
`);

async function answers(url: string): Promise<boolean> {
	try {
		return (await fetch(url)).ok;
	} catch {
		return false;
	}
}

/** A tenant token for the seeded admin. */
async function tenantToken(): Promise<string> {
	const key = stack!.admin.key;
	const dot = key.indexOf('.');
	const login = await fetch(`${KONKLAV_SUBJECT}/v1/login/secret-key`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ key_id: key.slice(0, dot), secret: key.slice(dot + 1) })
	});
	if (!login.ok) throw new Error(`signing in: ${login.status} ${await login.text()}`);
	const { access_token } = (await login.json()) as { access_token: string };
	const moved = await fetch(`${KONKLAV_SUBJECT}/v1/login/tenant`, {
		method: 'POST',
		headers: { authorization: `Bearer ${access_token}`, 'content-type': 'application/json' },
		body: JSON.stringify({ tenant: stack!.tenant })
	});
	if (!moved.ok) throw new Error(`moving into the tenant: ${moved.status} ${await moved.text()}`);
	return ((await moved.json()) as { access_token: string }).access_token;
}

/** The akt type documents in `dir`, by key: each JSON file that is one. */
function typeDocuments(dir: string): Map<string, unknown> {
	const found = new Map<string, unknown>();
	if (!existsSync(dir) || !statSync(dir).isDirectory()) return found;
	for (const name of readdirSync(dir).filter((n) => n.endsWith('.json')).sort()) {
		try {
			const document = JSON.parse(readFileSync(join(dir, name), 'utf8')) as { key?: unknown; states?: unknown };
			if (typeof document.key === 'string' && Array.isArray(document.states)) found.set(document.key, document);
		} catch {
			// Not JSON, or not a type document: not one of its types.
		}
	}
	return found;
}

/** Defines `documents` with `token`, again and again while one that names
 * a type not yet defined is refused and another goes in. */
async function define(token: string, documents: Map<string, unknown>) {
	let left = [...documents.entries()];
	while (left.length > 0) {
		const refused: [string, unknown][] = [];
		let last = '';
		for (const [key, document] of left) {
			try {
				await graphql(token, 'mutation($d: JSON!) { defineAktType(document: $d) { key } }', { d: document });
			} catch (e) {
				refused.push([key, document]);
				last = `${key}: ${e instanceof Error ? e.message : String(e)}`;
			}
		}
		if (refused.length === left.length) {
			stop();
			throw new Error(`the ${example} example's akt types can't be defined: ${last}`);
		}
		left = refused;
	}
}

/** GraphQL against the server with `token`. */
async function graphql<T>(token: string, query: string, variables: Record<string, unknown> = {}): Promise<T> {
	const response = await fetch(`${KURIA}/graphql`, {
		method: 'POST',
		headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' },
		body: JSON.stringify({ query, variables })
	});
	const body = (await response.json()) as { data?: T; errors?: unknown[] };
	if (!response.ok || body.errors) throw new Error(`${response.status}: ${JSON.stringify(body.errors ?? body)}`);
	return body.data as T;
}
