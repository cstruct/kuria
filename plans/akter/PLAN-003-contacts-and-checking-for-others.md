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

<!-- akt: PLAN-003-T01 -->
---
type: task
revision: 3
rank: b0R
state: done
in: [PLAN-003]
title: T1 Spec and nomenclature
---

<!-- field: description -->
Spec and nomenclature: `docs/spec/contacts.md` (10 rules:
`contact.add`, `contact.unique-email`, `contact.edit`, `contact.list`,
`contact.reference`, `contact.show`, `contact.values`, `contact.link`,
`contact.erase`, `contact.erasure`); `user` values naming contacts in
`akt.field.kinds`; checking for someone in `akt.field.checkbox`; both in
`draft.render`; `kuria:contacts:read` and `kuria:contacts:manage` in
`authz.catalog`; the terms contact and checked for (`d323315`).

<!-- akt: PLAN-003-T02 -->
---
type: task
revision: 3
rank: b0S
state: done
in: [PLAN-003]
title: T2 Contacts
---

<!-- field: description -->
Contacts: migration 7 (`contact`, a unique index on the tenant and
`lower(email)`), `kuria_store::contacts`, GraphQL `Contact`
(`displayName`, `name`, `email` only to those who may list contacts,
`user`, `linkedAt`, `createdBy`, `createdAt`), `contacts`, `addContact`,
`editContact`, `eraseContact`; `Permission::ReadContacts` and
`ManageContacts`; codes `INVALID_CONTACT`, `DUPLICATE_EMAIL`, `LINKED`,
`NOT_FOUND`, `FORBIDDEN` (`fd66166`).

<!-- akt: PLAN-003-T03 -->
---
type: task
revision: 3
rank: b0T
state: done
in: [PLAN-003]
title: T3 Values naming someone
---

<!-- field: description -->
Values naming someone: `kuria_engine::Someone`; `Value::User(Someone)`
and `Value::Checkbox { on_behalf_of }`, stored values from before still
reading; `check_contacts` in the write path; GraphQL `Someone` union for
`UserValue.user` and `CheckboxValue.for`, `Contact` loaded lazily when a
value names it; `someone_name` shared by GraphQL and drafts (`fc0ecc7`).

<!-- akt: PLAN-003-T04 -->
---
type: task
revision: 3
rank: b0U
state: done
in: [PLAN-003]
title: T4 Linking and erasure
---

<!-- field: description -->
Linking and erasure: `UserInfo.email_verified`; `record_person`
links the tenant's unlinked contact with the verified email
(`contacts::link`, case-insensitive), best effort like the name;
`erasure::erase_subject` deletes linked contacts' names and emails. The
fake OpenID provider's userinfo email and verification are settable
(`31c2f44`).

<!-- akt: PLAN-003-T05 -->
---
type: task
revision: 3
rank: b0V
state: done
in: [PLAN-003]
title: T5 Example and docs
---

<!-- field: description -->
Example and docs: the release manager adds the head of product as
a contact and checks go for them; once linked, the release shows the
user they became while its values stay. The release manager role gains
`kuria:contacts:manage`. READMEs, architecture (GraphQL, identity,
erasure, storage) and the requirements' coverage are current. The
walk-through links through the store; sign-in linking itself is tested
in `tests/browser_sessions.rs`.

<!-- akt: PLAN-003-G01 -->
---
type: gap
revision: 1
rank: b0W
state: open
in: [PLAN-003]
title: "G1 Contacts aren't deduplicated against users who already signed in"
---

<!-- field: description -->
Contacts aren't deduplicated against users who already signed in:
a contact added for someone who is already a user stays unlinked until
they next sign in.

<!-- akt: PLAN-003-G02 -->
---
type: gap
revision: 1
rank: b0X
state: open
in: [PLAN-003]
title: "G2 Changing a user's email at their identity provider doesn't move contact links; a link, once made, i…"
---

<!-- field: description -->
Changing a user's email at their identity provider doesn't move
contact links; a link, once made, is kept.

<!-- akt: PLAN-003-G03 -->
---
type: gap
revision: 1
rank: b0Y
state: open
in: [PLAN-003]
title: "G3 A contact linked to the wrong person (an email reused by someone else) can't be unlinked yet"
---

<!-- field: description -->
A contact linked to the wrong person (an email reused by someone
else) can't be unlinked yet.

<!-- akt: PLAN-003-D01 -->
---
type: decision
revision: 2
rank: b0Z
state: decided
in: [PLAN-003]
title: "Contacts, linked later"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Contacts, linked later**, over free-text names or contacts that stay
apart from users: the same Bob is one contact, and once he signs in his
past approvals show him as the user he became (chosen by the user).

<!-- field: why -->
, over free-text names or contacts that stay
apart from users: the same Bob is one contact, and once he signs in his
past approvals show him as the user he became (chosen by the user).

<!-- akt: PLAN-003-D02 -->
---
type: decision
revision: 2
rank: b0a
state: decided
in: [PLAN-003]
title: Every checkbox can be checked for someone else
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Every checkbox can be checked for someone else**, not only fields that
opt in; who checked it is always recorded beside whom it was for (chosen
by the user).

<!-- field: why -->
, not only fields that
opt in; who checked it is always recorded beside whom it was for (chosen
by the user).

<!-- akt: PLAN-003-D03 -->
---
type: decision
revision: 2
rank: b0b
state: decided
in: [PLAN-003]
title: "`user` fields take contacts too"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**`user` fields take contacts too**, the same way (chosen by the user).

<!-- field: why -->
, the same way (chosen by the user).

<!-- akt: PLAN-003-D04 -->
---
type: decision
revision: 2
rank: b0c
state: decided
in: [PLAN-003]
title: Only a verified email links
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Only a verified email links**, and only at sign-in to the tenant, so
an unverified or foreign account can't take over a contact's approvals.

<!-- field: why -->
, and only at sign-in to the tenant, so
an unverified or foreign account can't take over a contact's approvals.

<!-- akt: PLAN-003-D05 -->
---
type: decision
revision: 2
rank: b0d
state: decided
in: [PLAN-003]
title: "`eraseContact` works on linked contacts too"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**`eraseContact` works on linked contacts too**: it deletes the
contact's own name and email; a linked one is shown by the user's
recorded name anyway, which erasing the subject deletes (T1).

<!-- field: why -->
it deletes the
contact's own name and email; a linked one is shown by the user's
recorded name anyway, which erasing the subject deletes (T1).

<!-- akt: PLAN-003-D06 -->
---
type: decision
revision: 2
rank: b0e
state: decided
in: [PLAN-003]
title: "A contact needs a name; the email is optional"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**A contact needs a name; the email is optional**, so a contact without
an email can be named but never linked (T1).

<!-- field: why -->
, so a contact without
an email can be named but never linked (T1).

<!-- akt: PLAN-003-D07 -->
---
type: decision
revision: 2
rank: b0f
state: decided
in: [PLAN-003]
title: Erasing frees the email
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Erasing frees the email**: the unique index covers only contacts with
an email, so after `eraseContact` the address can be given to a new
contact (T2).

<!-- field: why -->
the unique index covers only contacts with
an email, so after `eraseContact` the address can be given to a new
contact (T2).

<!-- akt: PLAN-003-D08 -->
---
type: decision
revision: 2
rank: b0g
state: decided
in: [PLAN-003]
title: "A contact's email is `null` to those who can't list contacts"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**A contact's email is `null` to those who can't list contacts**, rather
than an error, so a query that shows a value naming a contact doesn't
fail for an akt reader (T2).

<!-- field: why -->
, rather
than an error, so a query that shows a value naming a contact doesn't
fail for an akt reader (T2).

<!-- akt: PLAN-003-D09 -->
---
type: decision
revision: 2
rank: b0h
state: decided
in: [PLAN-003]
title: "`UserValue.user` became the `Someone` union"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**`UserValue.user` became the `Someone` union**, so clients select
`... on User { … }` and `... on Contact { … }`; no client depended on it
yet (T3).

<!-- field: why -->
, so clients select
`... on User { … }` and `... on Contact { … }`; no client depended on it
yet (T3).

<!-- akt: PLAN-003-D10 -->
---
type: decision
revision: 2
rank: b0i
state: decided
in: [PLAN-003]
title: "`CheckboxValue.for` is always set"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**`CheckboxValue.for` is always set**: the checker when the box was
checked for themselves, so clients show one "for" without a fallback
(T3).

<!-- field: why -->
the checker when the box was
checked for themselves, so clients show one "for" without a fallback
(T3).

<!-- akt: PLAN-003-D11 -->
---
type: decision
revision: 2
rank: b0j
state: decided
in: [PLAN-003]
title: "Checking for oneself stores no `for`"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Checking for oneself stores no `for`**, so `true` and `{"for": <me>}`
give the same record and hash (T3).

<!-- field: why -->
, so `true` and `{"for": <me>}`
give the same record and hash (T3).

<!-- akt: PLAN-003-D12 -->
---
type: decision
revision: 2
rank: b0k
state: decided
in: [PLAN-003]
title: "Linking is best effort, like recording the name"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Linking is best effort, like recording the name**: a sign-in doesn't
fail because a contact couldn't be linked; the next sign-in tries again
(T4).

<!-- field: why -->
a sign-in doesn't
fail because a contact couldn't be linked; the next sign-in tries again
(T4).

<!-- akt: PLAN-003-D13 -->
---
type: decision
revision: 2
rank: b0l
state: decided
in: [PLAN-003]
title: "Only `email_verified: true` links"
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Only `email_verified: true` links**: a provider that doesn't say is
treated as unverified (T4).

<!-- field: why -->
a provider that doesn't say is
treated as unverified (T4).

<!-- akt: PLAN-003-D14 -->
---
type: decision
revision: 2
rank: b0m
state: decided
in: [PLAN-003]
title: Stored values never change when a contact links
---

<!-- field: context -->
Recorded in the plan "Contacts and Checking for Others", migrated from `.agents/plans/contacts-and-checking-for-others.md`.

<!-- field: decision -->
**Stored values never change when a contact links**: revisions stay
append only and their hashes stay valid; only how they are shown changes.

<!-- field: why -->
revisions stay
append only and their hashes stay valid; only how they are shown changes.
