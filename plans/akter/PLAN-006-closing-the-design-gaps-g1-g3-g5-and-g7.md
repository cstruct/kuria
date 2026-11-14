---
number: PLAN-006
type: plan
revision: 13
rank: b1p
state: done
title: "Closing the design gaps G1, G3, G5 and G7"
---

<!-- field: goal -->
- **G5:** an akt's history names the users and contacts its changes set,
  rather than "a user".
- **G1:** dropping a card on another board column offers the transitions
  that lead there, instead of only refusing.
- **G3:** akt types can carry an icon and a colour, shown wherever the type
  is: the sidebar, Home, an akt's header, lists.
- **G7:** a contact can be linked to a user by hand, by someone who manages
  contacts. Password sign-ins never link one, since Konklav reports a
  password's email as unverified and nothing delivers Konklav's emails to
  verify one (chosen by the user over email-link sign-in or a Konklav
  change).

<!-- field: design -->
## Design

### G5: names in history

`AktRevision.named: [Someone!]!` lists the users and contacts that a
revision's `changes` name: user values, and a checkbox's `by` and `for`.
They are resolved on the server, so names are shown only to those who may
read the akt, as `contact.show` and `person.name` already require. There is
no lookup of a user by id. `history.ts` shows each id by the name `named`
gives.

### G1: dropping on another column

A column's `states` (in the container's document) say which states of each
type fall in it. Dropping a card on another column finds the transitions
from the card's state whose `to` falls in that column (`app.board-drop`).
- **None:** the column refuses the drop, as now.
- **Otherwise:** a dialog lists them. A transition that needs no fields is
  performed there, against the member's revision. One that does opens the
  member's page with `?transition=<key>`, which opens its dialog.

Board members gain `revision`, and their type gains `transitions`.

### G3: icons and colours

`presentation.icon` names one of a fixed set of icons, and
`presentation.colour` one of a fixed palette. Both are enums in GraphQL
(`AktTypeIcon`, `AktTypeColour`), so the app's map from names to icons and
tokens can't miss one. The defaults are no icon and neutral. Each colour is
a token pair, checked for contrast in light and dark like the tones.
- **Shown in:** the sidebar, Home's type cards and recent list, an akt's
  header, and the type's list page.
- **Example:** the example's documents get icons and colours.

### G7: linking by hand

- **Who is a user here:** people are recorded globally, so Kuria now also
  records which tenant each subject signed in to: `tenant_person`, written
  at browser sign-in and backfilled from current sessions.
- **Listing them:** `users` lists the people who signed in to the caller's
  tenant, by name. It requires `kuria:contacts:read` or `manage`, like
  `contacts` (`person.list`).
- **Linking:** `linkContact(contact, user)` requires
  `kuria:contacts:manage`. It links an unlinked contact to a user who has
  signed in to the tenant (`contact.link-by-hand`); an already linked
  contact is refused as `LINKED`, an unknown user as `UNKNOWN_USER`.
- **Erasure:** erasing a subject also forgets their `tenant_person` rows.
- **The app:** the contacts page offers "Link to a user" with a picker.
  User pickers in akt fields offer the tenant's users too, not only "Me"
  and contacts.

<!-- akt: PLAN-006-T01 -->
---
type: task
revision: 3
rank: b1q
state: done
in: [PLAN-006]
title: T1 Spec
---

<!-- field: description -->
Spec: `app.history` (names), `app.board-drop`,
`akt.type.presentation` (icon, colour), `person.list`,
`contact.link-by-hand`; nomenclature.

<!-- akt: PLAN-006-T02 -->
---
type: task
revision: 3
rank: b1r
state: done
in: [PLAN-006]
title: T2 G5
---

<!-- field: description -->
G5: `AktRevision.named`; history names.

<!-- akt: PLAN-006-T03 -->
---
type: task
revision: 3
rank: b1s
state: done
in: [PLAN-006]
title: T3 G1
---

<!-- field: description -->
G1: board drop with transitions; `?transition=`.

<!-- akt: PLAN-006-T04 -->
---
type: task
revision: 3
rank: b1t
state: done
in: [PLAN-006]
title: T4 G3
---

<!-- field: description -->
G3: icon and colour in config, GraphQL and app; the example.

<!-- akt: PLAN-006-T05 -->
---
type: task
revision: 3
rank: b1u
state: done
in: [PLAN-006]
title: T5 G7
---

<!-- field: description -->
G7: `tenant_person`, `users`, `linkContact`; the contacts page
and user pickers.

<!-- akt: PLAN-006-T06 -->
---
type: task
revision: 3
rank: b1v
state: done
in: [PLAN-006]
title: "T6 Tests (server, unit, end-to-end, axe), docs, the gaps closed in the earlier plans"
---

<!-- field: description -->
Tests (server, unit, end-to-end, axe), docs, the gaps closed in
the earlier plans.

<!-- akt: PLAN-006-D01 -->
---
type: decision
revision: 2
rank: b1w
state: decided
in: [PLAN-006]
title: "Linking by hand needs `kuria:contacts:manage`"
---

<!-- field: context -->
Recorded in the plan "Closing the design gaps G1, G3, G5 and G7", migrated from `.agents/plans/design-gaps.md`.

<!-- field: decision -->
**Linking by hand needs `kuria:contacts:manage`**, not a new permission:
whoever may add, edit and erase contacts may also say who one is.

<!-- field: why -->
, not a new permission:
whoever may add, edit and erase contacts may also say who one is.

<!-- akt: PLAN-006-D02 -->
---
type: decision
revision: 2
rank: b1x
state: decided
in: [PLAN-006]
title: "Lucide's `triangle-alert`"
---

<!-- field: context -->
Recorded in the plan "Closing the design gaps G1, G3, G5 and G7", migrated from `.agents/plans/design-gaps.md`.

<!-- field: decision -->
**Lucide's `triangle-alert`** is imported by its `alert-triangle` alias,
the name this version of `@lucide/svelte` ships it under.

<!-- field: why -->
is imported by its `alert-triangle` alias,
the name this version of `@lucide/svelte` ships it under.

<!-- akt: PLAN-006-D03 -->
---
type: decision
revision: 2
rank: b1y
state: decided
in: [PLAN-006]
title: Users are listed to readers of contacts only
---

<!-- field: context -->
Recorded in the plan "Closing the design gaps G1, G3, G5 and G7", migrated from `.agents/plans/design-gaps.md`.

<!-- field: decision -->
**Users are listed to readers of contacts only**, like contacts, since
the list says who uses the tenant.

<!-- field: why -->
, like contacts, since
the list says who uses the tenant.

<!-- akt: PLAN-006-D04 -->
---
type: decision
revision: 2
rank: b1z
state: decided
in: [PLAN-006]
title: Names in history come with the revision
---

<!-- field: context -->
Recorded in the plan "Closing the design gaps G1, G3, G5 and G7", migrated from `.agents/plans/design-gaps.md`.

<!-- field: decision -->
**Names in history come with the revision**, not from a user lookup by
id, because people are recorded across tenants.

<!-- field: why -->
, not from a user lookup by
id, because people are recorded across tenants.
