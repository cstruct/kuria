---
number: PLAN-013
type: plan
revision: 11
rank: b3a
state: active
title: Outcomes and testers in filters and transitions
owner: albin@mattsson.io
---

<!-- field: goal -->
A tester finds the test cases they run, and which are still pending, and a protocol or a release can only move on when its outcome allows. Three things become possible, on a server and in an akt directory alike:

- **Filtering by a field that names several people:** `kuria ls test_case --where testers=@me` lists a tester's test cases, and the app's "My work" on Home shows them.
- **Transitions that require an outcome:** an akt type document says which outcomes a transition allows. In the example, a protocol can't be finished while a test case is pending. A release that contains protocols can be made to ship only on a pass, since outcomes already add up through containment.
- **Filtering by outcome:** test cases by outcome across the tenant ("my test cases still pending"), and any akt type's members of one container by outcome ("the failed test cases of this run").

Success: the parity test runs these filters and an outcome-gated transition, refused and then taken, with the same answers on a server and in a directory. The test-protocols walkthrough finishes a run only once nothing is pending, and finds a tester's pending test cases through "My work" and the list's outcome filter.

<!-- field: design -->
## Where it starts

- PLAN-012 added `multiple` user fields, `verdict` fields and outcomes (`akt.outcome`). An outcome is derived each time it is read, from an akt's own verdicts if its type has a `verdict` field, else from its members' outcomes, recursively. It is never stored (PLAN-012-D05). `kuria-engine`'s `outcomes` holds the rules; the server's `akter/outcomes.rs` and local mode's `answers::outcome` walk the members.
- Filters (`akt.filter`) are field conditions turned into JSON containment (`@>`) over `akt.fields` in `kuria-store` (`Match::Contains`), built in `items/server/src/graphql/filter.rs`, and matched in Rust in local mode (`reads/list.rs`). A `multiple` user field is refused as `BAD_REQUEST`, so Home's "My work" (`items/web/src/lib/home/work.ts`) leaves such fields out.
- A transition's checks (`kuria_engine::transition`) see the akt's state and fields only: its `from` states and the fields it `requires`.

## Constraints

- **The same rules everywhere:** the server and local mode through `kuria-engine`; the app shows the server's answers. The parity test holds them together.
- **Outcomes stay derived** (PLAN-012-D05): nothing here stores one.
- **Pages stay correct:** a filter that can't be applied in the query that pages a list is refused, not applied to a page after the fact.

## Filtering by several people

A `multiple` user field stores a JSON list of subjects and `{"contact": …}`. A condition naming someone matches when the list holds them: containment of `{"<field>": [<someone>]}`, as `choices` already does. The contacts linked to a named user count too, as for a single `user` field. Local mode matches the same way. "My work" asks for such fields again.

## Transitions that require an outcome

A transition MAY name the outcomes it allows: `"outcomes": ["pass", "fail", "skip"]`, meaning not pending. Validation refuses it on a type without outcomes, and an unknown outcome. `kuria_engine::transition` takes the akt's current outcome alongside its state and fields, and refuses a transition whose `outcomes` don't include it, saying the outcome and what is allowed (`Refusal::Outcome`). The server works out the outcome inside the write's transaction, after locking the akt's member tree (PLAN-013-D02); local mode works it out under its directory lock. The app's transition dialog shows the refusal as any other.

## Filtering by outcome

`akter(type:, outcome:)`, `members(type:, outcome:)`, `kuria ls --outcome <outcome>`, and the app's list filter:

- **Types with their own `verdict` field:** the outcome is a SQL expression over the row's `fields` (the verdicts and the assignee list), so a tenant-wide list pages correctly. The expression follows `outcomes::of_verdicts`, and a test compares the two on every case of the engine's table.
- **One container's members, of any type with outcomes:** the members are loaded, their outcomes worked out as when read, and filtered before paging. A container's members are bounded, as boards already are.
- **Anything else** (tenant-wide lists of a type whose outcome comes from its members) is refused as `BAD_REQUEST` (PLAN-013-D01).

## The example

`test_protocol`'s `finish` allows `pass`, `fail` and `skip`. The walkthrough finds the tester's pending test cases in "My work" and with the list's outcome filter.

## Order

Spec and nomenclature, then the filter for several people, then outcome-gated transitions, then filtering by outcome, then the example and its walkthrough.

<!-- field: verification -->
- `kuria-engine`: the transition refusal; the SQL outcome expression against `outcomes::of_verdicts` on every case of its table (a store test against Postgres).
- `kuria-config`: `outcomes` refused on a type without outcomes, and an unknown outcome.
- Server (`tests/templates.rs` and filters): `testers` conditions including a linked contact; outcome filters on a type with verdicts, tenant-wide and paged, and on one container's members; a container type refused tenant-wide; `finish` refused while pending and taken once not.
- CLI: local tests, and the parity test extended with `--where testers=@me`, `--outcome`, and the gated `finish`.
- App: unit tests for "My work" and the list's outcome filter; the test-protocols walkthrough in `mise run e2e`.
- `mise run lint`, `mise run test` and `mise run e2e` pass.

<!-- akt: PLAN-013-D01 -->
---
type: decision
revision: 2
rank: b3b
state: decided
in: [PLAN-013]
title: "Filter by outcome in SQL for verdicts, in memory for one container"
---

<!-- field: context -->
An outcome is derived from verdicts and members each time it is read (PLAN-012-D05). A list pages in SQL. A test case's outcome follows from its own row, but a protocol's or a release's follows from its whole member tree.

<!-- field: decision -->
Filter by outcome in SQL for types with their own `verdict` field, everywhere. Filter in memory for one container's members, of any type with outcomes. Refuse a tenant-wide outcome filter on a type whose outcome comes from its members, as `BAD_REQUEST`.

<!-- field: why -->
This covers the questions people ask ("my test cases still pending", "the failed test cases of this run") without storing outcomes. A cached outcome table would have to be updated on every verdict, assignee, membership, type definition and import, with every ancestor, and could drift. If tenant-wide protocol lists by outcome are needed, that cache is the way, as its own decision (PLAN-013-G02).

<!-- akt: PLAN-013-D02 -->
---
type: decision
revision: 2
rank: b3c
state: decided
in: [PLAN-013]
title: A gated transition locks its member tree
---

<!-- field: context -->
A transition that requires an outcome reads the akt's member tree in its transaction. Without a lock, a verdict given on a member at the same moment could change the outcome after the check and before the commit.

<!-- field: decision -->
The server locks the rows of the akt's member tree (`FOR SHARE`, in id order) before working out the outcome for a transition that names `outcomes`. Verdicts on those members wait for it, or it waits for them. Local mode's directory lock already serializes every write.

<!-- field: why -->
A gated transition is rare and a tree small, so the cost is low, and it keeps "can't finish while pending" true under concurrency. Taking the locks in one order avoids deadlocks with copies and batches, which lock their rows in id order too.

<!-- akt: PLAN-013-T01 -->
---
type: task
revision: 4
rank: b3d
state: todo
in: [PLAN-013]
links: [Blocks PLAN-013-T02, Blocks PLAN-013-T03, Blocks PLAN-013-T04]
title: "Spec several-people filters, outcome filters and outcome-gated transitions"
---

<!-- field: description -->
`akt.filter`: a `multiple` user field matches by anyone it names; a list filtered by outcome (`akt.filter.outcome`), as PLAN-013-D01 says. `akt.transition` or a new `akt.transition.outcomes`: a transition naming the outcomes it allows, refused otherwise. `cli.read` (`ls --outcome`), `app.filters` (the outcome filter) and `app.my-work` (several-people fields). Nomenclature if a term is needed.

<!-- field: acceptance -->
- Spec rules with ids, and no implementation yet.

<!-- akt: PLAN-013-T02 -->
---
type: task
revision: 4
rank: b3e
state: todo
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Filter by a field naming several people
---

<!-- field: description -->
`filter.rs` allows `multiple` user fields, matching `{"<field>": [<someone>]}` and the contacts linked to a user. Local mode's `--where` matches the same. Home's "My work" asks for such fields again (`work.ts`), and the CLI's `--where` value for them is read as one person.

<!-- field: acceptance -->
- Server and local tests, including a linked contact; "My work" unit tests.

<!-- akt: PLAN-013-T03 -->
---
type: task
revision: 4
rank: b3f
state: todo
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Transitions that require an outcome
---

<!-- field: description -->
`kuria-config`: `outcomes` on a transition, validated. `kuria-engine`: `transition` takes the current outcome and refuses one not allowed (`Refusal::Outcome`). Server: the outcome worked out under PLAN-013-D02's locks for a gated transition. Local mode: through `answers::outcome`. The app shows the refusal; `kuria schema` and the app's transition list say what a transition allows.

<!-- field: acceptance -->
- Engine, config and server tests, including a verdict racing a gated transition; local tests.

<!-- akt: PLAN-013-T04 -->
---
type: task
revision: 4
rank: b3g
state: todo
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Filter by outcome
---

<!-- field: description -->
`outcome:` on `akter` and `members` in GraphQL, `kuria ls --outcome`, and the app's list filter. A SQL expression over `fields` for types with their own verdict field, held to `outcomes::of_verdicts` by a store test over the engine's cases. In-memory filtering of one container's members. A tenant-wide filter on a container type refused (PLAN-013-D01). Local mode filters the same.

<!-- field: acceptance -->
- The SQL expression agrees with the engine on every case; paging stays correct; local and server answer alike.

<!-- akt: PLAN-013-T05 -->
---
type: task
revision: 4
rank: b3h
state: todo
in: [PLAN-013]
links: [Blocked by PLAN-013-T02, Blocked by PLAN-013-T03, Blocked by PLAN-013-T04]
title: "The example: a gated finish and a tester's pending work"
---

<!-- field: description -->
`test_protocol`'s `finish` allows `pass`, `fail` and `skip`. Its README says so, and how a tester finds their pending test cases. The test-protocols walkthrough: `finish` refused while a test case is pending, then taken; the tester's test cases in "My work"; the list filtered by outcome. The parity test runs `--where testers=@me`, `--outcome` and the gated `finish` on both sides.

<!-- field: acceptance -->
- The walkthrough and the parity test pass in `mise run e2e`.

<!-- akt: PLAN-013-G01 -->
---
type: gap
revision: 1
rank: b3i
state: open
in: [PLAN-013]
title: "A tester's test cases waiting on their own verdict"
---

<!-- field: description -->
A tester's work is the test cases still waiting on their own verdict, which neither the testers filter nor the outcome says: a test case is pending until every tester answers. A condition on the verdict field, such as `verdict=me:pending`, would say it, as JSON over the verdicts and the assignee list. Is it needed once the testers and outcome filters exist?

<!-- akt: PLAN-013-G02 -->
---
type: gap
revision: 1
rank: b3j
state: open
in: [PLAN-013]
title: Tenant-wide outcome filters on container types
---

<!-- field: description -->
PLAN-013-D01 refuses a tenant-wide outcome filter on a type whose outcome comes from its members, such as "every failed protocol". If that is needed, it takes an outcome cache kept on every write that could change one, and its ancestors', which amends PLAN-012-D05.
