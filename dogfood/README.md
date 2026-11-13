# Dogfooding

Kuria's own plans live in a Kuria on this machine: a Konklav and a Kuria
that keep their data between runs, unlike the end-to-end stack (`e2e/`),
which starts empty every time. Plans are the planning example's akt types
([docs/examples/planning](../docs/examples/planning/README.md)), read and
written with the `kuria` CLI ([.agents/plans/PLANS.md](../.agents/plans/PLANS.md)).

## Running it

```sh
mise run dogfood:serve        # Konklav, then Kuria on http://localhost:18190; Ctrl-C stops Kuria
mise run dogfood:kuria -- ls plan --state active
mise run dogfood:down         # stops Konklav, keeping its data
```

The first `dogfood:serve` makes Konklav's keys and secrets, seeds it, makes
the database `kuria_dogfood`, and builds the CLI. Define the planning
example once, then move the Markdown plans in:

```sh
mise run dogfood:kuria -- types define docs/examples/planning/{task,gap,decision,plan}.json
node dogfood/scripts/migrate.ts     # DRY=1 says what it would make
```

| What | Where |
|---|---|
| Kuria | `http://localhost:18190`, the database `kuria_dogfood` on the development Postgres, and the development Valkey and S3 gateway |
| Konklav | the compose project `kuria-dogfood`, ports 18180–18182, its Postgres on the volume `konklav-data` |
| Who signs in | the tenant `kuria`, with the role `planner` (every Kuria action): you, by `DOGFOOD_EMAIL` or Git's email and a password made at seeding, and the agent, a service principal with a secret key |

`mise run dogfood:reset` forgets all of it. Its ports don't clash with the
end-to-end stack's, so the tests can run beside it.

## Files

- [`compose.yaml`](compose.yaml), [`konklav/config.styx`](konklav/config.styx):
  Konklav, with Kuria's origin as its issuer and login page.
- [`scripts/prepare.ts`](scripts/prepare.ts): Konklav's signing keys and
  secrets, made once into `.state/` (never committed).
- [`scripts/setup.ts`](scripts/setup.ts): seeds Konklav once and writes
  `.state/kuria.env` (Kuria's configuration), `.state/agent.env`
  (`KURIA_URL`, `KURIA_TENANT`, `KURIA_KEY`) and `.state/seeded.json` (who
  can sign in, with your password).
- [`scripts/serve.ts`](scripts/serve.ts): Kuria, on a database it keeps.
- [`scripts/kuria.ts`](scripts/kuria.ts): the CLI as the agent, behind
  `mise run dogfood:kuria`; paths are the caller's.
- [`scripts/migrate.ts`](scripts/migrate.ts): moves `.agents/plans/*.md`
  into plans, tasks, gaps and decisions, once each (`.state/migrated.json`).

## Signing in yourself

In a browser, at `http://localhost:18190`, with the email and password in
`.state/seeded.json`. From a terminal, as yourself rather than the agent:

```sh
kuria login --url http://localhost:18190
```

## Backups

The data is a Docker volume and a database: `mise run dogfood:kuria --
export --whole-tenant -o <dir>` writes it all as a bundle, which `import`
restores into an empty tenant.
