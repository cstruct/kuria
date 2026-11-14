---
number: PLAN-003
type: plan
revision: 25
rank: b0Q
state: done
title: Contacts and Checking for Others
---

<!-- field: goal -->
After this milestone:

- A tenant keeps **contacts**: people it names by name and email before (or
  without) them being users. When someone signs in to the tenant with that
  email, verified, the contact becomes linked to their subject, and every
  value that named the contact shows them as that user.
- A `user` field names a user or a contact, so "Release manager: Bob" works
  before Bob has an account.
- Any `checkbox` can be checked for someone else: the record keeps who
  checked it and when, from the change, and who it was checked for, a user
  or a contact, from the input.

Success is the release walk-through releasing with a go checked by the
release manager for a contact, and that contact, later signing in with a
verified email, shown as the user they became, in GraphQL and in drafts.

<!-- field: design -->
## Constraints And Assumptions

- Builds on the field kinds of `docs/spec/akter.md` as they are after the
  field-kinds milestone (`a9ccdd6`): a `user` value is a subject id string;
  a checkbox takes `true` or `false` and stores
  `{ "checked_by", "checked_at" }`; the server supplies the `Change` (who,
  when) the engine records.
- Revisions are append only and hashed. A value that named a contact keeps
  naming the contact after it is linked; linking changes how it is shown,
  never what was stored.
- `person` (`docs/spec/people.md`) stays what Kuria recorded about a
  subject at browser sign-in, not tenant-scoped. Contacts are per tenant.
- Konklav's userinfo serves `email` with `email_verified`
  (`oidc.provider.claims` in Konklav's federation spec). Only a verified
  email links a contact; an unverified one could claim someone's approvals.
- Existing stored values must still read: a plain subject id string stays a
  valid `user` value, and a checkbox record without `for` is for its
  checker.
- `kuria-engine` stays free of I/O: the server checks that a contact exists
  in the tenant, as it does for attachments, and hands the engine names to
  show.

## Design

### Terms

- `contact`: a person a tenant names in `user` fields and checkboxes who
  may not be a user: a name and an email, linked to the subject who first
  signs in to the tenant with that email verified. Added to
  `docs/nomenclature.md` before use.
- In GraphQL, a value names `Someone`: a union of `User` and `Contact`.

### Values

| Kind | Input | Stored |
|---|---|---|
| `user` | a subject id, or `{ "contact": id }` | the same |
| `checkbox` | `true`, `false`, or `{ "for": <a user value> }` | `{ "checked_by", "checked_at", "for"? }` |

- `for` naming the checker is the same as `true`, and isn't stored.
- Checking a checked box still changes nothing, whoever it is for; to
  check it for someone else, clear it first. A record's `for` is part of
  the record that stays until the box is cleared.
- The engine gets `Someone::{User(Uuid), Contact(Uuid)}`;
  `Value::User(Someone)` and `Value::Checkbox { by, at, on_behalf_of:
  Option<Someone> }`.
- The write path refuses a contact that isn't the tenant's, for every
  changed value that names one (as `hold_attachments` does).

### Contacts

- Table `contact`: id, tenant, name, email (optional), linked subject
  (optional) and when, created by and when. One contact per email per
  tenant, compared case-insensitively.
- GraphQL: `contacts` (query), `addContact`, `editContact` (name and email,
  only while unlinked), `eraseContact`.
- Actions, tenant-wide: `kuria:contacts:read` to list contacts and see
  their emails; `kuria:contacts:manage` to add, edit and erase them.
  Anyone who may read an akt sees the name of a contact its values name;
  values must show.
- Shown by: the linked user's recorded name, else the contact's name, else
  its email, else its id (as `person.name` falls back).

### Linking

- At a browser sign-in to a tenant, if userinfo says `email_verified` and
  gives an email, the tenant's unlinked contact with that email is linked
  to the subject. A contact once linked stays linked.
- Only at sign-in: then the subject is known to be a member of the tenant
  and the email is known to be verified.

### Erasure

- Erasing a subject (Konklav's erasure) also deletes the name and email of
  every contact linked to them; the contact stays, showing the subject's
  id.
- `eraseContact` deletes an unlinked contact's name and email; values that
  name it show its id.

### Showing

- `UserValue { field user: Someone }`; `CheckboxValue { field checked by:
  User at for: Someone }`, `for` the checker when none was named.
- Drafts (`draft.render`): `user` gives the name; `checkbox` gives `yes`
  when checked for its checker, and `yes, for Bob (checked by Alice)` when
  checked for someone else. `Value::display` takes the names the server
  looked up, so the engine needs no I/O.

<!-- field: verification -->
- `mise run test`; contacts and values through GraphQL against a fresh
  database, linking through the fake OpenID provider in
  `items/server/tests/support/konklav.rs` with verified and unverified
  emails.
- `tracey query untested` shows only `auth.service-principal`.
