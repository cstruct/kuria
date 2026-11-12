# Browser Sessions

Browsers authenticate to Kuria with a cookie, not with tokens they hold
themselves. Kuria signs them in as a confidential OIDC client of Konklav,
keeps the tokens it receives on the server, and gives the browser only an
opaque session cookie. Other clients keep presenting tokens in the
`Authorization` header ([authentication](authentication.md)).

## Signing in

r[session.login.start]
`GET /auth/login` MUST start an authorization code flow at Konklav's
authorization endpoint, as Konklav's discovery document names it, with a
random `state`, PKCE with `S256`, and the scope `openid`. With a `tenant`
parameter, it MUST also request the scope `tenant:<id>`. It MUST record the
attempt on the server for at most ten minutes, and bind it to the browser
with a login cookie holding the `state`: `HttpOnly`, `Secure`,
`SameSite=Lax` (so it returns with Konklav's redirect) and `Max-Age=600`.

r[session.login.default-tenant]
A sign-in that names no tenant MUST be for the server's configured default
tenant, if it has one.

r[session.login.rate-limit]
`GET /auth/login` records an attempt for anyone who asks, so it MUST be
rate limited per client address, with the limit shared by every instance of
the server through Valkey, and refusals answered with 429 and `Retry-After`.
The client address MUST be the connection's peer, or the `X-Forwarded-For`
address a configured trusted proxy names; a client cannot name its own.

r[session.login.return-to]
`GET /auth/login` MAY name where to go once signed in, as `return_to`. Kuria
MUST accept only a path on its own origin: one that starts with a single `/`
and holds no backslash. Anything else MUST be replaced by `/`.

r[session.login.callback]
`GET /auth/callback` MUST complete a sign-in only when its `state` equals
the browser's login cookie and names an attempt that has not expired, and,
when the response carries `iss`, when it is Konklav's issuer. An attempt
MUST be usable once. Kuria MUST then exchange the code at Konklav's token
endpoint with the attempt's code verifier, authenticating with its client
secret, and MUST verify the access token it receives through the relying
SDK, requiring a tenant token, before it creates a session. Any failure MUST
be answered with 400 and create no session.

r[session.login.replace]
A sign-in MUST end the browser's previous session, if it presented one, so
that a browser holds one session at a time. Signing in to another tenant is
a new sign-in.

## The session cookie

r[session.cookie]
A session MUST be identified by an opaque cookie of 32 random bytes, named
`__Host-kuria_session`, with `HttpOnly`, `Secure`, `SameSite=Strict`,
`Path=/` and no `Domain`. Only a deployment configured for insecure cookies,
for local development over HTTP, MAY drop the `__Host-` prefix and `Secure`
from it and from the login cookie.

r[session.storage]
Kuria MUST store a session's tokens on the server, encrypted with AES-256-GCM
under a configured key and bound to the session they belong to. It MUST
store the SHA-256 hash of the session cookie, never the cookie itself.

## Requests

r[session.request]
A request with a session cookie and no `Authorization` header MUST be
authenticated with that session's access token, exactly as if it had
presented it as a bearer token. A request with an `Authorization` header
MUST be authenticated by that header alone.

r[session.unknown]
A session cookie that names no session MUST be treated as no token, and the
answer MUST clear the cookie.

r[session.refresh]
When a session's access token expires within a minute, Kuria MUST refresh it
at Konklav's token endpoint before using it, and store the rotated refresh
token. Refreshes of one session MUST be serialized across every instance of
the server, so that two requests never present the same refresh token.

r[session.refresh.failure]
When Konklav refuses a session's refresh token, Kuria MUST delete the
session, treat the request as presenting no token, and clear the cookie.
When Konklav cannot be reached, Kuria MUST answer 503 and keep the session.

## Cross-site requests

r[session.origin]
A request that would be authenticated by its session cookie, and that is
not a `GET`, `HEAD` or `OPTIONS` request or is a WebSocket upgrade, MUST
carry an `Origin` header naming one of the configured origins. Otherwise
Kuria MUST refuse it with 403 before it uses the session.

r[session.graphql-json]
`POST /graphql` MUST refuse a body that is not `application/json` with 415,
so that a cross-site page can only send it after a CORS preflight.

## Signing out

r[session.logout]
`POST /auth/logout` MUST delete the browser's session, revoke its refresh
token at Konklav, clear the cookie, and answer 204. A failure to revoke MUST
NOT keep the session.
