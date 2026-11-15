---
number: PLAN-013
type: plan
revision: 12
rank: b3a
state: done
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
revision: 8
rank: b3d
state: done
in: [PLAN-013]
links: [Blocks PLAN-013-T02, Blocks PLAN-013-T03, Blocks PLAN-013-T04]
title: "Spec several-people filters, outcome filters and outcome-gated transitions"
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec says what PLAN-013 builds. No implementation yet.

**`docs/spec/akter.md`:**
- `akt.filter`: a `user` field naming several people matches by any one of them.
- `akt.filter.outcome` (new): a list of one akt type filtered by outcome if the type has a `verdict` field, and a container's members of one type if the type has outcomes. The outcome is as it is when the list is answered, and paging works as `akt.page` says. Anything else is refused as `BAD_REQUEST` (PLAN-013-D01).
- `akt.transition.outcomes` (new): a transition may name the outcomes it allows. It is refused unless the akt has outcomes and its outcome is one of them, saying which. No verdict or membership in the member tree may change between the check and the revision (PLAN-013-D02).
- `akt.type.validation`: the outcomes a transition allows are `pass`, `fail`, `skip` or `pending`, each once, and only on a type with a `verdict` field or one that contains an akt type. Containment, not "contains a type with outcomes", because types may be defined in any order, so whether a contained type has outcomes isn't known when the document is checked. An akt of such a type without outcomes is refused at the transition.

**`docs/spec/cli.md`:** `schema` gives the outcomes each transition allows; `ls` takes `--outcome`, and its table has the outcome where akter have one (as PLAN-012 built).

**`docs/spec/frontend.md`:**
- `app.transitions`: a transition the outcome doesn't allow is shown as unavailable, saying what it allows.
- `app.filters`: a type's list filters by outcome where the type has a `verdict` field.
- `app.my-work`: user fields naming several people count.

No new term was needed. `mise run lint` passes, Tracey included.

<!-- field: acceptance -->
- Spec rules with ids, and no implementation yet.

<!-- akt: PLAN-013-T02 -->
---
type: task
revision: 8
rank: b3e
state: done
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Filter by a field naming several people
assignee: albin@mattsson.io
---

<!-- field: description -->
A field naming several people is filtered by any one of them, on a server and in an akt directory, and counts in "My work" again.

- **Server** (`graphql/filter.rs`): `multiple` user fields are filterable. A condition naming someone matches by containment of `{"<field>": [<someone>]}`, for the named subject, subjects aliased to them by an import, and the contacts linked to them, as for a single user field.
- **CLI:** `--where testers=…` reads the value as one person (`@me`, an email or a name). Local mode's matcher (`reads/list.rs`) matches a list of people by any one.
- **App:** Home's "My work" asks for these fields again (`work.ts`). The list's filter bar already offered every user field, so filtering a type's list by testers works now.

**Tests:**
- `tests/contacts.rs`: `a_field_naming_several_people_matches_by_any_of_them` covers `me`, including through my linked contact, a contact, and another subject.
- `tests/local.rs`: testers by `@me` and by email.
- `work.test.ts`.
- The Home and test-protocols e2e tests pass.

Lint, `mise run test` (337 Rust, 61 app) pass. The server README is updated.

<!-- field: acceptance -->
- Server and local tests, including a linked contact; "My work" unit tests.

<!-- akt: PLAN-013-T03 -->
---
type: task
revision: 8
rank: b3f
state: done
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Transitions that require an outcome
assignee: albin@mattsson.io
---

<!-- field: description -->
A transition may name the outcomes it allows, and is refused unless the akt's outcome, as it is when the transition is performed, is one of them.

- **Config** (`document.rs`, `validate.rs`): `outcomes` on a transition. Validation refuses an unknown outcome, one named twice, and outcomes on a type with neither a `verdict` field nor `contains`.
- **Engine:** `transition` takes `held`, what the members add up to, and refuses with `Refusal::Outcome` ("`finish` requires the outcome to be pass, fail or skip, and it is pending"). A type with its own `verdict` field goes by its verdicts as the transition leaves them, so a verdict the transition gives counts.
- **Server:** for a gated transition, `outcomes::held_locked` share-locks the member tree in id order (`containers::share_akter`) and reads it again until it finds nothing new, then works out the members' outcome (PLAN-013-D02). `Transition.outcomes` in GraphQL.
- **Local mode:** the outcome from `answers::outcome`, under the directory lock.
- **CLI:** `kuria schema` says "only if the outcome is pass, fail, skip", and its JSON has `outcomes`.
- **App:** a transition the outcome doesn't allow is a disabled button, described by "Finish: only when the outcome is pass, fail or skip" (`outcomeRefusal` in `document.ts`).

**Tests:**
- Config: `checks_the_outcomes_a_transition_allows`.
- Engine: `a_transition_may_require_an_outcome` and `a_container_s_transition_goes_by_what_its_members_hold`.
- Server (`tests/templates.rs`): `a_protocol_finishes_only_with_an_outcome_it_allows`, and `a_finish_waits_for_a_verdict_in_its_tree`, where another transaction holds a test while taking its verdict back. The finish waits, then is refused as pending. Without the share lock this test fails.
- Local: `a_protocol_finishes_only_with_an_outcome_it_allows`, schema included.
- App: `document.test.ts`.

Lint, `mise run test` (63 app tests and every Rust suite) and the parity and test-protocols e2e tests pass. The example gets its gated finish, and an e2e check of the unavailable button, in T05.

<!-- field: acceptance -->
- Engine, config and server tests, including a verdict racing a gated transition; local tests.

<!-- akt: PLAN-013-T04 -->
---
type: task
revision: 8
rank: b3g
state: done
in: [PLAN-013]
links: [Blocked by PLAN-013-T01, Blocks PLAN-013-T05, Resolves PLAN-012-G02]
title: Filter by outcome
assignee: albin@mattsson.io
---

<!-- field: description -->
Lists filter by outcome, as PLAN-013-D01 decided.

- **Store** (`akter.rs`): `Match::Outcome` is a SQL condition over an akt's own verdicts and assignees. For each assignee it takes the verdict given for them, then applies `of_verdicts`' rules. It composes like any other condition, so tenant-wide lists, search and members page in SQL. `tests/outcomes.rs` holds it to `outcomes::of_verdicts` on the engine's cases, with a contact assignee, a missing assignee list and a verdict from someone no longer assigned.
- **Server** (`graphql/akter.rs`): `outcome:` on `akter` and `members`, part of the cursor's list. `by_outcome` picks the store condition for a type with a `verdict` field. A container's members of a type whose outcome comes from its members are filtered in memory, read 200 at a time in rank order until the page is full (`Members::held`). Refused as `BAD_REQUEST`: no `type`, a type without outcomes, and a tenant-wide list of a type whose outcome comes from its members.
- **CLI:** `ls --outcome pass|fail|skip|pending`, any case, against the server. Local mode filters with `answers::outcome` and refuses the same lists as the server (`local_outcome`).
- **App:** a type's list with a `verdict` field has an Outcome filter, kept in the address as `outcome=`, and an Outcome column (`filters.ts`: `filtersByOutcome`, `outcomeOf`).
- **Docs:** the server and CLI READMEs.

**Tests:**
- Store: `the_outcome_filter_agrees_with_the_engine`.
- Server (`tests/templates.rs`): `tests_and_protocols_filter_by_outcome_a_page_at_a_time` pages tenant-wide test lists, a protocol's tests and a suite's protocols one or two at a time. `other_lists_refuse_an_outcome`.
- Local: `tests_and_protocols_filter_by_outcome`, including the refusals.
- App: `filters.test.ts`.

Lint, `mise run test` (348 Rust tests, 64 app tests) and the e2e tests for parity, test protocols, shell, accessibility, members and the release flow pass. Parity of `--outcome` and the list filter in the browser come with the example in T05.

<!-- field: acceptance -->
- The SQL expression agrees with the engine on every case; paging stays correct; local and server answer alike.

<!-- akt: PLAN-013-T05 -->
---
type: task
revision: 8
rank: b3h
state: done
in: [PLAN-013]
links: [Blocked by PLAN-013-T02, Blocked by PLAN-013-T03, Blocked by PLAN-013-T04]
title: "The example: a gated finish and a tester's pending work"
assignee: albin@mattsson.io
---

<!-- field: description -->
The test-protocols example uses what PLAN-013 built: a gated finish, and a tester's pending work.

- **Example:** `test_protocol`'s `finish` allows `pass`, `fail` and `skip`. The README says a run can't be finished while a test case is pending, and how a tester finds their test cases: "My work", `--where testers=@me`, and the outcome filter in the app and with `--outcome`.
- **Walkthrough** (`e2e/tests/test-protocols.spec.ts`): the second tester finds the run's test cases in "My work". With the card case pending again, **Finish** is disabled, described by "Finish: only when the outcome is pass, fail or skip", and axe passes. The test-case list filtered to Pending holds the card case and not the others. Once the last verdict is given, Finish is enabled and taken.
- **Parity** (`e2e/tests/parity.spec.ts`, the protocols test): `schema`, a refused gated `finish`, `ls --in <p> --where testers=@me`, `ls --outcome fail`, `ls --in <p> --outcome pending`, a refused tenant-wide `ls <protocol> --outcome`, and the finish taken at the end.
- **A parity gap the finish found:** an akt file applied unchanged to a final akt answered "nothing changed" against a server, which skips unchanged akter, but was refused locally as final. `kuria_engine::changes_anything` lets local `apply` ask first. Tested in the engine, in `tests/local.rs`, and by the parity test.
- **Left as it is:** tenant-wide `ls` in local mode orders uncommitted akter by their file's modification time, so a container's members, sharing one file, tie. The parity test lists by tester within the protocol, where both sides use rank order.

Lint, `mise run test` (349 Rust, 64 app) and `mise run e2e` (29 tests) pass.

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
