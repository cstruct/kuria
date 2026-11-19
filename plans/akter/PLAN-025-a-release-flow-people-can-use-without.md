---
number: PLAN-025
type: plan
revision: 28
rank: b5p
state: active
title: "A release flow people can use without knowing Kuria's model"
owner: albin@mattsson.io
---

<!-- field: goal -->
Someone who knows releases, but not Kuria, can go through the release-management example from start to finish without a misstep: create a backlog and fill it quickly, plan a release, start a candidate and pull tasks into it from its backlog, move cards across its board, and run a test protocol for the candidate. Along the way they never land on an unstyled form, never fill in a form only to be refused, never meet a primary button that ends or approves something they have only just started, and never read "akt", "member", "container" or "transition".

It works when the fixes below are each checked off against the design review of 19 Nov 2026 (the artifact "Kuria release flow review": 16 critical, 37 major and 22 minor findings, numbered by flow, such as 4.2 for flow 4's second finding), and the end-to-end tests walk each of its seven flows in the new way.

<!-- field: design -->
## Where this comes from

The review walked seven flows of `mise run example release-management` as Devi Larsen (developer) and Rami Moberg (release manager), at 1440 and 375 pixels wide:

1. creating a backlog,
2. adding tasks to it,
3. creating a release,
4. starting a release candidate,
5. filling the candidate with backlog tasks and new tasks,
6. moving cards,
7. running a test protocol.

The detail pages and boards share a quiet, consistent visual system. The flows around them don't, and most findings come from eight shared patterns:

- two design languages, where the create route is a browser-default form;
- the primary action is the first transition, not the next best step;
- the data model's words in the copy;
- emails and numbers where people expect names and titles;
- unlabelled badges;
- context lost when navigating;
- permissions found out only after a form is filled in;
- dialogs and toasts each built separately.

The finding numbers below are the review's.

Finding 5.4, a release manager refused a task, was mostly the example's stale seed. `4b6a0e5` fixed that. The pattern it shows remains for any role that really may not create something.

## Constraints

- **Kuria knows no akt type.** Every fix is generic, driven by what the type document says: its states and tones, its transitions, `board`, `board.parent`, its fields and `presentation`. A fix never names `release_candidate`.
  - Where the app can't infer what the review asks for, the type says it, through `presentation` or another member of the type document. That member is specified in `docs/spec/akter.md` first, then used by the example.
- **Spec first.** Each task changes or adds its `app.*` rules in `docs/spec/frontend.md`, or its `akt.*` rules, before the code. It references them as `r[impl …]` and `r[verify …]`.
  - Rules most touched: `app.header`, `app.trail`, `app.fields`, `app.transitions`, `app.board-drop`, `app.board-create`, `app.members`, `app.verdicts`, `app.copy`, `app.errors`, `app.navigation`, `app.narrow`.
- **Konklav decides permissions.** The app may ask what the viewer may do, but it must not guess from role names. Konklav, through the server, stays the authority on every write (G01).
- **Shared components first.** The create route (`items/web/src/routes/types/[key]/new/+page.svelte`), the akt header, `Members.svelte`, the dialog shell and the toast hold most findings. Fixing them clears most of the list, so they come first in task order.
- **The words.** Copy uses the type's own names (its `name`, its states' and transitions' names), and docs/nomenclature.md's terms stay for code and docs. A user-facing word that is new goes into nomenclature.md first (D01).

## Order

These tasks fix things everyone meets, and come first:

- T01: the create route
- T02: permissions
- T03: the primary action
- T04: names
- T05: copy
- T06: status and badges
- T07: trails
- T08: dialogs and toasts

These are new ways of working, built on them:

- T09: quick-add on boards
- T10: moves without a modal
- T11: pulling from a parent board
- T12: fields by state
- T13: candidate readiness
- T14: running a protocol

These come last:

- T15: navigation, home and history
- T16: narrow screens
- T17: the example, its README and its end-to-end walk

Any of T09 to T14 may grow into a plan of its own when it is started. Leave the task here, linked to that plan.

<!-- field: verification -->
- **Per task:** the spec rules it changes, with `r[verify …]` in app unit tests (`items/web`, Vitest) or end-to-end tests. `mise run lint` and `mise run test`.
- **End to end:** each flow walked in `e2e/` against the release-management example:
  - `e2e/tests/release-flow.spec.ts`, or the existing specs it extends: create a backlog, add five tasks without leaving the board, plan a release, start a candidate, pull three tasks from its backlog, move a card with no dialog and undo it, copy a protocol for the candidate, report verdicts inline, finish the run, accept the candidate.
  - Each flow runs as the role the review used. Axe runs on each new dialog, sheet and form.
- **By eye:** `mise run example release-management` at 1440 and 375 wide, against the review's screenshots. Every finding is either fixed, or dropped with a reason on its task.
- **Against the review:** each task lists the finding numbers it closes. Once T17 is done, a pass over the review's "Suggested order of work" finds nothing left open.

<!-- akt: PLAN-025-T01 -->
---
type: task
revision: 6
rank: b5q
state: done
in: [PLAN-025]
links: [Blocks PLAN-025-T09]
title: "Create forms in the app's design system, validated before they are sent"
assignee: albin@mattsson.io
---

<!-- field: description -->
Covers findings 1.1, 1.2, 1.3, 2.2, 3.2 (the doubled label) and 1.4 (the client-side half).

- Build the create route (`items/web/src/routes/types/[key]/new/+page.svelte`) from the same field components as the detail page's inline editors:
  - labels above the fields, hints below them;
  - no fieldset chrome;
  - one visible label per control.
- Put the primary button on the right, with Cancel next to it.
- Check required fields in the browser before sending, the way the verdict dialog does: an asterisk on the label, and the hint replaced by what is missing.
- Move the revision's reason into an optional "Note for the history" disclosure.
- Show refusals in an alert block (`app.errors`) that says what to do next.
- Rules: `app.fields`, `app.errors`, and a new `app.create` for the create page.

## Done

The create page (`items/web/src/routes/types/[key]/new/+page.svelte`) is now built from the app's own parts:

- a card of fields, each labelled once, above its control, with its hint below and required fields marked;
- Cancel and Create on the right;
- the note for the history under a closed disclosure.

`FieldEditor` lost its fieldset chrome everywhere it is used, and `SomeoneSelect` no longer repeats the label. Required fields left empty are named on the field, and nothing is sent (`missingRequired`). A refusal shows in the new `Alert`.

Spec: `app.create`.

Verified:

- unit tests in `creating.test.ts`;
- `mise run e2e`, in which every test that creates through the page passed;
- by eye in the example.

<!-- field: acceptance -->
- Every type's create page uses the detail page's field styles, with no browser-default fieldset or button.
- Submitting without a required field shows the problem on the field, and sends nothing.
- A refusal shows as an alert with its code's message.
- Axe passes on the create page.

<!-- akt: PLAN-025-T02 -->
---
type: task
revision: 4
rank: b5r
state: done
in: [PLAN-025]
title: Offer only the actions the viewer may perform
---

<!-- field: description -->
Covers findings 1.4, 5.4 and the home page's 3 ("New" on every tile).

- Hide "New", a column's "+", and the Add and copy menus' items that the viewer may not use, or show them disabled with the reason.
- The reason names who can, where the type says so, as in "Only developers can add tasks to Autumn backlog".
- Needs the server to answer what the viewer may do per akt type, through Konklav (G01).
- On the home page, offer "New" only for types the viewer may create that live outside a container. For a type created inside a container, link to where it is made instead.
- Rules: `app.board-create`, `app.home`, and a new `app.permitted` stating that the app offers no action the viewer can't perform.

## Done

G01 is resolved: `AktType.viewerMay` (`authz.viewer`).

The app offers by it (`app.permitted`):

- "New" on the home page and on a type's list, only for types the viewer may create;
- a column's "+", only for types they may create;
- edits, the board's Add, card menus and verdicts, only where they may edit;
- transitions, only where they may perform them;
- Make a copy, only where they may create;
- Archive and Unarchive, only where they may archive;
- the "In" panel's remove and move, only for containers they may edit.

Not done:

- "Say who can". Konklav doesn't answer who holds a permission, so actions are hidden rather than explained.
- New only for "top-level" types. Kuria lets any type be created on its own, so the home page shows New for every type the viewer may create.

Verified:

- `an_akt_type_says_what_the_viewer_may_do` in items/server/tests/authorization.rs;
- the end-to-end test "a reader is offered nothing to create, edit or move" in e2e/tests/home.spec.ts;
- by eye as Hedda Lund in the example.

<!-- field: acceptance -->
- Signed in as a role with read only, no create action shows anywhere.
- As a role that may create in some types, only those types' actions show.
- The server still refuses a write the viewer may not make. The app's hiding is a courtesy only, and an end-to-end test checks the refusal.

<!-- akt: PLAN-025-T03 -->
---
type: task
revision: 5
rank: b5s
state: done
in: [PLAN-025]
links: [Blocks PLAN-025-T13]
title: "Make the next best step the primary action, and put transitions in a status menu"
---

<!-- field: description -->
Covers findings 1.5, 3.7, 4.2, 7.3 and 7.10.

- The header promotes one action. Today that is the first transition the state allows, which gives "Close" on a new backlog, "Accept" on an empty candidate and "Keep as template" on a fresh run.
- Instead:
  - Transitions move into a menu on the state pill, or into ⋯.
  - The primary action becomes the content action, such as Add task.
  - A transition is primary only where the type marks it for that state (G02).
- A transition into a `success` or `danger` toned state is never primary by default. Its dialog confirms with a summary of the akt.
- Rules: `app.header`, `app.transitions`, and `akt.type.presentation` if G02 adds a member.

## Done

G02 is resolved: `presentation.primary`, validated in items/config.

The header (`actionsOf` in `header.ts`) now offers first:

- the action the presentation names for the state; else
- creating the first type the board's first column offers, on a container the viewer may add to; else
- none.

No transition is primary by default. A transition to a final state says, when confirmed, that the akt stays there.

The example names primary actions as follows:

- test protocols: Start on a draft, Make a copy on a template, Finish on a run;
- tasks and epics: their next step.

Backlogs and releases get "New …" instead of Close or Start testing. Candidates get no primary action, so Accept is no longer one.

Not done: transitions are still buttons beside each other, not in a status menu. That is left to T06's status line.

Verified:

- `header.test.ts`;
- the presentation test in items/config/src/validate.rs;
- `mise run e2e`.

<!-- field: acceptance -->
- The review's four cases each show the intended primary action:
  - a new backlog: Add task;
  - an empty release: New candidate;
  - an untested candidate: no Accept as primary;
  - a copied draft run: Start.
- The other transitions are still one menu away, each with its name.

<!-- akt: PLAN-025-T04 -->
---
type: task
revision: 4
rank: b5t
state: done
in: [PLAN-025]
title: "Show people by name with an avatar, and pick them by name"
---

<!-- field: description -->
Covers findings 2.6, 3.2 (default "Nobody", options as emails), 5.6 and 7.6.

- Users are shown by email while contacts are shown by name. `person.name` falls back to the email when Kuria recorded no name, and the example's people seem never to have given one (G03).
- Fix the source of the names first.
- Then show an avatar (initials) with the name everywhere a user appears: cards, details, verdicts, history, and the header's account. Show the email on hover.
- The user picker shows names and avatars and searches by name.
- A `user` field the creator would usually fill in defaults to them ("Me (Rami Moberg)") where the type says so (G02 or a field member).
- Rules: `person.name`, `app.fields`, `app.pickers`.

## Done

G03 is resolved. A sign-in no longer erases the name a bundle recorded (`person.from-sign-in`). The example's people keep their names after signing in, and the account shows "Rami Moberg" rather than rm@shop.example.

Also:

- A user value is shown with an initials avatar (`Avatar`), as is the account.
- A picker names the viewer "Me (their name)".
- A single `user` field may say `"default": "creator"` (`akt.field.default`), which the create page fills with the creator. The example's release manager and test protocol owner do.

Not done:

- The email on hover. A user's email isn't asked for where values are shown, and contacts' emails are shown only to those who may read contacts.
- Avatars and chips on board cards. That is T06.

Verified:

- `a_sign_in_without_a_name_keeps_the_recorded_one` in items/server/tests/browser_sessions.rs;
- the default test in items/config;
- `people.test.ts`;
- by eye as Rami in the example.

<!-- field: acceptance -->
- In the example, no email shows as a person's label anywhere in the seven flows.
- The release form's release manager defaults to the signed-in release manager.

<!-- akt: PLAN-025-T05 -->
---
type: task
revision: 5
rank: b5u
state: done
in: [PLAN-025]
links: [Blocks PLAN-025-T15]
title: "Write the app's copy in the type's words, not the model's"
---

<!-- field: description -->
Covers the copy pattern and findings 1.2 (home), 1.7, 2.10, 4.3 (placeholder), 5.5, 6.4, 7.4 and 7.7.

Replace each sentence that describes the implementation:

- "An existing akt…" becomes "Add existing task or epic…", from the types the container accepts.
- "In · Not in any container" becomes "Part of Autumn release", or is hidden when empty.
- "Its state changes by a transition…" is removed.
- The copy dialog's description becomes "Copies the protocol into a new run…".
- "6 akter" becomes the type's plural, and "AKT TYPES" becomes "Work".
- "Layout · 5 members placed" is hidden from Details.
- "Someone else, by subject id…" is kept only for admins.

Plurals need the type to name them, unless the type's name pluralises simply (G04). Record the user-facing words in docs/nomenclature.md (D01).

## Done

The app's words are in docs/nomenclature.md ("The app's words"), and `app.words` requires them.

Rewritten:

- Home and sidebar: "Work", counts in the type's words ("6 tasks"), and plainer empty states.
- An akt's page: "Contents" instead of "Members". "Add items" offers "New <type>", only for the types the viewer may create, and "Existing task or epic…".
- "Part of" instead of "In", hidden when the akt is in nothing.
- "Add to…" and "Move to another <type>…" instead of the container wording.
- Moving a card no longer explains transitions.
- The copy dialog no longer describes the implementation.
- Refusals use everyday words (errors.ts).
- The picker's escape hatch reads "Someone not listed…".

G04's first half is built: `presentation.plural`. The example's release notes say theirs. Sidebar groups are left for T15, and G04 stays open until then.

Not done: "Layout · 5 members placed" now reads "5 items placed", but it is still shown in Details. Hiding it, with reordering from the sections' header, goes with T15.

Verified:

- unit tests (`home.test.ts`, `text.test.ts`, `history.test.ts`);
- the config test for `plural`;
- the affected e2e specs (home, members, archive, release-flow, test-protocols, links), with their selectors moved to the new words.

<!-- field: acceptance -->
- A grep of `items/web/src` finds no user-facing string containing "akt", "akter", "member", "container" or "transition", except in admin-only screens.
- Each string replaced is covered by a unit or end-to-end assertion on its new text.

<!-- akt: PLAN-025-T06 -->
---
type: task
revision: 4
rank: b5v
state: done
in: [PLAN-025]
title: "Label status badges, and drop the ones a board already says"
---

<!-- field: description -->
Covers findings 2.8, 2.9, 3.6, 6.6, 6.8, 7.2 and 7.20.

- The header shows a labelled status line instead of two unlabelled grey pills: state in its tone, then the outcome, as in "Status: Planning · Tests: none yet".
- Cards on a board grouped by state drop the state pill.
- Cards show the type's card fields as chips and avatars, not as label/value pairs.
- A member whose outcome comes from verdicts hides its own state, such as "Open", which never changes.
- Numbers on narrow cards stay on one line, with an ellipsis.
- Rules: `app.header`, `app.members`, `app.verdicts`.

## Done

Changes:

- The header's state and outcome are labelled: "Status: Open" and "Result: Failed" (`app.header`).
- Board cards show a choice as a chip and a user as an avatar with their name, instead of label and value pairs (`shownFields`' `as`).
- A card shows its state only when its column holds more than one state of its type (`showsState`).
- Members whose outcome comes from verdicts, such as test cases, no longer show a state that never changes. That applies in sections, and in lists, where the Status column goes (`judgedType`).
- Lists show people and chips the same way.
- A card's footer no longer wraps its number.

Spec: `app.header`, `app.members`.

Not done: the state's colour by tone already existed. A status menu, with transitions moved into it, was left out: transitions stay buttons, and only one is ever primary (T03).

Verified:

- `members.test.ts`;
- e2e members, test-protocols, release-flow, archive, akt-page and accessibility;
- by eye on a backlog and a test run in the example.

<!-- field: acceptance -->
- No header shows two unlabelled pills.
- A state-grouped board's cards carry no state pill.
- A test case shows one outcome mark.

<!-- akt: PLAN-025-T07 -->
---
type: task
revision: 5
rank: b5w
state: done
in: [PLAN-025]
links: [Blocks PLAN-025-T09]
title: "Trails by place, headings by title, numbers as eyebrows"
---

<!-- field: description -->
Covers findings 2.1 (the "Task › New" trail), 2.5, 4.1, 4.5 and 5.6.

- The trail follows where an akt lives, its containers from the outermost, as in "Autumn 4.4.0 › Candidate 1". It no longer follows the type, as in "Task › TASK-0012".
- The number moves to the eyebrow, the heading shows the title, and links and the "In" line lead with titles.
- Creating from a board returns to that board unless the person asks to see what they made. That also lands with T09.
- An akt whose title is its number, as candidates are today, gets a title from the type's presentation. The example names candidates by build or "Candidate 1" (T17).
- Rules: `app.trail`, `app.header`, `akt.type.presentation` (`title`).

## Done

The trail follows where an akt is: its first container's own container, then that container, then the akt, as in "Summer › Summer backlog › Save a product for later". An akt that is in nothing, or one reached by following references, keeps the type-first trail. Every akt on the trail leads with its title, with its number under it (`app.trail`). The "Part of" card links by title too, with the type and number under it. The heading already showed the title, with the number in the eyebrow.

Not done:

- Returning to the board after creating from it. That goes with T09's quick-add.
- Candidates titled by build. That is T17, the example.

Verified:

- `trail.test.ts`;
- e2e members, release-flow, links, documents, test-protocols and shell;
- by eye on a task in a backlog in a release.

<!-- field: acceptance -->
- A task made in a backlog shows "Autumn backlog › Gift cards at checkout".
- A candidate's heading is never its number.
- Back and forward still restore each trail, as `app.trail` requires.

<!-- akt: PLAN-025-T08 -->
---
type: task
revision: 5
rank: b5x
state: done
in: [PLAN-025]
links: [Blocks PLAN-025-T10]
title: "One dialog, one toast, one picker"
---

<!-- field: description -->
Covers findings 1.10, 4.4, 5.2, 5.3, 6.5, 6.7, 7.15 and 7.18.

- **One dialog shell:** a title in words (not "Report on TP-005-C02"), Cancel then the primary action on the right, and a body that scrolls with the picker results inside it. Opening it never shifts the page (`scrollbar-gutter: stable`).
- **One toast:** "<title> moved to <state>", "Copied to <title>", with Undo where it can be undone (T10).
- **The picker:** clears and blurs its input once something is picked, and highlights the new row.
- Creating anything invalidates the sidebar's counts.
- One transition dialog serves the header, the board and the card menu alike.
- Rules: `app.dialog-focus`, `app.transitions`, `app.pickers`.

## Done

Changes:

- Every dialog ends alike: Cancel, or Close for the version diff, then its action on the right. A deleting action, the verdict's Delete, sits apart at the start (`Dialog`'s `cancel` and `start`). The dialogs' own Cancel buttons are gone.
- Pickers in dialogs show their results inside the dialog (`Combobox`'s `inline`, bits-ui's `ContentStatic`). A picker empties its input once something is picked.
- `scrollbar-gutter: stable` keeps the page from shifting when a dialog opens.
- Toasts say what changed and where it went: "<title> moved to <state>", "Copied to <title>", "<title> archived" (`app.toasts`).
- The sidebar's counts are fetched again when akter change (`query`'s `additionalTypenames`).
- The markdown editor's reference picker is labelled "Item".

Spec: `app.dialog-focus`, `app.pickers`, `app.toasts`.

Not done: Undo in toasts, which comes with T10's moves.

Verified:

- unit tests;
- the full e2e suite, run after this commit, with any follow-up fixed in its own commit;
- by eye on the add dialog.

<!-- field: acceptance -->
- Every dialog in the seven flows has the same footer.
- No toast quotes a number or a transition key.
- The page doesn't move when a dialog opens.
- The sidebar's counts match after creating something.

<!-- akt: PLAN-025-T09 -->
---
type: task
revision: 6
rank: b5y
state: done
in: [PLAN-025]
links: [Blocked by PLAN-025-T01, Blocked by PLAN-025-T07]
title: "Quick-add in board columns, with a side sheet for details"
---

<!-- field: description -->
Covers findings 1.8, 1.9, 2.1, 2.3 and 2.4.

- A column offering creation (`app.board-create`) gets an inline title input. Enter creates the akt in the column, and focus stays in the input for the next one.
- Shift+Enter, or an expand button, opens a side sheet with the type's other fields while the board stays visible. That covers the fields asked at creation (T12).
- An empty board shows an empty state in its first column, with that input and "Add existing…".
- The board's "Add" becomes "Add existing…", and a column's "+" is the only "new here".
- Rules: `app.board-create`, plus a new `app.quick-add`.

## Done

Changes:

- A column offering a type whose akter need only a title has an "Add a task…" input. Enter creates the akt in the column and its parent boards, says "Added …", and keeps focus for the next. A column offering several such types gets a choice of which (`app.quick-add`; `quickAddable`, `titleFieldOf`).
- Shift+Enter opens the create page with the title filled in.
- The create page, opened from a container's board ("+", Add, Shift+Enter), goes back there once the akt is made, saying it was added (`app.create`).
- Cards with nothing to show no longer leave an empty row.

Also fixed, for T07: following a reference keeps the containers the trail began with, so an index's document starts its trail with the index when references are followed too.

Not done:

- The side sheet. It is recorded as a gap of this plan.
- An empty state inside the first column. The board's columns already show when empty, with the quick-add input in them.
- Renaming "Add" to "Add existing…". "Add items" now offers New and Existing, so it stays.

Verified:

- `creating.test.ts` and `trail.test.ts`;
- e2e "a column adds tasks by their titles, staying on the board": five by keyboard, then Shift+Enter;
- e2e "a column creates its types…", which now comes back to the board;
- the markdown editor's trail test;
- by eye in the example.

<!-- field: acceptance -->
- An end-to-end test adds five tasks to a backlog with the keyboard alone, without the page navigating.
- Each task is created in the backlog and in its parent boards (`board.parent`).
- Axe passes on the side sheet.

<!-- akt: PLAN-025-T10 -->
---
type: task
revision: 5
rank: b5z
state: done
in: [PLAN-025]
links: [Blocked by PLAN-025-T08]
title: "Board moves without a modal, undone from a toast"
---

<!-- field: description -->
Covers findings 5.7, 6.2, 6.3, 6.4 and 6.5.

- Dropping a card on a column with exactly one transition from its state, and one that requires no fields, performs it at once and shows a toast with Undo.
- A dialog opens only when a transition needs fields, or when several transitions lead to the column.
- Undo performs the transition back, where one exists (G05).
- A column the card can't go to says why when the card is dragged over it, as in "Start it first".
- The card menu gains "Move to ▸" with the allowed states, and menu items lose the number ("Move up").
- Rules: `app.board-drop`, `app.transitions`, `app.accessible` (moving without a pointer).

## Done

Changes:

- A drop on a column with one transition there, needing no fields, moves the card at once. The toast says "<title> moved to <column>" and offers Undo when a transition leads back (`undoOf`, G05).
- A dialog opens only when several transitions lead there, or when one needs fields. Those fields are asked for on the member's page, as before.
- A column the dragged card can't go to says "Can't move here from <status>".
- A card's menu offers "Move to <column>" for each column a transition reaches, so cards move by keyboard. The other items lose the number: "Move up", "Move down", "Remove from here".
- Toasts can carry an action (`toast(…, action)`).
- The example's tasks gain "Put back".

Spec: `app.board-drop`.

Verified:

- `members.test.ts` (`undoOf`) and `labels.test.ts`;
- the e2e members test: a drop moves at once, Undo moves it back, and "Move to In progress" from the menu;
- release-flow.

<!-- field: acceptance -->
- Dragging a task from To do to In progress makes one write and shows no dialog. Undo returns it.
- The same move is possible from the keyboard through "Move to".

<!-- akt: PLAN-025-T11 -->
---
type: task
revision: 2
rank: b60
state: todo
in: [PLAN-025]
links: [Blocks PLAN-025-T17]
title: Pull many members from a parent board at once
---

<!-- field: description -->
Covers findings 3.8, 4.3, 4.7, 5.1 and 5.8.

- A container with a parent board (`board.parent`) gets "Pull from <parent>". It opens a sheet showing the parent's board by column, where several akter can be ticked. Akter already in another container of the same type are marked.
- The global search becomes a secondary tab, and lists only akter from open containers.
- The parent board is asked for on the create form, defaulting to the parent container's own linked board when it has one. That covers 4.3: a release's backlog becomes the candidate's default.
- After linking a parent board, the container offers the pull, as in "Pull tasks from Autumn backlog (3 ready)".
- A release shows its linked backlog in a "Planned from" area.
- "Remove TASK-0012" becomes "Remove from candidate", and is no longer shown in red.
- Rules: `board.parent`, `app.members`, `app.links`, `app.board-create`.

<!-- field: acceptance -->
- An end-to-end test pulls three tasks into a candidate in one go.
- A task already in another candidate is marked.
- Tasks of a closed backlog aren't offered.

<!-- akt: PLAN-025-T12 -->
---
type: task
revision: 2
rank: b61
state: todo
in: [PLAN-025]
links: [Blocks PLAN-025-T17]
title: Ask for fields in the state they belong to
---

<!-- field: description -->
Covers findings 2.3 (every field on day one), 3.1 and 3.3.

- A type says which fields are asked at creation and which belong to a later state or transition (G06). Examples: a release's "Released on", its "Go for release" approval, and a task's pull request.
- The create form asks only for the creation fields. The others show on the akt's page once its state reaches them, or in the transition that sets them.
- "Go for release" becomes its own step, with the approver, "on behalf of", and a confirmation.
- Rules: `akt.type` (a new member, specified in `docs/spec/akter.md`), `app.create`, `app.fields`.

<!-- field: acceptance -->
- Creating a release asks for its title, version and manager only.
- "Go for release" can't be ticked from the create form.
- Bundles and the CLI read and write the new member (`kuria types schema` shows it).

<!-- akt: PLAN-025-T13 -->
---
type: task
revision: 3
rank: b62
state: todo
in: [PLAN-025]
links: [Blocked by PLAN-025-T03, Blocks PLAN-025-T17]
title: "Candidate readiness: a checklist, and Accept gated by its test runs"
---

<!-- field: description -->
Covers findings 4.2, 4.6 and 7.14.

- A candidate's Accept is allowed only when its outcome is a pass, using `akt.transition.outcomes`, which already exists, in the example's type.
- The header shows why a transition is unavailable next to its button, as in "3 of 5 cases need a verdict", linking to the first pending case.
- The candidate shows a readiness checklist built from its members' outcomes.
- The test record is computed from the finished runs, or dropped from the type if the runs are the record.
- Accepting confirms with a summary.
- Rules: `app.transitions`, `app.header`; the example's `release_candidate.json`.

<!-- field: acceptance -->
- An end-to-end test can't accept a candidate whose runs aren't all passed, and sees why next to Accept.
- It can accept once they are.

<!-- akt: PLAN-025-T14 -->
---
type: task
revision: 2
rank: b63
state: todo
in: [PLAN-025]
links: [Blocks PLAN-025-T17]
title: "Run a protocol for a candidate, with verdicts inline"
---

<!-- field: description -->
Covers findings 7.1, 7.5, 7.8, 7.9, 7.11, 7.12, 7.13, 7.16 and 7.19.

- Copying a template asks where the copy goes, defaulting to the newest container of a type that holds it in a testing state. The copy is named after it, as in "Shop acceptance test: 4.4.0-rc.1".
- Verdict controls show only in the states where verdicts are given: not on templates or drafts (G06).
- Each case gets Pass, Fail and Skip inline. A fail or a skip opens the description in place, required as `akt.field.verdict.description` says. The case's steps and expected result show beside them.
- Each tester gets a row with avatar, name, status chip and description. Your own row holds your controls.
- A collapsed section shows a chevron and its count of cases.
- "All" and "None" in the copy dialog look like buttons.
- This reverses PLAN-024's dialog for reporting a verdict (G07).
- Rules: `app.copy`, `akt.copy`, `app.verdicts`, `app.sections`.

<!-- field: acceptance -->
- An end-to-end test copies the protocol for a candidate, starts it, reports pass, fail with a description and skip inline, finishes it, and sees the candidate's outcome change.
- No verdict control shows on the template.

<!-- akt: PLAN-025-T15 -->
---
type: task
revision: 3
rank: b64
state: todo
in: [PLAN-025]
links: [Blocked by PLAN-025-T05, Blocks PLAN-025-T17]
title: "Navigation by job, a readable home and a readable history"
---

<!-- field: description -->
Covers home findings 1 to 3, and findings 1.6, 2.7 and 2.11.

- The sidebar groups types by the job they belong to, such as Planning, Releasing and Testing, with groups named by the types (G04). It no longer lists them alphabetically. Tasks are findable there.
- Home tiles let names wrap to two lines.
- History renders values through the fields' display labels and order. It groups entries by person and minute ("Devi added 3 tasks") and collapses all but the latest five.
- The Details card hides when it would only repeat the title, and the heading becomes editable in place.
- Rules: `app.navigation`, `app.home`, `app.history`, `app.inline-edit`, `akt.type.presentation`.

<!-- field: acceptance -->
- In the example, Tasks are reachable from the sidebar without "Show more".
- A backlog with three tasks shows a single history entry for adding them.

<!-- akt: PLAN-025-T16 -->
---
type: task
revision: 2
rank: b65
state: todo
in: [PLAN-025]
links: [Blocks PLAN-025-T17]
title: Boards and pages that work at phone width
---

<!-- field: description -->
Covers the 375-pixel findings 1 and 2.

- On narrow screens, a board's columns stack as collapsible groups, or snap-scroll with tabs naming each column and its count.
- The board comes before Details, Links and History.
- Rules: `app.narrow`.

<!-- field: acceptance -->
- At 375 wide, every column of a three-column board is reachable, with its count visible, and the board is the first thing after the header.

<!-- akt: PLAN-025-T17 -->
---
type: task
revision: 7
rank: b66
state: todo
in: [PLAN-025]
links: [Blocked by PLAN-025-T11, Blocked by PLAN-025-T12, Blocked by PLAN-025-T13, Blocked by PLAN-025-T14, Blocked by PLAN-025-T15, Blocked by PLAN-025-T16]
title: "The release-management example, its README and an end-to-end walk of the seven flows"
---

<!-- field: description -->
Covers what the generic tasks leave to the example.

- Candidates are titled by build, or "Candidate 1".
- Accept requires passed runs.
- The types are grouped by job.
- Fields are asked by state, using the members G02, G04 and G06 decide.
- Write the bundle again, and update docs/examples/release-management/README.md and the server's example tests (`items/server/tests/examples.rs`).
- Add the end-to-end walk the plan's verification describes.

<!-- field: acceptance -->
- `mise run e2e` walks all seven flows as their roles.
- The example's README describes the flows as the app now shows them.
- `mise run example release-management` matches it by eye.

<!-- akt: PLAN-025-G01 -->
---
type: gap
revision: 2
rank: b67
state: closed
in: [PLAN-025]
title: How the app learns what the viewer may do
---

<!-- field: description -->
The app has no way to ask whether the viewer may create an akt of a type, or edit a container. `Viewer` holds only the subject, the tenant and the user. Konklav decides through the relying client.

Options:

- a `viewer.may(action, aktType)` field, or permissions on `AktType`, answered from Konklav's check;
- a batch answer for every type in one request.

Either way it must stay a hint, and the server still checks every write. Resolve before T02.

<!-- field: resolution -->
Built. `AktType.viewerMay { create edit transition archive }` answers what the request's subject may do with the type's akter, decided by the same Konklav requirements as the writes (`authz.viewer`, in docs/spec/authorization.md). Each is decided only when asked for, and remembered for the request. A transition's own action, such as `release:approve`, isn't part of `transition`, so the app may still offer a transition the server then refuses. That refusal is shown (`app.errors`). The app offers by it (`app.permitted`); every write is still decided on its own.

<!-- akt: PLAN-025-G02 -->
---
type: gap
revision: 2
rank: b68
state: closed
in: [PLAN-025]
title: "How a type says its primary action, and the fields its creator fills in"
---

<!-- field: description -->
T03 needs the primary action per state, where the app can't infer it. T04 needs to know when a `user` field defaults to the creator.

Possible additions to `presentation`:

- `primary: {<state>: <transition or "add">}`;
- a field member such as `default: "creator"`.

Alternatively, a rule with no new member: content actions always first, and transitions never primary. Decide in `docs/spec/akter.md` before T03 and T04.

<!-- field: resolution -->
Decided and built. `presentation.primary` names, per state, the action an akt's page offers first: a transition from that state, or `copy` (`akt.type.presentation`, validated in items/config). Without one, the page offers creating the first akt type its board's first column offers, on a container the viewer may add to, else no primary action. No transition is primary by default (`app.header`).

A `user` field that is not `multiple` may say `"default": "creator"` (`akt.field.default`). The create page then offers its creator as the value.

<!-- akt: PLAN-025-G03 -->
---
type: gap
revision: 2
rank: b69
state: closed
in: [PLAN-025]
title: "Why the example's users show as emails"
---

<!-- field: description -->
`person.name` shows the recorded name, else the email. Kuria records names from the `profile` claims at a browser sign-in (`person.from-sign-in`). In the example, Devi and Rami show as their emails although the bundles name them.

Find out whether:

- Konklav's subjects have no name to give, since the example's setup gives none;
- the claims leave the name out; or
- people who haven't signed in have nothing recorded.

Then fix the source: the seed, Konklav's claims, or names taken from a bundle's people. Resolve before T04.

<!-- field: resolution -->
Found and fixed. Konklav keeps no profile data, so its userinfo gives no name, and every browser sign-in replaced the recorded name with none. Names from a bundle's people were lost the first time each person signed in, and they then showed as their email. A sign-in now keeps what was recorded for a claim it doesn't receive (`person.from-sign-in`, tested in items/server/tests/browser_sessions.rs). People with no recorded name are still shown by email (`person.name`).

<!-- akt: PLAN-025-G04 -->
---
type: gap
revision: 1
rank: b6A
state: open
in: [PLAN-025]
title: Plurals and job groups for akt types
---

<!-- field: description -->
T05 wants "6 tasks" and T15 wants the sidebar grouped by job. The type document has a `name` but no plural, and `navigation` is only `main` or `more`.

Decide whether `presentation` gains:

- a `plural`;
- a `group`, the name of a sidebar group, beside or instead of `navigation`.

Resolve before T05 and T15.

<!-- akt: PLAN-025-G05 -->
---
type: gap
revision: 2
rank: b6B
state: closed
in: [PLAN-025]
title: What Undo means for a move
---

<!-- field: description -->
T10 undoes a move by performing the transition back. A type may have none (Done to Open), or the transition back may require fields, or the outcome may refuse it.

Decide whether Undo:

- is offered only when a transition back exists and needs no fields; or
- is a short delay before the write is sent, so that nothing is written until the toast closes.

The second needs `app.live` to tolerate the wait. Resolve before T10.

<!-- field: resolution -->
Decided and built (`app.board-drop`). Undo is offered only when a transition leads straight from the new state back to the former one and requires no fields (`undoOf`). It performs that transition, with the member's current revision. Writes are not held back until the toast closes, which would have fought live updates and other readers. A type without a way back, such as one whose akter can't be reopened, offers no Undo.

The example's tasks gain "Put back", In progress to To do, so that a start can be undone.

<!-- akt: PLAN-025-G06 -->
---
type: gap
revision: 1
rank: b6C
state: open
in: [PLAN-025]
title: How a type says the states its fields and verdicts belong to
---

<!-- field: description -->
T12 asks fields only in their state. T14 shows verdict controls only on runs.

Neither is in the type document today. Transitions can require fields, but a field can't say it belongs to a later state, and a `verdict` field is editable in every state.

Possible additions:

- a field member `from: [<state>…]`, or `asked: creation | <transition>`;
- for verdicts, the states in which they are given.

Specify in `docs/spec/akter.md` with the server's enforcement, not only the app's hiding. Resolve before T12 and T14.

<!-- akt: PLAN-025-G07 -->
---
type: gap
revision: 1
rank: b6D
state: open
in: [PLAN-025]
title: "Inline verdicts against PLAN-024's dialog"
---

<!-- field: description -->
PLAN-024 just made reporting a verdict a dialog, so that the description is said deliberately. The review counts about 240 clicks for a 30-case protocol with two testers, and asks for inline Pass, Fail and Skip, with the description inline for a fail or a skip.

Decide whether inline controls supersede the dialog, keeping Edit and Delete in a menu, or whether the dialog stays and gains keyboard shortcuts.

A decision that supersedes PLAN-024's design goes in this plan. Resolve before T14.

<!-- akt: PLAN-025-D01 -->
---
type: decision
revision: 3
rank: b6E
state: decided
in: [PLAN-025]
title: "The app speaks in the type's words; Kuria's terms stay in code and docs"
decided_by: albin@mattsson.io
---

<!-- field: context -->
The review found "akt", "akter", "member", "container" and "transition" throughout the app's copy. Release managers and testers don't use these words. They are docs/nomenclature.md's terms for Kuria's model, which code, specs and docs must use.

<!-- field: decision -->
- The app's user-facing text names things by the type document: its `name`, its states' and transitions' names, and its fields' names.
- Where a generic word is unavoidable, it is an everyday one ("item", "part of", "move", "status"), recorded in docs/nomenclature.md as the app's word for the term.
- Admin screens, the type editor and the CLI keep Kuria's terms.

<!-- field: why -->
The people who use a tenant think in its types. Admins and agents, who define types, need the model's words to be exact. One vocabulary can't serve both, and the type document already holds the tenant's words.

<!-- akt: PLAN-025-G08 -->
---
type: gap
revision: 1
rank: b6F
state: open
in: [PLAN-025]
title: "A side sheet for creating with more fields, over the board"
---

<!-- field: description -->
T09 built quick-add, with Shift+Enter opening the whole create page and that page coming back to the board. The review asked for more: a side sheet over the board, holding the type's other fields, so that the board stays in view while they are filled in.

Building it means turning the create page's form into a component that both the route and a sheet can hold. Kept apart from T09 so that quick-add could land first.
