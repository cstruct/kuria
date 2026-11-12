# kuria-server

The Kuria server: a GraphQL API over HTTP, authenticated through Konklav.

## Responsibilities

- Serving the GraphQL endpoint (`POST /graphql`) and a health check
  (`GET /healthz`).
- Authenticating every request as a Konklav relying system, through
  `konklav-axum` and the relying client. The GraphQL endpoint admits only
  tenant tokens.
- Signing browsers in as a confidential OIDC client of Konklav
  (`/auth/login`, `/auth/callback`, `/auth/logout`), keeping their tokens
  encrypted on the server, and authenticating their requests by an
  `HttpOnly` session cookie.
- Refreshing browser sessions' access tokens before they expire, one refresh
  per session at a time across every instance.
- Rate limiting sign-in starts per client address, shared between instances
  through Valkey.
- Deleting an erased subject's browser sessions when Konklav erases them,
  and catching up on erasures it may have missed from a time stored in the
  database.
- Refusing cookie requests from origins it isn't configured for.
- Following Konklav's change feed as a service principal, so revoked sessions
  and new signing keys take effect without a restart.
- Handing resolvers the subject and tenant of the verified token, and nothing
  from the request, as the tenant to act in.

Not yet: akter, subscriptions, and authorization finer than the token kind.

## Entry points

- `app(host, public_url, sessions)` in [`src/http.rs`](src/http.rs) builds
  the router from any `RelyingHost`, so tests can stand in for Konklav.
  Without `sessions` it serves token-authenticated requests only.
- [`src/session.rs`](src/session.rs): browser sessions, split into the
  cipher, cookies, token resolution and refresh, the request layer, and the
  `/auth` routes.
- [`src/oidc.rs`](src/oidc.rs): the OIDC client for Konklav's OpenID
  provider: discovery, the authorization URL, code exchange, refresh and
  revocation.
- [`src/erasure.rs`](src/erasure.rs): `handle_erasures` consumes the relying
  client's erasure notices; `ErasedSubjects` is where the list of erased
  subjects comes from, so tests can stand in for Konklav.
- [`src/graphql.rs`](src/graphql.rs): the schema.
- [`src/main.rs`](src/main.rs) reads [`Config`](src/config.rs), migrates the
  database, discovers Konklav's OpenID provider, starts the relying client,
  waits up to 30 seconds for its caches to be trusted, and serves.

## Running

The server is configured from the environment:

| Variable | Meaning |
|---|---|
| `KURIA_LISTEN` | Address to listen on, `0.0.0.0:8080` by default |
| `KURIA_PUBLIC_URL` | The URL clients address the server by; DPoP proofs are checked against it, and the OIDC callback is `<it>/auth/callback` |
| `KURIA_ALLOWED_ORIGINS` | Comma-separated origins cookie requests may come from; the public URL's origin by default |
| `KURIA_SECURE_COOKIES` | `true` (default) for `Secure`, `__Host-` cookies; `false` only for local development over HTTP |
| `KURIA_SESSION_KEY` | 32 bytes in base64, the key browser sessions' tokens are encrypted under (`openssl rand -base64 32`) |
| `KURIA_OIDC_CLIENT_ID` | Kuria's OIDC client at Konklav: confidential, with the code and refresh grants, the `tenant` scope, and `<public URL>/auth/callback` as its redirect URI |
| `KURIA_OIDC_CLIENT_SECRET` | That client's secret |
| `DATABASE_URL` | Kuria's Postgres database |
| `KURIA_VALKEY_URL` | Valkey, as a `redis://` URL, for state the instances share |
| `KURIA_RATE_LIMIT_SECRET` | At least 32 bytes, shared by every instance; rate-limit keys are hashed under it before they reach Valkey |
| `KURIA_TRUSTED_PROXIES` | Comma-separated addresses or CIDR networks whose `X-Forwarded-For` names the client; none by default |
| `KONKLAV_ISSUER` | Konklav's issuer, as its tokens' `iss` names it; the OpenID provider is discovered from it |
| `KONKLAV_AUDIENCE` | The audience Kuria's tokens carry |
| `KONKLAV_RELYING_URL` | Konklav's relying surface |
| `KONKLAV_SUBJECT_URL` | Konklav's subject-facing surface, where the service principal logs in |
| `KONKLAV_SERVICE_KEY` | The service principal's secret key, as `<key id>.<secret>` |

```sh
mise run serve
```

## Testing

```sh
mise run test
```

- [`tests/authentication.rs`](tests/authentication.rs) sends requests through
  the router with a fake relying host
  ([`tests/support/mod.rs`](tests/support/mod.rs)): missing, invalid and
  wrong-kind tokens, Konklav being unreachable, the viewer, the tenant, the
  URL proofs are checked against, and the health check.
- [`tests/browser_sessions.rs`](tests/browser_sessions.rs) runs against a
  fake Konklav OpenID provider served over HTTP
  ([`tests/support/konklav.rs`](tests/support/konklav.rs)) and a fresh
  database each: signing in and its refusals, the cookie's attributes,
  storage, replacing a session, origins, JSON-only GraphQL, refreshing
  (including five concurrent requests refreshing once), refused and failed
  refreshes, signing out, and rate limits on starting a sign-in (per client
  address, shared between two servers, and with and without a trusted proxy).
  They need the development Postgres and Valkey (`mise run db:up`).
- [`tests/erasure.rs`](tests/erasure.rs) runs the erasure task against a
  stand-in for Konklav's list and a fresh database: live notices, catching
  up from the stored time, the time never moving back, and retrying a
  failed catch-up while live notices keep being handled.
- Unit tests in `src/` cover the configuration, the cipher, the cookies and
  `return_to`.

No test runs against a real Konklav server yet, so following the change feed
(`auth.service-principal`) is not verified, and the OIDC client is tested
against a fake of Konklav's OpenID provider only.

## Traceability

- Specs: [`authentication.md`](../../docs/spec/authentication.md)
  (`auth.*`), [`browser-sessions.md`](../../docs/spec/browser-sessions.md)
  (`session.*`) and [`erasure.md`](../../docs/spec/erasure.md)
  (`erasure.*`).
- Design: [`docs/architecture.md`](../../docs/architecture.md).
