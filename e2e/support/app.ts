/** Working in Kuria's app as a person does: by roles and labels. */
import { expect, type Page } from '@playwright/test';

/** Opens the akt `number`. */
export async function open(page: Page, number: string) {
	await page.goto(`/akt/${encodeURIComponent(number)}`);
	await expect(page.getByRole('heading', { level: 1, name: number })).toBeVisible();
}

/** Fills a text, date or URL field by its name, in `scope`. */
type Fill = string | { choose: string } | { check: true; for?: string } | { url: string; label?: string };

async function fill(page: Page, fields: Record<string, Fill>) {
	for (const [name, value] of Object.entries(fields)) {
		const group = page.getByRole('group', { name: new RegExp(`^${escape(name)}( \\(required\\))?$`) });
		if (typeof value === 'string') {
			await group.getByLabel(name, { exact: true }).fill(value);
		} else if ('choose' in value) {
			await group.getByLabel(name, { exact: true }).selectOption({ label: value.choose });
		} else if ('check' in value) {
			await group.getByRole('checkbox', { name }).check();
			if (value.for) await group.getByLabel('Checked for').selectOption({ label: value.for });
		} else {
			await group.getByLabel('URL').fill(value.url);
			if (value.label) await group.getByLabel('Label').fill(value.label);
		}
	}
}

function escape(text: string): string {
	return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Creates an akt of the type `key`, inside `inside` if given, and answers
 * its number. */
export async function create(
	page: Page,
	key: string,
	fields: Record<string, Fill>,
	inside?: string
): Promise<string> {
	await page.goto(`/types/${key}/new${inside ? `?in=${encodeURIComponent(inside)}` : ''}`);
	await fill(page, fields);
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(page).toHaveURL(/\/akt\//);
	return (await page.getByRole('heading', { level: 1 }).textContent())!.trim();
}

/** Performs a transition of the open akt, filling the fields it asks for. */
export async function transition(page: Page, name: string, fields: Record<string, Fill> = {}) {
	await page.getByRole('button', { name: `${name}…`, exact: true }).click();
	const form = page.getByRole('form', { name });
	await fill(page, fields);
	await form.getByRole('button', { name, exact: true }).click();
}

/** Performs a transition and waits for the state it leads to. */
export async function moved(page: Page, name: string, to: string, fields: Record<string, Fill> = {}) {
	await transition(page, name, fields);
	await expect(page.getByTestId('state')).toHaveText(to);
}

/** Adds `members` to the open container. */
export async function addMembers(page: Page, ...members: string[]) {
	for (const member of members) {
		await page.getByLabel('Member number', { exact: true }).fill(member);
		await page.getByRole('button', { name: 'Add member' }).click();
		await expect(page.getByRole('region', { name: 'Members' }).getByRole('link', { name: member })).toBeVisible();
	}
}

/** Adds the open akt to `container`. */
export async function addTo(page: Page, container: string) {
	await page.getByLabel('Container number', { exact: true }).fill(container);
	await page.getByRole('button', { name: 'Add to container' }).click();
	await expect(page.getByRole('list', { name: 'Containers' }).getByRole('link', { name: container })).toBeVisible();
}

/** The numbers in a column of the open container's board, in order. */
export async function column(page: Page, name: string): Promise<string[]> {
	const links = page.getByRole('region', { name, exact: true }).getByRole('listitem').getByRole('link');
	return (await links.allTextContents()).map((t) => t.trim());
}

/** The text a field of the open akt shows. */
export async function shown(page: Page, key: string): Promise<string> {
	return ((await page.locator(`dd[data-field="${key}"]`).innerText()) ?? '').trim();
}
