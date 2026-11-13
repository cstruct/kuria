import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { Cli, finished, waitForLine } from '../support/cli.ts';
import { newPage } from '../support/people.ts';
import { KURIA, seeded } from '../support/stack.ts';

// r[verify cli.login]
// r[verify cli.secret]
// r[verify cli.dpop]
// r[verify cli.discovery]
// r[verify cli.profile]
test('kuria login signs in by a link, refreshes, and logs out', async ({ browser }) => {
	const { tenant, people } = seeded();
	const person = people.developer;
	const cli = new Cli();

	// The tenant is Kuria's default one: only the URL is named.
	const login = cli.start(['login', '--no-browser']);
	const done = finished(login);
	const link = await waitForLine(login, /^https?:\/\//);
	expect(link).toContain('client_id=kuria-cli');
	expect(link).toContain('dpop_jkt=');

	const page = await newPage(browser);
	await page.goto(link);
	await page.getByLabel('Email').fill(person.email);
	await page.getByLabel('Password').fill(person.password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'The CLI is signed in' })).toBeVisible();

	const signedIn = await done;
	expect(signedIn.code, signedIn.stderr).toBe(0);
	expect(signedIn.stdout).toContain(`Signed in to ${KURIA}`);

	// The profile names Kuria and the tenant; its secrets are in the store.
	const config = readFileSync(join(cli.dir, 'config.toml'), 'utf8');
	expect(config).toContain(`url = "${KURIA}"`);
	expect(config).toContain(`tenant = "${tenant}"`);
	const secretsFile = join(cli.dir, 'secrets', 'default.json');
	const secrets = JSON.parse(readFileSync(secretsFile, 'utf8'));
	expect(signedIn.stdout + signedIn.stderr).not.toContain(secrets.refresh_token);
	expect(signedIn.stdout + signedIn.stderr).not.toContain(secrets.access_token);

	// Without KURIA_URL, the profile says where.
	const whoami = await cli.run(['whoami', '--format', 'json'], { KURIA_URL: '' });
	expect(whoami.code, whoami.stderr).toBe(0);
	expect(JSON.parse(whoami.stdout)).toMatchObject({
		url: KURIA,
		tenant,
		subject: person.subject,
		signedInWith: 'link',
		profile: 'default'
	});

	// An access token about to expire is refreshed, and the refresh token
	// rotated.
	writeFileSync(secretsFile, JSON.stringify({ ...secrets, expires_at: 0 }));
	const refreshed = await cli.run(['whoami']);
	expect(refreshed.code, refreshed.stderr).toBe(0);
	const rotated = JSON.parse(readFileSync(secretsFile, 'utf8'));
	expect(rotated.expires_at).toBeGreaterThan(Date.now() / 1000);
	expect(rotated.access_token).not.toBe(secrets.access_token);
	expect(rotated.dpop_key).toBe(secrets.dpop_key);

	const logout = await cli.run(['logout']);
	expect(logout.code, logout.stderr).toBe(0);
	expect(existsSync(secretsFile)).toBe(false);
	const after = await cli.run(['whoami', '--format', 'json']);
	expect(after.code).toBe(4);
	expect(JSON.parse(after.stderr).error.code).toBe('NOT_SIGNED_IN');
	// The session ended at Konklav too: its refresh token is refused.
	writeFileSync(secretsFile, JSON.stringify({ ...rotated, expires_at: 0 }));
	const revoked = await cli.run(['whoami', '--format', 'json']);
	expect(revoked.code).toBe(4);
});

// r[verify cli.secret-key]
// r[verify cli.errors]
test('with KURIA_KEY, kuria signs in as software and saves nothing', async () => {
	const { tenant, clients } = seeded();
	const cli = new Cli({ KURIA_KEY: clients.admin.key });
	const whoami = await cli.run(['whoami', '--format', 'json']);
	expect(whoami.code, whoami.stderr).toBe(0);
	expect(JSON.parse(whoami.stdout)).toMatchObject({ url: KURIA, tenant, signedInWith: 'secret-key', profile: null });
	expect(existsSync(join(cli.dir, 'secrets'))).toBe(false);
	expect(existsSync(join(cli.dir, 'config.toml'))).toBe(false);

	const wrong = await cli.run(['whoami', '--format', 'json'], { KURIA_KEY: `${clients.admin.key}x` });
	expect(wrong.code).toBe(4);
	expect(JSON.parse(wrong.stderr).error).toMatchObject({ code: expect.any(String), message: expect.any(String) });

	const nowhere = await cli.run(['whoami'], { KURIA_URL: 'http://127.0.0.1:9' });
	expect(nowhere.code).toBe(5);
});
