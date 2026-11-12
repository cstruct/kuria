/**
 * Konklav's subject-facing API, which the server proxies on this origin.
 * Every request carries a DPoP proof; the session's access token lives in
 * this object only, and its refresh token is Konklav's HttpOnly cookie.
 */

/** A pending sign-in another client started, as Konklav describes it. */
export interface LoginRequest {
	client_id: string;
	scopes: string[];
	tenant: string | null;
	prompt: string | null;
	login_hint: string | null;
}

export class KonklavError extends Error {
	constructor(
		readonly status: number,
		readonly code: string | undefined,
		message: string
	) {
		super(message);
	}
}

export type Prover = (method: string, url: string, accessToken?: string) => Promise<string>;

/** Signing in through Konklav, as the `browser` client. */
export class Konklav {
	#accessToken: string | undefined;

	constructor(
		private readonly origin: string,
		private readonly prove: Prover,
		private readonly fetcher: typeof fetch = fetch
	) {}

	get signedIn(): boolean {
		return this.#accessToken !== undefined;
	}

	async #call<T>(method: string, path: string, body?: unknown): Promise<T> {
		const url = new URL(path, this.origin).href;
		const headers: Record<string, string> = {
			// Konklav requires a custom header on browser endpoints (CSRF defence).
			'x-konklav-client': 'kuria-login',
			dpop: await this.prove(method, url, this.#accessToken)
		};
		if (this.#accessToken) headers.authorization = `DPoP ${this.#accessToken}`;
		if (body !== undefined) headers['content-type'] = 'application/json';
		const response = await this.fetcher(url, {
			method,
			headers,
			body: body === undefined ? undefined : JSON.stringify(body),
			credentials: 'same-origin'
		});
		const text = await response.text();
		const data = text ? JSON.parse(text) : null;
		if (!response.ok) {
			throw new KonklavError(
				response.status,
				data?.error?.code,
				data?.error?.message ?? `Konklav answered ${response.status}`
			);
		}
		return data as T;
	}

	loginRequest(id: string): Promise<LoginRequest> {
		return this.#call('GET', `/v1/oauth2/login-requests/${encodeURIComponent(id)}`);
	}

	/** Restores the session from Konklav's cookie; whether there was one. */
	async restore(): Promise<boolean> {
		try {
			const tokens = await this.#call<{ access_token: string }>('POST', '/v1/refresh');
			this.#accessToken = tokens.access_token;
			return true;
		} catch {
			this.#accessToken = undefined;
			return false;
		}
	}

	/** Signs in with a password, answering whether a second factor is still
	 * needed. */
	async signIn(identifier: string, password: string): Promise<{ partial: boolean }> {
		const tokens = await this.#call<{ access_token: string; token_kind: string }>(
			'POST',
			'/v1/login/password',
			// The refresh token comes back as an HttpOnly cookie, not in the body.
			{ identifier, password, client: 'browser' }
		);
		this.#accessToken = tokens.access_token;
		return { partial: tokens.token_kind === 'partial' };
	}

	/** Moves the session into `tenant`. */
	async selectTenant(tenant: string): Promise<void> {
		const tokens = await this.#call<{ access_token: string }>('POST', '/v1/login/tenant', {
			tenant
		});
		this.#accessToken = tokens.access_token;
	}

	/** Accepts the request with this session; where to send the browser. */
	async accept(id: string): Promise<string> {
		const answer = await this.#call<{ redirect_to: string }>(
			'POST',
			`/v1/oauth2/login-requests/${encodeURIComponent(id)}/accept`
		);
		return answer.redirect_to;
	}

	/** Ends this session at Konklav, and every session started from it. */
	async signOut(): Promise<void> {
		await this.#call('POST', '/v1/sign-out');
		this.#accessToken = undefined;
	}

	/** Refuses the request with `error`; where to send the browser. */
	async reject(id: string, error: string): Promise<string> {
		const answer = await this.#call<{ redirect_to: string }>(
			'POST',
			`/v1/oauth2/login-requests/${encodeURIComponent(id)}/reject`,
			{ error }
		);
		return answer.redirect_to;
	}
}
