# kuria-server

The Kuria server: a GraphQL API over HTTP, authenticated through Konklav.

## Responsibilities

- Serving the GraphQL endpoint (`POST /graphql`), GraphQL subscriptions over
  WebSocket (`GET /graphql/ws`, `graphql-transport-ws`), attachments
  (`POST /attachments`, `GET /attachments/{id}`), and a health check
  (`GET /healthz`).
- Publishing every change event from the outbox to Valkey, and delivering
  them to `changes` subscriptions in order, after catching a client up from
  its cursor.
- Authenticating every request as a Konklav relying system, through
  `konklav-axum` and the relying client. The GraphQL endpoint admits only
  tenant tokens.
- Signing browsers in as a confidential OIDC client of Konklav
  (`/auth/login`, `/auth/callback`, `/auth/logout`), keeping their tokens
  encrypted on the server, and authenticating their requests by an
  `HttpOnly` session cookie.
- Recording people's names and emails from Konklav's userinfo endpoint at
  browser sign-in, and showing `user` fields and revision authors by name;
  and, when userinfo says the email is verified, linking the tenant's
  contact with that email to the subject signing in.
- Refreshing browser sessions' access tokens before they expire, one refresh
  per session at a time across every instance.
- Rate limiting sign-in starts per client address, shared between instances
  through Valkey.
- Deleting an erased subject's browser sessions, recorded name and email,
  and the names and emails of contacts linked to them when Konklav erases
  them,
  and catching up on erasures it may have missed from a time stored in the
  database.
- Refusing cookie requests from origins it isn't configured for.
- Serving the built app ([`src/web.rs`](src/web.rs)) for every `GET` no
  other route takes, with a fallback page for the app's paths, a Content
  Security Policy allowing its inline bootstrap by hash, and caching for
  hashed assets.
- As Konklav's login application, proxying Konklav's subject-facing surface
  (`/v1`, `/.well-known`) on its own origin for the login page, without
  Kuria's cookies; and discovering the provider from Konklav directly.
- Printing its GraphQL schema (`kuria-server schema`) for the app's
  generated types.
- Following Konklav's change feed as a service principal, so revoked sessions
  and new signing keys take effect without a restart.
- Handing resolvers the subject and tenant of the verified token, and nothing
  from the request, as the tenant to act in.

- Defining akt types and creating, editing and transitioning akter through
  GraphQL, with every change written as an akt revision and a change event
  in one transaction ([`src/akter.rs`](src/akter.rs) and its submodules are
  the only code that writes them).
- Containers: adding, removing and transferring members (recorded on the
  akter involved), only in the states a container accepts them in; creating
  akter inside containers with `{parent}` numbers, and moving akter in their
  tenant's one priority order.
- Attachments ([`src/attachments.rs`](src/attachments.rs)): storing
  uploads in S3-compatible object storage under their SHA-256 per tenant,
  recognising and summarising SPDX and CycloneDX SBOMs, and answering
  downloads to whoever may read an akt holding the file, or to its uploader
  while none does. The write path takes into `attachment` fields only the
  tenant's attachments the caller may read, only SBOMs into `sbom` fields,
  and records which akter hold which attachments.
- Contacts ([`src/graphql/contacts.rs`](src/graphql/contacts.rs)):
  adding, editing, erasing and listing the people a tenant names before
  they are users, one per email, with `kuria:contacts:manage` and
  `kuria:contacts:read`. `user` values and checked boxes name a user or a
  contact (`Someone` in GraphQL); the write path takes only the tenant's
  contacts, and drafts show them by name.
- Drafting a field, such as release notes, from the akter around its akt by
  the field's recipe, as an ordinary edit.

- Authorizing every operation on akt types and akter by asking Konklav,
  through the relying client, for Kuria's actions tenant-wide or at the akt
  type ([`src/authorize.rs`](src/authorize.rs)). Decisions are remembered for
  one request. Akter a subject may not read are invisible to them.
- Refusing DPoP-bound tokens on `/graphql`: their proof is answered once, so
  Kuria can't decide more than once for them.

Not yet: links other than containment, and signatures.

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
  `performTransition`, `addToContainer`, `removeFromContainer`,
  `transferMember`, `moveAkt`, `draftField`, `addContact`, `editContact`,
  `eraseContact`; and the query `contacts`.
  An akt has its `title`, an akt type its `presentation` (title field,
  card and list fields) and each state its `tone`; `akter(search:)` finds
  akter by a number's beginning or a title's words.
  A revision's `by` is `User { id name email }`; `user` fields are
  `UserValue { user: Someone }`, `Someone` being a `User` or a
  `Contact { id displayName name email user }`; checkboxes are
  `CheckboxValue { checked by at for: Someone }`; git references and
  images `GitValue` and `DockerValue`; attachments
  `AttachmentValue { attachment { name mediaType size sha256 sbom url } }`.
  `Akt` also has `rank`, `members`, `containers` and `board` (its members
  in its akt type's columns, then `unmapped`, each in rank order), and a
  revision its `membership`.
  Errors carry a `code` extension: `INVALID_AKT_TYPE` (with `problems`),
  `UNKNOWN_AKT_TYPE`, `NOT_FOUND`, `REFUSED`, `STALE` (with
  `currentRevision`), `FORBIDDEN`, `UNAVAILABLE`, `NOT_CONTAINABLE`,
  `ALREADY_MEMBER`, `NOT_MEMBER`, `EXCLUSIVE`, `CYCLE`, `FROZEN`,
  `NOT_ACCEPTING`, `CREATED_IN`, `BAD_REQUEST`, `INVALID_CONTACT`,
  `DUPLICATE_EMAIL`, `LINKED` or `INTERNAL`. The attachment endpoints answer
  JSON errors with a `code` of `invalid_name`, `not_found`, `unavailable`
  or `internal`, and 413 for an upload over the limit.
- [`src/changes.rs`](src/changes.rs): the [relay](src/changes/relay.rs),
  which publishes the outbox to Valkey and is woken by every committed write,
  and the [hub](src/changes/hub.rs), which wakes on Valkey, reads change
  events from Postgres in order and broadcasts them per tenant. The
  `changes` subscription is in
  [`src/graphql/changes.rs`](src/graphql/changes.rs).
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
`kuria:akter:read`, `kuria:akter:create`, `kuria:akter:edit`,
`kuria:akter:transition`, `kuria:contacts:read` and
`kuria:contacts:manage`, the scope kind `kuria_akt_type`, and any action an
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
| `KURIA_S3_BUCKET` | The bucket attachments are stored in |
| `KURIA_S3_ENDPOINT` | An S3-compatible endpoint other than AWS, such as the development gateway; `http://` endpoints are allowed |
| `KURIA_S3_REGION` | The bucket's region, `us-east-1` by default |
| `KURIA_S3_ACCESS_KEY`, `KURIA_S3_SECRET_KEY` | Credentials for the bucket; without them, the standard AWS environment and instance credentials |
| `KURIA_ATTACHMENT_LIMIT` | The largest upload in bytes, 100 MiB by default |
| `KURIA_WEB_DIR` | The built app (`items/web/build`) to serve; none by default |
| `KURIA_LOGIN_APP` | `true` to be Konklav's login application: proxying its subject-facing surface, with Konklav's issuer this server's public URL and its login URL `<public URL>/login`; `false` by default |
| `KURIA_DEFAULT_TENANT` | The tenant a sign-in that names none is for; none by default |
| `KURIA_TRUSTED_PROXIES` | Comma-separated addresses or CIDR networks whose `X-Forwarded-For` names the client; none by default |
| `KONKLAV_ISSUER` | Konklav's issuer, as its tokens' `iss` names it; the OpenID provider is discovered from it |
| `KONKLAV_AUDIENCE` | The audiences Kuria accepts tokens for, comma-separated: its OIDC client's, and Konklav's own (`konklav`) if software that signs in to Konklav directly, with a secret key, calls Kuria |
| `KONKLAV_RELYING_URL` | Konklav's relying surface |
| `KONKLAV_SUBJECT_URL` | Konklav's subject-facing surface, where the service principal logs in and the OpenID provider is discovered |
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
  address, shared between two servers, and with and without a trusted proxy),
  recording the person, and linking a contact by a verified email only, in
  the tenant signed in to only, and never relinking one.
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
  creating inside several containers, transfers between exclusive
  containers, containers accepting members only in some states, a move in
  a backlog showing in an
  epic, moves as rank events, concurrent moves, and the permissions for
  arranging, moving, creating inside and listing.
- [`tests/boards.rs`](tests/boards.rs): stories and bugs in shared columns,
  rank order within a column, `unmapped`, a container without columns,
  members the subject can't read, and state references checked across akt
  types when either is defined.
- [`tests/drafts.rs`](tests/drafts.rs): release notes drafted from a
  release's candidates, hotfix and an epic inside a candidate, leaving out
  a backlog's unshipped item, what the drafter can't read and a rejected
  candidate's items, refused as an edit would be, users and attachments by
  name, and each component's newest artifact across candidates, leaving
  out a component whose newest artifact is a removal.
- [`tests/release_flow.rs`](tests/release_flow.rs) walks one release with
  the example configuration in
  [`docs/examples/release-management`](../../docs/examples/release-management/README.md),
  read straight from its files, as a developer, a release manager and a
  build pipeline: from planning through two release candidates with their
  artifacts and an SBOM, a removal, the release manager's go, the release
  and a hotfix, to drafted notes and artifacts and a closed, frozen
  release.
- [`tests/changes.rs`](tests/changes.rs) runs real servers on local ports
  against the development Valkey and subscribes over WebSocket: delivery in
  order, catching up from a cursor, leaving out what the subscriber can't
  read, publishing only identifiers, a change on one instance reaching
  another's subscribers, and ending when the token expires.
- [`tests/attachments.rs`](tests/attachments.rs) uploads and downloads
  through the router with object storage in memory: metadata and
  percent-encoded names, authentication, the size limit, a file stored
  once per tenant, SBOM summaries, who may download an attachment held or
  not, and which attachments `attachment` and `sbom` fields take.
- [`tests/storage.rs`](tests/storage.rs) uploads and downloads through the
  development S3 gateway (versitygw, started by `mise run db:up`).
- [`tests/contacts.rs`](tests/contacts.rs): adding and listing contacts
  and their permissions, names and emails checked, one per email per
  tenant regardless of case, editing until linked, a linked contact shown
  as its user, and erasing; `user` fields and checkboxes naming contacts,
  only the tenant's, emails hidden from akt readers, and a linked contact
  shown as its user in values and drafts while the stored values stay.
- [`tests/presentation.rs`](tests/presentation.rs): akt types' presentation
  and tones, titles by the presentation or by default and kept through
  edits and transitions, and searching by number and title words, across
  types, only what the subject may read, with `%` and `_` taken literally.
- [`tests/web.rs`](tests/web.rs): the built app and its fallback page,
  CSP and caching; the proxy to Konklav (paths, bodies, Kuria's cookies
  left out, cookies and redirects passed back); and discovering the
  provider from Konklav when its issuer is Kuria's origin.
- [`tests/erasure.rs`](tests/erasure.rs) runs the erasure task against a
  stand-in for Konklav's list and a fresh database: live notices, catching
  up from the stored time, the time never moving back, retrying a failed
  catch-up while live notices keep being handled, and erasing names and
  the contacts linked to the subject.
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
  [`drafts.md`](../../docs/spec/drafts.md) (`draft.*`),
  [`changes.md`](../../docs/spec/changes.md) (`changes.*`) and
  [`people.md`](../../docs/spec/people.md) (`person.*`) and
  [`attachments.md`](../../docs/spec/attachments.md) (`attachment.*`) and
  [`contacts.md`](../../docs/spec/contacts.md) (`contact.*`),
  [`login.md`](../../docs/spec/login.md) (`login.proxy`,
  `login.discovery`, `login.csp`) and
  [`frontend.md`](../../docs/spec/frontend.md) (`app.serve`, `app.csp`).
- Design: [`docs/architecture.md`](../../docs/architecture.md).
