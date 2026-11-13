# Architecture

This document describes how Kuria is built: the parts it consists of, where its data lives, and the decisions that shape both. What Kuria has to do is in [requirements.md](requirements.md), and the terms used here are defined in [nomenclature.md](nomenclature.md).

This is a starting point. Decisions marked *open* are not settled yet.

## Overview

```
 browser ──── Svelte app (Vite)
 terminal ─── kuria CLI (Rust)
                 │  GraphQL queries and mutations over HTTP
                 │  GraphQL subscriptions over WebSocket
                 ▼
 code host, ── Kuria server (Rust, axum) ──── Konklav
 monitoring,     │  GraphQL endpoint            tokens, authorization data
 scanners ──────►│  HTTP endpoints for webhooks,
   (webhooks)    │  attachments and exports
                 │
          ┌──────┴───────────────┬────────────────────┐
          ▼                      ▼                    ▼
       Postgres                Valkey             object storage
         akter, revisions,       live change          (S3-compatible)
         links, configuration,   events and a         attachments'
         the change-event        short window of      bytes
         outbox, attachments'    recent ones,
         metadata                rate limits
```

- **Kuria server**: one Rust binary serving GraphQL, a few plain HTTP endpoints, and WebSocket subscriptions. It can run as several instances behind a load balancer. Instances share nothing except Postgres and Valkey.
- **Svelte app**: a single-page app written in Svelte and built with Vite, talking to the server only through GraphQL, plus the HTTP endpoints for files.
- **`kuria` CLI**: a command-line client for people and agents, in this workspace (`items/cli`), talking to the same GraphQL and HTTP endpoints with a DPoP-bound Konklav session ([spec](spec/cli.md)).
- **Postgres**: the record. It holds every akt, its full history, its links, the runtime configuration, and every change event.
- **Valkey**: state the instances share that is not a record: live change events, a short window of recent ones, and rate limits. Losing it loses nothing that cannot be rebuilt from Postgres, so it is run without persistence.
- **Object storage**: any S3-compatible store, holding attachments' bytes under their SHA-256 per tenant. Locally, versitygw.
- **Konklav**: authenticates people and software and publishes what they may do. Kuria is a Konklav relying system.

## Server

### Stack

- **axum** for HTTP and WebSocket, matching Konklav.
- **async-graphql** for the GraphQL schema, resolvers, dataloaders and subscriptions (the `graphql-ws` protocol).
- **diesel-async** with Postgres for storage, matching Konklav.
- **redis** (the client library) for Valkey, which speaks Redis' protocol. Rate limits use Konklav's relying client's Valkey-backed `RateLimit`.
- **tracing** with OpenTelemetry export for logs and traces.

### Crates

The workspace splits by ownership, as Konklav does:

| Crate | Owns | I/O |
|---|---|---|
| `config` | Akt type documents: fields, states, transitions, containment, boards, draft recipes and number patterns, and their validation. | none |
| `engine` | What a create, edit or transition produces, field values, state hashes, rank keys, board placement and drafts. | none |
| `store` | Diesel models, migrations and queries. | Postgres |
| `server` | axum routes, the GraphQL schema and resolvers, Konklav integration, browser sessions, erasure, and the write path for akter. | all |
| `test-support` | Test databases and Valkey for the other crates' tests. | Postgres |

Planned, once there is a reason: a tiny `core` for identifier types several crates share, and an `api` crate for transport types when a client other than the server needs them.

`config` and `engine` take values in and return values out. Every rule that decides whether an akt may move, and what it looks like afterwards, can therefore be tested without a database. That is where most of the tests should be.

### Writing an akt

Every change to an akt goes through one path, `items/server/src/akter.rs` and its submodules:

1. Load the akt, its current revision, and the akt type version it was created with.
2. Ask Konklav whether the caller may (see Identity and access), and `engine` whether the change is allowed and what it produces.
3. In one transaction: append the revision, update the current state, write memberships, and append a change event to the outbox.
4. Reject an edit or transition if the akt's current revision is no longer the one the client named (optimistic concurrency).

No other code writes to the akt tables. Every path takes its locks in one order, so no two changes deadlock: the tenant's order lock, then akt rows by id, then number counters, then the change-event counter.

## GraphQL schema

### One fixed schema

Akt types are defined at runtime, but the GraphQL schema is not generated from them. The schema is the same for every tenant and every configuration:

```graphql
type Akt {
  id: ID!
  number: String!
  type: AktType!
  state: State!
  revision: Int!
  rank: String!
  fields: [FieldValue!]!
  members(type: String, where: [Condition!], first: Int, after: String): AktConnection!
  links: [Link!]!
  containers: [Akt!]!
  board(first: Int): Board!
  revisions: [AktRevision!]!
}

union FieldValue = TextValue | ChoiceValue | ChoicesValue | DateValue
                 | UserValue | CheckboxValue | ReferenceValue | GitValue
                 | DockerValue | AttachmentValue

# Whom a user field or a checkbox names: a user, or a contact who may not
# be one yet.
union Someone = User | Contact
```

Typing comes from the field kind, not from the akt type. The Svelte app reads the `AktType` (its fields, states and transitions) and builds forms, boards and views from it.

A schema generated per configuration would give each akt type its own GraphQL type, but the frontend's generated client code would then depend on a tenant's configuration and break whenever it changed. That trade is not worth it.

### Mutations

Mutations follow what people do, not table rows: `defineAktType`, `createAkt` (optionally inside containers), `editAkt`, `performTransition`, `addToContainer`, `removeFromContainer`, `moveAkt` and `draftField`; and for contacts `addContact`, `editContact`, `linkContact` and `eraseContact`. Edits, transitions and drafts take the revision the client last saw and fail if it is stale; arranging members and moving don't, so that a board drag isn't refused because someone edited an item. Links: `addLink` and `removeLink` ([spec](spec/links.md)). Later: `sign`.

Errors carry a `code` extension the client can act on, such as `STALE`, `FORBIDDEN`, `NOT_FOUND`, `CYCLE`, `FROZEN` or `DUPLICATE_EMAIL`; the server README lists them all.

### Limits

- Typed links make deep queries easy to write and expensive to run. The server enforces a maximum depth and complexity per query.
- Nested lookups avoid one query per row: a list's akt types are read in one query, and a board's members in one, whatever its size. Dataloaders come when links make nesting deeper.
- Authorization is checked per operation, by akt type and transition, not only at the endpoint (see Identity and access).

## Storage

| Table | Holds |
|---|---|
| `akt_type`, `akt_type_version` | Each akt type key's latest version, and every version as submitted. Immutable once created. |
| `akt` | The current state of each akt: type version, state, fields as JSONB, current revision, and rank. |
| `akt_revision` | Every change to every akt, append only (a trigger refuses updates and deletes): author, transition, reason, the fields that changed, a membership it added or removed, and the SHA-256 of the result. |
| `akt_membership` | Current memberships, container and member. Their history is in both akter's revisions. |
| `tenant_counter` | Per-tenant counters and locks: akt number sequences per pattern scope, the change-event sequence, and the order lock. |
| `change_event` | The outbox: one row per change, in order (a sequence per tenant), naming the tenant, the akt and, for a `revision` event, its new revision, or for a `rank` event, none; and when it was published to Valkey. |
| `attachment` | Each upload: tenant, name, media type, size, SHA-256, uploader and time, and an SBOM's summary. Its bytes are in object storage at `<tenant>/<SHA-256>`. |
| `akt_attachment` | Which field of which akt holds which attachment, kept by the write path, so a download is decided by the akt types holding the file. |
| `contact` | People a tenant names before, or without, them being users: a name and an optional email, unique per tenant regardless of case, and the subject it is linked to once they sign in with that email verified, or someone links it by hand. Erasing it keeps the row without name or email. |
| `tenant_person` | Which tenants each subject signed in to through a browser, so a tenant's users can be listed. |
| `person` | The name and email recorded at a subject's last browser sign-in, from Konklav's userinfo endpoint. Erased with the subject. |
| `erasure_watermark` | The time Kuria has handled Konklav's erasures up to: the latest erasure read from Konklav's list. One row, moved only forward. |
| `login_attempt` | Browser sign-ins started at Konklav and not completed: the `state`, the PKCE verifier and where to return. Kept for ten minutes. |
| `browser_session` | Browser sessions: the SHA-256 of the session cookie, the subject and tenant, and the Konklav tokens, encrypted. |

Planned: `akt_link` (typed links besides containment, [spec](spec/links.md)), `subject_alias` (from imports), `signature` and `obligation`.

- `akt_revision` is the record of truth. `akt` is a projection of it that is kept in the same transaction, so it is always consistent and can be rebuilt.
- Fields are JSONB in `akt` so that queries can filter on any field with GIN indexes. Each value is validated against its field kind before it is written, so the JSON has a known shape.
- Memberships are rows, not JSON, because boards and drafts query them in both directions; links will be too.
- Every revision records the configuration version it was validated against, so an old akt can still be read correctly after its akt type has changed.
- An akt in a final state can't be edited or transitioned, and if it is a container its members are frozen. It can still join or leave other containers: a finished item can still go into a release candidate.
- All tables carry the tenant, taken from Konklav. Every query filters on it. *Open:* whether to also use Postgres row-level security as a second line of defence.

## Containers, order and boards

Containment is the one link kind Kuria builds in ([containers](spec/containers.md), [ordering](spec/ordering.md), [boards](spec/boards.md)).

- An akt type says what its akter may contain (`contains`, optionally `exclusive`, and `while` for the states it accepts them in), and an akt type may have to be created inside another (`created_in`), which `{parent}` in its number pattern then renders: `RELEASE-2026-0001-RC-0001`.
- An akt can be in many containers, such as a backlog, an epic and a release candidate. Containment is acyclic, checked with a recursive query under the container's lock.
- Every akt has one rank in its tenant, a fractional-indexing key sorted byte by byte (`COLLATE "C"`). Every list of members and every board orders by it, so a move made on one board shows on every board. A move places an akt directly before or after a neighbour, disturbing nothing else.
- Every rank change takes the tenant's order lock first, so two never compute the same key; the unique index on `(tenant_id, rank)` stays as a guard.
- A board maps the states of the container's member types into columns its akt type configures, then an `unmapped` column. Placement is a pure function in `engine`.
- A multiline text field can be drafted from the akter around it by a recipe on the field ([drafts](spec/drafts.md)), such as release notes from what a release's candidates and hotfixes contain. A draft is an ordinary edit.

`docs/examples/release-management` shows these together, and `items/server/tests/release_flow.rs` walks a release through it.

## Real-time updates

Postgres is the record of every change, and Valkey tells every instance that there is something new. Nothing is written to Valkey that is not already committed in Postgres ([spec](spec/changes.md)).

1. **Outbox.** The transaction that writes a revision, or moves an akt, also appends a `change_event` row, with the tenant's next sequence number. Taking that number locks the tenant's counter until commit, so sequence numbers follow commit order: once event N is visible, every event before it is too.
2. **Publishing.** Every instance runs a relay that takes unpublished rows (`for update skip locked`, so instances never publish the same row at once), adds each to its tenant's stream in Valkey (`XADD`, trimmed to a bounded length), and marks it published. The relay wakes as soon as its own instance writes, and every half second otherwise, so a row an instance left unpublished is picked up by any other. An event may be published twice; nothing depends on it being once.
3. **Fan-out.** Each instance keeps one hub per tenant it has subscribers in, and one reader that waits on those tenants' streams (`XREAD BLOCK`). A stream entry is only a wake-up: the hub then reads the tenant's change events after the last one it delivered, from Postgres, in sequence order, and broadcasts them to its subscriptions. Streams published by different instances may interleave out of order; reading from Postgres puts them back in order.
4. **Delivery.** A subscription delivers small events ("akt X is now at revision N", or "akt X moved"), leaving out akter the subscriber may not read. The client fetches what it needs through ordinary queries.
5. **Catching up.** A client keeps the last sequence number it saw and reconnects with it as `after`. The subscription first joins the hub, then reads everything after the cursor from Postgres, then continues with the hub's events past what it already delivered, so nothing is missed or repeated in between. A subscriber that falls behind the hub is caught up from Postgres the same way.

Catching up reads Postgres, not the Valkey stream: it is exact, has no window, and is one code path. The streams are bounded, so a short window of recent events is there if something later wants it.

Subscriptions are served over WebSocket at `/graphql/ws`, authenticated like `/graphql`, and end when the connection's access token expires; the client reconnects with its cursor.

Sending events rather than whole objects means permissions are checked in one place, the query resolvers, and Valkey never holds field values or personal data, only tenants and sequence numbers.

## Identity and access

- Requests are authenticated with `konklav-axum`. Each route carries a route policy, and the relying client verifies the token, its session and any DPoP proof once per request, then makes one decision. The GraphQL endpoint admits only tenant tokens.
- API clients (integrations, scripts, scanners) send a Konklav access token in the `Authorization` header, as a bearer or DPoP token.
- Browsers never hold tokens. Kuria acts as their backend: a confidential OIDC client of Konklav that signs them in with the authorization code flow and PKCE, keeps the tokens in `browser_session`, encrypted, and gives the browser an opaque `HttpOnly`, `SameSite=Strict` cookie ([spec](spec/browser-sessions.md)).
  - A layer in front of `konklav-axum` turns the cookie into `Authorization: Bearer <access token>`, so cookie requests are verified exactly like any other.
  - Access tokens are refreshed on the server shortly before they expire. Konklav rotates refresh tokens and treats a reused one as theft, so a refresh holds the session's row lock (`select … for update`) and two requests, on any instance, never present the same refresh token.
  - Cookies are sent automatically, so cookie requests that can change something, and every WebSocket upgrade, must come from an allowed `Origin`, and `POST /graphql` takes only `application/json`, which a cross-site page cannot send without a CORS preflight.
  - WebSocket upgrades carry the cookie like any request, so subscriptions need no token in the URL.
  - Serving the frontend on Kuria's own site is required by `SameSite=Strict`; in development, Vite proxies `/graphql` and `/auth` to the server.
- Operations are authorized one permission at a time ([spec](spec/authorization.md)). Kuria's actions (`kuria:akt_types:define`, `kuria:akter:read`, `…:create`, `…:edit`, `…:transition`, and tenant-wide `kuria:contacts:read` and `…:manage`) are Konklav catalog data, required tenant-wide or at the scope `kuria_akt_type:<key>`, so a Konklav role grants them for every akt type or for one. A transition can name one more action of its own, such as `capa:approve_closure`. Each permission is decided by the relying client for the request's bearer token, mostly from its cached entitlements, and remembered for the rest of the request.
- People: Konklav keeps no profile data, so Kuria records a subject's name and email from userinfo at browser sign-in ([spec](spec/people.md)). A tenant can also name people who aren't users yet as contacts; the first subject to sign in to the tenant with a contact's email, verified, is linked to it, and values naming the contact show that user from then on without changing ([spec](spec/contacts.md)). Only a verified email links, so nobody takes over someone else's approvals by claiming an address. Konklav never verifies a password's email, so someone who manages contacts can also link a contact to a user who has signed in to the tenant, by hand.
- Akter a subject may not read are invisible: `null`, left out of lists, and `NOT_FOUND` to mutations, decided before anything that could reveal them, such as a stale revision.
- Roles named in akt type configuration map to Konklav roles. Kuria evaluates those roles against the data Konklav publishes, and adds its own checks for what only Kuria knows, such as separation of duties on a specific akt.
- *Open:* signing. A signature should probably require the signer to authenticate again at the moment of signing. How Konklav supports that, and what Kuria stores as proof, has to be settled before `akt_revision` and `signature` are fixed.

## Erasure

When Konklav erases a subject, Kuria deletes what it holds about them ([spec](spec/erasure.md)). The relying client hands over a notice for each erasure, and says when some may have been missed, which it does on every start because it keeps its feed position in memory. Kuria then reads Konklav's durable list of erased subjects from the time stored in `erasure_watermark`, deletes, and only then moves that time forward. Every instance does this; deleting is idempotent and the stored time only moves forward, so they don't conflict.

Today that means browser sessions, the recorded name and email, the tenants they signed in to, and the name and email of every contact linked to the subject; the contact itself stays, so the values naming it still read. Akter will make it harder: a revision records who made it, and a QMS has to keep records for years.

- *Open:* what erasure does to akter. The likely answer is that records keep the subject's identifier, which Konklav keeps without personal data after an erasure, while anything Kuria copied about the person (names, email addresses in free text, attachments) is removed or redacted. The records' retention requirements have to be settled against that.

## Other HTTP endpoints

Some things are simpler outside GraphQL:

- **Webhooks** from code hosts, pipelines, monitoring and scanners. Each source has its own endpoint and verifies its own signatures. Events are deduplicated by their source identifier before they open or change an akt.
- **Attachments**: `POST /attachments` takes the file as its body, named by a percent-encoded `X-Attachment-Name`, up to a configured size, and answers its identifier; GraphQL fields then hold that identifier. `GET /attachments/{id}` streams it back from object storage, always as a download (`Content-Disposition: attachment`, `nosniff`, a sandboxing CSP), so an uploaded page never runs on Kuria's origin.
- **Bundles**: `GET /export` and `POST /import` move selected akt types, with or without their akter, or a whole tenant, as a tar archive ([spec](spec/bundles.md)). Akter keep their numbers and every revision, hashes checked on import; subjects of another Konklav map to this tenant's users by alias, so revisions never change. They serve backups, audits that don't depend on Kuria running, and moving a tenant between instances.
- **The CLI's relay**: `/cli/authorized` is the `kuria-cli` client's redirect URI. Konklav requires exact redirect URIs and has no device grant, so `kuria login` prints a link that can be opened anywhere and collects the code from the relay; the code is bound to the CLI's PKCE verifier and DPoP key, so the relay can't use it ([spec](spec/cli.md)).

## Frontend

- **Svelte** and **Vite**, with TypeScript.
- **graphql-codegen** generates typed operations from the fixed schema.
- **urql** (its framework-independent core, with `@urql/svelte`) and **Graphcache** as the GraphQL client: a normalized cache, and subscription events that mark the akter they name as stale so they are refetched.
- **SvelteKit** in SPA mode (`adapter-static`, no server-side rendering), in `items/web`. The server serves the build from its own origin, with a fallback page for the app's paths ([spec](spec/frontend.md)).
- Forms, boards and akt views are built from `AktType` definitions at runtime, not written per akt type. A type's **presentation** (`presentation` in its document: its title field, the fields its cards and lists show, a tone per state, and an icon and colour for the type) says how its akter look; without one, the app's defaults apply ([spec](spec/akter.md)).
- **The look**: Kuria's own design tokens (colours for light and dark, spacing, type, radii; `prefers-color-scheme`, with contrast checked against WCAG 2.2 AA in unit tests) and a few components of its own, on **Bits UI**'s accessible primitives for the menus, dialogs, comboboxes and command palette that are hard to write well. Icons from `@lucide/svelte`. A gallery at `/design` shows them all.
- **The pages**: a sidebar of akt types with counts (a drawer below 900 pixels), a top bar with breadcrumbs, and search by number or title from anywhere (`/` or Cmd-K). Home shows what changed most recently and each akt type. An akt's page leads with its title, state and transitions; fields are edited one at a time in place, multiline text is sanitized Markdown, containers and members are picked by number or title, and the history is a timeline. A container shows the member types its board maps as columns of cards and every other type as a table, reordered by dragging or from each member's menu; dropping a card on another column offers the transitions that lead there. Akter by type are a table filtered by state and sorted by rank or recent change.
- **Accessibility**: every control has a role and a name, and the end-to-end tests run axe against WCAG 2.2 A and AA on every main page in both appearances and at a phone's width.
- **The login page.** Konklav has no user interface; its authorization endpoint hands the browser to a login application. Kuria's `/login` is that application ([spec](spec/login.md)): it signs the person in through Konklav's subject-facing API with a DPoP-bound session, moves it into the requested tenant, and accepts the login request. Konklav's refresh cookie must be on the page's origin, so the server proxies Konklav's `/v1` and `/.well-known`, and Kuria's public URL is Konklav's issuer. Kuria itself discovers the provider from Konklav directly, so it doesn't depend on its own proxy to start.
- **End-to-end tests** run Playwright against the built app, the server and a seeded Konklav (`e2e/`), walking the release-management example in a browser; the same stack serves the app for clicking around and for the screenshots in [`items/web/README.md`](../items/web/README.md).

## Decided

- Akt types are written as whole documents and submitted through the API (`defineAktType`); each submission becomes a new, immutable akt type version. A UI editor or a CLI pushing files from version control can sit on top.
- Akt numbers come from a number pattern in each akt type, such as `CAPA-{year}-{seq:4}`, counting per tenant and per pattern as rendered without its sequence. `{parent}` counts separately within each parent.
- One shared priority order per tenant, rather than an order per container.
- Joining and leaving a container is a revision of both akter; a move is not.
- Release notes are an akt drafted from members, not a special document.
- Board columns are configured on the container's akt type.
- A container's akt type can say in which states it accepts each member type (`while`), and draft recipes can count akter only in some states (`states`), only the newest per value of a field (`latest_by`), and leave out akter by a choice (`exclude`): a release's artifacts are the newest per component across all its candidates and hotfixes, and a removal is just a newest artifact that `exclude` drops.
- Moving an akt from one container to another is a transfer: one change, recorded once on the akt and on both containers.
- States one akt type names of another, in board columns and draft recipes, are checked whenever either is defined: a reference must name a state the latest version has, and a new version can't drop a state another still names. Definitions in a tenant are checked one at a time.
- Attachments' bytes are in S3-compatible object storage, content-addressed per tenant; their metadata, SBOM summaries and holders are in Postgres. An upload no akt holds yet is visible only to its uploader.

## Open decisions

- Signing: re-authentication and the proof stored with a signature.
- Row-level security in Postgres alongside tenant filtering in queries.
- What erasure does to attachments a subject uploaded, and to the names a bundle carries for them. (Bundles hold attachments' bytes by content hash, [spec](spec/bundles.md).)
- Upgrading akter to a newer akt type version. An akt stays on the version it was created with, so changing an akt type only affects new akter.
- DPoP-bound tokens on `/graphql`. Kuria decides once per permission an operation needs, and a DPoP proof is answered once, so such tokens are refused for now. Decided: Konklav 1.8 will let one accepted presentation answer every decision of its request, still checking the token and session each time, and Kuria then accepts them (`authz.sender-constrained`); the CLI needs it.
