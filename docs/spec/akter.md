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
- the board's columns are valid (`board.columns`).

r[akt.type.tenant]
Akt types MUST belong to one tenant. A request MUST only read, use or
define the akt types of its own tenant.

## Fields

r[akt.field.kinds]
A field MUST be of one of these kinds, and a value MUST be refused unless it
fits the field's kind:

| Kind | Value |
|---|---|
| `text` | A string. A `text` field is single-line unless it says `multiline`, and then refuses line breaks. |
| `choice` | The key of one of the field's options. |
| `choices` | A list of distinct keys of the field's options. |
| `date` | A calendar date, `YYYY-MM-DD`. |
| `subject` | A Konklav subject identifier. |
| `reference` | A link to something outside Kuria: an absolute `http` or `https` URL, and optionally a label. |

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
