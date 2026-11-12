/**
 * Signing in: the server is the browser's backend, so signing in is a
 * redirect to its `/auth/login`, which returns to `returnTo`.
 */
// r[impl app.sign-in]
export function signInUrl(returnTo: string): string {
	const path = returnTo.startsWith('/') && !returnTo.startsWith('//') ? returnTo : '/';
	return `/auth/login?return_to=${encodeURIComponent(path)}`;
}

/** Sends the browser to sign in, returning to the page it is on. */
export function signIn(): void {
	location.assign(signInUrl(location.pathname + location.search));
}

/** Whether a response says the request isn't signed in. */
export function signedOut(status: number | undefined): boolean {
	return status === 401;
}
