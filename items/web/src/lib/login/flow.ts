import { KonklavError, type Konklav, type LoginRequest } from './konklav';

/** What the login page shows, or where it sends the browser. */
export type Step =
	| { kind: 'message'; text: string }
	| { kind: 'form'; request: LoginRequest; error?: string }
	| { kind: 'redirect'; to: string };

function prompts(request: LoginRequest): string[] {
	return (request.prompt ?? '').split(' ').filter(Boolean);
}

/** Moves the session into the request's tenant, if it names one, and
 * accepts the request. */
// r[impl login.tenant]
// r[impl login.accept]
async function complete(konklav: Konklav, id: string, request: LoginRequest): Promise<string> {
	if (request.tenant) {
		try {
			await konklav.selectTenant(request.tenant);
		} catch (error) {
			if (error instanceof KonklavError && error.status === 403) {
				throw new KonklavError(
					403,
					'NO_MEMBERSHIP',
					`You aren't a member of the tenant ${request.client_id} asked for.`
				);
			}
			throw error;
		}
	}
	return konklav.accept(id);
}

/** Where the login page starts, for the login request `id`. */
// r[impl login.request]
// r[impl login.single-sign-on]
// r[impl login.prompt-none]
export async function start(konklav: Konklav, id: string | null): Promise<Step> {
	if (!id) {
		return { kind: 'message', text: 'There is no sign-in to complete here. Go back to the app.' };
	}
	let request: LoginRequest;
	try {
		request = await konklav.loginRequest(id);
	} catch {
		return {
			kind: 'message',
			text: 'This sign-in request has expired. Go back to the app and try again.'
		};
	}
	const asked = prompts(request);
	const restored = !asked.includes('login') && (await konklav.restore());
	if (restored) {
		try {
			return { kind: 'redirect', to: await complete(konklav, id, request) };
		} catch (error) {
			if (asked.includes('none')) {
				return { kind: 'redirect', to: await konklav.reject(id, 'login_required') };
			}
			return { kind: 'form', request, error: messageOf(error) };
		}
	}
	if (asked.includes('none')) {
		return { kind: 'redirect', to: await konklav.reject(id, 'login_required') };
	}
	return { kind: 'form', request };
}

/** Signs in with the form's email and password, and completes the request. */
// r[impl login.password]
export async function signIn(
	konklav: Konklav,
	id: string,
	request: LoginRequest,
	email: string,
	password: string
): Promise<Step> {
	try {
		const { partial } = await konklav.signIn(email, password);
		if (partial) {
			return {
				kind: 'form',
				request,
				error: 'This account needs a second factor, which this page can’t sign in with yet.'
			};
		}
		return { kind: 'redirect', to: await complete(konklav, id, request) };
	} catch (error) {
		return { kind: 'form', request, error: messageOf(error) };
	}
}

/** Ends the page's own Konklav session, if it can be restored, so that
 * signing out of Kuria signs the person out here too. */
// r[impl login.sign-out]
export async function signOut(konklav: Konklav): Promise<Step> {
	if (await konklav.restore()) {
		try {
			await konklav.signOut();
		} catch {
			// Restored but not ended: Konklav will end it when it expires.
		}
	}
	return { kind: 'message', text: 'You’re signed out.' };
}

/** Refuses the request because the person cancelled. */
export async function cancel(konklav: Konklav, id: string): Promise<Step> {
	return { kind: 'redirect', to: await konklav.reject(id, 'access_denied') };
}

function messageOf(error: unknown): string {
	if (error instanceof KonklavError && error.code === 'INVALID_CREDENTIALS') {
		return 'Wrong email or password.';
	}
	return error instanceof Error ? error.message : String(error);
}
