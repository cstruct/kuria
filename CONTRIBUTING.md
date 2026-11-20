# Contributing

## Setting up

Kuria builds with [mise](https://mise.jdx.dev), which pins Rust, Node and
[tracey](https://github.com/bearcove/tracey) in [`mise.toml`](mise.toml),
and runs its services in Docker. On Linux, the CLI also needs libdbus's
headers and `pkg-config` to build, since it keeps its sign-in in the
Secret Service (`libdbus-1-dev pkg-config` on Debian and Ubuntu).

```bash
mise install
```

```bash
mise run db:up
```

`db:up` starts the development Postgres, Valkey and S3 gateway
([`dev/compose.yaml`](dev/compose.yaml)), on ports that don't clash with
Konklav's; `db:down` stops them and `db:reset` forgets their data. The
Postgres is an image of Kuria's own, built the first time from
[`dev/postgres`](dev/postgres/Dockerfile): Postgres 18 with
[pg_search](https://github.com/paradedb/paradedb/tree/main/pg_search),
which text search needs, preloaded. Every Kuria database runs on it, so a
Postgres without pg_search refuses Kuria's migrations. pg_search is
AGPL-3.0, and managed Postgres services don't offer it.
`mise tasks` lists every task.

## How work is done

- **Words first:** use the terms in
  [docs/nomenclature.md](docs/nomenclature.md); a new term goes there
  before it is used anywhere else.
- **Spec first:** requirements are Tracey rules (`r[...]`) in
  [docs/spec/](docs/spec/README.md). Write or change the rule before the
  code, and mark the code and tests that implement and verify it with
  `r[impl ...]` and `r[verify ...]`.
- **Plans:** work that spans commits or sessions gets a plan in
  [`plans/`](plans), following
  [.agents/plans/PLANS.md](.agents/plans/PLANS.md). Read and change plans
  with `mise run plans -- <command>`, which needs nothing running, and
  commit each change with the reason the command gives back.
- **Item READMEs:** each item under `items/` has a README covering what it
  does, how to run and test it, and its traceability. Keep it current with
  the item.
- **Agents** follow the same rules; [AGENTS.md](AGENTS.md) points them here.

## Checking your change

```bash
mise run lint
```

```bash
mise run test
```

`lint` checks formatting, clippy (pedantic, warnings as errors), the app's
lints and types, that `items/web/schema.graphql` is what the server serves
(`mise run web:schema` rewrites it), the spec references, and `plans/`.
`test` starts the services and runs every Rust test, each database test on
its own database, and the app's tests.

For a change to the server, the app or the CLI, also run the end-to-end
tests, which start a seeded Konklav and drive the built app with
Playwright ([e2e/README.md](e2e/README.md)):

```bash
mise run e2e:install
```

```bash
mise run e2e
```

## CI

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs `lint` and
`test` on every pull request and push to `main`. The end-to-end tests take
about seven minutes, so CI runs them only when started by hand: Actions →
CI → Run workflow.
