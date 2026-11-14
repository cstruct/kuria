---
number: PLAN-004
type: plan
revision: 34
rank: b0n
state: done
title: Frontend and End-to-End Tests
---

<!-- field: goal -->
After this milestone:

- Kuria serves a single-page app, built with SvelteKit in SPA mode on Vite,
  from its own origin. People sign in through Kuria's **login page**, which
  acts as Konklav's login application, and then work with akter: lists,
  akt pages, forms, transitions, revisions, containers and boards, drafts,
  attachments, contacts and live updates. Every view is built from the
  akt types at runtime; nothing in the app knows the example's akt types.
- `mise run e2e` starts a seeded Konklav, the Kuria server serving the
  built app, and Playwright, which walks the release-management example as
  a developer and a release manager in two browsers, with the build
  pipeline registering artifacts through the API.

Success is the Playwright walk-through passing from a clean checkout:
planning a release, two candidates (one rejected) with artifacts and an
SBOM, the go checked for a contact, the release, a hotfix, drafted notes
and artifacts showing exactly what the API walk-through
(`items/server/tests/release_flow.rs`) asserts, and the release closed.

<!-- field: design -->
## Constraints And Assumptions

- The GraphQL schema is fixed (`docs/architecture.md`, "One fixed schema"):
  the app reads `AktType` documents and builds forms, boards and views from
  them, and generated client types come from that one schema.
- Browsers never hold Konklav tokens for Kuria: the server is the backend
  for the frontend (`docs/spec/browser-sessions.md`). Cookie requests that
  change something must name an allowed `Origin`, and `/graphql` takes only
  JSON. The app must be served from Kuria's own origin (`SameSite=Strict`).
- Konklav has no user interface. Its authorization endpoint hands the
  browser to a login application (`oidc_provider.login_url`), which signs
  the person in through Konklav's subject-facing API and accepts the login
  request. Konklav's refresh cookie for that session is `HttpOnly` on the
  login application's origin, so the login page and Konklav's
  subject-facing surface must share an origin. Konklav's notes-app example
  (`examples/notes-app` in the Konklav repository: `web/sso/login.js`,
  `rust/src/bin/login.rs`) is the reference: about 300 lines.
- Konklav sessions started by the login page should be DPoP-bound
  (`browser.require_dpop`), with a non-extractable WebCrypto key in
  IndexedDB. That is separate from Kuria's own OIDC client, whose tokens
  are bearer tokens, since `/graphql` refuses DPoP-bound tokens
  (`authz.sender-constrained`).
- Seeding Konklav: `konklav-server bootstrap <set>` applies tenants,
  people with passwords, roles and the catalog; OIDC clients and service
  principals are control-plane resources, registered with the bootstrap
  token (the notes app's `setup` service does this).
- Konklav 1.7.0 is the version Kuria pins (`=1.7.0` crates).
- Specs first. Behaviour other code or people depend on gets Tracey rules;
  visual design does not.

## Design

### Layout

- `items/web`: the SvelteKit app (TypeScript, `adapter-static` with a
  fallback page, no server-side rendering). `npm` with a lockfile; Node and
  the package manager pinned in `mise.toml`.
- `items/web/src/lib/graphql`: operations, and the types `graphql-codegen`
  generates from the schema. The schema comes from the server:
  `kuria-server schema` prints its SDL, checked in as `items/web/schema.graphql`
  and checked for drift in `mise run lint`.
- `e2e/`: Playwright tests, the Compose file for the end-to-end stack,
  Konklav's configuration, the bootstrap set and the setup script.

### Serving

- The Kuria server serves the built app from `KURIA_WEB_DIR`, if set:
  static files with long-lived caching for hashed assets, and the fallback
  page for every other `GET` that isn't an API route, with a strict CSP.
- In development, Vite serves the app and proxies `/graphql` (with
  WebSocket), `/auth`, `/attachments`, `/v1` and `/.well-known` to the
  server, as the architecture already says.

### Login page

- Kuria's server proxies Konklav's subject-facing surface (`/v1/*`,
  `/.well-known/*`) when `KURIA_LOGIN_APP` is on, so Kuria's public URL is
  Konklav's issuer and its `login_url` is `<public URL>/login`.
- Kuria discovers the OpenID provider from `KONKLAV_SUBJECT_URL` instead of
  the issuer, still checking that the discovered issuer is
  `KONKLAV_ISSUER`, so starting doesn't depend on its own proxy.
- `/login?login_request=…`: shows the request, signs in with email and
  password (`/v1/login/password`, `client: browser`), restores an existing
  session (`/v1/refresh`), and accepts or rejects the request. Single
  sign-on, `prompt=none` and `prompt=login` as in the notes app. MFA:
  refused with a message at first (see gaps).
- Tenant: Kuria's `/auth/login` takes `tenant`. The app starts a sign-in
  with the tenant from its URL or, without one, lets the login page choose
  among the person's tenants, if Konklav's login requests support that (to
  confirm in T1).

### The app

- The GraphQL client: urql with Graphcache, keyed by `Akt.id`; the
  `changes` subscription marks the akter it names stale so they refetch.
- Signed out (`/graphql` answers 401): send the browser to
  `/auth/login?return_to=<path>`.
- Views, all built from akt types:
  - home: akt types the viewer sees, and akter by type;
  - akt page: fields by kind, each with its editor (text, multiline,
    choice, choices, date, user or contact, checkbox and checking for
    someone, reference, git, docker, attachment with upload); transitions
    with the fields they require; revisions with authors and changes;
    containers and members; drafting a field;
  - board: a container's columns, members in rank order, moving by drag
    and by keyboard;
  - creating an akt, also inside containers; adding and transferring
    members;
  - contacts: listing, adding, editing, erasing.
- Errors: each GraphQL `code` gets a message; `STALE` reloads the akt and
  keeps the person's input.
- Accessibility first: every control has a role and a label, which is what
  Playwright selects by. `data-testid` only where no role fits.

### End-to-end stack

- `e2e/compose.yaml`: Konklav (`konklav/konklav:1.7.0`), its Postgres with
  `pg_jsonschema` (built from a copy of Konklav's `dev/postgres`
  Dockerfile), one-off services for keys, migrations, the bootstrap set and
  setup. Kuria's own Postgres, Valkey and versitygw are the development
  ones (`dev/compose.yaml`).
- `e2e/konklav/seed.styx` and `catalog.json`: tenant `shop`; a developer
  and a release manager with passwords (test-only, in the repository); the
  roles of `docs/examples/release-management/README.md`, scope-bound at
  `kuria_akt_type`; Kuria's actions and scope kind in the catalog.
- `e2e/setup.ts`: registers Kuria's confidential OIDC client and its
  service principal, and the build pipeline's service principal, through
  the control plane, and writes their secrets to `e2e/.env` (ignored).
- `mise run e2e`: brings the stack up, builds the app, starts the Kuria
  server with `KURIA_WEB_DIR` and the e2e environment, runs Playwright, and
  tears it down. Playwright's `webServer` waits for `/healthz`.

### The walk-through

- `e2e/release-flow.spec.ts`, two browser contexts (developer, release
  manager) and a pipeline client calling `/graphql` and `/attachments` with
  a service principal's token:
  1. Define the example's akt types (an admin, through the API).
  2. Plan: release, backlog, epic and items; reorder the backlog and see
     the epic follow.
  3. RC1 with items and the pipeline's artifacts and SBOM; work moving on
     its board; rejected.
  4. RC2 rebuilding the backend and removing search; accepted with its
     evidence.
  5. The release refused without the go; the release manager adds the head
     of product as a contact and checks go for them; released.
  6. A hotfix with an item and an artifact; deployed.
  7. Notes drafted and approved: text and artifacts as the API test
     asserts; the SBOM's summary shown and downloadable.
  8. Closed, and frozen.
- Traces and screenshots kept on failure.

<!-- field: verification -->
- `mise run lint`: Rust, the app (ESLint, Prettier, `svelte-check`), the
  schema file's drift, and Tracey.
- `mise run test`: Rust tests and the app's Vitest tests.
- `mise run e2e`: Playwright against the full stack, from a clean state
  each run.
- By hand: `mise run serve` and the Vite dev server, signing in through
  the login page.
