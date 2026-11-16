// Starts the example stack's Konklav, keeping the one already running when
// it is this checkout's: compose takes seconds even when nothing changes.
//
// Every checkout and worktree shares the compose project `kuria-example`,
// its ports and its data volume, but each has its own `.state/`. So the
// Konklav found may hold another checkout's seeding, or none this one
// recorded: unless it takes this checkout's bootstrap token and holds the
// tenant `.state/seeded.json` names, its data is thrown away and setup
// seeds it again.
import { execFileSync } from 'node:child_process';
import { readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { KONKLAV_CONTROL, KONKLAV_SUBJECT, STATE, seeded } from './stack.ts';

const compose = ['compose', '-f', join(import.meta.dirname, '..', 'compose.yaml')];
const docker = (...args: string[]) => execFileSync('docker', [...compose, ...args], { stdio: 'inherit' });

async function ready(): Promise<boolean> {
	try {
		return (await fetch(`${KONKLAV_SUBJECT}/readyz`)).ok;
	} catch {
		return false;
	}
}

async function untilReady() {
	for (let i = 0; !(await ready()); i++) {
		if (i > 1800) throw new Error('Konklav never became ready');
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
}

/** Whether the running Konklav holds what this checkout seeded. */
async function ours(): Promise<boolean> {
	const stack = seeded();
	if (!stack) return false;
	const token = /bootstrap_token (\S+)/.exec(readFileSync(join(STATE, 'secrets.styx'), 'utf8'))![1];
	const response = await fetch(`${KONKLAV_CONTROL}/v1/tenants/${stack.tenant}`, {
		headers: { authorization: `Bearer ${token}` }
	});
	return response.ok;
}

function start() {
	docker('up', '-d', '--wait', '--wait-timeout', '300', 'postgres');
	docker('up', '-d', 'konklav');
}

if (!(await ready())) {
	if (seeded()) {
		// Stopped by `example:down`, most likely: its data is kept.
		start();
		await untilReady();
	}
}
if (!(await ready()) || !(await ours())) {
	if (seeded() || (await ready())) {
		console.log("the example stack's Konklav holds another checkout's data, or none this one seeded: starting it afresh");
	}
	docker('down', '--volumes');
	rmSync(join(STATE, 'seeded.json'), { force: true });
	rmSync(join(STATE, 'kuria.env'), { force: true });
	start();
}
