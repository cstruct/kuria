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
