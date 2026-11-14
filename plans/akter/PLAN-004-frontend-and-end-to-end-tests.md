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

<!-- akt: PLAN-004-T01 -->
---
type: task
revision: 3
rank: b0o
state: done
in: [PLAN-004]
title: "T1 `docs/spec/login.md` (9 rules) and `docs/spec/frontend.md` (9 rules), `session.login.default-tenant…"
---

<!-- field: description -->
`docs/spec/login.md` (9 rules) and `docs/spec/frontend.md` (9
rules), `session.login.default-tenant`, the terms app and login page, the
architecture's frontend section, and Tracey reading `items/web` and `e2e`
(`2939650`). G1 settled: Konklav's login request names the tenant from
the `tenant:<id>` scope, and the login page moves its session there with
`POST /v1/login/tenant` before accepting; a sign-in naming no tenant uses
`KURIA_DEFAULT_TENANT`.

<!-- akt: PLAN-004-T02 -->
---
type: task
revision: 3
rank: b0p
state: done
in: [PLAN-004]
title: T2 Server support
---

<!-- field: description -->
Server support: `kuria_server::web::{Web, LoginProxy}`;
`KURIA_WEB_DIR`, `KURIA_LOGIN_APP`, `KURIA_DEFAULT_TENANT`;
`Provider::discover_at` through `KONKLAV_SUBJECT_URL`, rebasing the
server-side endpoints; the session cookie read only on `/graphql`,
`/attachments` and `/auth/`; `kuria-server schema` (`b964507`).

<!-- akt: PLAN-004-T03 -->
---
type: task
revision: 3
rank: b0q
state: done
in: [PLAN-004]
title: T3 App scaffold
---

<!-- field: description -->
App scaffold: `items/web` from `sv create` (SvelteKit 3, Svelte 5,
Vite 8, TypeScript, ESLint, Prettier, Vitest, `adapter-static` with an
`index.html` fallback, SSR off); `schema.graphql` and codegen (client
preset and introspection); urql with Graphcache (keys derived from the
introspection, `AktType` by key and version), the `changes` subscription
invalidating the akt and the lists and boards that may hold it, and the
signed-out redirect; a home page and akter by type. mise: Node 24.20.0,
`web:install`, `web:schema`, `web:dev`, `web:build`; `lint` and `test`
include the app and check the schema file for drift (`1ceeda8`).

<!-- akt: PLAN-004-T04 -->
---
type: task
revision: 3
rank: b0r
state: done
in: [PLAN-004]
title: T4 Login page
---

<!-- field: description -->
Login page: `src/lib/login` (`dpop.ts`, the non-extractable key in
IndexedDB; `konklav.ts`, the subject-facing API with
`x-konklav-client: kuria-login`; `flow.ts`, what the page does for each
request, prompt, restored session and tenant) and `/login`; Vitest
covers the flow against a stand-in for Konklav. The real Konklav is
first met in T6 (`43ec6ae`).

<!-- akt: PLAN-004-T05 -->
---
type: task
revision: 3
rank: b0s
state: done
in: [PLAN-004]
title: T5 Views
---

<!-- field: description -->
Views: `src/lib/akt` (`values.ts`: each kind's editor state, input
and text; `errors.ts`: a message per code and the refused field;
`document.ts`: what a type contains and drafts; `upload.ts`; the field
editor, the someone picker and value display), the akt page (fields,
editing, transitions with their required fields, drafting, containers
with adding, removing and transferring, members as a board with moving
and creating inside, history), creating an akt in containers (`?in=`),
akter by type, contacts, and a top bar. Vitest covers the value logic,
refusals and documents; the pages are exercised in T7 (`c539a63`).

<!-- akt: PLAN-004-T06 -->
---
type: task
revision: 3
rank: b0t
state: done
in: [PLAN-004]
title: T6 End-to-end stack
---

<!-- field: description -->
End-to-end stack: `e2e/compose.yaml` (`cstruct/konklav:1.7.0`
and Konklav's `pg_jsonschema` Postgres, from a copy of its Dockerfile),
`e2e/konklav/` config and secrets, `scripts/keys.ts`, `scripts/setup.ts`
(seeding over the control plane), `scripts/serve.ts` (Kuria on a fresh
`kuria_e2e` database), `support/`, and `tests/sign-in.spec.ts`: signing in
through the login page against the real Konklav, a wrong password, and
single sign-on. mise `e2e:install`, `e2e:up` (always a fresh Konklav),
`e2e:down`, `e2e`. Kuria's `KONKLAV_AUDIENCE` takes a list (`b792bfd`).

<!-- akt: PLAN-004-T07 -->
---
type: task
revision: 3
rank: b0u
state: done
in: [PLAN-004]
title: "T7 The release walk-through in a browser (`e2e/tests/release-flow.spec.ts`, about 20 seconds)"
---

<!-- field: description -->
The release walk-through in a browser
(`e2e/tests/release-flow.spec.ts`, about 20 seconds): types defined by
the admin through the API, the developer and the release manager in two
browsers, the pipeline's SBOM and artifacts through the API, the
backlog reordered and the epic following, two candidates (one
rejected), the go checked for a contact, a hotfix, the notes' text and
artifacts exactly as the API walk-through asserts, the SBOM downloaded,
the release board by outcome, and the closed release frozen. Fixes it
found in the app: the someone picker's name, clearing an input as its
change starts (a race with the next entry), and keeping a page's data
while a change refetches it (`961af92`).

<!-- akt: PLAN-004-T08 -->
---
type: task
revision: 3
rank: b0v
state: done
in: [PLAN-004]
title: T8 Docs and CI
---

<!-- field: description -->
Docs and CI: `e2e/README.md`, the app's README (layout, Tracey
and `.svelte`), the example's README (trying it in a browser), and
`.github/workflows/ci.yml` with lint and test, and the end-to-end tests
uploading Playwright's report on failure. Kuria has no CONTRIBUTING.md;
the READMEs and `mise.toml` tasks describe running things. The workflow
hasn't run on GitHub yet.

<!-- akt: PLAN-004-G01 -->
---
type: gap
revision: 2
rank: b0w
state: closed
in: [PLAN-004]
title: G1 Tenant selection
---

<!-- field: description -->
Tenant selection: whether Konklav's login requests let the login
page choose the tenant, or the app must always start with one. Confirm
against Konklav's federation spec in T1.

<!-- field: resolution -->
Tenant selection: whether Konklav's login requests let the login
page choose the tenant, or the app must always start with one. Confirm
against Konklav's federation spec in T1.

<!-- akt: PLAN-004-G02 -->
---
type: gap
revision: 2
rank: b0x
state: closed
in: [PLAN-004]
title: "G2 Konklav is published as `cstruct/konklav:1.7.0`; its Postgres with `pg_jsonschema` isn't, so the st…"
---

<!-- field: description -->
Konklav is published as `cstruct/konklav:1.7.0`; its Postgres with
`pg_jsonschema` isn't, so the stack uses the image Konklav's own
development setup builds, or builds it from a copy of that Dockerfile.

<!-- field: resolution -->
Konklav is published as `cstruct/konklav:1.7.0`; its Postgres with
`pg_jsonschema` isn't, so the stack uses the image Konklav's own
development setup builds, or builds it from a copy of that Dockerfile.

<!-- akt: PLAN-004-G03 -->
---
type: gap
revision: 1
rank: b0y
state: open
in: [PLAN-004]
title: G3 The login page refuses accounts that need MFA at first
---

<!-- field: description -->
The login page refuses accounts that need MFA at first.

<!-- akt: PLAN-004-G04 -->
---
type: gap
revision: 1
rank: b0z
state: open
in: [PLAN-004]
title: "G4 Kuria's origin is Konklav's issuer when Kuria is the login application"
---

<!-- field: description -->
Kuria's origin is Konklav's issuer when Kuria is the login
application. A Konklav shared by several apps would want its login page
on its own origin; the same build could serve it there later.

<!-- akt: PLAN-004-G05 -->
---
type: gap
revision: 2
rank: b10
state: closed
in: [PLAN-004]
title: "G5 Tracey doesn't read `.svelte` files, so the app's rules are implemented and annotated in `.ts` modu…"
---

<!-- field: description -->
Tracey doesn't read `.svelte` files, so the app's rules are
implemented and annotated in `.ts` modules the components call
(`labels.ts`, `editing.ts`, `document.ts`), and verified there and in the
Playwright specs.

<!-- field: resolution -->
Tracey doesn't read `.svelte` files, so the app's rules are
implemented and annotated in `.ts` modules the components call
(`labels.ts`, `editing.ts`, `document.ts`), and verified there and in the
Playwright specs.

<!-- akt: PLAN-004-G06 -->
---
type: gap
revision: 2
rank: b11
state: closed
in: [PLAN-004]
title: "G7 Contacts don't link for people who sign in with a password"
---

<!-- field: description -->
Contacts don't link for people who sign in with a password: Konklav
1.7 reports a password's email as unverified, and serves another
credential's verified email only when the sign-in credential has none.
Linking works for email-link and federated sign-ins (tested in the
server with a fake provider), which the login page doesn't offer yet. A
Konklav change (verified password emails, or preferring a verified email
over the sign-in credential's), or email-link sign-in on the login page,
would close it. Closed instead by linking by hand
(`contact.link-by-hand`, chosen by the user;
[design-gaps.md](design-gaps.md)).

<!-- field: resolution -->
Contacts don't link for people who sign in with a password: Konklav
1.7 reports a password's email as unverified, and serves another
credential's verified email only when the sign-in credential has none.
Linking works for email-link and federated sign-ins (tested in the
server with a fake provider), which the login page doesn't offer yet. A
Konklav change (verified password emails, or preferring a verified email
over the sign-in credential's), or email-link sign-in on the login page,
would close it. Closed instead by linking by hand
(`contact.link-by-hand`, chosen by the user;
[design-gaps.md](design-gaps.md)).

<!-- akt: PLAN-004-G07 -->
---
type: gap
revision: 1
rank: b12
state: open
in: [PLAN-004]
title: G6 The visual design is open
---

<!-- field: description -->
The visual design is open: a plain, accessible layout first.

<!-- akt: PLAN-004-D01 -->
---
type: decision
revision: 2
rank: b13
state: decided
in: [PLAN-004]
title: SvelteKit in SPA mode
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**SvelteKit in SPA mode**, not plain Svelte with a hand-written router:
file-based routes and load functions for free, a static build any host or
the Kuria server serves, no server-side rendering (chosen by the user).

<!-- field: why -->
, not plain Svelte with a hand-written router:
file-based routes and load functions for free, a static build any host or
the Kuria server serves, no server-side rendering (chosen by the user).

<!-- akt: PLAN-004-D02 -->
---
type: decision
revision: 2
rank: b14
state: decided
in: [PLAN-004]
title: "Kuria's own login page against a real Konklav"
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Kuria's own login page against a real Konklav**, not a test-only login
app or a fake provider: a deployment needs a login page anyway, and the
tests then exercise Konklav's tokens, sessions and the relying client
(chosen by the user).

<!-- field: why -->
, not a test-only login
app or a fake provider: a deployment needs a login page anyway, and the
tests then exercise Konklav's tokens, sessions and the relying client
(chosen by the user).

<!-- akt: PLAN-004-D03 -->
---
type: decision
revision: 2
rank: b15
state: decided
in: [PLAN-004]
title: "The app's inline bootstrap is allowed by hash"
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The app's inline bootstrap is allowed by hash**, computed from the
fallback page when the server starts, so the CSP needs no
`'unsafe-inline'` for scripts (T2).

<!-- field: why -->
, computed from the
fallback page when the server starts, so the CSP needs no
`'unsafe-inline'` for scripts (T2).

<!-- akt: PLAN-004-D04 -->
---
type: decision
revision: 2
rank: b16
state: decided
in: [PLAN-004]
title: The session cookie is read only on the API and sign-in paths
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The session cookie is read only on the API and sign-in paths**, so the
app's files and the login page's requests to Konklav never resolve or
refresh a Kuria session (T2).

<!-- field: why -->
, so the
app's files and the login page's requests to Konklav never resolve or
refresh a Kuria session (T2).

<!-- akt: PLAN-004-D05 -->
---
type: decision
revision: 2
rank: b17
state: decided
in: [PLAN-004]
title: "Operations live in `.ts` modules"
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Operations live in `.ts` modules**, never in components, so codegen
reads them without parsing Svelte (T3).

<!-- field: why -->
, never in components, so codegen
reads them without parsing Svelte (T3).

<!-- akt: PLAN-004-D06 -->
---
type: decision
revision: 2
rank: b18
state: decided
in: [PLAN-004]
title: The changes subscription invalidates broadly
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The changes subscription invalidates broadly**: the akt, every akt
list, and the boards, members and containers of every akt a page shows,
rather than tracking which views hold which akt (T3).

<!-- field: why -->
the akt, every akt
list, and the boards, members and containers of every akt a page shows,
rather than tracking which views hold which akt (T3).

<!-- akt: PLAN-004-D07 -->
---
type: decision
revision: 2
rank: b19
state: decided
in: [PLAN-004]
title: Picking a user
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Picking a user**: the server lists no users, since Konklav keeps the
memberships; the app offers the viewer, the tenant's contacts, and a
subject id typed in (T3, for T5).

<!-- field: why -->
the server lists no users, since Konklav keeps the
memberships; the app offers the viewer, the tenant's contacts, and a
subject id typed in (T3, for T5).

<!-- akt: PLAN-004-D08 -->
---
type: decision
revision: 2
rank: b1A
state: decided
in: [PLAN-004]
title: Boards move members with buttons
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Boards move members with buttons**, up and down within a column, which
are keyboard-accessible and what Playwright drives; dragging can come
with the visual design (T5).

<!-- field: why -->
, up and down within a column, which
are keyboard-accessible and what Playwright drives; dragging can come
with the visual design (T5).

<!-- akt: PLAN-004-D09 -->
---
type: decision
revision: 2
rank: b1B
state: decided
in: [PLAN-004]
title: "The akt page shows values, and edits on request"
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The akt page shows values, and edits on request** ("Edit fields"), so
a change made elsewhere shows at once unless the person is editing, and
never overwrites their input (T5).

<!-- field: why -->
("Edit fields"), so
a change made elsewhere shows at once unless the person is editing, and
never overwrites their input (T5).

<!-- akt: PLAN-004-D10 -->
---
type: decision
revision: 2
rank: b1C
state: decided
in: [PLAN-004]
title: Seeding over the control plane only
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Seeding over the control plane only**, not a bootstrap set: one script
makes the tenant, roles, people, OIDC clients and service principals,
and writes Kuria's configuration (T6).

<!-- field: why -->
, not a bootstrap set: one script
makes the tenant, roles, people, OIDC clients and service principals,
and writes Kuria's configuration (T6).

<!-- akt: PLAN-004-D11 -->
---
type: decision
revision: 2
rank: b1D
state: decided
in: [PLAN-004]
title: Software signs in to Konklav with a secret key
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Software signs in to Konklav with a secret key**, then moves into the
tenant, so its tokens carry Konklav's own audience; Kuria accepts that
beside its OIDC client's (`KONKLAV_AUDIENCE=kuria,konklav`). The client
credentials grant only gives identity tokens, which can't move into a
tenant (T6).

<!-- field: why -->
, then moves into the
tenant, so its tokens carry Konklav's own audience; Kuria accepts that
beside its OIDC client's (`KONKLAV_AUDIENCE=kuria,konklav`). The client
credentials grant only gives identity tokens, which can't move into a
tenant (T6).

<!-- akt: PLAN-004-D12 -->
---
type: decision
revision: 2
rank: b1E
state: decided
in: [PLAN-004]
title: urql posts every query
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**urql posts every query** (`preferGetMethod: false`): the server takes
GraphQL only as a JSON POST (T6).

<!-- field: why -->
(`preferGetMethod: false`): the server takes
GraphQL only as a JSON POST (T6).

<!-- akt: PLAN-004-D13 -->
---
type: decision
revision: 2
rank: b1F
state: decided
in: [PLAN-004]
title: The app keeps no token or key in web storage
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The app keeps no token or key in web storage**; SvelteKit keeps only
its navigation state there, which the sign-in test allows (T6).

<!-- field: why -->
; SvelteKit keeps only
its navigation state there, which the sign-in test allows (T6).

<!-- akt: PLAN-004-D14 -->
---
type: decision
revision: 2
rank: b1G
state: decided
in: [PLAN-004]
title: The head of product gets an email-link credential
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The head of product gets an email-link credential** beside her
password: Konklav reports a password's email as unverified and an email
link's as verified, and only a verified email links a contact (T6).

<!-- field: why -->
beside her
password: Konklav reports a password's email as unverified and an email
link's as verified, and only a verified email links a contact (T6).

<!-- akt: PLAN-004-D15 -->
---
type: decision
revision: 2
rank: b1H
state: decided
in: [PLAN-004]
title: Playwright
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**Playwright**: real browsers, contexts per person in one test, traces on
failure, and role-based locators that keep the app accessible.

<!-- field: why -->
real browsers, contexts per person in one test, traces on
failure, and role-based locators that keep the app accessible.

<!-- akt: PLAN-004-D16 -->
---
type: decision
revision: 2
rank: b1I
state: decided
in: [PLAN-004]
title: The pipeline acts through the API in the walk-through
---

<!-- field: context -->
Recorded in the plan "Frontend and End-to-End Tests", migrated from `.agents/plans/frontend-and-e2e.md`.

<!-- field: decision -->
**The pipeline acts through the API in the walk-through**, since it is
software; people act through the app.

<!-- field: why -->
, since it is
software; people act through the app.
