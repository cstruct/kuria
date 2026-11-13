import { expect, test } from '@playwright/test';
import { newPage, signIn } from '../support/people.ts';
import { seeded } from '../support/stack.ts';

// r[verify app.sign-in]
// r[verify login.request]
// r[verify login.password]
// r[verify login.tenant]
// r[verify login.accept]
// r[verify login.proxy]
// r[verify app.serve]
test('the release manager signs in through the login page', async ({ browser }) => {
	const page = await signIn(browser, seeded().people.releaseManager);
	await expect(page).toHaveURL(/localhost:18090\/$/);
	// Signed in to the seeded tenant: the API answers.
	await expect(page.getByRole('heading', { name: 'Recently changed' })).toBeVisible();
	await expect(page.getByRole('alert')).toHaveCount(0);
	// No token or key went to web storage: only SvelteKit's navigation state.
	const stored = await page.evaluate(() =>
		[localStorage, sessionStorage].flatMap((storage) =>
			Object.keys(storage).map((key) => [key, storage.getItem(key) ?? ''])
		)
	);
	for (const [key, value] of stored) {
		expect(key).toMatch(/^sveltekit:/);
		// No JWT, which every Konklav token is.
		expect(value).not.toMatch(/eyJ[\w-]+\.[\w-]+\./);
	}
});

// r[verify login.password]
test('a wrong password is refused without saying which part was wrong', async ({ browser }) => {
	const page = await newPage(browser);
	await page.goto('/');
	await page.getByLabel('Email').fill(seeded().people.developer.email);
	await page.getByLabel('Password').fill('not the password');
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('alert')).toHaveText('Wrong email or password.');
});

// r[verify login.single-sign-on]
test('a second sign-in in the same browser needs no password', async ({ browser }) => {
	const page = await signIn(browser, seeded().people.developer);
	// Kuria's session ends; Konklav's, on the login page, doesn't.
	await page.context().clearCookies({ name: /kuria/ });
	await page.goto('/contacts');
	await expect(page.getByRole('navigation', { name: 'Kuria' })).toBeVisible();
	await expect(page).toHaveURL(/\/contacts$/);
});
