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
