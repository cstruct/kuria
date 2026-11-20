---
number: PLAN-026
type: plan
revision: 23
rank: b6I
state: active
title: A CAPA flow in the release-management example
owner: albin@mattsson.io
---

<!-- field: goal -->
A quality manager can run a CAPA in the release-management example the way a regulated software company's CAPA procedure asks, from the problem that started it to its closure, and the releases, hotfixes and test protocols it touches are linked to it.

In practice they can:

- open a CAPA from a problem report, with its fields carried over and a link back;
- have QA/RA confirm a CAPA is needed, or record why it isn't;
- record the investigation;
- plan corrections, corrective actions and preventive actions, each with an owner, a due date and evidence;
- ship a correction as a hotfix that is deployed first and reviewed afterwards;
- trace a corrective action to the tasks and release candidates that implement it;
- check effectiveness against a test case;
- close the CAPA with one of the procedure's closure outcomes, and reopen it, keeping the closure in its history.

Kuria enforces the procedure's rules, so they don't depend on people remembering them:

- a CAPA can't close while one of its actions is open;
- a due date, once planned, can't move without a recorded reason;
- the root cause is required for a corrective CAPA, and the causal hypothesis and trigger for a preventive one;
- reopening asks for what the procedure asks.

It works when:

- the example's bundle holds a CAPA worked from start to finish;
- `mise run e2e` walks one through the app;
- each new engine rule is in `docs/spec/` with `r[impl]` and `r[verify]` references;
- nothing in Kuria's code names `capa`.

<!-- field: design -->
## Where this comes from

The model is OneTwo Analytics' QMS procedures, in `onetwo/documents/procedures/`:

- the CAPA Procedure (OTAID-466110905-415, v3.0);
- Issue Classification (-1332);
- Software Problem Resolution (-1337, and the older -401);
- Software Release (-1302).

They are one real instance of the CAPA row in `docs/requirements.md` ("several actions of different kinds under one CAPA; effectiveness check before closure; closure outcomes that reopen; due dates that may not be moved"). The example stays generic: it names no organization, and the names are its own.

### The CAPA procedure, in short

1. **Qualify and open.** The record holds a problem statement, evidence, its severity (Major or Critical), an impact and risk summary with a link to the risk record, and any containment taken. Its owner is whoever opens it, or the person they assign.
2. **QA/RA confirms** a CAPA is needed (severity, safety, security, regulatory impact, unacceptable risk, systemic or recurring). If it isn't needed, the reason is recorded and the issue goes back to the process it came from.
3. **Investigate:**
   - affected users, systems and versions;
   - for corrective actions, the root cause with evidence (5 Whys, Fishbone);
   - for preventive actions, a causal hypothesis and the evidence that triggered it;
   - whether vigilance reporting or customer communication is needed, with links to those records.
4. **Plan actions:**
   - each action is a correction, a corrective action, a preventive action or a verification;
   - each has an owner, a due date and the evidence it must leave;
   - the effectiveness method and acceptance criteria are set before closure.
5. **Implement:**
   - corrections ship through the correction/hotfix route: deployed now, reviewed, approved and documented after it stabilizes (Software Release §9.1);
   - product changes go through change management into the normal development and release flow;
   - evidence is linked (pull requests, test evidence, releases).
6. **Verify effectiveness**, and check that the actions caused no adverse impact.
7. **Close**, approved by QA/RA, with one of four outcomes:
   - completed;
   - not completed by its due date, and reopened;
   - ineffective, and reopened;
   - superseded by a linked CAPA.
8. **Reopen** when evidence is incomplete, effectiveness fails, the problem recurs, an adverse impact turns up, or the due date passed. A reopen records:
   - the reason;
   - the remaining actions;
   - a revised target;
   - the owner's confirmation;
   - the impact of the delay;
   - whether resources or escalation are needed.

   The original due date may not be pushed back over and over.

Where CAPAs come from: issue classification and problem resolution send Major or Critical issues with unacceptable risk to a CAPA. The older problem-resolution procedure (-401) links the fixed problem report to its CAPA.

### What Kuria lacks for it

These parts already work as configuration:

- containment (a CAPA contains its actions, created in it);
- choice fields;
- `user` fields that default to the creator;
- `date`, `reference` and `git` fields;
- fields given only in some states;
- a transition with an action of its own (`capa:approve_closure`);
- link kinds;
- a `CAPA-{year}-{seq:4}` number pattern.

These need engine work, each a generic rule in `docs/spec/akter.md` or `links.md`:

- **Transitions gated on other akter** (T01): a transition can require its akt's members, or the targets of one of its link kinds, to be in final states. A CAPA can't close while one of its actions is open, and an action can't finish while a task implementing it is open.
- **Requirements that depend on a value** (T02): a transition can require a field only when another field has certain values. Root cause is required when the CAPA's kind includes corrective; causal hypothesis and trigger when it includes preventive.
- **Locked fields** (T03): a field can lock in given states. It changes only through an edit that gives a reason, which the revision records, and the app and CLI say it is locked. An action's due date locks once it is planned.
- **Reopening** (T04): a transition can leave a final state if it says so (D01). Edits stay refused in final states.
- **Due dates** (T05): a `date` field can be a due date. Cards, lists and pages show it as overdue once it passes while the akt isn't final, and lists filter on it. Any akt type can use this, not only CAPAs.
- **Creating a linked akt** (T06): from an akt's page, or with `kuria new … --link`, create an akt of a type a link kind names, linked to it, with the fields both types share by key carried over. This is how a problem report escalates to a CAPA.

### What the example gains

All of this is configuration of the release-management example (`docs/examples/release-management/`):

- **`capa`** (T08), numbered `CAPA-{year}-{seq:4}`.
  - States: *Open*, *Investigating*, *Planning*, *Implementing*, *Verifying*, *Closed* (final, success), *Not required* (final).
  - Confirming it (*Open* → *Investigating*) and declining it (→ *Not required*, needing a rationale) need `capa:confirm`.
  - Closing needs `capa:approve_closure`, every action final (T01), effectiveness results, an adverse-impact check, and a closure outcome.
  - *Reopen* leaves *Closed* back to *Implementing* (T04) and requires the reopen fields.
  - Fields include: problem statement, severity, kind (correction, corrective, preventive; several choices), owner (the creator by default), impact, a risk record (`reference`), affected versions, root cause or causal hypothesis and trigger (T02), vigilance and communication decisions (each a choice with a reference), effectiveness method and criteria, results and adverse-impact check.
  - Links:
    - *Originated from* a task or problem report (G01);
    - *Superseded by* another CAPA;
    - *Verified by* test cases or test protocols.
- **`capa_action`** (T08), numbered `CAPA-2026-0001-A01` and created in its CAPA (D03).
  - States: *Planned*, *In progress*, *Done*, *Dropped* (needing a rationale).
  - Fields: kind, owner, due date (a due date, locked from *In progress*, T03 and T05), the evidence it must leave, and the evidence it left.
  - *Implemented by* links to tasks, hotfixes and release candidates gate *Done* (T01).
- **Hotfixes become corrections** (T09, D04).
  - *Deploy* needs the build, the reason, and an assessment that the change is low risk to implement, but no approval.
  - A new state after *Deployed*, *Reviewed* (final), needs `release:approve` and the retrospective review.
  - Until it is reviewed, the hotfix shows it is waiting for that review (its state's tone).
- **Problem reports** (T07, G01) carry severity, priority, the screening answer, and a resolution with "won't fix" reasons from a fixed list. From one, *New CAPA* creates a CAPA linked as originated from it (T06).
- **A QA/RA role** in the README's Konklav roles holds `capa:confirm` and `capa:approve_closure`, with create, edit and transition at `capa` and `capa_action`.
- **The bundle** (T10) tells one CAPA through the spring release:
  1. A failed acceptance run in the rejected candidate leads to a problem report.
  2. The problem report escalates to a CAPA.
  3. The CAPA's correction ships as spring's hotfix, later reviewed.
  4. Its corrective action is a task in the summer backlog.
  5. Its verification is a new test case in the acceptance protocol.
  6. A second CAPA is closed as ineffective and reopened, to show the history.

### Constraints

- **Kuria knows no akt type.** Every engine rule is generic and is specified before it is built. Its code and tests never name `capa`, and its verification uses its own small fixtures.
- **Konklav decides permissions.** `capa:confirm` and `capa:approve_closure` are catalog actions named by transitions, like `release:approve`.
- **This branch, `capa-flow`, starts from `ui-ux-walkthrough`** (PLAN-025, not yet merged), because the example and the app it builds on are there. Rebase onto `main` once PLAN-025 lands.
- **Out of scope:** risk records, vigilance cases and customer communications as akt types (they are `reference` fields for now, G03); signatures; computed fields such as hotfix eligibility derived from severity and priority (G04).

## Order

1. The engine rules, T01 to T06, in any order. T08 needs T01 to T05; T06 is first used by T07's problem reports.
2. The example's types: T07, T08 and T09.
3. Then the bundle, docs and the end-to-end walk: T10, T11 and T12.

<!-- field: verification -->
- `mise run lint` (including Tracey) and `mise run test` pass. Each new rule has server tests against fixtures that don't name `capa`:
  - `akt.transition.members` and `akt.transition.links` (T01);
  - conditional `requires` (T02);
  - locked fields (T03);
  - reopening (T04);
  - due dates (T05);
  - creating a linked akt (T06).
- `items/server/tests/examples.rs` defines the example's types, including `capa` and `capa_action`. A test in the shape of `release_flow.rs`, such as `capa_flow.rs`, checks these refusals:
  - closing a CAPA while an action is open;
  - investigating a corrective CAPA past *Investigating* without a root cause;
  - moving a planned action's due date without a reason;
  - closing without `capa:approve_closure`;
  - finishing an action while a task implementing it is open.

  It then works one CAPA to *Closed*, reopens it, and closes it again, with the history showing both closures.
- `KURIA_BOOTSTRAP_BUNDLE=docs/examples/release-management/bundle` imports the bundle's CAPAs. `mise run example release-management` shows them in the sidebar under a Quality group. An action past its due date reads as overdue.
- `mise run e2e` walks a CAPA in the app:
  1. a developer reports a problem;
  2. QA/RA escalates it with *New CAPA* and confirms it;
  3. the owner records the root cause and plans a correction and a corrective action;
  4. the release manager deploys the correction as a hotfix and reviews it later;
  5. the corrective action's task ships in a candidate;
  6. a tester passes the new test case;
  7. QA/RA closes the CAPA as completed.
- `docs/examples/release-management/README.md` describes the CAPA flow as the app shows it, and its Konklav roles include QA/RA.

<!-- akt: PLAN-026-T01 -->
---
type: task
revision: 6
rank: b6J
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Transitions that wait for members and linked akter
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec `akt.transition.members` and `akt.transition.links` in `docs/spec/akter.md` and `links.md`.

- A transition can require that every member of given akt types, or every target of a given link kind, is in a final state, and optionally in a success one.
- The server checks this within the transition's revision, as `akt.transition.outcomes` does.
- The refusal names the akter still open.
- `kuria types schema` and the app's transition hints say why the transition waits.

## Done

- **Spec:** `akt.transition.members`, `akt.transition.links` and `akt.transition.waits` in `docs/spec/akter.md`. `akt.type.validation`, `app.transitions` and `cli.read` say what they add.
  - A transition says `waits_for: {members: [<contained type>], links: [<link kind>]}`, and `requires_links: [<link kind>]` for a link it needs. D02's superseded CAPA needs the latter.
  - "Optionally in a success state" was left out: a CAPA waits for actions that are done or dropped, both final, and nothing yet needs more.
- **Config** (`items/config`): `WaitsFor` and `requires_links` on `TransitionDef`, validated against `contains` and `links`.
- **Engine** (`items/engine/src/waits.rs`): `waits::of` decides from members and links the caller finds. `transition` takes an `Around` (what members add up to, and what it waits for), into which an `Option<Outcome>` still turns, and refuses as `Refusal::Waits`.
- **Server** (`items/server/src/akter/waits.rs`): the members and link targets are share-locked, then read again, inside the transition's revision. `Akt.waits` and `Transition.waitsForMembers`, `waitsForLinks` and `requiresLinks` are served over GraphQL.
- **CLI:** local mode decides the same way. `types schema` says what each transition waits for, and `show` says what each allowed transition waits for now, also as JSON.
- **App:** a waiting transition's button is disabled, with what it waits for beside it: each akt by number and title, linked, then the links it lacks.

## Verified

- `items/config` test `checks_what_a_transition_waits_for`.
- `items/engine` test `a_transition_waits_for_members_and_links`.
- `items/server/tests/waits.rs`.
- `items/cli/tests/local.rs` test `a_case_closes_once_what_it_waits_for_is_final`.
- `items/web` test `waitingFor`.
- `mise run lint` and `mise run test`.

The app's hint is checked in a browser by T12's walk.

<!-- field: acceptance -->
Closing a fixture container with an open member is refused, naming the open member. It succeeds once that member is final.

The same works for link targets, and the app says why the transition is unavailable.

<!-- akt: PLAN-026-T02 -->
---
type: task
revision: 2
rank: b6K
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Fields required only when another field has certain values
---

<!-- field: description -->
Spec conditional requirements.

- A transition's `requires` entry can be `{field, when: {<field>: [values]}}`. The field is required only when the other field, a `choice` or `choices` field, holds one of the values (for `choices`, includes one).
- A type document whose condition names a field or option that doesn't exist is refused.
- `kuria types schema` and the app say when a field is required.

<!-- field: acceptance -->
A fixture type requiring `root_cause` when `kind` includes `corrective` refuses the transition without it. It allows it when `kind` is only `preventive`.

<!-- akt: PLAN-026-T03 -->
---
type: task
revision: 2
rank: b6L
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: "Fields locked in some states, changed only with a reason"
---

<!-- field: description -->
Spec locked fields.

- A field can name the states it is `locked` in. In those states, an edit or transition that changes it is refused unless it gives a reason, which the revision records as the reason for that change.
- A locked field with no value can be given once without a reason.
- The app shows the field as locked, and asks for a reason to change it.
- `kuria set` and `apply` already take `--reason`.

<!-- field: acceptance -->
Changing a locked fixture field without a reason is refused as `LOCKED`, naming the field. With a reason it succeeds, and the history shows the reason beside the change.

<!-- akt: PLAN-026-T04 -->
---
type: task
revision: 2
rank: b6M
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Reopening an akt from a final state
---

<!-- field: description -->
Spec reopening (D01). Replace "no transition starts from a final state" with "no transition starts from a final state unless it says `reopens`".

- Edits stay refused in final states.
- A container that is no longer final can take members again (`container.frozen`).
- Archive rules count a reopened akt as open.
- Its history reads "Reopened", with the transition's fields.

<!-- field: acceptance -->
A fixture akt is closed, reopened with the transition's required fields, and closed again. Its history holds both closures, and it can be edited only while it is open.

<!-- akt: PLAN-026-T05 -->
---
type: task
revision: 2
rank: b6N
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Due dates that read as overdue
---

<!-- field: description -->
Spec due dates. A `date` field can say `"due": true`.

- Once the date has passed while the akt isn't in a final state, cards, lists and its page show the field as overdue, in a danger tone.
- `kuria ls --overdue` and the list's filter find overdue akter.
- Nothing is stored: overdue is worked out when read, as outcomes are.

<!-- field: acceptance -->
A fixture akt with a past due date reads as overdue on its card, in the list and on its page, and `--overdue` lists it. Once it is final, it is no longer overdue.

<!-- akt: PLAN-026-T06 -->
---
type: task
revision: 2
rank: b6O
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T10]
title: Creating an akt linked to the one you are on
---

<!-- field: description -->
Spec creating a linked akt.

- `createAkt` takes links to add in the akt's first revision.
- For a link kind whose target types the viewer may create, the source's page offers *New …*.
- The new akt starts with the fields both types share by key and kind, which the form shows, to keep or change.
- In the CLI: `kuria new <type> --link <kind>:<number>`, with `--carry` to copy the shared fields.

<!-- field: acceptance -->
From a fixture akt, *New …* opens a form filled with the shared fields, and the akt it creates is linked back in its first revision. The CLI does the same.

<!-- akt: PLAN-026-T07 -->
---
type: task
revision: 4
rank: b6P
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T10, Resolves PLAN-026-G01]
title: Problem reports in the release-management example
---

<!-- field: description -->
Add a `problem_report` akt type to the example (G01). Problem reports live in the backlog, ranked beside tasks, with states open, classified, fixed, won't fix and residual anomaly. Their fields:

- severity: critical, major, minor or negligible;
- priority: P0, P1 or P2;
- the screening answer: yes, no or uncertain;
- the risk record, a `reference`;
- the resolution: fixed, or won't fix with a reason from a fixed list (cannot reproduce, by design, unsupported version, risk acceptable, workaround documented) and a rationale.

<!-- field: acceptance -->
A problem report can be created on a backlog's board and classified. It closes as won't fix only with a reason and a rationale.

<!-- akt: PLAN-026-T08 -->
---
type: task
revision: 7
rank: b6Q
state: todo
in: [PLAN-026]
links: [Blocked by PLAN-026-T01, Blocked by PLAN-026-T02, Blocked by PLAN-026-T03, Blocked by PLAN-026-T04, Blocked by PLAN-026-T05, Blocks PLAN-026-T10]
title: CAPA and CAPA action akt types
---

<!-- field: description -->
Add `capa` and `capa_action` to `docs/examples/release-management/`, as the plan's design describes, using T01 to T05.

- Add a *Quality* sidebar group and presentation: an icon and the states' tones.
- Add the link kinds *Originated from*, *Superseded by*, *Verified by* and *Implemented by*.
- Add `capa:confirm` and `capa:approve_closure` to the Konklav catalog of the example and of the end-to-end seed.

<!-- field: acceptance -->
`items/server/tests/examples.rs` defines the types. A new `capa_flow.rs` checks each refusal the plan's verification lists, then works a CAPA to *Closed*, reopens it, and closes it again.

<!-- akt: PLAN-026-T09 -->
---
type: task
revision: 2
rank: b6R
state: todo
in: [PLAN-026]
links: [Blocks PLAN-026-T10]
title: Hotfixes deployed first and reviewed after
---

<!-- field: description -->
Rework hotfixes into the correction route (D04).

- *Deploy* needs the build, the reason, and an assessment that the change is low risk to implement, but no approval.
- *Review*, to a new final state *Reviewed*, needs `release:approve` and the retrospective review.
- *Deployed* gets a warning tone.

Rules that count deployed hotfixes (release notes, archiving) count *Deployed* and *Reviewed* alike. Update `release_flow.rs` and the end-to-end release walk.

<!-- field: acceptance -->
A hotfix deploys without approval and shows that it waits for review. Reviewing it needs `release:approve`, and its tasks still reach the release notes.

<!-- akt: PLAN-026-T10 -->
---
type: task
revision: 7
rank: b6S
state: todo
in: [PLAN-026]
links: [Blocked by PLAN-026-T06, Blocked by PLAN-026-T07, Blocked by PLAN-026-T08, Blocked by PLAN-026-T09, Blocks PLAN-026-T11, Blocks PLAN-026-T12]
title: The CAPA story in the example bundle
---

<!-- field: description -->
Extend `docs/examples/release-management/bundle/` with the plan's CAPA story:

1. The spring release's failed acceptance run leads to a problem report.
2. The problem report escalates to a CAPA.
3. The CAPA's correction is the spring hotfix.
4. Its corrective action is a task in the summer backlog.
5. Its verification is a new test case.

A second CAPA was closed as ineffective and reopened. Regenerate the bundle as `docs/examples/README.md` says.

<!-- field: acceptance -->
`KURIA_BOOTSTRAP_BUNDLE` imports it. `mise run example release-management` shows both CAPAs with their links, and an action that is overdue.

<!-- akt: PLAN-026-T11 -->
---
type: task
revision: 2
rank: b6T
state: todo
in: [PLAN-026]
links: [Blocked by PLAN-026-T10]
title: Document the CAPA flow
---

<!-- field: description -->
- `docs/examples/release-management/README.md`:
  - the CAPA flow: how it fits, in the app and from the CLI;
  - the QA/RA Konklav role;
  - the akt-type table.
- `docs/nomenclature.md`: the new terms (correction, corrective action, preventive action, reopening, due date, locked field).
- `docs/requirements.md`: its "Not yet" line.

<!-- field: acceptance -->
The README describes the CAPA flow as the app shows it, and every new term is in `docs/nomenclature.md`.

<!-- akt: PLAN-026-T12 -->
---
type: task
revision: 2
rank: b6U
state: todo
in: [PLAN-026]
links: [Blocked by PLAN-026-T10]
title: An end-to-end walk of a CAPA
---

<!-- field: description -->
Add `e2e/tests/capa-flow.spec.ts`, walking what the plan's verification lists:

1. A developer reports a problem.
2. QA/RA escalates it and confirms the CAPA.
3. The owner investigates and plans its actions.
4. The release manager deploys the correction and reviews it.
5. The corrective task ships in a candidate.
6. A tester passes the new test case.
7. QA/RA closes the CAPA.

Add a QA/RA person to the end-to-end seed.

<!-- field: acceptance -->
`mise run e2e` passes, the new walk included, at desktop and phone widths.

<!-- akt: PLAN-026-G01 -->
---
type: gap
revision: 3
rank: b6V
state: closed
in: [PLAN-026]
links: [Resolved by PLAN-026-T07]
title: "Problem reports: their own type, or a kind of task?"
---

<!-- field: description -->
The procedures keep a problem report as a backlog ticket with Type "Bug", plus severity and priority. The example's task already has `kind` (feature, fix, chore).

- **A `bug` kind on `backlog_item`:** simple, but every task carries classification fields it rarely uses, and won't-fix states that features don't have.
- **A `problem_report` type in the backlog:** it can have its own states (open, classified, fixed, won't fix, residual anomaly) and a *New CAPA* link, and it ranks beside tasks on the backlog's board.

Leaning towards its own type. Decide before T07.

<!-- field: resolution -->
Decided: a `problem_report` akt type of its own, living in the backlog and ranked beside tasks on its board. It has its own states (open, classified, fixed, won't fix, residual anomaly), its classification fields, and *New CAPA* through an *Originated from* link (T06). `backlog_item` keeps `kind` as feature, fix or chore, without classification fields. T07 builds it.

<!-- akt: PLAN-026-G02 -->
---
type: gap
revision: 1
rank: b6W
state: open
in: [PLAN-026]
title: Separation of duties when closing a CAPA
---

<!-- field: description -->
The CAPA procedure has QA/RA approve closure, but doesn't say the approver must differ from the CAPA's owner. `docs/requirements.md` lists separation of duties as a workflow need.

Leave it out of this plan unless the procedure, or an auditor, needs it.

<!-- akt: PLAN-026-G03 -->
---
type: gap
revision: 1
rank: b6X
state: open
in: [PLAN-026]
title: "Risk records, vigilance cases and customer communications"
---

<!-- field: description -->
A CAPA links to a risk record and, where needed, to vigilance and customer communication records. None of them are akt types in any example yet, so this plan keeps them as `reference` fields.

Turning them into links is later work, for a risk-management example.

<!-- akt: PLAN-026-G04 -->
---
type: gap
revision: 1
rank: b6Y
state: open
in: [PLAN-026]
title: Enforcing hotfix eligibility
---

<!-- field: description -->
Problem resolution allows a hotfix only for Minor/Negligible severity, acceptable risk and P0 priority, with a documented assessment that the change is low risk to implement. Those values live on the problem report, not on the hotfix.

Checking them would need a transition condition on a linked akt's field values (beyond T01), or computed fields. For now, the hotfix records the assessment as a required field and nothing is enforced. Is that enough?

<!-- akt: PLAN-026-D01 -->
---
type: decision
revision: 3
rank: b6Z
state: decided
in: [PLAN-026]
title: Reopening is a transition that leaves a final state
decided_by: albin@mattsson.io
---

<!-- field: context -->
The CAPA procedure's closure outcomes include "Closed - not completed by due date; reopened" and "Closed - ineffective; reopened". A CAPA is also reopened when the problem recurs after closure. Kuria refuses any transition from a final state.

<!-- field: decision -->
A transition can say `reopens` and start from a final state. A CAPA closes to a final *Closed*, and *Reopen* takes it back to *Implementing*, requiring the reopen fields. Edits stay refused while it is closed.

<!-- field: why -->
`docs/requirements.md` asks that reopening be a normal transition that keeps the full history. A non-final "closed" would let a closed CAPA be edited, and would leave it counted as open for archiving and lists. Opening a new CAPA every time would split one problem's history. The procedure allows a new linked CAPA too, and *Superseded by* stays for that.

<!-- akt: PLAN-026-D02 -->
---
type: decision
revision: 3
rank: b6a
state: decided
in: [PLAN-026]
title: The closure outcome is a choice the close transition requires
decided_by: albin@mattsson.io
---

<!-- field: context -->
The procedure names four closure outcomes. Kuria's outcomes (`akt.outcome`) come from verdicts, not from a choice someone makes.

<!-- field: decision -->
`closure_outcome` is a `choice` field that *Close* requires: completed, not completed by due date, ineffective, or superseded. The two "reopened" outcomes are followed by *Reopen*. A superseded CAPA needs a *Superseded by* link (T01, as a link condition).

<!-- field: why -->
It keeps the procedure's words on the record and needs no engine change beyond T01 and T04.

<!-- akt: PLAN-026-D03 -->
---
type: decision
revision: 3
rank: b6b
state: decided
in: [PLAN-026]
title: CAPA actions are akter contained in their CAPA
decided_by: albin@mattsson.io
---

<!-- field: context -->
Each action has a kind, an owner, a due date, evidence and a state, and links to the tasks, hotfixes or candidates that implement it. A structured table (rows of fields) isn't in Kuria yet.

<!-- field: decision -->
`capa_action` is an akt type, created in its CAPA and numbered after it, like a test case in its protocol.

<!-- field: why -->
Actions need their own states, due dates and links, which rows of a table wouldn't have. Containment, created-in and numbering after a parent already exist, and *Close* waits for members (T01).

<!-- akt: PLAN-026-D04 -->
---
type: decision
revision: 3
rank: b6c
state: decided
in: [PLAN-026]
title: "Hotfixes record the deferred review as a state, not as an obligation"
decided_by: albin@mattsson.io
---

<!-- field: context -->
The correction route deploys first, and does the review, approval and documentation once it has stabilized. `docs/requirements.md` describes obligations that transitions create.

<!-- field: decision -->
A hotfix gets a *Reviewed* state after *Deployed*. *Deployed* is the visible "review still owed", in a warning tone.

<!-- field: why -->
States already show what is outstanding on boards and lists. A general obligations feature is bigger, and isn't needed for this flow.
