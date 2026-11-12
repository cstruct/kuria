# Architecture

This document describes how Kuria is built: the parts it consists of, where its data lives, and the decisions that shape both. What Kuria has to do is in [requirements.md](requirements.md), and the terms used here are defined in [nomenclature.md](nomenclature.md).

This is a starting point. Decisions marked *open* are not settled yet.

## Overview

```
 browser ──── React app (Vite)
                 │  GraphQL queries and mutations over HTTP
                 │  GraphQL subscriptions over WebSocket
                 ▼
 code host, ── Kuria server (Rust, axum) ──── Konklav
 monitoring,     │  GraphQL endpoint            tokens, authorization data
 scanners ──────►│  HTTP endpoints for webhooks,
   (webhooks)    │  attachments and exports
                 │
          ┌──────┴───────────────┐
          ▼                      ▼
       Postgres                Valkey
         akter, revisions,       live change events and a short
         links, configuration,   window of recent ones,
         the change-event        rate limits
         outbox
```

- **Kuria server**: one Rust binary serving GraphQL, a few plain HTTP endpoints, and WebSocket subscriptions. It can run as several instances behind a load balancer. Instances share nothing except Postgres and Valkey.
- **React app**: a single-page app built with Vite, talking to the server only through GraphQL, plus the HTTP endpoints for files.
- **Postgres**: the record. It holds every akt, its full history, its links, the runtime configuration, and every change event.
- **Valkey**: state the instances share that is not a record: live change events, a short window of recent ones, and rate limits. Losing it loses nothing that cannot be rebuilt from Postgres, so it is run without persistence.
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
| `core` | Identifier and revision types used by several crates. Kept tiny. | none |
| `config` | The model of akt types, fields, states, transitions and link kinds, and its validation. | none |
| `engine` | Evaluating a transition against an akt and its configuration: conditions, prohibitions, required fields, derived fields, obligations. | none |
| `store` | Diesel models, migrations, queries, and writing revisions. | Postgres |
| `api` | The GraphQL schema and transport types. | none |
| `server` | axum routes, resolvers, Konklav integration, subscriptions, webhooks. | all |

`config` and `engine` take values in and return values out. Every rule that decides whether an akt may move, and what it looks like afterwards, can therefore be tested without a database. That is where most of the tests should be.

### Writing an akt

Every change to an akt goes through one path:

1. Load the akt, its current revision, and the configuration version of its akt type.
2. Ask `engine` whether the requested transition or edit is allowed, and what it produces: new field values, derived fields, obligations, links to create.
3. In one transaction: append the revision, update the current state, write links and obligations, and append a change event to the outbox.
4. Reject the write if the akt's current revision is no longer the one loaded in step 1 (optimistic concurrency).

No other code writes to the akt tables.

## GraphQL schema

### One fixed schema

Akt types are defined at runtime, but the GraphQL schema is not generated from them. The schema is the same for every tenant and every configuration:

```graphql
type Akt {
  id: ID!
  number: String!
  type: AktType!
  state: State!
  fields: [FieldValue!]!
  links(kind: String, direction: LinkDirection): [Link!]!
  revisions: [AktRevision!]!
}

union FieldValue = TextValue | ChoiceValue | ScaleValue | DateValue
                 | SubjectValue | ReferenceValue | TableValue | ...
```

Typing comes from the field kind, not from the akt type. The React app reads the `AktType` (its fields, states and transitions) and builds forms, boards and views from it.

A schema generated per configuration would give each akt type its own GraphQL type, but the frontend's generated client code would then depend on a tenant's configuration and break whenever it changed. That trade is not worth it.

### Mutations

Mutations follow what people do, not table rows: `createAkt`, `editAkt`, `performTransition`, `addLink`, `sign`. Each takes the revision the client last saw, and fails if it is stale.

### Limits

- Typed links make deep queries easy to write and expensive to run. The server enforces a maximum depth and complexity per query.
- Nested lookups (an akt's links, a link's target, a target's type) go through dataloaders to avoid one query per row.
- Authorization is checked in resolvers, per akt type, field and transition, not only at the endpoint.

## Storage

| Table | Holds |
|---|---|
| `akt_type_version` | Each version of each akt type's configuration. Immutable once used. |
| `akt` | The current state of each akt: type, configuration version, state, and fields as JSONB. |
| `akt_revision` | Every change to every akt, append only: author, capacity they acted in, transition, reason, the fields that changed, and a hash of the result. |
| `akt_link` | Typed links between akter: kind, source, target, and optionally the target revision it pins. |
| `signature` | Electronic signatures: signer, meaning, the revision and hash signed, and when. |
| `obligation` | Open obligations created by transitions, with due dates. |
| `attachment` | Metadata of uploaded files. The bytes live in object storage. |
| `change_event` | The outbox: one row per change, in order (a sequence per tenant), naming the tenant, the akt and its new revision, and when it was published to Valkey. |
| `erasure_watermark` | The time Kuria has handled Konklav's erasures up to: the latest erasure read from Konklav's list. One row, moved only forward. |
| `login_attempt` | Browser sign-ins started at Konklav and not completed: the `state`, the PKCE verifier and where to return. Kept for ten minutes. |
| `browser_session` | Browser sessions: the SHA-256 of the session cookie, the subject and tenant, and the Konklav tokens, encrypted. |

- `akt_revision` is the record of truth. `akt` is a projection of it that is kept in the same transaction, so it is always consistent and can be rebuilt.
- Fields are JSONB in `akt` so that queries can filter on any field with GIN indexes. Each value is validated against its field kind before it is written, so the JSON has a known shape.
- Links are rows, not JSON, because traceability queries run over them and need indexes in both directions.
- Every revision records the configuration version it was validated against, so an old akt can still be read correctly after its akt type has changed.
- Once an akt reaches a final state, `store` refuses any further revision except a transition the configuration explicitly allows from that state.
- All tables carry the tenant, taken from Konklav. Every query filters on it. *Open:* whether to also use Postgres row-level security as a second line of defence.

## Real-time updates

Postgres is the record of every change, and Valkey carries them to the instances and keeps a short window of recent ones. Nothing is written to Valkey that is not already committed in Postgres.

1. **Outbox.** The transaction that writes a revision also appends a `change_event` row, with the tenant's next sequence number. The event names the tenant, the akt and its new revision, and nothing else.
2. **Publishing.** After the commit, the writing instance adds the event to the tenant's stream in Valkey (`XADD`, trimmed to a bounded length) and marks the row published. A relay, run by one instance at a time under a Postgres advisory lock, publishes any row still unpublished a few seconds later, so an instance that dies between commit and publish loses nothing. An event may therefore be published twice; consumers ignore sequence numbers they have seen.
3. **Delivery.** Each instance reads the streams of the tenants it has subscribers in (`XREAD BLOCK`) and passes each event to the subscriptions that may see that akt. A subscription delivers small events ("akt X is now at revision N"), and the client fetches what it needs through ordinary queries.
4. **Catching up.** A client keeps the last sequence number it saw. When it comes back, it asks for everything after it: the server answers from the Valkey stream while the cursor is inside its window, and from `change_event` in Postgres when it is older. Either way the client gets the same events in the same order, filtered by the same permissions.

Sending events rather than whole objects means permissions are checked in one place, the query resolvers, and Valkey never holds field values or personal data, only identifiers.

## Identity and access

- Requests are authenticated with `konklav-axum`. Each route carries a route policy, and the relying client verifies the token, its session and any DPoP proof once per request, then makes one decision. The GraphQL endpoint admits only tenant tokens.
- API clients (integrations, scripts, scanners) send a Konklav access token in the `Authorization` header, as a bearer or DPoP token.
- Browsers never hold tokens. Kuria acts as their backend: a confidential OIDC client of Konklav that signs them in with the authorization code flow and PKCE, keeps the tokens in `browser_session`, encrypted, and gives the browser an opaque `HttpOnly`, `SameSite=Strict` cookie ([spec](spec/browser-sessions.md)).
  - A layer in front of `konklav-axum` turns the cookie into `Authorization: Bearer <access token>`, so cookie requests are verified exactly like any other.
  - Access tokens are refreshed on the server shortly before they expire. Konklav rotates refresh tokens and treats a reused one as theft, so a refresh holds the session's row lock (`select … for update`) and two requests, on any instance, never present the same refresh token.
  - Cookies are sent automatically, so cookie requests that can change something, and every WebSocket upgrade, must come from an allowed `Origin`, and `POST /graphql` takes only `application/json`, which a cross-site page cannot send without a CORS preflight.
  - WebSocket upgrades carry the cookie like any request, so subscriptions need no token in the URL.
  - Serving the frontend on Kuria's own site is required by `SameSite=Strict`; in development, Vite proxies `/graphql` and `/auth` to the server.
- *Open:* authorization finer than the endpoint. A DPoP proof is accepted once, so for API clients presenting DPoP, resolvers cannot each ask the relying client for a decision. Browser requests present a bearer token, which can be decided on several times. Per-akt-type and per-transition checks will need decisions evaluated from the request's verified claims and cached entitlements, or one decision per request covering everything its operation needs.
- Roles named in akt type configuration map to Konklav roles. Kuria evaluates those roles against the data Konklav publishes, and adds its own checks for what only Kuria knows, such as separation of duties on a specific akt.
- *Open:* signing. A signature should probably require the signer to authenticate again at the moment of signing. How Konklav supports that, and what Kuria stores as proof, has to be settled before `akt_revision` and `signature` are fixed.

## Erasure

When Konklav erases a subject, Kuria deletes what it holds about them ([spec](spec/erasure.md)). The relying client hands over a notice for each erasure, and says when some may have been missed, which it does on every start because it keeps its feed position in memory. Kuria then reads Konklav's durable list of erased subjects from the time stored in `erasure_watermark`, deletes, and only then moves that time forward. Every instance does this; deleting is idempotent and the stored time only moves forward, so they don't conflict.

Today that means browser sessions. Akter will make it harder: a revision records who made it, and a QMS has to keep records for years.

- *Open:* what erasure does to akter. The likely answer is that records keep the subject's identifier, which Konklav keeps without personal data after an erasure, while anything Kuria copied about the person (names, email addresses in free text, attachments) is removed or redacted. The records' retention requirements have to be settled against that.

## Other HTTP endpoints

Some things are simpler outside GraphQL:

- **Webhooks** from code hosts, pipelines, monitoring and scanners. Each source has its own endpoint and verifies its own signatures. Events are deduplicated by their source identifier before they open or change an akt.
- **Attachments**: upload and download, streamed to and from object storage.
- **Exports**: a complete export of akter with their revisions, signatures, links and attachments, for audits and backups that do not depend on Kuria being available.

## Frontend

- **Vite** and **React**, with TypeScript.
- **graphql-codegen** generates typed operations from the fixed schema.
- **urql** with **Graphcache** as the GraphQL client: a normalized cache, and subscription events that mark the akter they name as stale so they are refetched.
- Forms, boards and akt views are built from `AktType` definitions at runtime, not written per akt type.

## Decided

- Akt types are written as whole documents and submitted through the API (`defineAktType`); each submission becomes a new, immutable akt type version. A UI editor or a CLI pushing files from version control can sit on top.
- Akt numbers come from a number pattern in each akt type, such as `CAPA-{year}-{seq:4}`, counting per tenant and per pattern as rendered without its sequence.

## Open decisions

- Signing: re-authentication and the proof stored with a signature.
- Row-level security in Postgres alongside tenant filtering in queries.
- Object storage for attachments, and how exports include them.
- Upgrading akter to a newer akt type version. An akt stays on the version it was created with, so changing an akt type only affects new akter.
- Authorization of akt types and akter: who may define akt types, and who may create, edit and transition akter of each type. Until then, any subject with a tenant token can do all of it.
