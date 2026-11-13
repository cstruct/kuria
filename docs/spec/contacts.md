# Contacts

A tenant often has to name people who aren't Kuria users yet: an approver
from another department, a supplier's quality contact, a release manager
who hasn't signed in. A contact names such a person by name and email.
`user` fields and checkboxes can name a contact wherever they can name a
user, and once the person signs in with a verified email, or someone who
manages contacts links it by hand, the contact is linked to them and they
are shown as the user they became.

## Keeping contacts

r[contact.add]
`addContact` MUST add a contact to the caller's tenant with a name, which
MUST NOT be empty, and optionally an email, recording who added it and
when. It MUST require `kuria:contacts:manage` tenant-wide, and be refused
as `FORBIDDEN` otherwise.

r[contact.unique-email]
A tenant MUST have at most one contact per email, compared without regard
to case. Adding a contact, or editing one, with an email another contact of
the tenant has MUST be refused as `DUPLICATE_EMAIL`.

r[contact.edit]
`editContact` MUST change a contact's name and email while it isn't
linked, and MUST be refused as `LINKED` once it is. It MUST require
`kuria:contacts:manage`.

r[contact.list]
`contacts` MUST list the tenant's contacts, with their emails and the users
they are linked to, to a subject with `kuria:contacts:read` or
`kuria:contacts:manage`, and be refused as `FORBIDDEN` to anyone else.

## Naming contacts

r[contact.reference]
A value MUST name only a contact of the akt's tenant; a value naming any
other MUST be refused as an invalid value of its field.

r[contact.show]
A contact a value names MUST be shown to anyone who may read the akt: as
the user it is linked to, shown by `person.name`, else by its own name,
else, once erased, by its identifier. Its email MUST only be shown as
`contact.list` allows.

r[contact.values]
A value MUST keep naming the contact it named when the contact is linked,
so that akt revisions never change; only how it is shown changes.

## Linking

r[contact.link]
At a browser sign-in to a tenant, if Konklav's userinfo gives an email and
says it is verified, the tenant's contact with that email, if it isn't
linked yet, MUST be linked to the subject signing in. An unverified email
MUST NOT link a contact. A contact, once linked, MUST stay linked to that
subject.

r[contact.link-by-hand]
`linkContact` MUST link a contact that isn't linked yet to a user who has
signed in to the caller's tenant (`person.tenant`), recording who linked
it and when. It MUST require `kuria:contacts:manage`, refuse a linked
contact as `LINKED`, and a subject who hasn't signed in to the tenant as
`UNKNOWN_USER`. A password's email, which Konklav never verifies, can't
link a contact (`contact.link`); linking by hand is how such a person's
contact becomes theirs.

## Erasure

r[contact.erase]
`eraseContact` MUST delete a contact's name and email, keeping the contact
so that the values naming it still read. It MUST require
`kuria:contacts:manage`.

r[contact.erasure]
When Konklav erases a subject, Kuria MUST delete the name and email of every
contact linked to them, as `contact.erase` does.
