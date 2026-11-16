// Starts the example stack's Konklav unless it already answers: compose
// takes seconds even when nothing changes, and a running Konklav is kept
// between starts.
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { KONKLAV_SUBJECT } from './stack.ts';

let ready = false;
try {
	ready = (await fetch(`${KONKLAV_SUBJECT}/readyz`)).ok;
} catch {
	// Not running.
}
if (!ready) {
	const compose = ['compose', '-f', join(import.meta.dirname, '..', 'compose.yaml')];
	execFileSync('docker', [...compose, 'up', '-d', '--wait', '--wait-timeout', '300', 'postgres'], { stdio: 'inherit' });
	execFileSync('docker', [...compose, 'up', '-d', 'konklav'], { stdio: 'inherit' });
}
