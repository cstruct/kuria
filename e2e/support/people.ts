/** People signing in to Kuria through its login page. */
import { expect, type Browser, type Page } from '@playwright/test';
import type { Person } from './stack.ts';

/** A browser of its own, from a client address of its own, so that the
 * server's sign-in rate limit counts each test apart. */
export async function newPage(browser: Browser): Promise<Page> {
	const address = `198.51.100.${1 + Math.floor(Math.random() * 254)}`;
	const context = await browser.newContext({ extraHTTPHeaders: { 'x-forwarded-for': address } });
	return context.newPage();
}

/** Signs `person` in from Kuria's home page, in a browser of their own. */
export async function signIn(browser: Browser, person: Person): Promise<Page> {
	const page = await newPage(browser);
	await page.goto('/');
	// Not signed in: the app sends the browser through Konklav to the login page.
	await expect(page.getByRole('heading', { name: /Sign in to continue to kuria/ })).toBeVisible();
	await page.getByLabel('Email').fill(person.email);
	await page.getByLabel('Password').fill(person.password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
	return page;
}
