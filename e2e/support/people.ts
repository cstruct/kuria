/** People signing in to Kuria through its login page. */
import { expect, type Browser, type Page } from '@playwright/test';
import type { Person } from './stack.ts';

/** Signs `person` in from Kuria's home page, in a browser of their own. */
export async function signIn(browser: Browser, person: Person): Promise<Page> {
	const context = await browser.newContext();
	const page = await context.newPage();
	await page.goto('/');
	// Not signed in: the app sends the browser through Konklav to the login page.
	await expect(page.getByRole('heading', { name: /Sign in to continue to kuria/ })).toBeVisible();
	await page.getByLabel('Email').fill(person.email);
	await page.getByLabel('Password').fill(person.password);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('navigation', { name: 'Kuria' })).toBeVisible();
	return page;
}
