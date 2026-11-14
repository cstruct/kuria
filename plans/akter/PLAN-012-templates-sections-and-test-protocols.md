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
revision: 4
rank: b3Q
state: closed
in: [PLAN-012]
links: [Resolved by PLAN-012-T01]
title: What a section holds besides tests
---

<!-- field: description -->
A section has an optional title and holds members and sections. A protocol's sections may want their own text, such as preconditions or a setup, or a section-level assignee. Decide in the spec whether a section has a Markdown body.

<!-- field: resolution -->
Settled in PLAN-012-T01: a section has an optional Markdown `text` beside its optional title (`akt.field.sections`), which the app renders as Markdown (`app.sections`), for preconditions or setup. Section-level assignees were left out: assignees belong to tests.

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
revision: 3
rank: b3S
state: closed
in: [PLAN-012]
links: [Resolved by PLAN-012-T05]
title: Changing a protocol after it was copied
---

<!-- field: description -->
A protocol copied from a template may need a test added, moved between sections or dropped. Adding a member and editing `sections` covers it, but the app needs a way to do both at once, and whether a test can be moved after verdicts were given is open.

<!-- field: resolution -->
A protocol copied from a template changes as any akt does: add a test as a member, then place it with the sections outline in the app (or edit the sections field in a file). A test may move between sections after verdicts were given, since the verdicts belong to the test, not the section (PLAN-012-T05).

<!-- akt: PLAN-012-G04 -->
---
type: gap
revision: 4
rank: b3T
state: closed
in: [PLAN-012]
links: [Resolved by PLAN-012-T01]
title: Copying links and containers outside the copied tree
---

<!-- field: description -->
A template's tests may link to requirements, or be in other containers too. Decide whether copies keep those links and memberships, or only those inside the copied tree.

<!-- field: resolution -->
Settled in PLAN-012-T01 (`akt.copy.links`, `akt.copy`). A link between two akter of the copied tree becomes a link between their copies. A link from the tree to an akt outside it is copied to the same akt with the same pin, so a copied test still verifies its requirement. Links into the tree from outside aren't copied. Memberships outside the tree aren't copied either: each member's copy is created in its copied parent only, and the copied akt goes where the copy says, else into the template's containers.

<!-- akt: PLAN-012-T01 -->
---
type: task
revision: 8
rank: b3U
state: done
in: [PLAN-012]
links: [Blocks PLAN-012-T02, Resolves PLAN-012-G01, Resolves PLAN-012-G04]
title: "Spec templates, sections, verdicts and outcomes"
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec, in four files:
- **`docs/spec/akter.md`:**
  - `akt.field.kinds` gains `sections` and `verdict`, and `user` lists for `multiple`.
  - New rules: `akt.field.multiple`; `akt.field.sections`, a list of member numbers and sections (optional title and Markdown text, collapsible, items), members only and once each, `of`, members leaving their sections when they leave; and `akt.field.verdict`, which names `assignees`, takes verdicts, `for` and `clear`, records who and when, refuses non-assignees, and drops a removed assignee's verdict.
  - Outcomes: `akt.outcome`, the rules for verdicts and over members, pending when empty; `akt.outcome.section`; and `akt.outcome.derived`, never stored.
  - Copying: `akt.copy`, the same type in its initial state, fields but not verdicts or checkboxes, readable members recursively, sections remapped, placed where given or in the template's containers; `akt.copy.only`; `akt.copy.links`; and `akt.copy.record`, the origin and all or nothing.
- **`docs/spec/frontend.md`:** `app.fields` gains several users and verdicts, plus `app.sections`, `app.verdicts` and `app.copy`.
- **`docs/spec/cli.md`:** `cli.file.kinds` gives the file forms, with outcome read-only; `cli.write` gains `verdict` and `copy`; and `show` gives outcomes and sections.
- **`docs/nomenclature.md`:** template, copy, section, assignee, verdict and outcome.

This settles PLAN-012-G01 (sections have optional text) and G04 (links out kept, memberships out not). Lint passes.

<!-- field: acceptance -->
- Rules for everything in PLAN-012's design.
- `mise run lint` passes.

<!-- akt: PLAN-012-T02 -->
---
type: task
revision: 8
rank: b3V
state: done
in: [PLAN-012]
links: [Blocked by PLAN-012-T01, Blocks PLAN-012-T03, Blocks PLAN-012-T04]
title: The field kinds and outcomes in config and engine
assignee: albin@mattsson.io
---

<!-- field: description -->
**`kuria-config`:** `FieldDef` gains `multiple` (user only), `of` (sections only, naming types the akt type contains) and `assignees` (verdict only, naming a multiple user field; a verdict field can't be required). `FieldKind` gains `sections` and `verdict`, and a type has at most one of each.

**`kuria-engine`:**
- `Value` gains `Users`, `Sections` and `Verdicts`, with their JSON forms. An empty multiple list is no value.
- `verdicts`: `Verdict`, `Given` (assignee, verdict, by, at), `Giving::from_input` for `pass`, `{verdict, for}` and `{clear, for}`, `apply`, and `only_of`.
- In `apply`, verdict inputs are taken last, against the assignees as the same input leaves them. Every verdict field then drops removed assignees' verdicts. `null` is refused.
- `sections`: `Item` and `Section`, `numbers`, `check` (members only, once, `of`), `without`, `copied` (for copies) and `outcome`. The stores call `check` and `without`, since the engine doesn't hold members.
- `outcomes`: `Outcome`, `of_verdicts`, `of_all`, `of_fields` and `has_outcomes`.

**Spec fix:** `akt.field.verdict` said a verdict *from* a non-assignee is refused, which contradicted D03's `for`. It now says a verdict must be *for* an assignee: anyone may give one for an assignee, and one's own only as an assignee.

**To keep everything compiling,** ahead of T03 and T05:
- GraphQL gains the kinds and `Field.multiple/of/assignees`, plus `UsersValue`, `SectionsValue` (with nested `SectionItem`/`Section`) and `VerdictValue`/`GivenVerdict`/`Verdict`. `schema.graphql` is regenerated.
- The app fetches them, shows them as text, and holds them read-only (`SHOWN`) until T05.
- A multiple user field isn't filterable yet.

**Tests:** config checks of the new options; outcome rules for every mix, sections, and `has_outcomes`; sections' JSON, checks, removal, copying and section outcomes; and verdicts through `edit` (own, `for`, outsiders, `null`, clearing, removed assignees, assignees and a verdict in one change). Lint, `mise run test` (329 Rust, 55 app) and all 27 e2e tests pass.

<!-- field: acceptance -->
- Unit tests for every outcome rule, nested sections and an empty one.

<!-- akt: PLAN-012-T03 -->
---
type: task
revision: 8
rank: b3W
state: done
in: [PLAN-012]
links: [Blocked by PLAN-012-T02, Blocks PLAN-012-T05, Blocks PLAN-012-T06]
title: "The server: values, outcomes and copyAkt"
assignee: albin@mattsson.io
---

<!-- field: description -->
**Sections on write** (`akter.rs`):
- `check_sections` checks a changed `sections` value against the akt's members, from the new `containers::members_with_types`, with the engine's `check`. It runs in `change_in` and `create_in`; a new akt has none, so its sections must be empty.
- A member removed or transferred out leaves its container's sections in that same revision: `leave_sections`, through `record_membership_changing`, which records the field change and hashes the new fields.

**Outcomes** (`akter/outcomes.rs`): `outcomes::of` derives an akt's outcome from its verdicts, else recursively from its members, deciding with the tenant's types whether a type has outcomes. GraphQL serves `Akt.outcome` and `Section.outcome` (a complex object over its items), as `Outcome`.

**Copying** (`akter/copy.rs`): `copyAkt(input: { number, only, in, reason })` runs in one transaction:
1. Walks the template's readable member tree, each akt once, and works out what `only` keeps: named akter, those between them and the root, and everything under them. Naming anything outside the tree is BAD_REQUEST.
2. Takes the order lock and the rows of the containers and outside link targets, as batches do.
3. Creates each copy through `create_in`, parents first, each in its parent's copy, with its fields minus verdicts, checkboxes and sections. The reason says what it was copied from.
4. Records each origin in the new table `akt_origin` (migration 13), served as `Akt.copiedFrom` to readers.
5. Sets each copy's sections once its members exist, through `sections::copied`.
6. Copies links: within the tree between the copies, and out to the same akt with its pin.

**Bundles** carry origins as `copiedFrom`, when the bundle holds the origin too. An import refuses one naming an akt the bundle lacks, and `bundle.contents` says so.

**Tests:**
- `tests/templates.rs` (3): sections against members, a leaving member, verdicts and outcomes up to sections and the protocol, and a copy of part of a template refused whole.
- `tests/bundles.rs` (1): origins kept through a whole-tenant move.

`schema.graphql` is regenerated and the server README updated. Lint, `mise run test` (333 Rust, 55 app) and all 27 e2e tests pass.

<!-- field: acceptance -->
- Server tests for copying a subset, verdicts, `for`, and outcomes.
- `items/web/schema.graphql` regenerated.

<!-- akt: PLAN-012-T04 -->
---
type: task
revision: 7
rank: b3X
state: done
in: [PLAN-012]
links: [Blocked by PLAN-012-T02, Blocks PLAN-012-T06]
title: "The CLI: copy, verdicts and outcomes, on a server and in a directory"
assignee: albin@mattsson.io
---

<!-- field: description -->
The CLI reads and writes the new kinds, gives verdicts and copies akter, on a server and in an akt directory, through the same engine rules.

**The akt file** (`cli.file.kinds`, `file.rs`, `values.rs`):
- A `multiple` user field is a list of people; `--set` takes them comma-separated.
- A sections field is a section of its own, its body YAML: member numbers and `{title, text, collapsible, items}`, keys in that order. `--set` takes it as `@<file>` too.
- A verdict field is a mapping of each assignee to `pass`, `fail` or `skip`, and `outcome` follows `state`. Both are read-only: `apply` refuses a change to either as `READ_ONLY`, and `--set verdict=…` too.

**Commands** (`writes/edit.rs`, `writes/copy.rs`):
- `kuria verdict <number> pass|fail|skip|clear [--for <user>]` edits the type's verdict field and answers the outcome after it.
- `kuria copy <number> [--only <number>…] [--in <container>…]` calls `copyAkt`, then walks the copy's members, parents first, to answer each copy with its original.
- `show` gives the outcome, each member's, and the members in their sections as a nested list with each section's outcome; `ls` adds an Outcome column when a row has one.

**The server** gains `SectionsValue.value`, the value as `editAkt` takes it. The CLI's first try asked for `items` five sections deep, past the GraphQL depth limit of 12, so every akt read was refused; and a fixed depth would have let `cat` then `apply` drop deeper sections. The CLI now reads `value` and works out section outcomes from its members' through `kuria_engine::sections::outcome`. The app should do the same in T05.

**Shared through the engine:** `kuria_engine::copying` now holds which akter a copy keeps (`kept`) and which values it leaves behind (`keeps`), used by the server's `copyAkt` and local mode's `copy`. `sections::from_json` and `to_json` are public, and so is `Verdict::read`.

**Local mode:**
- Sections are read from their YAML when the directory loads, and written back as YAML.
- Verdicts are kept by assignee, without who gave one for them or when, which git says. A file holds no `outcome`: `cli.local.layout` now says it is derived on read (`answers::outcome`, recursive, guarding a cycle).
- Every create, `set`, `do` and `apply` checks sections against the akt's members (`Writer::check_sections`, refused as the server does). `remove` and `transfer` take a leaving member out of its container's sections.
- `verdict` runs as `@me`. `copy` (`local/copy.rs`) walks the tree, creates each copy through `create_with` (split out of `create`), sets sections renamed to the copies, and copies links through `add_link`, keeping the original's pin.

**Tests:**
- `tests/local.rs` (3 new):
  - Sections refusing non-members and duplicates, then own and `--for` verdicts, refusals, and outcomes up through a section to the protocol, in `show`, `show --format json` and `ls`.
  - A protocol's files applied unchanged; a changed verdict or outcome refused; a test leaving its section.
  - A partial and a whole copy, a checked box read and left behind.
- `engine/copying.rs`: what `only` keeps.
- `e2e/tests/parity.spec.ts`: a second parity test runs a protocol sequence on the server and in a directory, `@me` set to whom the server names the client by: sections, verdicts, outcomes in `cat` and `ls`, copies, a removal, and files applied unchanged and with a verdict changed.

The guide, the CLI and server READMEs and `schema.graphql` are updated.

Lint, `mise run test` (337 Rust, 55 app) and all 28 e2e tests pass.

<!-- field: acceptance -->
- Local-mode tests, and the parity test extended.

<!-- akt: PLAN-012-T05 -->
---
type: task
revision: 8
rank: b3Y
state: done
in: [PLAN-012]
links: [Blocked by PLAN-012-T03, Blocks PLAN-012-T06, Resolves PLAN-012-G03]
title: "The app: sections, verdicts, outcomes and copying"
assignee: albin@mattsson.io
---

<!-- field: description -->
The app shows sections, verdicts and outcomes, edits several people and sections, and copies akter.

**Reading** (`operations.ts`): `SectionsValue.value` (the JSON form, at any depth), `outcome` on the akt and its board members, `copiedFrom`, the field flags `of` and `assignees`, and `kind`/`assignees` on members' types. `copyAkt` is a new mutation.

**Logic, tested without a browser:**
- `sections.ts`: a value read as a tree and given back as `editAkt` takes it; each section's outcome from its members' (`outcomes.ts`, the engine's rules); and the outline's moves (up, down, into the section above, out, remove, add, edit), copied through JSON, since `structuredClone` can't read Svelte's state proxies.
- `verdicts.ts`: a member's assignees, each one's verdict, and the viewer's own; the input that gives or takes back a verdict.
- `values.ts`: drafts for several people (`USERS`) and sections (`SECTIONS`); verdicts stay shown, given with their buttons.
- `history.ts`: sections and verdicts said in words, not JSON.

**Components:**
- `Sections`: members in nested sections, each with its Markdown text and outcome. A collapsible one is a `<details>`, and the page keeps which are open across reloads, since giving a verdict reloads the akt. Members no section names follow under "Not in a section", through `Members`.
- `VerdictButtons`: assignees and their verdicts. An assignee gets Pass, Fail and Skip (`aria-pressed`) and Take back, in a section, in a members list (with Outcome and Verdicts columns) and on the test's own page. Board cards show outcomes.
- `CopyDialog`: "Make a copy" in the header, sections and members as checkboxes, all checked at first (a section ticks what it holds), then the copy opens. A copy says "Copied from …".
- `SectionsEditor`: sections edited as an outline (title, Markdown text, collapsible), with "Place" for members not in them. `PeopleSelect`: several people as a checkbox list.
- The new-akt form leaves out verdicts and sections, which come once the akt exists.

**G03:** a protocol copied from a template changes as any akt does: add a test as a member, then place it in the outline. A test may move between sections after verdicts were given, since the verdicts are the test's.

**Tests:**
- 61 app unit tests (sections, outcomes, copy choices, verdicts, drafts, history).
- `e2e/tests/protocols.spec.ts`: a developer opens a protocol's collapsible section, gives, changes and takes back verdicts in sections and in a list, sees outcomes add up, edits the outline, opens a test, and copies part of the protocol. axe checks the sections, the outline and the copy dialog.
- The e2e seed grants the developer work at `test_protocol` and `test_case`, the keys the T06 example will use.

The app and e2e READMEs are updated.

Lint, `mise run test` (337 Rust, 61 app) and all 29 e2e tests pass.

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
