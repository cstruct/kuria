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

- Defining akt types and creating, editing and transitioning akter through
  GraphQL, with every change written as an akt revision and a change event
  in one transaction ([`src/akter.rs`](src/akter.rs) and its submodules are
  the only code that writes them).
- Containers: adding and removing members (recorded on both akter), creating
  akter inside containers with `{parent}` numbers, and moving akter in their
  tenant's one priority order.
- Drafting a field, such as release notes, from the akter around its akt by
  the field's recipe, as an ordinary edit.

- Authorizing every operation on akt types and akter by asking Konklav,
  through the relying client, for Kuria's actions tenant-wide or at the akt
  type ([`src/authorize.rs`](src/authorize.rs)). Decisions are remembered for
  one request. Akter a subject may not read are invisible to them.
- Refusing DPoP-bound tokens on `/graphql`: their proof is answered once, so
  Kuria can't decide more than once for them.

Not yet: subscriptions, publishing change events to Valkey, links and
signatures.

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
- [`src/graphql.rs`](src/graphql.rs): the schema, with
  [`src/graphql/akter.rs`](src/graphql/akter.rs) for akt types, akter and
  revisions. Queries: `viewer`, `aktTypes`, `aktType`, `akt`, `akter`.
  Mutations: `defineAktType`, `createAkt` (with `in`), `editAkt`,
  `performTransition`, `addToContainer`, `removeFromContainer`, `moveAkt`,
  `draftField`.
  `Akt` also has `rank`, `members`, `containers` and `board` (its members
  in its akt type's columns, then `unmapped`, each in rank order), and a
  revision its `membership`.
  Errors carry a `code` extension: `INVALID_AKT_TYPE` (with `problems`),
  `UNKNOWN_AKT_TYPE`, `NOT_FOUND`, `REFUSED`, `STALE` (with
  `currentRevision`), `FORBIDDEN`, `UNAVAILABLE`, `NOT_CONTAINABLE`,
  `ALREADY_MEMBER`, `NOT_MEMBER`, `EXCLUSIVE`, `CYCLE`, `FROZEN`,
  `CREATED_IN`, `BAD_REQUEST` or `INTERNAL`.
- [`src/akter.rs`](src/akter.rs): the write path for akt types and akter,
  with [`create`](src/akter/create.rs),
  [`membership`](src/akter/membership.rs) and
  [`ordering`](src/akter/ordering.rs) and [`draft`](src/akter/draft.rs).
  Every path takes its locks in one
  order: the tenant's order lock, akt rows by id, number counters, then the
  change-event counter.
- [`src/main.rs`](src/main.rs) reads [`Config`](src/config.rs), migrates the
  database, discovers Konklav's OpenID provider, starts the relying client,
  waits up to 30 seconds for its caches to be trusted, and serves.

## Running

Konklav's catalog must declare Kuria's actions, `kuria:akt_types:define`,
`kuria:akter:read`, `kuria:akter:create`, `kuria:akter:edit` and
`kuria:akter:transition`, the scope kind `kuria_akt_type`, and any action an
akt type's transitions name. Roles grant them tenant-wide or at one akt type
(the scope id is the akt type's key).

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
- [`tests/akter.rs`](tests/akter.rs) goes through GraphQL against a fresh
  database: akt type versions and their validation, tenant isolation,
  numbering (also ten concurrent creates), field kinds and refusals, edits
  and their revisions and hashes, stale revisions, transitions, pinned akt
  type versions, append-only revisions, and change events in order.
- [`tests/authorization.rs`](tests/authorization.rs) uses a fake decider
  that grants actions per token, tenant-wide or at an akt type
  ([`tests/support/decider.rs`](tests/support/decider.rs)): defining, which
  akt types and akter are visible, creating, editing, transitions and their
  own actions, tenant-wide grants, Konklav failing, decisions lasting one
  request, and DPoP-bound tokens.
- [`tests/containers.rs`](tests/containers.rs): many containers per akt,
  removing, exclusivity, cycles, frozen containers, membership revisions on
  both akter, release candidates and hotfixes numbered inside their release,
  creating inside several containers, a move in a backlog showing in an
  epic, moves as rank events, concurrent moves, and the permissions for
  arranging, moving, creating inside and listing.
- [`tests/boards.rs`](tests/boards.rs): stories and bugs in shared columns,
  rank order within a column, `unmapped`, a container without columns, and
  members the subject can't read.
- [`tests/drafts.rs`](tests/drafts.rs): release notes drafted from a
  release's candidates, hotfix and an epic inside a candidate, leaving out
  a backlog's unshipped item and what the drafter can't read, and refused
  as an edit would be.
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
  (`session.*`), [`erasure.md`](../../docs/spec/erasure.md)
  (`erasure.*`), [`akter.md`](../../docs/spec/akter.md) (`akt.*`) and
  [`authorization.md`](../../docs/spec/authorization.md) (`authz.*`),
  [`containers.md`](../../docs/spec/containers.md) (`container.*`) and
  [`ordering.md`](../../docs/spec/ordering.md) (`order.*`) and
  [`boards.md`](../../docs/spec/boards.md) (`board.*`) and
  [`drafts.md`](../../docs/spec/drafts.md) (`draft.*`).
- Design: [`docs/architecture.md`](../../docs/architecture.md).
