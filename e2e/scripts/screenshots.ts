// Takes the screenshots in items/web/README.md from the server that
// `mise run e2e:serve` runs, after `mise run e2e:walk` has left a release
// behind: Home, a release candidate's board and artifacts, a backlog's
// cards, the release notes, akter by type, and a candidate at a phone's
// width, in the light appearance and, for some, the dark.
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { chromium, type Page } from '@playwright/test';
import { api } from '../support/api.ts';
import { KURIA, seeded } from '../support/stack.ts';

const OUT = join(import.meta.dirname, '..', '..', 'items', 'web', 'screenshots');
mkdirSync(OUT, { recursive: true });

const admin = await api(seeded().clients.admin);
/** The numbers of up to five akter of `type`, oldest first. */
async function numbers(type: string): Promise<string[]> {
	const data = await admin.graphql<{ akter: { nodes: { number: string }[] } }>(
		'query($t: String!) { akter(type: $t, first: 5) { nodes { number } } }',
		{ t: type }
	);
	return data.akter.nodes.map((a) => a.number).sort();
}
const [candidates, backlogs, notes] = await Promise.all([
	numbers('release_candidate'),
	numbers('backlog'),
	numbers('release_notes')
]);
if (candidates.length < 2 || notes.length === 0) {
	throw new Error('No walked-through release: run `mise run e2e:walk` against this server first.');
}

const browser = await chromium.launch();

/** A browser signed in as the release manager, at `width` in `scheme`. */
async function signedIn(width: number, height: number, colorScheme: 'light' | 'dark'): Promise<Page> {
	const context = await browser.newContext({
		viewport: { width, height },
		colorScheme,
		extraHTTPHeaders: {
			'x-forwarded-for': `203.0.113.${1 + Math.floor(Math.random() * 254)}`
		}
	});
	const page = await context.newPage();
	const { email, password } = seeded().people.releaseManager;
	await page.goto(KURIA);
	await page.getByLabel('Email').fill(email);
	await page.getByLabel('Password').fill(password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await page.getByRole('button', { name: 'Sign out' }).waitFor();
	return page;
}

/** Opens `path`, waits for `ready`, and saves the viewport as `name`. */
async function shoot(page: Page, name: string, path: string, ready: string) {
	await page.goto(`${KURIA}${path}`);
	await page.getByRole('heading', { name: ready }).first().waitFor();
	await page.waitForLoadState('networkidle');
	await page.screenshot({ path: join(OUT, `${name}.png`) });
	console.log(`  ${name}.png`);
}

console.log(`Writing to ${OUT}:`);
for (const scheme of ['light', 'dark'] as const) {
	const page = await signedIn(1440, 960, scheme);
	await shoot(page, `home-${scheme}`, '/', 'Recently changed');
	await shoot(page, `candidate-${scheme}`, `/akt/${candidates[1]}`, 'Members');
	if (scheme === 'light') {
		await shoot(page, 'backlog-light', `/akt/${backlogs[0]}`, 'Members');
		await shoot(page, 'notes-light', `/akt/${notes[0]}`, 'Details');
		await shoot(page, 'list-light', '/types/backlog_item', 'Backlog item');
	}
	await page.context().close();
}
const phone = await signedIn(390, 844, 'light');
await shoot(phone, 'candidate-phone', `/akt/${candidates[1]}`, 'Details');
await browser.close();
