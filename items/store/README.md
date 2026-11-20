# kuria-store

Kuria's Postgres storage: the connection pool, the embedded migrations, and
the queries the server runs.

## Responsibilities

- Building a pool (`connect`) and applying migrations
  (`run_pending_migrations`), which are embedded from
  [`migrations/`](migrations/). They need Postgres with pg_search
  preloaded, for text search: the first that does creates the extension,
  which takes a superuser, so where Kuria's role isn't one an operator
  creates `pg_search` (with `CASCADE`, for pgvector) first.
- The Diesel schema ([`src/schema.rs`](src/schema.rs)), kept in step with the
  migrations by hand.
- Queries, by area:
  - [`sessions`](src/sessions.rs): login attempts and browser sessions,
    including the row lock a refresh holds.
  - [`akter`](src/akter.rs): akt types and their versions, akter (with
    their titles, kept by the write path), searching akter by a number's
    beginning or a title's words, akt revisions, per-tenant counters, and the change-event outbox: taking
    unpublished events (`for update skip locked`), marking them published,
    and reading them in order with their akter's numbers and types.
  - [`containers`](src/containers.rs): memberships, the cycle check (a
    recursive query), ranks with their neighbours, rank change events, the
    tenant's order lock, and locking several akter in a fixed order.
  - [`people`](src/people.rs): the names and emails recorded at sign-in.
  - [`contacts`](src/contacts.rs): contacts, telling a taken email apart
    from other failures, editing only unlinked ones, and linking the
    unlinked one with an email, whatever its case.
  - [`attachments`](src/attachments.rs): attachments' metadata, which akt
    fields hold which attachment, and the akt types of an attachment's
    holders.
  - [`drafts`](src/drafts.rs): live drafts, each a compacted Yjs state
    and the updates after it, and every update with who made it, when and
    the fields it changed, kept even when compacted.
  - [`erasure`](src/erasure.rs): deleting what Kuria holds about a subject
    (sessions, their name and email, and those of contacts linked to them),
    and the time erasures are handled up to.

The migrations also make `akt_revision` append only: a trigger refuses every
update and delete. Ranks use `COLLATE "C"`, so they sort byte by byte as the
engine's rank keys require, and are unique per tenant.

Out of scope: what stored values mean. Browser sessions' tokens arrive here
already encrypted by the server.

## Testing

```sh
mise run db:up
cargo test -p kuria-store
```

- [`tests/containers.rs`](tests/containers.rs): unique ranks per tenant,
  byte-order sorting, memberships stored once and never of an akt itself,
  finding akter contained through other containers, members in rank order
  by readable type, and the containment migration ranking existing akter in
  creation order (applied step by step with `run_next_migration`).

Most queries are also covered by the server's database tests.
