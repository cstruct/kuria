---
number: PLAN-018
type: plan
revision: 10
rank: b4u
state: active
title: "A server that starts with a bundle, and example data in every example"
owner: albin@mattsson.io
---

<!-- field: goal -->
Start a Kuria server with a bundle already imported, and every example in `docs/examples` ships one.

- **Bootstrapping:** `KURIA_BOOTSTRAP_BUNDLE=<bundle directory>` makes `kuria-server` import that bundle into a tenant before it serves, the same import `POST /import` and `kuria import` do. A tenant that already holds akt types, akter or contacts makes the server refuse to start, with an error saying why, rather than merging into it.
- **Example data:** release-management, documents, planning and test-protocols each hold `bundle/`, an exported bundle with their akt types and realistic akter with real histories, written by several people. One starts a server with an example loaded without walking through the app (`mise run e2e:walk` today).
- **Kept valid:** a test imports every example's bundle into a fresh database, and checks it holds the example's akt types as their files define them, so a format or type change that leaves a bundle behind fails `mise run test`. `mise run examples:bundles` writes them again.

Success: `mise run e2e:example -- planning` serves the planning example's akter on http://localhost:18090 to the seeded people; a second start against the same database refuses; `mise run test` imports all four bundles.

<!-- field: design -->
## Where it starts

- **Bundles** (`docs/spec/bundles.md`, `items/server/src/bundles/`): a directory of `bundle.json` and `attachments/<sha256>`, a tar archive on the wire, possibly with `subjects.json`. `bundles::import::import(pool, storage, tenant, importer, archive)` checks everything, then writes it in one transaction. A whole-tenant bundle already goes only into an empty tenant (`bundle.import.tenant`); a `types` bundle refuses clashes.
- **Configuration** is the environment, read by `items/server/src/config.rs` and listed in `items/server/README.md`. `KURIA_DEFAULT_TENANT` names the tenant a sign-in naming none is for.
- **The examples** are akt type documents only. `e2e/scripts/explore.ts` (`mise run e2e:serve`) defines the release-management, test-protocols and documents types in a fresh database; `mise run e2e:walk` walks a release through the app to leave data behind.

## Bootstrapping (PLAN-018-D01, D02)

- **Variables:** `KURIA_BOOTSTRAP_BUNDLE`, a bundle directory as `kuria export` writes it (or its tar archive, a file); `KURIA_BOOTSTRAP_TENANT`, the tenant it goes into, else `KURIA_DEFAULT_TENANT`, and start-up fails if neither is set; `KURIA_BOOTSTRAP_SUBJECTS`, a JSON file mapping the bundle's subjects to the tenant's, as `subjects.json` does (`bundle.import.request`). No command-line flag: the server is configured by its environment only.
- **When:** after migrations and before listening, so nothing is served before the import and a failed one never serves.
- **Fresh** means the tenant holds no akt type, akt or contact, the test `bundle.import.tenant` already makes, checked in the import's transaction for every scope. Per tenant, not per database: a database holds many tenants, and others don't stop it. An unfresh tenant makes start-up fail naming the tenant and the variable to unset, even if it holds this very bundle: the variable is for a first start, and a restart with it set must be noticed rather than skipped.
- **Recorded** as any import (`bundle.import.recorded`), imported by the nil subject: no one asked, the server did. Its change events wake subscribers as usual.
- **Subjects:** by `KURIA_BOOTSTRAP_SUBJECTS`, else by email to users who signed in to the tenant (none, in a fresh one), else unmapped and shown by the bundle's recorded names (`bundle.import.subjects`).

## Example bundles (PLAN-018-D03)

- **Where:** `docs/examples/<example>/bundle/`, a `types` bundle of all the example's akt types with their akter, so it loads into a fresh tenant by bootstrapping and into any tenant without them by `kuria import`.
- **Made through the API, not by hand:** `items/server/tests/examples.rs` builds each example's data through GraphQL against a fresh database, as the people of the end-to-end seed (dev@, rm@ and hedda@shop.example, and the build pipeline), each a subject of its own, with transitions, reasons, links, memberships, versions and attachments as the app would make them; then exports it. The test that does so is ignored; `mise run examples:bundles` runs it. Identifiers and times are those of the run, so regenerating rewrites the files whole.
- **Valid:** the same file's test imports each checked-in bundle by bootstrapping into a fresh database and checks the akt types it holds are the example's documents, latest version, as `docs/examples/<example>/*.json` holds them.
- **Serving one:** `mise run e2e:example -- <example>` is `e2e:serve` with that example bootstrapped into the seeded tenant, its subjects mapped by email to the seeded people, and the other examples' types defined as before.

<!-- field: verification -->
- `cargo test -p kuria-server --test bootstrap`: a fresh tenant imports and its akter read back; a tenant with an akt type refuses with the variable named; another tenant's data doesn't stop it; missing tenant and unreadable bundle fail.
- `cargo test -p kuria-server --test examples`: every example's bundle imports into a fresh database, holding the example's akt types.
- `mise run examples:bundles` rewrites the bundles, which then pass the test above.
- `mise run e2e:example -- release-management` serves the example's release to the seeded people; stopping and starting it again on the same database refuses (the script drops the database first, so this is checked by hand with `mise run serve`).
- `mise run lint`, `mise run test`, `mise run e2e`.

<!-- akt: PLAN-018-T01 -->
---
type: task
revision: 5
rank: b4v
state: done
in: [PLAN-018]
title: Spec bootstrapping and example bundles
assignee: albin@mattsson.io
---

<!-- field: description -->
`docs/spec/bundles.md` gained "Bootstrapping" (`bundle.bootstrap.variables`, `.tenant`, `.import`, `.fresh`, `.before-serving`, `.recorded`) and "The examples' bundles" (`bundle.examples`). `docs/nomenclature.md` defines `bootstrap bundle` and `example bundle`; `items/server/README.md` lists `KURIA_BOOTSTRAP_BUNDLE`, `KURIA_BOOTSTRAP_TENANT` and `KURIA_BOOTSTRAP_SUBJECTS`. Checked by reading against PLAN-018-D01 to D03; `tracey` sees the rules once the code references them (PLAN-018-T02, T03).

<!-- field: acceptance -->
`docs/spec/bundles.md` has rules for bootstrapping (`bundle.bootstrap.*`): the variables, which tenant, what fresh means, refusing to start, recording and subjects; and for the examples' bundles. `docs/nomenclature.md` defines `bootstrap bundle` and `example bundle`. `items/server/README.md` lists the variables.

<!-- akt: PLAN-018-T02 -->
---
type: task
revision: 5
rank: b4w
state: done
in: [PLAN-018]
title: Bootstrap a bundle when the server starts
assignee: albin@mattsson.io
---

<!-- field: description -->
- `config.rs` reads `KURIA_BOOTSTRAP_BUNDLE`, `KURIA_BOOTSTRAP_TENANT` (else `KURIA_DEFAULT_TENANT`) and `KURIA_BOOTSTRAP_SUBJECTS` into `Config::bootstrap`, failing on a bundle with no tenant, a tenant that isn't an id, or subjects without a bundle.
- `bundles::bootstrap` (`items/server/src/bundles/bootstrap.rs`) reads the bundle's directory, or its tar archive, and imports it through `import::import_unpacked` with `Tenants::Fresh`, which checks the tenant is empty in the import's transaction and refuses with `BundleError::NotFresh`, naming the tenant and `KURIA_BOOTSTRAP_BUNDLE`. The importer is the nil subject.
- `main.rs` bootstraps after migrations and storage, before binding the listener; a refusal ends start-up with `bootstrapping KURIA_BOOTSTRAP_BUNDLE=<path>: <why>`.
- Tests: `config.rs`'s `reads_the_bootstrap_bundle_and_its_tenant`; `tests/bootstrap.rs` (fresh imports, from a directory and a tar; unfresh by akt or contact, and a second start, refused and untouched; another tenant's data not counting; subjects mapped; missing and unknown-format bundles failing). `tests/support/world.rs` is a fresh-database `World` the examples' bundles reuse, and the fake host takes `as-<subject>` tokens. `cargo clippy`, `tracey query validate` clean.

<!-- field: acceptance -->
`kuria-server` with `KURIA_BOOTSTRAP_BUNDLE` imports the bundle into the bootstrap tenant before listening, and refuses to start, naming the tenant and the variable, if the tenant isn't fresh. `items/server/tests/bootstrap.rs` checks a fresh tenant imports, an unfresh one refuses, another tenant's data doesn't count, a missing tenant and an unreadable bundle fail; `config.rs` tests read the variables. The code carries `r[impl bundle.bootstrap.*]`, the tests `r[verify …]`.

<!-- akt: PLAN-018-T03 -->
---
type: task
revision: 5
rank: b4x
state: done
in: [PLAN-018]
title: "Example bundles, made through the API and checked to import"
assignee: albin@mattsson.io
---

<!-- field: description -->
- `items/server/tests/examples.rs`: `write_example_bundles` (ignored; `mise run examples:bundles`) defines each example's akt types in a fresh database and makes its akter through GraphQL as Devi Larsen (dev@), Rami Moberg (rm@), Hedda Lund (hedda@shop.example) and the build pipeline, fixed subjects `01900000-0000-7000-8000-00000000d00{1..4}`, then exports a `types` bundle with akter to `docs/examples/<example>/bundle/`:
  - release-management: spring released with a rejected and an accepted candidate, artifacts with an SBOM, approved notes, the go given for the head of product, a deployed hotfix, closed; summer being planned (28 akter).
  - documents: a quality manual in folders, documents referring to each other and to a diagram image, released, one twice and one retired; an archived handbook (9).
  - planning: an active plan with tasks in every state, `blocks` links, a closed gap resolved by a decision, a superseded decision; a done plan and a dropped one (15).
  - test-protocols: a template with sections, two runs copied from it with verdicts from two testers, one passed and one failed, and a draft (20).
- `every_example_s_bundle_imports_into_a_fresh_database` bootstraps each into a fresh database, checks its akt types are the example's files (compared as `AktTypeDocument`, which the server normalizes to) and that every akter reads back.
- Verified: `mise run examples:bundles` then `cargo test -p kuria-server --test examples`; clippy and `tracey query validate` clean.

<!-- field: acceptance -->
`docs/examples/{release-management,documents,planning,test-protocols}/bundle/` each hold a `types` bundle of the example's akt types with realistic akter and history, written through GraphQL by `items/server/tests/examples.rs` (ignored test, `mise run examples:bundles`). Its other test imports each into a fresh database by bootstrapping and checks the types match the example's files.

<!-- akt: PLAN-018-T04 -->
---
type: task
revision: 1
rank: b4y
state: todo
in: [PLAN-018]
title: "Load the examples: READMEs and mise run e2e:example"
---

<!-- field: acceptance -->
Each example's README says what its bundle holds and how to load it (`KURIA_BOOTSTRAP_BUNDLE`, `kuria import`). `mise run e2e:example -- <example>` serves an example loaded into the seeded tenant with subjects mapped to the seeded people. `docs/examples` and `e2e/README.md` mention it.

<!-- akt: PLAN-018-T05 -->
---
type: task
revision: 1
rank: b4z
state: todo
in: [PLAN-018]
title: Verify and open the pull request
---

<!-- field: acceptance -->
`mise run lint`, `mise run test` and `mise run e2e` pass; a PR against `main` is open.

<!-- akt: PLAN-018-D01 -->
---
type: decision
revision: 2
rank: b50
state: decided
in: [PLAN-018]
title: Bootstrapping is configured by environment variables
decided_by: albin@mattsson.io
---

<!-- field: context -->
The task asks for a setting or a command-line flag. `config.rs` reads everything from the environment, and the server has no flags beyond its `render` and `schema` subcommands.

<!-- field: decision -->
`KURIA_BOOTSTRAP_BUNDLE` (a bundle directory, or its tar archive), `KURIA_BOOTSTRAP_TENANT` (else `KURIA_DEFAULT_TENANT`, else start-up fails) and `KURIA_BOOTSTRAP_SUBJECTS` (a JSON file mapping subjects), environment only.

<!-- field: why -->
One way to configure the server, which containers and the e2e scripts already use. The default tenant is the one people land in, which is where an example should be; a variable of its own lets a deployment bootstrap another.

<!-- akt: PLAN-018-D02 -->
---
type: decision
revision: 2
rank: b51
state: decided
in: [PLAN-018]
title: "Fresh is per tenant, and an unfresh one refuses start-up"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A bootstrap must not merge into data already there. A database holds many tenants; a restart with the variable still set would find the tenant holding what it imported last time.

<!-- field: decision -->
Fresh is per tenant: no akt type, akt or contact, as `bundle.import.tenant` tests, checked in the import's transaction for any scope. An unfresh tenant fails start-up with an error naming the tenant and the variable to unset, even if it holds this bundle already.

<!-- field: why -->
Other tenants are other people's data and don't bear on this one. Refusing a restart, rather than skipping, is what the task asks, and noticing a variable left set is safer than guessing that what's there is what the bundle would write.

<!-- akt: PLAN-018-D03 -->
---
type: decision
revision: 2
rank: b52
state: decided
in: [PLAN-018]
title: Example bundles are made through GraphQL by an ignored server test
decided_by: albin@mattsson.io
---

<!-- field: context -->
Example data could be written by hand as akt directories (`kuria import --from-dir`, whose history is made up from git), exported after the browser walkthrough (needs Konklav and the whole stack, and covers only release-management), or made through the API in-process.

<!-- field: decision -->
`items/server/tests/examples.rs` makes each example's data through GraphQL against a fresh database, as the end-to-end seed's people, each a subject of its own, and exports a `types` bundle with akter to `docs/examples/<example>/bundle/`. Its ignored test writes them (`mise run examples:bundles`); its other test imports each by bootstrapping into a fresh database.

<!-- field: why -->
Real revisions, transitions, reasons and versions, as the server makes them, with nothing running but Postgres; the bundle the server imports, not a format it converts. A `types` bundle loads into any tenant without those types, by `kuria import` as well. Regenerating rewrites identifiers and times, which is fine for files written only when a format or an example changes.
