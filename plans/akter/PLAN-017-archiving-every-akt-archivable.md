---
number: PLAN-017
type: plan
revision: 19
rank: b4u
state: active
title: "Archiving: every akt archivable, partitioned, hidden by default, and archived by rules"
owner: albin@mattsson.io
---

<!-- field: goal -->
Every akt can be archived: kept, read-only, out of the way. Lists, boards, members and search show archived akter only when asked; an archived akt still opens by its number or a reference, saying it is archived. Akt types archive their akter by themselves, by rules they declare.

- **Archived is a flag every akt has,** beside its state, not a state: archiving and unarchiving are actions, kept in the akt's history with who and why.
- **Out of the way:** archived akter live in their own partition of `akt`, so what people work with stays small however much is archived.
- **Rules archive by themselves:** a release once at least two newer releases are released or closed; release candidates when their release is archived; anything 90 days after it reached a final state.

Success: the release-management example's walkthrough, in `mise run e2e`, releases three releases and sees the first archived by rule, hidden from the release list and search until **Show archived**, its candidates archived with it, read-only, and back to work when unarchived; the CLI does the same, and the live partition's queries never touch the archived one.

<!-- field: design -->
## Where it starts

- **`akt`** (`items/store/migrations/2026-11-12-000003_akter`) is one table, `unique (tenant_id, number)`, with about fifteen tables referencing `akt (id)`: revisions, containment, links, attachments held, origins, versions, live drafts, diagrams.
- **States** are an akt type's own; a final state freezes a container's members (`akt.containers`). Outcomes say whether a state is a success.
- **Reads:** the GraphQL `akter` query (type, state, conditions, outcome, search, paging) serves lists, boards' columns, the sidebar's counts and the CLI's `ls` and `find`; `members`, `board` and the change feed read akter too.
- **Nothing happens by itself** today: every change is a person's or a client's.

## Archived (PLAN-017-D01, D03)

- **A flag, not a state:** `archived` beside `state`, with `archived_at`, `archived_by` (a person, or a rule) and an optional reason. A transition's state and an akt's outcome are unchanged by archiving.
- **Actions:** `archiveAkt` and `unarchiveAkt`, each needing Konklav's `kuria:akter:archive` at the akt's type; each appends a revision (`archive` / `unarchive`, its reason, no field changes) and a change event, so history and the change feed show it.
- **Read-only:** an archived akt refuses edits, transitions, link and membership changes, releases and live-draft updates as `ARCHIVED`, until unarchived. Reading it, its versions, PDFs and references to it work as before; a container's member list still names it when asked.
- **Containers:** archiving a container archives nothing else by itself: members follow only by a rule (below).

## Partitioned on archived (PLAN-017-D02)

- **`akt` is list-partitioned on `archived`:** `akt_live` (`false`) and `akt_archived` (`true`). Archiving updates the flag; Postgres moves the row.
- **Keys:** the primary key becomes `(id, archived)`. Every foreign key to an akt becomes `(akt_id, akt_archived)` referencing `akt (id, archived)` `on update cascade`, so a move carries its references along; the referencing tables gain `akt_archived` (and `target_archived`, `container_archived`, … where they name two).
- **Numbers stay unique** in a table of their own, `akt_number (tenant_id, number) primary key, akt_id unique`, written with the akt; a partitioned table can't hold a unique key without the partition key.
- **The history tables too** (PLAN-017-G01): `akt_revision`, `akt_draft_update` and `change_event` are list-partitioned on their akt's `akt_archived` the same way, so an archive moves an akt's history with it.
- **Queries name the partition:** every list, board, member, count and search read says `archived = false` unless asked otherwise, so Postgres prunes to `akt_live`; reads by id or number look in both. Indexes are per partition.
- **Migration:** one migration builds the partitioned table, copies, swaps and rebuilds the keys, in a transaction; its test checks counts, keys and `EXPLAIN` pruning on a seeded database.

## Showing archived (PLAN-017-D03)

- **GraphQL:** `archived: ArchivedFilter = LIVE` (`LIVE`, `ARCHIVED`, `ALL`) on `akter`, `members`, `board` and the counts; `Akt.archived` with `at`, `by` and `reason`.
- **Boards** (PLAN-017-G03): a board shows only live members, unless its container is archived itself, when it shows them all; **Show archived** adds the rest to a live container's board, and WIP limits count live members only.
- **App:** lists, boards and search show live akter; **Show archived** (kept in the URL) adds archived ones, marked as such; a container's members say "3 archived" with a toggle; the sidebar counts live akter. An archived akt's page says it is archived, by whom and why, with **Unarchive**; **Archive** is in its menu.
- **CLI:** `kuria archive <number>… [--reason]`, `kuria unarchive <number>…`; `ls`, `find` and `members` take `--archived` (only archived) and `--all`. Bundles carry it.
- **Local mode** (PLAN-017-G02): archiving removes the akt's entry from its file, so git's history keeps it; `akter/archived.md` lists each archived number with the commit that last held it, so its number isn't reused, references to it read as archived, and `kuria unarchive` puts it back from that commit. Rules run in local mode as on a server, on the changes the CLI makes; `kuria check --fix` runs the sweep.

## Rules (PLAN-017-D04, D05)

An akt type document may declare `"archive": { "rules": [ … ] }`, each rule one of:

- **`{ "newer": 2, "in": ["released", "closed"] }`:** archive an akt in one of `in` once at least `newer` other akter of its type reached one of `in` after it did, by when each reached it; with `"within": "<container type>"`, only those sharing such a container count. Needs when an akt reached its state: `akt.state_since`, kept with every transition and filled from revisions by the migration.
- **`{ "with_container": "release" }`:** archive an akt when a container of that type holding it is archived; unarchiving the container unarchives the akter this rule archived with it.
- **`{ "days_in": 90, "in": ["closed"] }`:** archive an akt that has been in one of `in`, a final state, for that many days.

When they run:

- **On change,** in the same transaction: a transition checks `newer` for the others of its type it may have overtaken; an archive or unarchive checks `with_container` for its members, and so on down, each in its own revision by the rule.
- **A sweep** checks every rule for every live akt of a type that declares any, every hour (`KURIA_ARCHIVE_SWEEP_SECONDS`), one instance at a time by a Postgres advisory lock: `days_in`, and every rule after a type's rules change, so rules apply to akter already there.
- **Who:** an archive by rule is by Kuria, naming the rule, as `by: { rule: "newer" }`.
- **Unarchived by hand, kept:** a person unarchiving an akt a rule archived marks it kept: rules leave it alone until someone archives it by hand again (PLAN-017-D05).
- **Checked when defined:** states in `in` must be the type's, `days_in` ones final, `with_container` a type that may contain it, `newer` at least 1.

## Examples

- **Release management:** `release` archives with `{ "newer": 2, "in": ["released", "closed"] }`; `release_candidate`, `hotfix` and `release_notes` with `{ "with_container": "release" }`; `backlog_item` with `{ "days_in": 90, "in": ["done"] }`, `done` being final.
- **Planning:** tasks, gaps and decisions archive with their plan (`with_container: plan`); plans by hand.

<!-- field: verification -->
- **Storage:** a migration test on a seeded database: counts and keys survive; an archive moves the row and its references follow; numbers stay unique across partitions; `EXPLAIN` of the list, board, member and search queries prunes to `akt_live`.
- **Server tests:** archive and unarchive with and without `kuria:akter:archive`; every write refused as `ARCHIVED` on an archived akt; history and change events; `archived` filters on every read; each rule kind on change and by the sweep, `within`, cascades down containers, unarchive bringing back what a container rule took, kept akter left alone, and rules refused when defined wrong.
- **CLI tests:** `archive`, `unarchive`, `--archived`, `--all`, against a server and in local mode; bundles round-trip `archived`.
- **App:** unit tests for the filter and labels; e2e: the release-management walkthrough as the goal says, with axe on the archived akt's page and **Show archived** lists.
- `mise run lint`, `mise run test` and `mise run e2e` pass.

<!-- akt: PLAN-017-D01 -->
---
type: decision
revision: 2
rank: b4v
state: decided
in: [PLAN-017]
title: "Archived is a flag beside state, not a state"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Archiving should work for every akt type, whatever its states, and keep what an akt's state and outcome say.

<!-- field: decision -->
Every akt has `archived`, with when, by whom (a person or a rule) and why, changed only by `archiveAkt` and `unarchiveAkt` under `kuria:akter:archive`; each is a revision with no field changes, and a change event.

<!-- field: why -->
A state would need every type to declare it and would lose the state it was archived in; a flag is the same for all types and reads as one filter.

<!-- akt: PLAN-017-D02 -->
---
type: decision
revision: 2
rank: b4w
state: decided
in: [PLAN-017]
title: akt is partitioned on archived
decided_by: albin@mattsson.io
---

<!-- field: context -->
Archived akter pile up over years; what people work with should stay as fast as it is with none archived.

<!-- field: decision -->
`akt` is list-partitioned on `archived` (`akt_live`, `akt_archived`); its key is `(id, archived)`, every foreign key to it `(akt_id, akt_archived)` `on update cascade`, and numbers are unique in `akt_number`. Every list, board, member, count and search read names the partition.

<!-- field: why -->
Chosen over a flag with partial indexes: the live partition's tables and indexes stay small and separately maintained, at the cost of wider keys and a migration that rebuilds them.

<!-- akt: PLAN-017-D03 -->
---
type: decision
revision: 2
rank: b4x
state: decided
in: [PLAN-017]
title: Archived akter are read-only and hidden unless asked for
decided_by: albin@mattsson.io
---

<!-- field: context -->
Archived means done with: it shouldn't change by accident, nor crowd lists and search.

<!-- field: decision -->
An archived akt refuses every write as `ARCHIVED` until unarchived, and opens by number or reference saying it is archived. Lists, boards, members, counts and search show live akter unless asked (`archived: ARCHIVED | ALL`, **Show archived**, `--archived`, `--all`).

<!-- field: why -->
Read-only keeps what was archived as it was; showing it only on request keeps it out of the way without hiding that it exists.

<!-- akt: PLAN-017-D04 -->
---
type: decision
revision: 2
rank: b4y
state: decided
in: [PLAN-017]
title: "Akt types declare archive rules: newer, with_container, days_in"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Archiving by hand doesn't scale: old releases and their candidates, long-closed items.

<!-- field: decision -->
A type's `archive.rules` may hold `newer` (at least N others of its type reached the named states after it did, optionally `within` a container type), `with_container` (a container of that type holding it was archived; unarchived with it) and `days_in` (that many days in a named final state). They run on the changes that can make them true, in the same transaction, and in an hourly sweep that also applies new rules to akter already there. A rule's archive is by Kuria, naming the rule.

<!-- field: why -->
Three kinds cover the cases asked for; declaring them per type keeps Kuria generic, and the sweep makes them eventually true however rules change.

<!-- akt: PLAN-017-D05 -->
---
type: decision
revision: 2
rank: b4z
state: decided
in: [PLAN-017]
title: Unarchiving by hand keeps an akt from rules
decided_by: albin@mattsson.io
---

<!-- field: context -->
A rule would archive again at once an akt someone unarchived to work on.

<!-- field: decision -->
Unarchiving an akt a rule archived marks it kept: rules leave it alone until someone archives it by hand again. Unarchiving a container brings back only what `with_container` archived with it.

<!-- field: why -->
A person's choice wins over a rule's, and stays until a person changes it.

<!-- akt: PLAN-017-T01 -->
---
type: task
revision: 7
rank: b50
state: done
in: [PLAN-017]
links: [Blocks PLAN-017-T02]
title: Spec archiving
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec written: `docs/spec/archive.md` (`archive.flag`, `.act`, `.authz`, `.history`, `.readonly`, `.storage`, `.storage.live`, `.read`, `.board`, `.rules`, `.rules.state-since`, `.rules.on-change`, `.rules.container`, `.rules.kept`, `.sweep`), `app.archive`, `cli.archive`, `cli.local.archive`, and the nomenclature (`archived`, `archive rule`, `kept`).

<!-- field: acceptance -->
- Lint (tracey) passes with the new rules and nothing implementing them yet named.

<!-- akt: PLAN-017-T02 -->
---
type: task
revision: 8
rank: b51
state: done
in: [PLAN-017]
links: [Blocked by PLAN-017-T01, Blocks PLAN-017-T03]
title: Partition akt on archived
assignee: albin@mattsson.io
---

<!-- field: description -->
Migration `000025_archive_partitions`: `akt_number` (id, tenant, number) unpartitioned, kept by a trigger, referenced by every table that only names an akt; `akt`, `akt_revision`, `akt_draft_update` and `change_event` list-partitioned on archived (`*_live`, `*_archived`), the last three referencing `akt (id, archived)` `on update cascade`, versions and commits their revision; append-only triggers allowing an archived-only update and, while `kuria.archiving` is set, the delete half of a move; live ranks unique per partition; `akt.state_since` filled from revisions and kept by transitions; archive columns. Diesel's 32-column tables; `AktRow` defaults to live. Store tests (`tests/archive.rs`): the migration keeps everything, a move carries revisions, events, draft updates and versions and leaves links, a move without `kuria.archiving` and rewrites refused, numbers unique across partitions, live reads plan only live partitions. 459 Rust tests pass.

<!-- field: acceptance -->
- A migration test on a seeded database: counts and keys kept, a move carrying its references, numbers unique across partitions, `EXPLAIN` pruning to `akt_live` for lists, boards, members and search.

<!-- akt: PLAN-017-T03 -->
---
type: task
revision: 10
rank: b52
state: done
in: [PLAN-017]
links: [Blocked by PLAN-017-T02, Blocks PLAN-017-T04, Blocks PLAN-017-T06, Blocks PLAN-017-T07]
title: Archive and unarchive on the server
assignee: albin@mattsson.io
---

<!-- field: description -->
`archiveAkt` and `unarchiveAkt` (`akter/archive.rs`): `kuria:akter:archive` at the type, a reason of one line, a revision `archive`/`unarchive` with no field changes and its change event, then the move (unarchive moves first); `By::Rule` authored by the nil subject for T04; kept set when a person unarchives what a rule archived. `ARCHIVED` refusals after every write's lock: edits, transitions (and drafts and verdicts through them), links, memberships, creating in an archived container, moves, releases, live-draft updates (the socket turns read-only) and clone pushes. Reads: `ArchivedFilter` (`LIVE` default, `ARCHIVED`, `ALL`) on `akter` (search too), `members`, `board` (all when the container is archived), `akterCount`; `Akt.archived { at by rule reason }`, `Akt.archivedMemberCount`. Ranks are computed against all akter, so an unarchived akt's rank is never taken. The action is in the authorization spec, both setup scripts (release-work gets it) and the server README. Server tests (`tests/archive.rs`, 3). 462 Rust tests pass.

<!-- field: acceptance -->
- Server tests for each, every write refused, every read filtered.

<!-- akt: PLAN-017-T04 -->
---
type: task
revision: 8
rank: b53
state: done
in: [PLAN-017]
links: [Blocked by PLAN-017-T03, Blocks PLAN-017-T05]
title: Archive rules on change
assignee: albin@mattsson.io
---

<!-- field: description -->
`archive.rules` in akt type documents (`ArchiveDef`, `ArchiveRuleDef` with `rule()` giving `ArchiveRule::{Newer, WithContainer, DaysIn}`), checked when defined: exactly one kind, counts at least 1, `in` named and the type's states, `days_in` states final; `with_container` and `within` name types (containment is the container's to declare, so a rule naming one that never holds the akt archives nothing; spec says so). Server (`akter/rules.rs`): rules read from each type's latest version; `newer` (and `within`, by shared containers of that type) applied after every transition, counting archived akter too and archiving only live, unkept ones; `with_container` followed down from every archive and unarchive, `archived_by` naming the container so unarchiving it brings back only what it took; archives by rule authored by Kuria (nil), revision reason "by the rule `kind`". `akter_in_states` in the store. Config test and 4 server tests (newer, within, nested containers and back, kept). 467 Rust tests pass.

<!-- field: acceptance -->
- Config and server tests for each rule kind, cascades, unarchive bringing back what a container took, kept akter, and wrong rules refused.

<!-- akt: PLAN-017-T05 -->
---
type: task
revision: 8
rank: b54
state: done
in: [PLAN-017]
links: [Blocked by PLAN-017-T04, Blocks PLAN-017-T08]
title: The archive sweep
assignee: albin@mattsson.io
---

<!-- field: description -->
The sweep (`Akter::sweep`, `rules::sweep_tenant`): for each tenant whose akt types' latest versions declare rules, in a transaction holding `pg_try_advisory_xact_lock` on the tenant (another instance's tenant is passed over), `newer` and `days_in` (counted from `state_since`) on each type's live akter, and `with_container` on the live members of archived containers of the named type. Run every `KURIA_ARCHIVE_SWEEP_SECONDS` (3600 by default) from the server binary, publishing its change events. Store: `types_with_archive_rules`, `try_lock_sweep`. Server test: nothing without rules, a rule added later, `days_in` at 89 and 91 days, a container archived before its members' rule, a locked tenant passed over then swept. 468 Rust tests pass.

<!-- field: acceptance -->
- Server tests with a short period and two instances: `days_in` archives, a changed rule applies, only one instance sweeps.

<!-- akt: PLAN-017-T06 -->
---
type: task
revision: 8
rank: b55
state: done
in: [PLAN-017]
links: [Blocked by PLAN-017-T03, Blocks PLAN-017-T08]
title: Archived in the app
assignee: albin@mattsson.io
---

<!-- field: description -->
Built in a worktree (`e2a926e`), merged. `lib/akt/archive.ts`: `?archived=all` as the `ALL` filter, "N archived", the banner text ("by Kuria, by the rule …" for a rule or the nil author). Type lists and search: **Show archived** (in the URL; the search dialog starts from the page's), archived rows badged with an icon and the word. Akt page: **Archive…** in a "More actions" menu (not a header button, as `document_index` has a transition named Archive), with an optional reason; an archived akt shows a badge and banner with **Unarchive**, and no edit, verdict, transition, link, member, order, container or release controls, nor the live-draft connection (Make a copy stays). Members: "N archived" and **Show archived** (not offered for an archived container, whose board shows all); cards badged; no reordering in an archived container. History says archived/unarchived it, by Kuria for rules. `ARCHIVED` refusal message. Cache: archive/unarchive mark lists and counts stale. The app shows no WIP limits, so nothing there. Tests: unit (116 app tests) and `e2e/tests/archive.spec.ts` (passes): archive with a reason, banner and no controls, history, board's archived count and toggle, lists and search, a developer refused, unarchive, axe.

<!-- field: acceptance -->
- Unit tests and e2e with axe on each view.

<!-- akt: PLAN-017-T07 -->
---
type: task
revision: 3
rank: b56
state: todo
in: [PLAN-017]
links: [Blocked by PLAN-017-T03, Blocks PLAN-017-T08]
title: "Archived in the CLI, local mode and bundles"
---

<!-- field: description -->
`kuria archive`, `kuria unarchive`, `--archived` and `--all` on `ls`, `find` and `members`; local mode's `archived:` front matter and rules run by `kuria check --fix`; bundles and akt directories carry `archived`.

<!-- field: acceptance -->
- CLI tests against a server and in local mode, parity e2e, bundles round-trip.

<!-- akt: PLAN-017-T08 -->
---
type: task
revision: 4
rank: b57
state: todo
in: [PLAN-017]
links: [Blocked by PLAN-017-T05, Blocked by PLAN-017-T06, Blocked by PLAN-017-T07]
title: The examples and the walkthrough
---

<!-- field: description -->
Release management's rules (releases `newer: 2`, their members `with_container: release`, backlog items `days_in: 90` in `done`), the planning example's (`with_container: plan`), and the release-management walkthrough as the goal says; docs.

<!-- field: acceptance -->
- All of `mise run e2e` passes, with lint and the tests.

<!-- akt: PLAN-017-G01 -->
---
type: gap
revision: 2
rank: b58
state: closed
in: [PLAN-017]
title: Partitioning the history tables too
---

<!-- field: description -->
Revisions, live-draft updates and change events grow with every akt, archived or not. Should `akt_revision` and the others be partitioned on their akt's `archived` as well, or by time, and when is it worth it?

<!-- field: resolution -->
Partitioned too: akt_revision, akt_draft_update and change_event on their akt's archived, moved with it (design, Partitioned on archived).

<!-- akt: PLAN-017-G02 -->
---
type: gap
revision: 2
rank: b59
state: closed
in: [PLAN-017]
title: Archived in local mode
---

<!-- field: description -->
An akt directory has no partitions: is `archived: true` in the front matter enough, or should archived akter's files move under `archive/`, so plans/ stays readable? And do rules run in `kuria check --fix`, or only on a server?

<!-- field: resolution -->
Archiving in local mode removes the akt from its file, git's history keeping it; akter/archived.md lists each archived number with the commit that last held it, for references, numbering and kuria unarchive (design, Showing archived).

<!-- akt: PLAN-017-G03 -->
---
type: gap
revision: 2
rank: b5A
state: closed
in: [PLAN-017]
title: "An archived akt in a live container's board"
---

<!-- field: description -->
A container's board shows its members by state. Should archived members leave its columns entirely, show folded at the bottom, or count only, and does a WIP limit count them?

<!-- field: resolution -->
A board shows only live members unless its container is archived, or Show archived is on; WIP limits count live members (design, Showing archived).
