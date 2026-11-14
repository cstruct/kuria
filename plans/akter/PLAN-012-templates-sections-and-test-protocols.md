---
number: PLAN-012
type: plan
revision: 17
rank: b3K
state: active
title: "Templates, sections and test protocols"
owner: albin@mattsson.io
---

<!-- field: goal -->
Any akt can be a template: copied, with the parts of it one chooses, into new akter of the same akt types. An akt can lay out its members in nested sections, each with an optional title and shown collapsible or not in the app. A test is assigned to one or more users, each of whom passes, fails or skips it, and a test, a section and a protocol each have an outcome derived from those verdicts.

A test protocol example puts it together: a test template is nested sections of tests; a protocol is created from it, with a subset of its sections and tests; testers record their verdicts, and the protocol's outcome follows from its tests.

Success: in the app and through the CLI, on a server and in an akt directory, a protocol copied from the example's template with two of its three sections holds copies of only those sections' tests, in their order; a tester's verdicts make its tests and sections pass, fail, skip or stay pending by the rules below; and the protocol's outcome is the same rules over its tests.

<!-- field: design -->
## Where it starts

- Akt types are runtime documents (`akt.type.document`) with fields of fixed kinds (`akt.field.kinds`); a `user` field holds one user or contact, and a `checkbox` records who checked it, when, and for whom (`akt.field.checkbox`).
- Containment (`docs/spec/containers.md`): an akt has members, in rank order, and is created inside a container (`created_in`); a write is one revision of each akt it changes.
- The app shows an akt's members on its board, or listed by type (`app.members`); the CLI and the parity test cover every write on a server and in an akt directory.
- Requirements already ask for templates: "a protocol is a template of test cases; a record is one run of it" (`docs/requirements.md`).

## Constraints

- **Generic:** nothing in Kuria knows tests or protocols. Sections, templates, verdicts and outcomes are field kinds and operations any akt type can use; the example is ordinary akt type documents.
- **The same rules everywhere:** the server, the CLI in local mode, and the app, through `kuria-engine` (PLAN-008-D02), held by the parity test.
- **Revisions as now:** each change to an akt is one revision; a derived outcome is never stored, so it never makes one.

## Design

### Sections: a field laying out an akt's members (decided: PLAN-012-D01)

- A new field kind, `sections`: a tree whose every section has an optional title, whether it is `collapsible`, and its items, in order: member akter, by number, and sections nested inside it. A field may say which akt types it places (`of`).
- Its value only names the akt's own members, each at most once; members it doesn't place are listed after it.
- A member removed from the container leaves the sections that placed it.

### Copying (decided: PLAN-012-D02)

- `copyAkt(number, only)` copies an akt into a new akt of the same akt type, in its initial state, with the template's field values, and copies its members, recursively, into new members of the copy, likewise. `only` names the sections and members to keep; the rest, and what is only in them, is left out. The copy's `sections` place the copies.
- One batch, all or nothing (`akt.batch`). Each copy is linked to what it was copied from, and its first revision says so.
- A template is an akt kept for copying: nothing more. The example keeps its templates in a `template` state that copies don't start in.

### Users and verdicts (decided: PLAN-012-D03)

- A `user` field may say `multiple`: then it holds a list of distinct users and contacts.
- A new field kind, `verdict`, names the `user` field whose people are the test's assignees. Each assignee gives their own verdict, `pass`, `fail` or `skip`, recorded with who and when, as a checkbox is, and may change or clear it. Someone else records one for an assignee with `for`. A verdict from, or for, anyone who isn't an assignee is refused.
- Removing an assignee drops their verdict.

### Outcomes (decided: PLAN-012-D04, D05)

- A test's outcome is `fail` if any assignee failed it; else `pending` while any assignee hasn't answered, or it has no assignee; else `skip` if all skipped it; else `pass`.
- A section's and an akt's outcome are the same rules over the outcomes of the tests they hold (members with a `verdict` field), recursively through nested sections and members: `fail` if any fails; else `pending` if any is pending or it holds none; else `skip` if all skip; else `pass`.
- Derived on every read, never stored: the API and the CLI answer it, and filters and transition conditions on it can come later (PLAN-012-G02).

### In the app

- An akt with a `sections` field shows its members in those sections, nested, each with its title and collapsed or open if `collapsible`, each test with its assignees, their verdicts and its outcome, and each section with its outcome.
- An assignee gives their verdict from the test's row; the copy dialog chooses the sections and tests to keep.

### The example

- `docs/examples/test-protocols`: `protocol` (with `sections`; states `template`, `draft`, `running`, `done`) and `test` (steps, expected result, assignees, `verdict`), created in a protocol.

## Order of work

Spec and nomenclature, then the engine and config, then the server, the CLI, the app, and the example last, with its e2e walkthrough.

<!-- field: verification -->
- Engine unit tests: the outcome rules for every mix of verdicts, nested sections, and an empty protocol; verdict inputs refused for non-assignees; `sections` refusing non-members and duplicates.
- Server tests: `copyAkt` with a subset, all or nothing; verdicts and `for`; a member removed leaving its sections; outcomes on read.
- Local-mode tests and the parity test: the same copy, verdicts and outcomes in an akt directory.
- An e2e walkthrough of the example in the app: copy two of three sections, record verdicts as two testers, and see each outcome.

<!-- akt: PLAN-012-D01 -->
---
type: decision
revision: 2
rank: b3L
state: decided
in: [PLAN-012]
title: "Sections are a field laying out an akt's members; tests are akter"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A test template is nested sections with tests in them. Sections could be akter themselves, or structure inside the one akt that holds the tests.

<!-- field: decision -->
Sections live inside the akt, as a `sections` field: a tree of sections, each with an optional title and whether it is collapsible, placing the akt's member akter by number. Tests are akter of their own, members of the protocol.

<!-- field: why -->
Tests need what akter have: assignees, verdicts, history, permissions and links. Sections only arrange them: as akter they would multiply records, numbers and revisions for what is layout.

<!-- akt: PLAN-012-D02 -->
---
type: decision
revision: 2
rank: b3M
state: decided
in: [PLAN-012]
title: A template is copied into new akter of the same types
decided_by: albin@mattsson.io
---

<!-- field: context -->
A protocol is created from a template, possibly with only some of its sections and tests.

<!-- field: decision -->
Copying an akt makes a new akt of its type in its initial state, with its fields, and copies the chosen part of its member tree likewise, linked back to what each copy came from. Any akt can be copied; a template is one kept for copying.

<!-- field: why -->
Templates and protocols then share their akt types, so nothing maps fields between types, and the same mechanism copies any structure, not only tests.

<!-- akt: PLAN-012-D03 -->
---
type: decision
revision: 2
rank: b3N
state: decided
in: [PLAN-012]
title: "Only a test's assignees give it verdicts"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A test is assigned to one or more users, who pass, fail or skip it.

<!-- field: decision -->
A `verdict` field takes one verdict per assignee of the `user` field it names. Each is recorded with who and when; someone else may record one `for` an assignee, as with checkboxes. Anyone else's is refused.

<!-- field: why -->
The rules range over the people meant to test it, not whoever happened to answer, and `for` covers recording on someone's behalf with the record saying so.

<!-- akt: PLAN-012-D04 -->
---
type: decision
revision: 2
rank: b3O
state: decided
in: [PLAN-012]
title: A test is pending until every assignee has answered
decided_by: albin@mattsson.io
---

<!-- field: context -->
The rules say when a test fails, skips or passes, but not what it is while assignees are still testing.

<!-- field: decision -->
`fail` as soon as any assignee fails it; otherwise `pending` until every assignee has answered, or if it has none; then `skip` if all skipped, else `pass`. Sections and protocols apply the same rules to the tests they hold; one holding none is `pending`.

<!-- field: why -->
A test passed by one tester while another hasn't looked shouldn't read as passed, and an empty protocol hasn't passed anything.

<!-- akt: PLAN-012-D05 -->
---
type: decision
revision: 2
rank: b3P
state: decided
in: [PLAN-012]
title: "An outcome is derived on read, never stored"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A protocol's outcome changes whenever a verdict in any of its tests does.

<!-- field: decision -->
Outcomes are computed when read, from the verdicts and the sections, by `kuria-engine`, on a server and in a directory alike.

<!-- field: why -->
Storing one would make a revision of the protocol and of every section for each verdict, against `akt.revision.record`'s meaning of a change, and could drift from the verdicts.

<!-- akt: PLAN-012-G01 -->
---
type: gap
revision: 2
rank: b3Q
state: open
in: [PLAN-012]
links: [Resolved by PLAN-012-T01]
title: What a section holds besides tests
---

<!-- field: description -->
A section has an optional title and holds members and sections. A protocol's sections may want their own text, such as preconditions or a setup, or a section-level assignee. Decide in the spec whether a section has a Markdown body.

<!-- akt: PLAN-012-G02 -->
---
type: gap
revision: 1
rank: b3R
state: open
in: [PLAN-012]
title: Outcomes in filters and transitions
---

<!-- field: description -->
An outcome is derived on read (PLAN-012-D05), so it can't yet be filtered on (`akt.filter`) or required by a transition, such as a protocol that can only be finished when no test is pending, or a release that needs a passed protocol. Both need the outcome computed in queries and in transition checks.

<!-- akt: PLAN-012-G03 -->
---
type: gap
revision: 2
rank: b3S
state: open
in: [PLAN-012]
links: [Resolved by PLAN-012-T05]
title: Changing a protocol after it was copied
---

<!-- field: description -->
A protocol copied from a template may need a test added, moved between sections or dropped. Adding a member and editing `sections` covers it, but the app needs a way to do both at once, and whether a test can be moved after verdicts were given is open.

<!-- akt: PLAN-012-G04 -->
---
type: gap
revision: 2
rank: b3T
state: open
in: [PLAN-012]
links: [Resolved by PLAN-012-T01]
title: Copying links and containers outside the copied tree
---

<!-- field: description -->
A template's tests may link to requirements, or be in other containers too. Decide whether copies keep those links and memberships, or only those inside the copied tree.

<!-- akt: PLAN-012-T01 -->
---
type: task
revision: 4
rank: b3U
state: todo
in: [PLAN-012]
links: [Blocks PLAN-012-T02, Resolves PLAN-012-G01, Resolves PLAN-012-G04]
title: "Spec templates, sections, verdicts and outcomes"
---

<!-- field: description -->
`docs/spec/akter.md`: the field kinds `sections` and `verdict`, `multiple` on `user`, the outcome rules, and copying. `docs/spec/frontend.md`: sections, verdicts, outcomes and the copy dialog in the app. `docs/spec/cli.md`: `copy`, `verdict`, outcomes, and the file forms. Terms in `docs/nomenclature.md`: template, section, verdict, outcome, copy. Decide PLAN-012-G01 and G04.

<!-- field: acceptance -->
- Rules for everything in PLAN-012's design.
- `mise run lint` passes.

<!-- akt: PLAN-012-T02 -->
---
type: task
revision: 4
rank: b3V
state: todo
in: [PLAN-012]
links: [Blocked by PLAN-012-T01, Blocks PLAN-012-T03, Blocks PLAN-012-T04]
title: The field kinds and outcomes in config and engine
---

<!-- field: description -->
`kuria-config` validates `sections` (`of`), `verdict` (the `user` field it names) and `multiple`; `kuria-engine` takes their inputs, refusing a non-assignee's verdict, keeps `sections` to members, and computes outcomes.

<!-- field: acceptance -->
- Unit tests for every outcome rule, nested sections and an empty one.

<!-- akt: PLAN-012-T03 -->
---
type: task
revision: 4
rank: b3W
state: todo
in: [PLAN-012]
links: [Blocked by PLAN-012-T02, Blocks PLAN-012-T05, Blocks PLAN-012-T06]
title: "The server: values, outcomes and copyAkt"
---

<!-- field: description -->
The new values through the write path and GraphQL; `sections` kept to members when members leave; `outcome` on `Akt` and on each section; `copyAkt(number, only)` as one batch, linking copies back.

<!-- field: acceptance -->
- Server tests for copying a subset, verdicts, `for`, and outcomes.
- `items/web/schema.graphql` regenerated.

<!-- akt: PLAN-012-T04 -->
---
type: task
revision: 3
rank: b3X
state: todo
in: [PLAN-012]
links: [Blocked by PLAN-012-T02, Blocks PLAN-012-T06]
title: "The CLI: copy, verdicts and outcomes, on a server and in a directory"
---

<!-- field: description -->
The file forms of the new kinds; `kuria copy <number> [--only <section or number>…]`; `kuria verdict <test> pass|fail|skip [--for <user>]`; outcomes in `cat`, `show` and `ls`; all of it in local mode through the engine.

<!-- field: acceptance -->
- Local-mode tests, and the parity test extended.

<!-- akt: PLAN-012-T05 -->
---
type: task
revision: 4
rank: b3Y
state: todo
in: [PLAN-012]
links: [Blocked by PLAN-012-T03, Blocks PLAN-012-T06, Resolves PLAN-012-G03]
title: "The app: sections, verdicts, outcomes and copying"
---

<!-- field: description -->
An akt's members in its sections, nested and collapsible; each test's assignees, verdicts and outcome, and an assignee's verdict buttons; each section's outcome; the copy dialog with its section and test choices.

<!-- field: acceptance -->
- Component tests, and the accessibility e2e test over the new views.

<!-- akt: PLAN-012-T06 -->
---
type: task
revision: 4
rank: b3Z
state: todo
in: [PLAN-012]
links: [Blocked by PLAN-012-T03, Blocked by PLAN-012-T04, Blocked by PLAN-012-T05]
title: A test protocol example
---

<!-- field: description -->
`docs/examples/test-protocols` with `protocol` and `test`, its README, and an e2e walkthrough: copy a protocol from the template with two of its three sections, record verdicts as two testers, and see each outcome.

<!-- field: acceptance -->
- The example defines cleanly and its walkthrough passes in `mise run e2e`.
