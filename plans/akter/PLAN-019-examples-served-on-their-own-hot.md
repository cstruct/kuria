---
number: PLAN-019
type: plan
revision: 5
rank: b5K
state: active
title: "Examples served on their own, hot-reloading and fast"
owner: albin@mattsson.io
---

<!-- field: goal -->
Run an example from `docs/examples` to click around in while the end-to-end tests run, with the app hot-reloading as you edit it, and get to signing in quickly.

- **Apart from e2e:** `mise run example <example>` runs a stack of its own: a Konklav on its own ports, its own Kuria database, and the app on its own port, so `mise run e2e` can run beside it (and beside `dogfood:serve`).
- **Hot reloading:** the app is served by Vite's dev server, passing the API to the Kuria server behind it, so editing `items/web` shows at once without `web:build`.
- **Fast:** after the first start, starting again takes seconds rather than the minute or more `e2e:example` takes: Konklav is kept running and seeded once, the app isn't built, and only Kuria's database starts fresh.

Success: with `mise run e2e` running, `mise run example planning` serves the planning example on http://localhost:18290 to its own people (Devi Larsen, Rami Moberg, Hedda Lund), editing a Svelte file shows without restarting, and a second start is ready in under 15 seconds.

<!-- field: design -->
## Where it starts

- **`mise run e2e:example <example>`** (PLAN-018) runs `e2e/scripts/explore.ts`: `e2e:up` throws the end-to-end Konklav away and seeds a new one, `web:build` builds the app, and `e2e/scripts/serve.ts` serves it on http://localhost:18090 with the example bootstrapped into the seeded tenant and its people mapped by email to the seeded ones. It shares Konklav, ports and the `kuria_e2e` database with `mise run e2e`, so the two can't run together.
- **The dogfooding stack** (`dogfood/`) shows how a stack of its own looks: a compose project with its own Konklav ports, seeded once and kept, and a database of its own on the development Postgres, Valkey and S3 gateway.
- **The app in dev mode** (`mise run web:dev`): Vite on :5173, proxying the API paths to `KURIA_DEV_SERVER`. The app must share the public origin with the server (SameSite=Strict cookie; Konklav's issuer and login page are Kuria's origin).

## The example stack (`example/`)

- **Konklav:** the compose project `kuria-example`, ports 18280–18282, its data on a volume, kept between starts. Its issuer and login page are http://localhost:18290, the app's origin.
- **Seeded once** (`example/scripts/setup.ts`), like the dogfooding stack: Kuria's actions, one tenant, a role with every action, and the examples' people as subjects with the very subject ids and emails the bundles hold (every example has the same people), each with a password. So bootstrapping needs no subjects mapping: the bundle's people are the tenant's users.
- **Kuria:** the server on 127.0.0.1:18291, the database `kuria_example` dropped and made again on each start, then the example bootstrapped into the seeded tenant (`bundle.bootstrap.*`). Only one example is loaded; the script checks the tenant holds exactly the bundle's types.
- **The app:** Vite's dev server on http://localhost:18290 (`--strictPort`), `KURIA_DEV_SERVER` pointing at the server; Kuria's public URL is Vite's. The proxy also passes `/export`, `/import` and `/cli`, so the CLI works against it too.
- **Tasks:** `example <example>` (Ctrl-C stops Kuria and Vite), `example:down` (stops Konklav, keeping it), `example:reset` (forgets it all).

## Faster

- No `web:build` and no `kuria-cli` build on start.
- Konklav isn't recreated or reseeded: `docker compose up -d` on a running project returns at once.
- Vite and the server start together; the server waits for nothing it doesn't need.

## Out of e2e

`e2e:example`, and `explore.ts`'s and `serve.ts`'s example handling, go: `e2e:serve` stays as it was before PLAN-018 added them.

<!-- field: verification -->
- With `mise run e2e` running, `mise run example documents` starts, holds only the documents example's akt types and its akter, and the people sign in.
- Editing a Svelte component shows in the open page without a restart.
- A second `mise run example planning` is ready (the printed sign-in table) in under 15 seconds; times recorded in the plan.
- `mise run lint`, `mise run test`.

<!-- akt: PLAN-019-T01 -->
---
type: task
revision: 3
rank: b5L
state: done
in: [PLAN-019]
title: An example stack of its own
---

<!-- field: description -->
example/: compose, Konklav config, prepare and setup (the examples' people with the bundles' subject ids), serve (fresh kuria_example, bootstrap, the server on :18291, Vite on :18290); mise tasks example, example:down, example:reset; Vite proxy passes /export, /import, /cli.

<!-- field: acceptance -->
mise run example documents serves only the documents example on http://localhost:18290 while mise run e2e runs, and its people sign in.

<!-- akt: PLAN-019-T02 -->
---
type: task
revision: 3
rank: b5M
state: done
in: [PLAN-019]
title: Take the examples out of e2e
---

<!-- field: description -->
Remove e2e:example and the example handling in e2e/scripts/explore.ts and serve.ts; point docs/examples' READMEs, e2e/README.md and docs/nomenclature.md at mise run example.

<!-- field: acceptance -->
No e2e:example; every README names mise run example.

<!-- akt: PLAN-019-T03 -->
---
type: task
revision: 4
rank: b5N
state: done
in: [PLAN-019]
title: Measure and speed up starting
---

<!-- field: description -->
Time a first and a second start, find what the second waits on, and cut it.

Measured on an M-series Mac (2026-11-16):

- `mise run e2e:example`: about a minute or more: `e2e:up` throws Konklav away and seeds it again, `web:build` builds the app, and the CLI is built.
- `mise run example`, first start (seeding Konklav, starting its containers): 25 s, 11 s of it `npm ci` for `web:install`.
- Second starts: 5.4 to 6.4 s from the command to the printed sign-in table, also beside `mise run e2e`.

What the second start waited on, and what was done:

- **`docker compose up`** took about 2 s even with Konklav running: `scripts/up.ts` skips it when Konklav's `/readyz` answers (0.5 s).
- **`cargo run`** added about 3 s even with nothing to build, as did starting `target/debug/kuria-server` right after `cargo build`: on macOS a binary cargo has just checked starts seconds later. The server runs from a copy in `.state/`, cloned again only when the build changes (the first start after a change pays it once).
- **Signing the checking admin in to Konklav** took about 1.5 s: it now starts with the script, while the database, build and server start.
- **Migrations** on a fresh database take under 0.2 s, so a template database wasn't worth it. The server itself is ready about 1.8 s after it starts, most of it waiting for the relying client's caches to be trusted.
- Running cargo outside mise's environment makes it rebuild everything (95 s): the script builds under it, as every stack does.

<!-- field: acceptance -->
A second start is ready in under 15 seconds, with times recorded here.
