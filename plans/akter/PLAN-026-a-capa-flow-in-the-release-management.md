---
number: PLAN-026
type: plan
revision: 24
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
revision: 6
rank: b6K
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Fields required only when another field has certain values
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec conditional requirements.

- A transition's `requires` entry can be `{field, when: {<field>: [values]}}`. The field is required only when the other field, a `choice` or `choices` field, holds one of the values (for `choices`, includes one).
- A type document whose condition names a field or option that doesn't exist is refused.
- `kuria types schema` and the app say when a field is required.

## Done

- **Spec:** `akt.transition.requires-when` in `docs/spec/akter.md`, with its validation in `akt.type.validation`. `app.transitions` and `cli.read` say what they add.
  - The rules sit in a list of their own, `requires_when: [{field, when: {<field>: [options]}}]`, rather than as entries of `requires`. `requires` stays a list of keys, which the API, the CLI's model and the app read as such.
  - `when` can name several fields; the field is required while all of them hold one of their options.
- **Config:** `RequiredWhen` on `TransitionDef`, validated: fields that exist, conditions only on `choice` and `choices` fields, options of their own, none named twice.
- **Engine:** `required_when` gives the fields required now, and `transition` refuses without them as `Refusal::Requires`, as for `requires`. The server and local mode both get this through the engine.
- **GraphQL:** `Transition.requiresWhen { field when { field options } }`.
- **CLI:** `types schema` says, for example, ", requires root_cause when kind is corrective", and gives `requiresWhen` in JSON.
- **App:** the transition dialog also asks for a conditionally required field while the akt's choices meet the condition.
  - It goes by the akt's values as they are, not by values changed in the same dialog; `app.transitions` says so.
  - A board move that needs a conditionally required field is refused and shows the refusal, rather than opening the akt.

## Verified

- `items/config` test `checks_what_a_transition_requires_when`.
- `items/engine` test `a_field_is_required_only_while_others_hold_certain_values`.
- `items/server/tests/requires_when.rs`.
- `items/cli/tests/local.rs` test `a_corrective_case_needs_its_root_cause`.
- `items/web` test `requiredFields, with conditions`.
- `mise run lint` and `mise run test`.

<!-- field: acceptance -->
A fixture type requiring `root_cause` when `kind` includes `corrective` refuses the transition without it. It allows it when `kind` is only `preventive`.

<!-- akt: PLAN-026-T03 -->
---
type: task
revision: 6
rank: b6L
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: "Fields locked in some states, changed only with a reason"
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec locked fields.

- A field can name the states it is `locked` in. In those states, an edit or transition that changes it is refused unless it gives a reason, which the revision records as the reason for that change.
- A locked field with no value can be given once without a reason.
- The app shows the field as locked, and asks for a reason to change it.
- `kuria set` and `apply` already take `--reason`.

## Done

- **Spec:** `akt.field.locked` in `docs/spec/akter.md`, with its validation in `akt.type.validation`. `app.fields` says how the app shows a locked field.
  - A field is locked by the akt's state before the change. A transition into a locked state can still change the field; the next change can't without a reason.
  - Clearing a locked value counts as changing it. A blank reason is no reason.
- **Config:** `locked` on `FieldDef`, naming states the type defines.
- **Engine:** `check_locked`, refusing as `Refusal::Locked`. The server's one path for edits and transitions, and local mode's `set`, `apply`, `do` and edits, each call it with the reason they were given.
- **API:** the server answers the refusal as `LOCKED`, and so does local mode. `FieldDef.locked` is served over GraphQL.
- **App:** a locked field that holds a value shows a lock beside its name. Its editor asks for a reason, marked as needed, and won't save without one.
- The revision's reason is the change's reason, as for any change; nothing is recorded per field.

## Verified

- `items/config` test `checks_multiple_users_sections_and_verdicts`, `locked` case.
- `items/engine` test `a_locked_field_changes_only_with_a_reason`.
- `items/server/tests/locked.rs`.
- `items/cli/tests/local.rs` test `a_due_date_moves_only_with_a_reason_once_work_starts`.
- `mise run lint` and `mise run test`.

The app's lock and reason are checked in a browser by T12's walk.

<!-- field: acceptance -->
Changing a locked fixture field without a reason is refused as `LOCKED`, naming the field. With a reason it succeeds, and the history shows the reason beside the change.

<!-- akt: PLAN-026-T04 -->
---
type: task
revision: 6
rank: b6M
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Reopening an akt from a final state
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec reopening (D01). Replace "no transition starts from a final state" with "no transition starts from a final state unless it says `reopens`".

- Edits stay refused in final states.
- A container that is no longer final can take members again (`container.frozen`).
- Archive rules count a reopened akt as open.
- Its history reads "Reopened", with the transition's fields.

## Done

- **Spec:** `akt.transition.reopens` in `docs/spec/akter.md`; `akt.type.validation` changes to match.
  - A transition that `reopens` starts only from final states, and is the only kind that may.
  - Once reopened, the akt follows its new state's rules: it can be edited again, and as a container it can gain and lose members.
  - `app.header`'s confirmation of a move to a final state says the akt stays there unless it is reopened, where a transition can reopen it.
- **History:** it shows the reopening as the transition it is, by its name ("Reopen"), with its fields, rather than as a fixed "Reopened". Archive rules need nothing: `state_since` moves with every transition.
- **Config:** `reopens` on `TransitionDef`, validated both ways.
- **Engine:** `transition` skips the final-state refusal for a transition that reopens; edits stay refused while final.
- **GraphQL:** `Transition.reopens`.
- **CLI:** `types schema` marks such a transition ", reopens".
- **App:** in a final state, the page and board drops offer only transitions that reopen.

## Verified

- `items/config` tests `checks_what_transitions_name` and `a_transition_that_reopens_starts_from_final_states_only`.
- `items/engine` test `a_closed_akt_reopens_only_by_a_transition_that_reopens`.
- `items/server/tests/reopen.rs`: frozen while closed, takes a member and an edit once reopened, and the history holds both the close and the reopen.
- `items/cli/tests/local.rs` test `a_closed_case_reopens`.
- `items/web` tests `availableTransitions`, `transitionsInto` and `confirmationOf`.
- `mise run lint` and `mise run test`.

<!-- field: acceptance -->
A fixture akt is closed, reopened with the transition's required fields, and closed again. Its history holds both closures, and it can be edited only while it is open.

<!-- akt: PLAN-026-T05 -->
---
type: task
revision: 6
rank: b6N
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T08]
title: Due dates that read as overdue
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec due dates. A `date` field can say `"due": true`.

- Once the date has passed while the akt isn't in a final state, cards, lists and its page show the field as overdue, in a danger tone.
- `kuria ls --overdue` and the list's filter find overdue akter.
- Nothing is stored: overdue is worked out when read, as outcomes are.

## Done

- **Spec:** `akt.field.due` and `akt.filter.overdue` in `docs/spec/akter.md`; `akt.type.validation` and `cli.read` (`--overdue`) say what they add.
  - "Today" is today in UTC, and a date due today isn't overdue yet.
  - The filter is for a list of one type only, not for a container's members.
- **Config:** `due` on `FieldDef`, refused on anything but a `date` field.
- **Engine:** `overdue(ty, current, today)`.
- **Store:** `Match::DateBefore` and `Match::StateIn`.
- **Server:**
  - `Akt.overdue`, the due fields it is overdue on;
  - `akter(overdue: true)`, which filters by the type's latest version: its due fields before today, in a state that isn't final;
  - `FieldDef.due`.
- **CLI:** `ls <type> --overdue`, against a server and in local mode, refused without a type or due fields.
- **App:**
  - an overdue date shows as a danger badge, "…, overdue", on cards, in member lists, in the type's list and on the akt's page;
  - the type's list has an *Overdue only* filter where the type has a due field.

## Verified

- `items/config` field validation, `due` case.
- `items/engine` test `an_akt_is_overdue_once_its_due_date_passes_while_open`.
- `items/server/tests/due.rs`.
- `items/cli/tests/local.rs` test `lists_the_actions_overdue`.
- `items/web` test `shownFields`, overdue case.
- `mise run lint` and `mise run test`.

The badge and the filter are seen in a browser by T10 and T12.

<!-- field: acceptance -->
A fixture akt with a past due date reads as overdue on its card, in the list and on its page, and `--overdue` lists it. Once it is final, it is no longer overdue.

<!-- akt: PLAN-026-T06 -->
---
type: task
revision: 6
rank: b6O
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T10]
title: Creating an akt linked to the one you are on
assignee: albin@mattsson.io
---

<!-- field: description -->
Spec creating a linked akt.

- `createAkt` takes links to add in the akt's first revision.
- For a link kind whose target types the viewer may create, the source's page offers *New …*.
- The new akt starts with the fields both types share by key and kind, which the form shows, to keep or change.
- In the CLI: `kuria new <type> --link <kind>:<number>`, with `--carry` to copy the shared fields.

## Done

- **Spec:** `link.create` and `link.create.carry` in `docs/spec/links.md`; `cli.write`'s `new` names the options.
  - The links are made in the transaction that creates the akt, as `addLink` makes them, permissions included. The new akt's first revision is its creation and its second the link, rather than one revision for both: the link code stays the one path, and the akt still never exists unlinked.
  - Carrying is the client's: the server takes the values it is given.
- **Server:** `CreateAktInput.links: [NewLinkInput { kind, to | from }]`.
- **CLI:**
  - `kuria new <type> --link <kind>:<number>` makes a link from the new akt, and `--linked-from <kind>:<number>` one to it, each more than once.
  - `--carry` starts with the linked akter's values for each field both types have by key and kind; `--set` replaces them, and verdicts and sections aren't carried.
  - It works against a server and in local mode, which writes nothing if a link is refused.
- **App:**
  - an akt's links card offers *New <type>* for each type the viewer may create that links to this akt's type, or that it links to, named by the link it would be;
  - the create page takes `?link=` or `?linkedFrom=` with `carry=1`, starts with the carried values, says what it will be linked with, and sends the link with `createAkt`.

## Verified

- `items/server/tests/linked_create.rs`: both directions, and a refused link leaves no akt.
- `items/cli/tests/local.rs` test `a_case_is_created_from_its_report_with_its_values`: carried by key and kind, given values win, refused links.
- `items/web` test `linkedCreations`.
- `mise run lint` and `mise run test`.

The app's flow is walked in a browser by T12, escalating a problem report to a CAPA. The CLI's server path has no test of its own; T12's seed or a CLI test against a server can add one.

<!-- field: acceptance -->
From a fixture akt, *New …* opens a form filled with the shared fields, and the akt it creates is linked back in its first revision. The CLI does the same.

<!-- akt: PLAN-026-T07 -->
---
type: task
revision: 8
rank: b6P
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T10, Resolves PLAN-026-G01]
title: Problem reports in the release-management example
assignee: albin@mattsson.io
---

<!-- field: description -->
Add a `problem_report` akt type to the example (G01). Problem reports live in the backlog, ranked beside tasks, with states open, classified, fixed, won't fix and residual anomaly. Their fields:

- severity: critical, major, minor or negligible;
- priority: P0, P1 or P2;
- the screening answer: yes, no or uncertain;
- the risk record, a `reference`;
- the resolution: fixed, or won't fix with a reason from a fixed list (cannot reproduce, by design, unsupported version, risk acceptable, workaround documented) and a rationale.

## Done

`docs/examples/release-management/problem_report.json`, numbered `PR-0001`, in the sidebar's new *Quality* group, with a bug icon.

- **States:** *Open*, *Classified*, *Residual anomaly*, *Fixed* (final) and *Won't fix* (final).
- **Classify** needs the screening answer ("Could it lead to a hazardous situation?"), severity and priority. It also needs the risk record when the answer is yes or uncertain (T02).
- **Mark fixed:**
  - it is fixed by tasks, linked as *Fixed by* (inverse *Fixes*), rather than by a pull request of its own, so a fix ships in candidates and hotfixes as tasks do and reaches the release notes;
  - it needs at least one such link, and waits for every task it links to that way to be done (T01).
- **Won't fix** needs a reason from the fixed list and a rationale, both given only in that state.
- **Accept as residual anomaly** needs a rationale and the risk record. The report stays open across releases, and can be marked fixed later.
- **Elsewhere in the example:**
  - backlogs contain problem reports, exclusively: *To do* while open, *In progress* while classified or a residual anomaly, *Done* once fixed or won't fix;
  - a fixed or won't-fix report archives itself after 90 days;
  - the developer's grants in the e2e and example seeds take in `problem_report`;
  - the server's example tests and the e2e loader define it, and the bundle was made again with the type and no akter of it (T10 adds those).

The README comes with T11.

## Verified

- `items/server/tests/problem_reports.rs`: classifying needs the risk record when the screening is uncertain; marking fixed needs a *Fixed by* link and waits for its task; won't fix needs its reason.
- `items/server/tests/examples.rs` and `release_flow.rs`.
- `mise run lint` and `mise run test`.

<!-- field: acceptance -->
A problem report can be created on a backlog's board and classified. It closes as won't fix only with a reason and a rationale.

<!-- akt: PLAN-026-T08 -->
---
type: task
revision: 11
rank: b6Q
state: done
in: [PLAN-026]
links: [Blocked by PLAN-026-T01, Blocked by PLAN-026-T02, Blocked by PLAN-026-T03, Blocked by PLAN-026-T04, Blocked by PLAN-026-T05, Blocks PLAN-026-T10]
title: CAPA and CAPA action akt types
assignee: albin@mattsson.io
---

<!-- field: description -->
Add `capa` and `capa_action` to `docs/examples/release-management/`, as the plan's design describes, using T01 to T05.

- Add a *Quality* sidebar group and presentation: an icon and the states' tones.
- Add the link kinds *Originated from*, *Superseded by*, *Verified by* and *Implemented by*.
- Add `capa:confirm` and `capa:approve_closure` to the Konklav catalog of the example and of the end-to-end seed.

## Done

### `capa.json`

Numbered `CAPA-2026-0001`, in the *Quality* group, with a shield icon, in teal.

- **States:** *Open*, *Investigating*, *Planning*, *Implementing*, *Verifying*, *Closed*, *Superseded* and *Not required*; the last three are final.
- **Confirm CAPA** (`capa:confirm`) needs severity, impact and the risk record. **No CAPA needed** (`capa:confirm`) needs the reason why.
- **Plan actions** needs the kinds of action, what is affected, and the vigilance and communication decisions. With `requires_when` (T02), it also needs:
  - the root cause, for a corrective CAPA;
  - the causal hypothesis and trigger, for a preventive one;
  - each record, where reporting or communication is required.
- **Start implementing** needs the effectiveness check and its acceptance criteria, both locked from then on (T03).
- **Verify effectiveness** waits for every action (T01). **Back to implementing** returns from verifying.
- **Close** (`capa:approve_closure`) waits for every action too, and needs the results, the adverse-impact check and the closure outcome (D02).
- **Supersede** (`capa:approve_closure`) needs a *Superseded by* link (T01's `requires_links`), and leads to its own final state rather than to a closure outcome.
- **Reopen** (`capa:confirm`, T04) needs:
  - the reason;
  - the remaining actions;
  - a revised target, a due date (T05);
  - the owner's confirmation;
  - the impact of the delay.
- **Links:** *Originated from* a problem report (inverse *Led to*), *Superseded by* a CAPA, and *Verified by* test cases and protocols.
- **Board:** it contains its actions, exclusively: *Planned*, *In progress* and *Done*.

### `capa_action.json`

Numbered `CAPA-2026-0001-A01`, created in its CAPA, under *Show more*.

- **Kinds:** correction, corrective, preventive or verification.
- **Fields:** an owner, the creator by default; a due date, due (T05) and locked once started (T03); the evidence it must leave, and the evidence it left.
- **States:** *Planned*, *In progress*, *Done* and *Dropped*, which needs a reason.
- **Finish** needs the evidence, and waits for what it is *Implemented by*: tasks, hotfixes or release candidates (T01).
- It archives with its CAPA.

### Konklav

- The e2e seed's catalog has `capa:confirm` and `capa:approve_closure`; the example stack's seed takes them from the documents, as it does `release:approve`.
- Both seeds have a `quality-work` role: create, edit, transition and archive, and both CAPA actions.
- The QA/RA person who holds it comes with T10 and T12.

### Found while building it

- `default: "creator"` is the app's offer only, so an action created through the API names its owner.
- A second reopening still needs the reopen fields, but they still hold the first reopening's values, so nothing makes them be given again. A reopen that clears them first would need a transition to clear fields, which Kuria lacks.

## Verified

- `items/server/tests/capa_flow.rs`, against the example's documents:
  - the developer can't confirm or close;
  - planning a corrective CAPA needs its root cause;
  - an overdue action reads as overdue, and its due date is locked once started;
  - verifying waits for the action, and the action for its task;
  - QA/RA closes, reopens with the procedure's fields, and closes again, with both closures in the history;
  - superseding needs a *Superseded by* link, and a CAPA can be declined with its reason.
- `items/server/tests/examples.rs`; the bundle was made again with both types, still without CAPAs (T10).
- `mise run lint` and `mise run test`.

<!-- field: acceptance -->
`items/server/tests/examples.rs` defines the types. A new `capa_flow.rs` checks each refusal the plan's verification lists, then works a CAPA to *Closed*, reopens it, and closes it again.

<!-- akt: PLAN-026-T09 -->
---
type: task
revision: 6
rank: b6R
state: done
in: [PLAN-026]
links: [Blocks PLAN-026-T10]
title: Hotfixes deployed first and reviewed after
assignee: albin@mattsson.io
---

<!-- field: description -->
Rework hotfixes into the correction route (D04).

- *Deploy* needs the build, the reason, and an assessment that the change is low risk to implement, but no approval.
- *Review*, to a new final state *Reviewed*, needs `release:approve` and the retrospective review.
- *Deployed* gets a warning tone.

Rules that count deployed hotfixes (release notes, archiving) count *Deployed* and *Reviewed* alike. Update `release_flow.rs` and the end-to-end release walk.

## Done

- **`hotfix.json`:**
  - *Deploy* needs the build and "Why it is low risk" (how isolated the change is, what it could break, how it is rolled back), and no approval.
  - *Review* (`release:approve`) leads to a new final state, *Reviewed*, and needs the retrospective review, given only there.
  - *Deployed* isn't final any more and reads as a warning, so a hotfix waiting for its review shows it; *Open* reads as active. Each state's next step is primary.
- **`release.json`:**
  - its board's *Accepted* column holds deployed and reviewed hotfixes;
  - *End maintenance* waits for every hotfix to be reviewed (T01), so no release closes with a review still owed.
- **`release_notes.json`:** the draft counts deployed and reviewed hotfixes alike.
- Archiving needed nothing: hotfixes archive with their release.
- **Tests:**
  - `release_flow.rs` deploys with the assessment; refuses to close the release before the review; refuses the review to the developer; then reviews and closes;
  - the example story in `examples.rs` reviews spring's hotfix, and the bundle was made again;
  - `e2e/tests/release-flow.spec.ts` deploys and reviews in the app.

## Verified

- `items/server/tests/release_flow.rs` and `examples.rs`.
- `mise run lint` and `mise run test`.
- The e2e walk was changed but not run: the e2e Konklav running here is the main checkout's (another session's). It runs with T12.

<!-- field: acceptance -->
A hotfix deploys without approval and shows that it waits for review. Reviewing it needs `release:approve`, and its tasks still reach the release notes.

<!-- akt: PLAN-026-T10 -->
---
type: task
revision: 11
rank: b6S
state: done
in: [PLAN-026]
links: [Blocked by PLAN-026-T06, Blocked by PLAN-026-T07, Blocked by PLAN-026-T08, Blocked by PLAN-026-T09, Blocks PLAN-026-T11, Blocks PLAN-026-T12]
title: The CAPA story in the example bundle
assignee: albin@mattsson.io
---

<!-- field: description -->
Extend `docs/examples/release-management/bundle/` with the plan's CAPA story:

1. The spring release's failed acceptance run leads to a problem report.
2. The problem report escalates to a CAPA.
3. The CAPA's correction is the spring hotfix.
4. Its corrective action is a task in the summer backlog.
5. Its verification is a new test case.

A second CAPA was closed as ineffective and reopened. Regenerate the bundle as `docs/examples/README.md` says.

## Done

`items/server/tests/examples.rs` tells the story in `capas`, after summer is planned. The examples have a fifth person, Ines Berg (`qa@shop.example`), QA/RA.

### Lost sessions

1. Devi reports *PR-0001*, "customers are signed out when the backend restarts", in summer's backlog: spring's is closed by then. It is classified major, P0, screening no. It is fixed by spring's existing "Sessions lost on restart" task, and marked fixed.
2. Ines creates *CAPA-2026-0001* from it with `createAkt`'s `links` (T06), linked *Originated from*. She confirms it and plans a correction and a corrective action, with:
   - the root cause;
   - customer communication required, with its notice;
   - the effectiveness check, which is locked once implementing starts.
3. Its actions:
   - *A01*, the correction, is *Implemented by* spring's hotfix, which is deployed and reviewed (T09), and so is done;
   - *A02*, the corrective action, is *Implemented by* a new summer task, "Keep sessions in Redis". It is in progress and due 2026-06-30, so it reads as overdue;
   - *A03* is a planned verification.
4. A new test case in the acceptance template, "Stay signed in across a restart", *Verifies* the CAPA. The CAPA stays *Implementing*, waiting for its actions.

### Slow search

*PR-0002*, from the first candidate's failed run, leads to *CAPA-2026-0002*, a preventive one with its causal hypothesis and trigger.

- Its one action is done, and the CAPA is closed as *Ineffective*: the timing ran on empty data.
- It is then reopened, with the reason, remaining actions, revised target, the owner's confirmation and the delay's impact (T04).

### Elsewhere

- The example stack's seed grants `qa@shop.example` the `quality-work` role at `capa`, `capa_action`, `problem_report` and `test_case`. Its people come from the bundle, so Ines signs in there.
- Making the bundles again also rewrites the documents and planning bundles' ids and times, as each time before; nothing else in them changed.

## Verified

- `mise run examples:bundles`, through the API against a fresh database.
- `items/server/tests/examples.rs`: the bundle imports into a fresh database with all its akter.
- `mise run lint` and `mise run test`.

Not seen in the app: the example stack running here is the main checkout's, so `mise run example release-management` wasn't started from this worktree.

<!-- field: acceptance -->
`KURIA_BOOTSTRAP_BUNDLE` imports it. `mise run example release-management` shows both CAPAs with their links, and an action that is overdue.

<!-- akt: PLAN-026-T11 -->
---
type: task
revision: 6
rank: b6T
state: done
in: [PLAN-026]
links: [Blocked by PLAN-026-T10]
title: Document the CAPA flow
assignee: albin@mattsson.io
---

<!-- field: description -->
- `docs/examples/release-management/README.md`:
  - the CAPA flow: how it fits, in the app and from the CLI;
  - the QA/RA Konklav role;
  - the akt-type table.
- `docs/nomenclature.md`: the new terms (correction, corrective action, preventive action, reopening, due date, locked field).
- `docs/requirements.md`: its "Not yet" line.

## Done

- **`docs/examples/release-management/README.md`:**
  - thirteen types in the table;
  - hotfixes deployed first and reviewed after;
  - the *Quality* group, the new icons, tones, navigation and defaults;
  - a new section, *Problem reports and CAPAs*, with how they fit, a CAPA in the app and from the CLI;
  - the bundle's two CAPAs and Ines Berg;
  - the two new Konklav actions, the developer's grant at `problem_report`, and a QA/RA role;
  - the third e2e walk.
- **`docs/nomenclature.md`:**
  - under *Akter*: `reopening`, `waiting transition`, `due date` (with `overdue`) and `locked field`; `state` now says a final state can be left by reopening;
  - a new *Quality* section with `problem report`, `CAPA`, `correction`, `corrective action`, `preventive action` and `effectiveness check`, marked as the example's words, not Kuria's.
- **`docs/requirements.md`:** what now runs, linked to the new section. "Not yet" names deadlines that escalate, obligations and separation of duties.
- **`docs/examples/README.md`**, and the server, CLI, engine and web READMEs, describe what each item now does.

## Verified

`mise run lint` (Tracey and Prettier included). The change is Markdown only, so `mise run test` wasn't rerun for it.

<!-- field: acceptance -->
The README describes the CAPA flow as the app shows it, and every new term is in `docs/nomenclature.md`.

<!-- akt: PLAN-026-T12 -->
---
type: task
revision: 6
rank: b6U
state: done
in: [PLAN-026]
links: [Blocked by PLAN-026-T10]
title: An end-to-end walk of a CAPA
assignee: albin@mattsson.io
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

## Done

`e2e/tests/capa-flow.spec.ts` walks it in the app:

1. The developer reports a problem in a backlog and classifies it. The dialog asks for the risk record once *Uncertain* is chosen in it.
2. QA/RA opens the report and chooses *New CAPA*. The create page says what it will be linked with, starts with the report's title, and creates the CAPA *Originated from* it.
3. QA/RA confirms the CAPA. *Plan actions* asks for the root cause once *Corrective action* is ticked.
4. *Start implementing* locks the effectiveness check, which shows a lock.
5. An action due in 2020 reads "2020-01-01, overdue" on the CAPA's board. *Verify effectiveness* is unavailable and says it waits for the action.
6. On the action, *New Task* isn't offered: QA/RA may not create tasks. A task the developers made is linked as *Implemented by*, and *Finish* says it waits for that task.
7. Once the task is done, the action finishes with its evidence; the CAPA is verified and closed, its dialog saying it stays closed unless reopened, and *Reopen* is offered.

Also:

- **The e2e seed:** Ines Berg (`qa@shop.example`), with `quality-work` at `capa`, `capa_action`, `problem_report` and `test_case`.
- **`e2e/support/app.ts`:** fills `choices` fields.

### Fixed on the way

The transition dialog didn't ask for a field the transition requires on a condition met by a value given in that same dialog: *Classify*'s risk record, which depends on the screening it asks for. The page's `held` now reads the values being edited before the saved ones, and `app.transitions` says so. This corrects what T02 recorded.

### Found

- A transition named *Close* shares its name with the dialog's close button, so the dialog has two buttons named "Close". The walk presses the submit button; an app that names its close button otherwise would read better.
- A closed CAPA shows its reopen fields, empty, until it is reopened, because they have no `states`. Giving them states would hide them until reopened, but keep them out of the second closure's view.

## Verified

- `mise run e2e`, the whole suite, on a freshly seeded stack, taken over from the main checkout with your approval: 53 passed. That includes the release walk, now with the hotfix reviewed, and the other walks with the new types defined.
- `capa-flow` then passed three times in a row once its selectors were fixed.
  - One run in between failed at once, reading the report it had just created as not found. It didn't recur in the next three runs; the cause wasn't found.
  - The full suite wasn't run again after those fixes, which touch only `capa-flow.spec.ts` and the dialog's `held`.
- Only desktop width: the suite's one Playwright project is desktop Chromium, so the phone width in the acceptance wasn't walked.
- `mise run lint` and `mise run test`.

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

<!-- akt: PLAN-026-G05 -->
---
type: gap
revision: 1
rank: b6d
state: open
in: [PLAN-026]
title: "A second reopening keeps the first one's answers"
---

<!-- field: description -->
Reopening a CAPA requires the reason, remaining actions, revised target, owner confirmation and impact of the delay. On a second reopening they still hold the first one's values, so the transition doesn't make anyone give them again. A transition that clears fields as it is performed would fix it; Kuria has none. Until then, the README says to give them again.
