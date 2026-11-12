# kuria-store

Kuria's Postgres storage: the connection pool, the embedded migrations, and
the queries the server runs.

## Responsibilities

- Building a pool (`connect`) and applying migrations
  (`run_pending_migrations`), which are embedded from
  [`migrations/`](migrations/).
- The Diesel schema ([`src/schema.rs`](src/schema.rs)), kept in step with the
  migrations by hand.
- Queries, by area:
  - [`sessions`](src/sessions.rs): login attempts and browser sessions,
    including the row lock a refresh holds.
  - [`akter`](src/akter.rs): akt types and their versions, akter, akt
    revisions, per-tenant counters, and the change-event outbox.
  - [`erasure`](src/erasure.rs): deleting what Kuria holds about a subject,
    and the time erasures are handled up to.

The migrations also make `akt_revision` append only: a trigger refuses every
update and delete.

Out of scope: what stored values mean. Browser sessions' tokens arrive here
already encrypted by the server.

## Testing

It is tested through the server's database tests, which create a fresh
database each with `kuria-test-support`.
