---
number: PLAN-005
type: plan
revision: 34
rank: b1J
state: done
title: A Usable Design for the App
---

<!-- field: goal -->
After this milestone, someone working the release-management example can:

- tell where they are and get anywhere from a sidebar: akt types,
  contacts, and search (also with a keyboard shortcut);
- open an akt and see, at the top, its type, number, **title** and state as
  a badge, with the transitions it can take as buttons, and its details
  (people, dates, containers) beside the content;
- edit one field in place, without turning the page into a form;
- see a container's members on a board whose cards show the title and the
  fields the akt type chooses, with members the board doesn't map shown as
  their own lists (a release's notes, a candidate's artifacts), not lumped
  into "Unmapped";
- add, remove and transfer members by picking akter by number or title,
  and move them by dragging or from the keyboard;
- read drafted text such as release notes rendered, and a history that
  says what changed, which memberships included;
- use it in light or dark mode, at laptop and phone widths.

Success is the release walk-through (`e2e/tests/release-flow.spec.ts`)
passing on the new design, an automated accessibility check passing on
every main page, and screenshots of the main pages in the app's README.

<!-- field: design -->
## Where It Starts

Screenshots taken after a walked-through release (2026-11-12) showed:

1. No visual design: browser defaults, full-width grey buttons, links
   everywhere, every section the same weight.
2. The akt page heads with the number, not the title; the state is a line
   of text; transitions sit below the fields.
3. Containment is managed by typing numbers into always-visible forms
   ("Container number", "From / To container number", "Member number").
4. Boards put unmapped member types (a release's backlog and notes, a
   candidate's artifacts) into "Unmapped"; every card shows ↑ ↓ ✕; long
   values overflow.
5. The home page is a bullet list of akt types; lists by type show numbers
   and states without titles; there is no search.
6. Multiline text such as release notes shows Markdown unrendered; the
   history says "changed memberships" without detail.
7. Editing is all-or-nothing: one form for every field, with an easily
   missed reason box.

The app can only guess a title or card fields today: nothing in an akt
type says which they are.

## Constraints And Assumptions

- The app builds every view from akt types and names none
  (`app.akt-types`); presentation hints come from the akt type document,
  with defaults when it has none.
- The server is the browser's backend; the app talks only GraphQL and the
  attachment endpoints, on its own origin, under a strict CSP: no inline
  scripts or styles beyond the bootstrap hash, no third-party origins. So
  fonts and icons are bundled, and Markdown is rendered to sanitized HTML
  in the app.
- Svelte 5 (runes), SvelteKit 3 in SPA mode, TypeScript; rules the app
  implements are annotated in `.ts` modules, since Tracey doesn't read
  `.svelte` files.
- Every control keeps an accessible role and name (`app.accessible`); the
  end-to-end tests keep selecting by them.

## Design

### Foundation

- **Design tokens** in `src/lib/design/tokens.css`: colour (neutral scale,
  an accent, and tones for states: neutral, active, success, warning,
  danger), each for light and dark (`prefers-color-scheme`, overridable);
  spacing, radius, shadow, a type scale, and a monospace for numbers and
  digests. Every component uses tokens, none uses raw colours.
- **Bits UI** (2.x, headless, accessible) for the primitives that are hard
  to get right: menu, dialog, popover, combobox, tooltip, tabs. Kuria's
  own components wrap them in `src/lib/ui`: `Button` (primary, secondary,
  ghost, danger), `IconButton`, `Badge`, `Menu`, `Dialog`, `Combobox`,
  `Card`, `Field` (label, hint, error), `EmptyState`, `Toast`.
- **Icons**: `@lucide/svelte`, bundled, imported per icon.
- **Text**: the system font stack; numbers, SHAs and digests in monospace
  and truncated in the middle with the whole value on hover and copy.

### Presentation hints

An optional `presentation` in the akt type document:

```json
"presentation": {
  "title": "title",
  "card": ["kind", "assignee"],
  "list": ["kind", "assignee"],
  "tones": { "rejected": "danger", "accepted": "success", "in_progress": "active" }
}
```

- `title`: a single-line `text` field shown as the akt's title.
- `card`: up to four fields shown on board cards, after the title.
- `list`: fields shown as columns when the type's akter are listed.
- `tones`: a tone per state key, for badges and board columns; states not
  named are neutral, and final states without a tone are muted.

Validated with the document: fields must exist and fit (the title a
single-line text field), states must exist. Without `presentation`, the
title is the first required single-line text field, cards show none, and
lists show the state only. GraphQL gains `AktType.presentation` and
`Akt.title`.

### Shell

- A sidebar: the tenant's akt types (with counts), Contacts, and search;
  collapsible to icons, a drawer on narrow screens.
- A top bar: breadcrumbs from the akt's containers, search (`/` or ⌘K),
  and the viewer with "Sign out".
- Search: by number prefix or words of the title, across the akt types the
  viewer reads, as a server query (`akter(search:)`).

### The akt page

- **Header**: type, number (copyable), title, state badge; the transitions
  its state allows as buttons, the first as primary; a "⋯" menu for less
  common actions (draft a field, add to a container, transfer).
- **Main column**: fields with inline editing (click or Enter on a field,
  its editor in place, Save/Cancel, optional reason); multiline text
  rendered as Markdown; then members.
- **Side column**: details: people fields, dates, references and
  artifacts' git and image values; "In" (containers) with add, remove and
  transfer through pickers.
- **Transitions** open a dialog asking only for the fields they require
  and a reason; refusals show in the dialog by field.
- **History**: a timeline, newest first, with relative times (absolute on
  hover), author names, transitions, field changes with old and new
  values, and memberships named ("added to RELEASE-2026-0001-RC-0002").

### Members, boards and lists

- A container whose akt type has a board shows it for the member types its
  columns name; every other member type gets its own compact list
  ("Artifacts", "Release notes"), not "Unmapped". "Unmapped" is kept only
  for members of mapped types in states no column names.
- Cards show the title, the presentation's card fields, the number and the
  state badge; actions (move up/down, remove) in a "⋯" menu. Dragging
  moves within and between columns (between columns only to show; a
  column change isn't a transition); the menu is the keyboard path.
- "Add member" and "New … here" become a single "Add" menu: create a new
  member of a contained type, or pick an existing akt.
- Lists by type: a table of number, title, the presentation's list fields,
  state and updated; filter by state; sort by rank or updated.

### Home

- Recently changed akter the viewer can read, and per-type cards with
  counts and "New".

<!-- field: verification -->
- `mise run lint`, `mise run test`, `mise run e2e`.
- Vitest for presentation defaults, Markdown sanitizing, search parsing
  and the components' logic in `.ts` modules.
- Playwright: the walk-through, axe on each main page in light and dark,
  and a narrow-viewport pass of the shell.
- Screenshots of the main pages at 1440 and 390 wide, compared by eye
  against this plan before each commit that changes them.

<!-- akt: PLAN-005-T01 -->
---
type: task
revision: 3
rank: b1K
state: done
in: [PLAN-005]
title: T1 Spec
---

<!-- field: description -->
Spec: `akt.type.presentation` (the hints, their validation, the
default title, neutral tones), `akt.title`, `akt.search` and a line in
`akt.type.validation` in `docs/spec/akter.md`; in
`docs/spec/frontend.md` `app.header`, `app.inline-edit`, `app.markdown`,
`app.members`, `app.pickers`, `app.search`, `app.history`, `app.themes`
(WCAG 2.2 AA contrast) and `app.narrow` (390 pixels); the terms
presentation, title and tone (`1ecd733`).

<!-- akt: PLAN-005-T02 -->
---
type: task
revision: 3
rank: b1L
state: done
in: [PLAN-005]
title: T2 Config and server
---

<!-- field: description -->
Config and server: `PresentationDef` and `Tone` in the akt type
document, validated (`presentation.*` problems); `AktType::title_field`
and `tone`; migration 8 adds `akt.title`, backfilled from each version's
title field, and an index on the lowercased number; the write path keeps
the title (`title_of`); `store::search_akter`; GraphQL `Akt.title`,
`AktType.presentation`, `State.tone` and `akter(search:)` (`5940db8`).

<!-- akt: PLAN-005-T03 -->
---
type: task
revision: 3
rank: b1M
state: done
in: [PLAN-005]
title: T3 Foundation
---

<!-- field: description -->
Foundation: `src/lib/design/tokens.css` and `base.css` (light,
dark by system preference or `data-theme`), `theme.ts` and a contrast
test over every pair used together; Bits UI 2.21 and `@lucide/svelte`;
`src/lib/ui` (Button, IconButton, Badge, Card, Field, EmptyState, Mono,
Menu, Dialog, Combobox, Toaster); `/design`, a gallery of them all,
driven in Chromium in both themes with no CSP violations (`c5f6564`).

<!-- akt: PLAN-005-T04 -->
---
type: task
revision: 3
rank: b1N
state: done
in: [PLAN-005]
title: T4 Shell
---

<!-- field: description -->
Shell: `src/lib/shell` (sidebar with type counts, top bar with
breadcrumbs, viewer and Sign out, a drawer below 900 pixels, search with
`/` and Cmd-K in a Bits UI command dialog); server `AktType.akterCount`
and `viewer.user`; `login.sign-out` (signing out ends the login page's
Konklav session too); `e2e/tests/shell.spec.ts`; per-test client
addresses in the end-to-end tests (`8d475a9`).

<!-- akt: PLAN-005-T05 -->
---
type: task
revision: 3
rank: b1O
state: done
in: [PLAN-005]
title: T5 The akt page
---

<!-- field: description -->
The akt page: a header with the type, the number (copyable), the
title as the heading, the state's tone and the transitions as buttons
(the first primary), each opening a dialog for the fields it requires;
Details with one field edited in place (`FieldRow`, `oneField`),
drafting from the field's row, and multiline text as sanitized Markdown
(`marked` and DOMPurify); a side column with containers added, removed
and moved through pickers, and the history as a timeline; members added
through a picker and "New" in a menu. Refusals show in the open editor
or dialog, else as a toast. `e2e/tests/akt-page.spec.ts`; the helpers
and walk-through use the pickers and dialogs.

<!-- akt: PLAN-005-T06 -->
---
type: task
revision: 3
rank: b1P
state: done
in: [PLAN-005]
title: "T6 Members, boards and lists"
---

<!-- field: description -->
Members, boards and lists: `members.ts` regroups the board's
`unmapped` column into a list per akt type no column names, keeping
"Unmapped" only for mapped types in states no column names (and, with
no columns, listing every member by type); cards show the title (the
number when untitled), the presentation's card fields, the number and
state, with a "⋯" menu (up, down, remove); lists are tables with the
presentation's list fields. Dragging reorders within a column or list
(`placeFor`, `moveAkt`); other columns refuse the drop. One "Add" menu
creates a member of each contained type or opens a picker dialog for an
existing akt. Akter by type is a table filtered by state and sorted by
rank or recently changed, in the URL. The top bar's crumbs shorten
instead of overlapping at 390 pixels. `e2e/tests/members.spec.ts`.

<!-- akt: PLAN-005-T07 -->
---
type: task
revision: 3
rank: b1Q
state: done
in: [PLAN-005]
title: T7 Home and the example
---

<!-- field: description -->
Home and the example: `akt.list` (akter listed most recently
changed first, with the `akt_recent` index); Home shows the ten akter
changed last (title, type, number, state, when) and a card per akt type
with its count and "New" (`app.home`). The example's documents name card
and list fields and state tones (the backlog keeps the defaults).
`e2e/tests/home.spec.ts`; the members test checks a card's fields and a
tone.

<!-- akt: PLAN-005-T08 -->
---
type: task
revision: 3
rank: b1R
state: done
in: [PLAN-005]
title: T8 Tests and docs
---

<!-- field: description -->
Tests and docs: `e2e/tests/accessibility.spec.ts` runs axe
(`@axe-core/playwright` 4.14) against WCAG 2.2 A and AA on every main
page, a menu, a dialog and an editor, in light and dark, and at 390
pixels with the drawer. It found three problems, now fixed: a menu
whose name didn't contain its visible text (the gallery), a field row
whose edit buttons and editor sat outside its `dt` and `dd`, and a board
that scrolls sideways but couldn't be reached from the keyboard when its
columns are empty (now a focusable region). `mise run e2e:screenshots`
takes the app README's screenshots; the architecture's frontend section
describes presentation, the look, the pages and accessibility.

<!-- akt: PLAN-005-G01 -->
---
type: gap
revision: 2
rank: b1S
state: closed
in: [PLAN-005]
title: "G1 Drag between board columns doesn't change state; whether it should offer the transition that leads…"
---

<!-- field: description -->
Drag between board columns doesn't change state; whether it
should offer the transition that leads there is a later decision.
Closed: a drop offers the transitions into that column
(`app.board-drop`; [design-gaps.md](design-gaps.md)).

<!-- field: resolution -->
Drag between board columns doesn't change state; whether it
should offer the transition that leads there is a later decision.
Closed: a drop offers the transitions into that column
(`app.board-drop`; [design-gaps.md](design-gaps.md)).

<!-- akt: PLAN-005-G02 -->
---
type: gap
revision: 3
rank: b1T
state: closed
in: [PLAN-005]
links: [Resolved by PLAN-007-T03]
title: "G2 (now `app.my-work`, T3 of [cli-and-dogfooding.md](cli-and-dogfooding.md)) \"My work\" (akter naming t…"
---

<!-- field: description -->
(now `app.my-work`, T3 of [cli-and-dogfooding.md](cli-and-dogfooding.md)) "My work" (akter naming the viewer in a user field) needs a server
query by user field; left for after this milestone.

<!-- field: resolution -->
Built as `app.my-work` in PLAN-007-T03: Home lists the akter whose user fields or checkboxes name the viewer.

<!-- akt: PLAN-005-G03 -->
---
type: gap
revision: 1
rank: b1U
state: open
in: [PLAN-005]
title: "G4 Once, in about eight end-to-end runs, a newly created akt's page stayed on \"Loading…\" in the walk-t…"
---

<!-- field: description -->
Once, in about eight end-to-end runs, a newly created akt's page
stayed on "Loading…" in the walk-through (the second release candidate);
not reproduced since. Suspect a Graphcache partial result (`akt: null`,
stale) whose network answer didn't reach the page. The page now says
"There is no akt" once loading is over, so it can't hang silently. Watch
for it; capture the trace if it recurs.

<!-- akt: PLAN-005-G04 -->
---
type: gap
revision: 2
rank: b1V
state: closed
in: [PLAN-005]
title: "G5 The history names users \"a user\""
---

<!-- field: description -->
The history names users "a user": revisions hold subject ids,
and the app has no query for a user by id. A `User` lookup, or names in
`AktRevision.changes`, would show who. Closed: `AktRevision.named`
([design-gaps.md](design-gaps.md)).

<!-- field: resolution -->
The history names users "a user": revisions hold subject ids,
and the app has no query for a user by id. A `User` lookup, or names in
`AktRevision.changes`, would show who. Closed: `AktRevision.named`
([design-gaps.md](design-gaps.md)).

<!-- akt: PLAN-005-G05 -->
---
type: gap
revision: 2
rank: b1W
state: closed
in: [PLAN-005]
title: "G3 Akt types' icons or colours are not part of `presentation` yet"
---

<!-- field: description -->
Akt types' icons or colours are not part of `presentation` yet.
Closed: `presentation.icon` and `colour`, shown by `TypeMark`
([design-gaps.md](design-gaps.md)).

<!-- field: resolution -->
Akt types' icons or colours are not part of `presentation` yet.
Closed: `presentation.icon` and `colour`, shown by `TypeMark`
([design-gaps.md](design-gaps.md)).

<!-- akt: PLAN-005-D01 -->
---
type: decision
revision: 2
rank: b1X
state: decided
in: [PLAN-005]
title: Own tokens with Bits UI
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Own tokens with Bits UI**, not shadcn-svelte or plain CSS: Kuria keeps
its look and few dependencies, and borrows only the accessible
primitives that are hard to write (chosen by the user).

<!-- field: why -->
, not shadcn-svelte or plain CSS: Kuria keeps
its look and few dependencies, and borrows only the accessible
primitives that are hard to write (chosen by the user).

<!-- akt: PLAN-005-D02 -->
---
type: decision
revision: 2
rank: b1Y
state: decided
in: [PLAN-005]
title: Presentation in the akt type document
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Presentation in the akt type document**, not heuristics alone: types
say which field is the title and what cards and lists show; the app
still has defaults (chosen by the user).

<!-- field: why -->
, not heuristics alone: types
say which field is the title and what cards and lists show; the app
still has defaults (chosen by the user).

<!-- akt: PLAN-005-D03 -->
---
type: decision
revision: 2
rank: b1Z
state: decided
in: [PLAN-005]
title: Straight to a plan
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Straight to a plan**, reviewed in the running app rather than in
mockups (chosen by the user).

<!-- field: why -->
, reviewed in the running app rather than in
mockups (chosen by the user).

<!-- akt: PLAN-005-D04 -->
---
type: decision
revision: 2
rank: b1a
state: decided
in: [PLAN-005]
title: "The server's board keeps its `unmapped` column"
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**The server's board keeps its `unmapped` column** (`board.unmapped`);
the app regroups members of akt types no column names into lists by
type (`app.members`), so other clients still get every member (T1).

<!-- field: why -->
(`board.unmapped`);
the app regroups members of akt types no column names into lists by
type (`app.members`), so other clients still get every member (T1).

<!-- akt: PLAN-005-D05 -->
---
type: decision
revision: 2
rank: b1b
state: decided
in: [PLAN-005]
title: Search matches a number prefix or every word of the title
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Search matches a number prefix or every word of the title**, most
recently changed first, over what the subject may read (T1).

<!-- field: why -->
, most
recently changed first, over what the subject may read (T1).

<!-- akt: PLAN-005-D06 -->
---
type: decision
revision: 2
rank: b1c
state: decided
in: [PLAN-005]
title: Screenshots are checked in
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Screenshots are checked in** (about 1 MB), taken by a script from the
walked-through release so they can be retaken after a change (T8).

<!-- field: why -->
(about 1 MB), taken by a script from the
walked-through release so they can be retaken after a change (T8).

<!-- akt: PLAN-005-D07 -->
---
type: decision
revision: 2
rank: b1d
state: decided
in: [PLAN-005]
title: Lists are most recently changed first
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Lists are most recently changed first** (`akt.list`), not newest
created, so Home needs no query of its own (T7).

<!-- field: why -->
(`akt.list`), not newest
created, so Home needs no query of its own (T7).

<!-- akt: PLAN-005-D08 -->
---
type: decision
revision: 2
rank: b1e
state: decided
in: [PLAN-005]
title: Sorting by rank is done in the app
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Sorting by rank is done in the app** over the 200 akter a type's list
reads (the server lists them most recently changed first); enough until
lists page (T6).

<!-- field: why -->
over the 200 akter a type's list
reads (the server lists them most recently changed first); enough until
lists page (T6).

<!-- akt: PLAN-005-D09 -->
---
type: decision
revision: 2
rank: b1f
state: decided
in: [PLAN-005]
title: A drop on another column is refused
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**A drop on another column is refused**, not turned into a transition
(G1); the column dims and says a transition moves it (T6).

<!-- field: why -->
, not turned into a transition
(G1); the column dims and says a transition moves it (T6).

<!-- akt: PLAN-005-D10 -->
---
type: decision
revision: 2
rank: b1g
state: decided
in: [PLAN-005]
title: Titled links on untitled cards
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Titled links on untitled cards** fall back to the number, named in
full for assistive technology though shortened on screen (T6).

<!-- field: why -->
fall back to the number, named in
full for assistive technology though shortened on screen (T6).

<!-- akt: PLAN-005-D11 -->
---
type: decision
revision: 2
rank: b1h
state: decided
in: [PLAN-005]
title: "Titles are stored on `akt`"
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Titles are stored on `akt`**, kept by the write path and backfilled by
the migration, since the title field differs per akt type and search
must look across them; revisions are unchanged (T2).

<!-- field: why -->
, kept by the write path and backfilled by
the migration, since the title field differs per akt type and search
must look across them; revisions are unchanged (T2).

<!-- akt: PLAN-005-D12 -->
---
type: decision
revision: 2
rank: b1i
state: decided
in: [PLAN-005]
title: "Title search isn't indexed yet"
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Title search isn't indexed yet**: `like '%word%'` over the tenant's
akter, fine at the example's size; a trigram or full-text index can come
when tenants grow (T2).

<!-- field: why -->
`like '%word%'` over the tenant's
akter, fine at the example's size; a trigram or full-text index can come
when tenants grow (T2).

<!-- akt: PLAN-005-D13 -->
---
type: decision
revision: 2
rank: b1j
state: decided
in: [PLAN-005]
title: "SvelteKit's route announcer is allowed by the hash of its style attribute (`style-src-attr 'unsafe-…"
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**SvelteKit's route announcer is allowed by the hash of its style
attribute** (`style-src-attr 'unsafe-hashes'`), the only inline style;
the app's stylesheet also hides it. Bits UI positions popups through
the CSSOM, which the CSP allows (T3).

<!-- field: why -->
**SvelteKit's route announcer is allowed by the hash of its style
attribute** (`style-src-attr 'unsafe-hashes'`), the only inline style;
the app's stylesheet also hides it. Bits UI positions popups through
the CSSOM, which the CSP allows (T3).

<!-- akt: PLAN-005-D14 -->
---
type: decision
revision: 2
rank: b1k
state: decided
in: [PLAN-005]
title: "`/design` stays in the app"
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**`/design` stays in the app**: a page of every component, for checking
them by eye and, in T8, with axe (T3).

<!-- field: why -->
a page of every component, for checking
them by eye and, in T8, with axe (T3).

<!-- akt: PLAN-005-D15 -->
---
type: decision
revision: 2
rank: b1l
state: decided
in: [PLAN-005]
title: Signing out signs out of the login page too
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Signing out signs out of the login page too** (`login.sign-out`):
Kuria is the login application here, so leaving its Konklav session
would sign the person straight back in (T4).

<!-- field: why -->
(`login.sign-out`):
Kuria is the login application here, so leaving its Konklav session
would sign the person straight back in (T4).

<!-- akt: PLAN-005-D16 -->
---
type: decision
revision: 2
rank: b1m
state: decided
in: [PLAN-005]
title: The sidebar becomes a drawer below 900 pixels
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**The sidebar becomes a drawer below 900 pixels**, decided in
`shell.ts` with a media query, not CSS alone, so it is one component
either way (T4).

<!-- field: why -->
, decided in
`shell.ts` with a media query, not CSS alone, so it is one component
either way (T4).

<!-- akt: PLAN-005-D17 -->
---
type: decision
revision: 2
rank: b1n
state: decided
in: [PLAN-005]
title: Transitions open a dialog even with nothing to ask
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Transitions open a dialog even with nothing to ask**, so a reason can
be given and nothing changes state by a stray click (T5).

<!-- field: why -->
, so a reason can
be given and nothing changes state by a stray click (T5).

<!-- akt: PLAN-005-D18 -->
---
type: decision
revision: 2
rank: b1o
state: decided
in: [PLAN-005]
title: Markdown rendered in the app and sanitized
---

<!-- field: context -->
Recorded in the plan "A Usable Design for the App", migrated from `.agents/plans/usable-design.md`.

<!-- field: decision -->
**Markdown rendered in the app and sanitized**: drafts produce Markdown,
and the CSP allows no other way to render it.

<!-- field: why -->
drafts produce Markdown,
and the CSP allows no other way to render it.
