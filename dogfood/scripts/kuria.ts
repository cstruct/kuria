// Runs the built `kuria` CLI as the dogfooding agent: KURIA_URL,
// KURIA_TENANT and KURIA_KEY from .state/agent.env, unless already set.
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { ROOT, envFile } from './stack.ts';

const binary = join(ROOT, 'target', 'debug', 'kuria');
const ran = spawnSync(binary, process.argv.slice(2), {
	stdio: 'inherit',
	env: { ...envFile('agent.env'), ...process.env }
});
if (ran.error) {
	console.error(`couldn't run ${binary}: ${ran.error.message} (build it: cargo build -p kuria-cli)`);
	process.exit(5);
}
process.exit(ran.status ?? 5);
