# The example stack

Serves one example from [`docs/examples`](../docs/examples/README.md) with
its bundle loaded, or, for one without a bundle, its akt types defined, to
click around in: a Konklav of its own, kept between
starts, and a fresh Kuria for every start, with the app from Vite's dev
server, so edits under `items/web` show at once. Its ports and database
don't clash with the end-to-end stack's (`e2e/`) or the dogfooding stack's
(`dogfood/`), so `mise run e2e` can run beside it.

## Running it

```sh
mise run example planning     # or release-management, documents
mise run example:down         # stops Konklav, keeping it
mise run example:reset        # forgets it all
```

It prints who can sign in at <http://localhost:18290> (use `localhost`):
the examples' people, Devi Larsen (`dev@shop.example`), Rami Moberg
(`rm@shop.example`) and Hedda Lund (`hedda@shop.example`), with local-only
passwords. Ctrl-C stops Kuria and Vite.

The first start makes Konklav's keys and secrets, seeds it and starts its
containers; later ones find it running and only make Kuria's database
again, so they are ready in about five seconds. The first start after the
server's code changes compiles it first.

Every checkout and worktree shares Konklav's compose project, ports and
data, but keeps its own `.state/`. A start that finds a Konklav not seeded
by this checkout (another checkout's, or one this checkout's `.state/`
doesn't name) throws its data away and seeds it again, which takes about
ten seconds. So does a start after the roles, grants or people in
[`scripts/seed.ts`](scripts/seed.ts) change: `.state/seeded.json` records
their fingerprint.

| What | Where |
|---|---|
| The app | Vite's dev server on <http://localhost:18290>, passing the API (and Konklav's login surface) to Kuria; Kuria's public URL |
| Kuria | `127.0.0.1:18291`, the database `kuria_example` on the development Postgres, made again on each start, and the development Valkey and S3 gateway |
| Konklav | the compose project `kuria-example`, ports 18280–18282, its Postgres on the volume `konklav-data` |
| Who signs in | the tenant `shop`, with the release-management example's roles as the end-to-end seed grants them: the developer works on backlogs, tests and documents, the release manager on releases and plans too, the head of product reads |

## How it loads an example

- The examples' bundles all name the same people. Setup seeds Konklav with
  those very subjects and emails, so a bundle's people are the tenant's
  users with no subjects mapping (`bundle.import.subjects`).
- Each start drops and makes `kuria_example`, then starts `kuria-server`
  with `KURIA_BOOTSTRAP_BUNDLE` set to the example's bundle, into the
  seeded tenant (`bundle.bootstrap.*`). An example without `bundle/` is its
  akt type documents alone: each JSON file in it with a `key` and `states`
  is defined through `defineAktType`, those naming types not yet defined
  again after the others, into an empty tenant. It checks the tenant holds
  just the example's akt types before printing who can sign in.
- The server runs from a copy in `.state/`, made again only when the build
  changes: on macOS a binary cargo has just checked takes seconds longer to
  start, which is also why it isn't `cargo run`.

## Files

- [`compose.yaml`](compose.yaml), [`konklav/config.styx`](konklav/config.styx):
  Konklav, with the app's origin as its issuer and login page.
- [`scripts/prepare.ts`](scripts/prepare.ts): Konklav's signing keys and
  secrets, made once into `.state/` (never committed).
- [`scripts/up.ts`](scripts/up.ts): starts Konklav's containers unless
  this checkout's Konklav already answers, and starts it afresh if the one
  found holds another checkout's data or an older seeding.
- [`scripts/seed.ts`](scripts/seed.ts): what setup seeds Konklav with
  (actions, roles, who holds them, the examples' people) and its
  fingerprint.
- [`scripts/setup.ts`](scripts/setup.ts): seeds Konklav once and writes
  `.state/kuria.env` (Kuria's configuration) and `.state/seeded.json` (who
  can sign in, and an admin's secret key for the check).
- [`scripts/serve.ts`](scripts/serve.ts): the database, the server and Vite.
