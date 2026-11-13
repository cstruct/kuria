# Akter

An akt is the one generic object in Kuria. What it holds, which states it
moves through and how its number reads all come from its akt type, which is
configuration a tenant submits at runtime. Every change to an akt is kept as
an akt revision.

Not yet covered here: links other than containment, signatures,
structured tables and obligations. Containing akter is in
[containers](containers.md), their order in [ordering](ordering.md), and who
may do what in [authorization](authorization.md).

## Akt types

r[akt.type.document]
An akt type MUST be defined by one document, submitted whole, naming its
key, its fields, its states, its transitions and its number pattern.
Submitting a document for a key MUST create the next akt type version of
that key, in the submitter's tenant. Akt type versions MUST never change
once created.

r[akt.type.validation]
Kuria MUST refuse an akt type document, saying why, unless:

- keys of the type, its fields, states, transitions and choice options are
  lowercase ASCII letters, digits and `_`, starting with a letter, and unique
  where they are siblings;
- exactly one state is initial;
- every transition starts from at least one state and ends in a state the
  type defines, and every field it requires is a field the type defines;
- no transition starts from a final state;
- every choice field has at least one option;
- the number pattern is valid (`akt.number.pattern`);
- every action a transition names is valid (`authz.transition-action`);
- the keys in `contains` and `created_in` are valid akt type keys, each listed
  once (`container.contains`, `container.created-in`);
- the board's columns are valid (`board.columns`);
- every draft recipe is valid (`draft.recipe`, `draft.line`);
- its presentation is valid (`akt.type.presentation`).

r[akt.type.tenant]
Akt types MUST belong to one tenant. A request MUST only read, use or
define the akt types of its own tenant.

r[akt.type.state-references]
Board columns (`board.columns`) and draft recipes (`draft.states`) name
states of other akt types. Defining an akt type MUST be refused, with a
problem for each, when:

- it names a state that the latest version of the named akt type doesn't
  have, if that akt type is defined; or
- it lacks a state that the latest version of another akt type names for
  it, so that the other akt type must stop naming the state first.

Akt types that aren't defined yet MAY be named, so that documents can be
submitted in any order. Definitions in one tenant MUST be checked one at a
time, so that two can't each pass against the other's old version.

r[akt.type.presentation]
An akt type MAY say how its akter are shown, in `presentation`:

| Member | Meaning |
|---|---|
| `title` | The key of a single-line `text` field, shown as each akt's title. |
| `card` | Up to four field keys, shown on board cards after the title. |
| `list` | Field keys, shown as columns where the type's akter are listed. |
| `tones` | Per state key, a tone: `neutral`, `active`, `success`, `warning` or `danger`. |
| `icon` | An icon shown with the type's name: `archive`, `book-open`, `box`, `bug`, `calendar`, `circle-check`, `clipboard-check`, `file-text`, `flag`, `flask-conical`, `git-branch`, `layers`, `list-checks`, `milestone`, `notebook-pen`, `package`, `rocket`, `shield-check`, `tag`, `target`, `triangle-alert`, `users`, `wrench` or `zap`. |
| `colour` | The colour the type is shown in: `neutral`, `blue`, `green`, `amber`, `red`, `purple`, `teal` or `pink`. |

Every key MUST name a field or state the type defines, each at most once
in its list, and the title a single-line `text` field; otherwise the
document MUST be refused. Without a `title`, an akt's title MUST be the
value of the type's first required single-line `text` field, if it has
one. A state without a tone MUST be shown as `neutral`. An icon or colour
not in its list MUST be refused; without one, a type MUST be shown without
an icon, in `neutral`.

## Fields

r[akt.field.kinds]
A field MUST be of one of these kinds, and a value MUST be refused unless it
fits the field's kind:

| Kind | Value |
|---|---|
| `text` | A string. A `text` field is single-line, refusing line breaks, unless it says `multiline`. |
| `choice` | The key of one of the field's options. |
| `choices` | A list of distinct keys of the field's options. |
| `date` | A calendar date, `YYYY-MM-DD`. |
| `user` | A person or piece of software: a user, as a Konklav subject identifier, shown by name ([people](people.md)), or a contact, as `{"contact": <identifier>}` ([contacts](contacts.md)). |
| `checkbox` | Checked or not; see `akt.field.checkbox`. |
| `reference` | A link to something outside Kuria: an absolute `http` or `https` URL, and optionally a label. |
| `git` | A commit in a git repository: the repository, as an `https` URL or an `ssh` remote such as `git@host:team/app.git`, and a commit SHA (40 or 64 lowercase hexadecimal digits), a tag, or both. |
| `docker` | A container image: its repository, such as `registry.example/team/backend`, its digest (`sha256:` and 64 lowercase hexadecimal digits), and optionally a tag. |
| `attachment` | A file uploaded to Kuria, by its identifier; see `akt.field.attachment`. |

r[akt.field.checkbox]
A `checkbox` field MUST take `true` to check it and `false` to clear it.
Checking it MUST record the subject who checked it and when, taken from the
change, never from the input, and the record MUST stay until the box is
cleared. A box MAY be checked for someone else with `{"for": <user>}`,
`<user>` being a value a `user` field takes; the record MUST then also say
whom it was checked for, and checking it for the checker MUST be the same
as `true`. A transition that requires a `checkbox` field MUST need it
checked.

r[akt.field.attachment]
An `attachment` field MUST only hold an attachment of the akt's tenant. A
field that says `"format": "sbom"` MUST only hold an attachment Kuria
recognised as an SBOM ([attachments](attachments.md)). `format` MUST be
refused on any other kind of field.

r[akt.field.required]
A field marked `required` MUST hold a value from the moment the akt is
created, and MUST NOT be cleared.

## Akter

r[akt.create]
Creating an akt MUST give it the latest version of its akt type, the type's
initial state, its next akt number, and the field values given, and MUST
record that as the akt's first akt revision. Values for fields the type does
not define MUST be refused.

r[akt.number.pattern]
An akt type's number pattern MUST be literal text with these placeholders:
`{year}` and `{month}` (of the akt's creation, in UTC, as 4 and 2 digits),
`{parent}` (the akt number of the container the akt was created in), and
exactly one `{seq}` or `{seq:N}`, the sequence number zero-padded to at least
N digits. `{parent}` MUST only be used by an akt type with `created_in`
(`container.created-in`). Literal text MUST NOT contain `{` or `}`.

r[akt.number.sequence]
The sequence number MUST count from 1 per tenant and per pattern as rendered
without its sequence, so that `CAPA-{year}-{seq:4}` gives `CAPA-2026-0001`,
`CAPA-2026-0002`, and starts again at `CAPA-2027-0001`, and
`{parent}-RC-{seq:4}` counts separately in each parent:
`RELEASE-2026-0001-RC-0002` and `RELEASE-2026-0002-RC-0001`. Akt numbers MUST be
unique in their tenant and MUST never change.

r[akt.edit]
Editing an akt MUST change only the field values given, where a `null`
clears a value. It MUST refuse an akt in a final state, and an edit that
would change nothing.

r[akt.transition]
Performing a transition MUST be refused unless the akt's current state is
one the transition starts from. It MAY change field values in the same step,
as an edit does. It MUST be refused unless every field the transition
requires holds a value afterwards. The akt then moves to the transition's
target state.

r[akt.version-pinned]
An akt MUST be validated against the akt type version it was created with,
also after newer versions of its akt type exist.

r[akt.title]
Every akt MUST be served with its title, as its type's presentation
(`akt.type.presentation`) gives it, or none, beside its number.

r[akt.list]
Akter MUST be listable, of one akt type and in one state if asked, only
those the subject may read (`authz.read`), at most a limit of them, the
most recently changed first.

r[akt.search]
Akter MUST be searchable by text: an akt matches when its number starts
with the text, ignoring case, or its title contains every word of the
text, ignoring case. A search MUST only answer akter the subject may read
(`authz.read`), and at most a limit of them, the most recently changed
first.

## Akt revisions

r[akt.revision.record]
Every change to an akt MUST append an akt revision, numbered from 1 without
gaps, recording the subject who made it, the transition if there was one,
the reason if one was given, the field values it changed, the resulting
state, and the SHA-256 hash of the akt's resulting state and field values.
Akt revisions MUST never change or be removed by any operation of Kuria's.

r[akt.revision.concurrency]
An edit or a transition MUST name the akt revision it was based on, and MUST
be refused if the akt has moved past it, so that no change is made on top of
one its author did not see.

r[akt.revision.atomic]
An akt's current state, its new akt revision and its change event MUST be
written in one transaction, so that none is ever stored without the others.

## Change events

r[akt.change-event]
Every akt revision MUST append a change event naming the tenant, the akt and
the revision, numbered from 1 per tenant in the order the changes committed.
Moves append change events too (`order.not-recorded`), numbered in the same
sequence.
