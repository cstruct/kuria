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
- the outcomes a transition allows are `pass`, `fail`, `skip` or
  `pending`, each once, and only on a type that has a `verdict` field or
  contains an akt type (`akt.transition.outcomes`);
- the keys in `contains` and `created_in` are valid akt type keys, each listed
  once (`container.contains`, `container.created-in`);
- the board's columns are valid (`board.columns`);
- every draft recipe is valid (`draft.recipe`, `draft.line`);
- its presentation is valid (`akt.type.presentation`);
- every `markdown` field's `references` names a link kind the document
  declares (`akt.field.markdown.references`);
- every `attachment` field's `media_types` are valid
  (`akt.field.attachment`);
- its `versions` are valid (`version.declare`);
- `live_drafts` comes with `versions` (`live.declare`);
- `git` is on a type that contains akt types (`git.repo.declare`).

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
| `navigation` | Where the app's sidebar lists the type: `main`, the default, or `more`, under "Show more" (`app.navigation`). |
| `image` | The key of an `attachment` field whose `media_types` are all images: an akt of the type is an image, and that field's value is its picture (`akt.field.markdown.images`). |

Every key MUST name a field or state the type defines, each at most once
in its list, and the title a single-line `text` field; otherwise the
document MUST be refused. Without a `title`, an akt's title MUST be the
value of the type's first required single-line `text` field, if it has
one. A state without a tone MUST be shown as `neutral`. An icon or colour
not in its list MUST be refused; without one, a type MUST be shown without
an icon, in `neutral`. A `navigation` other than `main` or `more` MUST be
refused, and an `image` naming a field that isn't an `attachment` field,
or one that doesn't list media types, or lists one that isn't `image/png`,
`image/jpeg`, `image/svg+xml` or `image/webp`.

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
| `user` | A person or piece of software: a user, as a Konklav subject identifier, shown by name ([people](people.md)), or a contact, as `{"contact": <identifier>}` ([contacts](contacts.md)). A list of them if the field says `multiple`; see `akt.field.multiple`. |
| `checkbox` | Checked or not; see `akt.field.checkbox`. |
| `reference` | A link to something outside Kuria: an absolute `http` or `https` URL, and optionally a label. |
| `git` | A commit in a git repository: the repository, as an `https` URL or an `ssh` remote such as `git@host:team/app.git`, and a commit SHA (40 or 64 lowercase hexadecimal digits), a tag, or both. |
| `docker` | A container image: its repository, such as `registry.example/team/backend`, its digest (`sha256:` and 64 lowercase hexadecimal digits), and optionally a tag. |
| `attachment` | A file uploaded to Kuria, by its identifier; see `akt.field.attachment`. |
| `sections` | The akt's members laid out in nested sections; see `akt.field.sections`. |
| `verdict` | Each assignee's verdict, `pass`, `fail` or `skip`; see `akt.field.verdict`. |
| `markdown` | Markdown whose headings keep ids and whose links may name akter; see `akt.field.markdown`. |

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
recognised as an SBOM ([attachments](attachments.md)). A field MAY list,
as `media_types`, the media types it takes: it MUST then only hold an
attachment of one of them (`attachment.media-types`). `format` and
`media_types` MUST be refused on any other kind of field, and a
`media_types` list naming a type twice, or something that isn't a media
type, MUST be refused.

r[akt.field.multiple]
A `user` field that says `multiple` MUST hold a list of distinct users and
contacts, in the order given, and is without a value when the list is
empty. `multiple` MUST be refused on any other kind of field.

r[akt.field.sections]
A `sections` field MUST hold a list of items, in order, each either an akt
number, naming a member of the akt, or a section: an optional `title`, an
optional Markdown `text`, whether it is `collapsible`, and its own list of
items. It MUST be refused if it names an akt that isn't a member of the
akt, or one more than once; one that says `of`, a list of akt type keys,
MUST also refuse a member of any other type. Members it doesn't name are
outside every section. A member removed from the akt MUST leave its
sections in the same change. An akt type MUST have at most one `sections`
field.

r[akt.field.markdown]
A line holding only `<!-- pagebreak -->` in a `markdown` field is a page
break (`pdf.render`, `app.sheets`).

A `markdown` field MUST hold CommonMark text, with GitHub's tables, task
lists, strikethrough and footnotes, and is without a value when empty. A
footnote is referred to as `[^<label>]` and defined, anywhere in the text,
as `[^<label>]: …`; one referred to but never defined is its text. A heading MAY
end with an id, `{#<id>}`, of lowercase ASCII letters, digits, `-` and
`_`, starting with a letter. Writing the field MUST give every heading
without an id one, the heading's text in lower case with every run of
other characters made one `-`, trimmed of `-` (`section` if that leaves
nothing), and followed by `-2`, `-3`, … if needed, so that every heading
id is unique within the akt across its `markdown` fields. A heading that
has an id MUST keep it when its text changes. A value giving one id to
two headings MUST be refused.

r[akt.field.markdown.references]
A link in a `markdown` field whose destination is `kuria:<number>`,
optionally followed by `@<version>` (`version.number`) and `#<heading
id>`, is a reference to that akt, that version of it and that heading. A
`markdown` field MAY name, as `references`, a link kind its type
declares: every write of the akt MUST then make the akt's links of that
kind the akter its `markdown` fields reference that exist and are of a
type the kind's `to` names, each once, unpinned. Any other reference is
broken. A write MUST NOT be refused for a broken reference; a release is
(`version.references`). `references` MUST be refused on any other kind of
field. An image whose destination is a reference embeds it
(`akt.field.markdown.images`), and is a reference like any other.

r[akt.field.markdown.images]
An image in a `markdown` field, `![<alt text>](kuria:<number>[@<version>])`,
embeds that akt's picture: the value, at the revision read, of the field
its type names as `image` (`akt.type.presentation`). An embedded image
MUST be served with the attachment it shows and that attachment's media
type, or as broken, saying why: the akt doesn't exist or the reader may
not read it, it isn't an image, it has no picture at that revision, it
doesn't have the version named, or the destination names a heading.

r[akt.field.markdown.diagrams]
A `markdown` field MAY say `"diagrams": true`. In such a field, a fenced
code block whose language is `diagram` is a diagram block: typst markup
in which fletcher 0.5.8 (`fletcher`, with `diagram`, `node` and `edge`),
cetz 0.5.2 (`cetz`, with `canvas` and `draw`) and cetz-plot 0.1.4 (`plot`
and `chart`) are imported, and nothing else can be imported or read
(`pdf.packages`). Every write of the field MUST render each of its
diagram blocks to SVG, sized to the diagram, and MUST be refused as
`INVALID_DIAGRAM`, naming the field, the block by its number counting
from 1, and typst's message, when one doesn't compile. Each rendering
MUST be kept as an attachment the akt holds as a diagram, by the field
and the block's source, and never let go of, so that every revision and
version is shown with its diagrams. A diagram block MUST be served with
its source and the attachment of its SVG. In any other field a `diagram`
block is code. `diagrams` MUST be refused on any other kind of field.

r[diagram.preview]
`renderDiagram(source)` MUST answer the SVG a diagram block with that
source renders to, or typst's message when it doesn't compile, to any
authenticated subject, storing nothing.

r[akt.field.verdict]
A `verdict` field MUST name, as `assignees`, a `multiple` `user` field of
its type, and MUST hold at most one verdict, `pass`, `fail` or `skip`, per
assignee. Giving one MUST take the verdict, or `{"verdict": <verdict>,
"for": <user>}` to give it for another assignee, `<user>` being a value a
`user` field takes, and `{"clear": true}` or `{"clear": true, "for":
<user>}` to take one back. It MUST record who gave it and when, taken from
the change, never from the input, and for whom when that is someone else.
A verdict MUST be refused unless it is for an assignee: one's own only if
one is an assignee, and anyone's, with `for`, only for an assignee. An
assignee removed from the `user` field MUST lose their verdict in the same
change. An akt type MUST have at most one `verdict` field.

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

r[akt.transition.outcomes]
A transition MAY name the outcomes it allows. Performing it MUST then be
refused unless the akt has outcomes (`akt.outcome`) and its outcome, as
it is when the transition is performed, is one of them, saying the
outcome and those allowed. No verdict or membership in the akt's member
tree MUST change between that check and the transition's revision.

r[akt.version-pinned]
An akt MUST be validated against the akt type version it was created with,
also after newer versions of its akt type exist.

r[akt.title]
Every akt MUST be served with its title, as its type's presentation
(`akt.type.presentation`) gives it, or none, beside its number.

r[akt.list]
Akter MUST be listable, of one akt type and in one state if asked, only
those the subject may read (`authz.read`), the most recently changed
first, a page at a time (`akt.page`).

r[akt.search]
Akter MUST be searchable by text: an akt matches when its number starts
with the text, ignoring case, or its title contains every word of the
text, ignoring case. A search MUST only answer akter the subject may read
(`authz.read`), the most recently changed first, a page at a time
(`akt.page`).

r[akt.page]
A list of akter, a search, and a container's members MUST be answered a
page at a time: at most `first` akter (50 by default, 200 at most), with a
cursor after the last of them and whether more follow. Asked again with
that cursor as `after`, it MUST answer the akter that follow the last one
in the list's order, so that an akt that keeps its place while the list
changes between pages is answered once. A cursor MUST be refused as
`BAD_REQUEST` by any other list, search or container than the one that
gave it.

r[akt.filter]
A list of akter of one akt type, and a container's members of one akt
type, MAY be filtered by field values: each condition names a field of
that type and a value, and an akt MUST match every condition. A field
holding a choice matches by option key; a `user` field, by the user or
contact it names, or one of those it names if it names several, or `me`
for the subject asking; a checkbox, by whom it
was checked for, or `me`; a date, by the date; a single-line `text` field,
by the whole text. A condition on a field of any other kind, on a field
the type lacks, or without one akt type to filter, MUST be refused as
`BAD_REQUEST`.

r[akt.filter.outcome]
A list of akter of one akt type MAY be filtered by outcome if the type has
a `verdict` field, and a container's members of one akt type if the type
has outcomes: only akter with that outcome, as it is when the list is
answered (`akt.outcome`), MUST be answered, a page at a time
(`akt.page`). Filtering a type without outcomes, or a list other than a
container's members of a type whose outcome comes from its members, MUST
be refused as `BAD_REQUEST`.

## Outcomes

r[akt.outcome]
An akt type has outcomes if it has a `verdict` field, or contains an akt
type that has them (`container.contains`). An akt of such a type MUST be
served with its outcome, `pass`, `fail`, `skip` or `pending`:

- with a `verdict` field: `fail` if any assignee's verdict is `fail`;
  otherwise `pending` if an assignee has given none, or it has no
  assignees; otherwise `skip` if every verdict is `skip`; otherwise `pass`;
- otherwise, from the outcomes of its members that have outcomes: `fail`
  if any is `fail`; otherwise `pending` if any is `pending`, or it has none;
  otherwise `skip` if all are `skip`; otherwise `pass`.

r[akt.outcome.section]
Each section of a `sections` field MUST be served with its outcome by the
same rules as a container's, over the outcomes of the members it names and
of the sections nested in it.

r[akt.outcome.derived]
An outcome MUST be derived from the verdicts and members as they are, each
time it is read, and MUST NOT be stored or recorded as a revision.

## Copying

r[akt.copy]
Copying an akt MUST create a new akt of the same akt type, as `akt.create`
does, with the akt's field values except its `verdict` and `checkbox`
values, and MUST copy its members the caller may read likewise, recursively,
each created in the copy of the akt it is a member of. Each copy's
`sections` MUST place the copies where the originals were. The copy MUST be
created in the containers given, or else in those of the akt copied, as
`akt.create` and `container.create-inside` require.

r[akt.copy.only]
A copy MAY name the akter of the copied tree to keep. It MUST then copy
those, the akter between them and the akt copied, and every member under
them, and leave out the rest, and the sections left holding nothing.
Naming an akt outside the tree MUST be refused as `BAD_REQUEST`.

r[akt.copy.links]
A link between two akter of the copied tree MUST be copied as a link
between their copies; a link from an akt of the tree to an akt outside it
MUST be copied to the same akt, with the same pinned revision. Links into
the tree from outside, and memberships outside it, MUST NOT be copied.

r[akt.copy.record]
Each copy's first revision MUST say which akt it was copied from, and the
API MUST answer it as the copy's origin. A copy MUST be all or nothing, in
one transaction (`akt.batch.atomic`).

## Akt revisions

r[akt.revision.record]
Every change to an akt, save one to the live draft of an akt that has one
(`live.edit`), MUST append an akt revision, numbered from 1 without
gaps, recording the subject who made it, the transition if there was one,
the reason if one was given, the field values it changed, the resulting
state, and the SHA-256 hash of the akt's resulting state and field values.
Akt revisions MUST never change or be removed by any operation of Kuria's.

r[akt.revision.concurrency]
An edit or a transition MUST name the akt revision it was based on, and MUST
be refused if the akt has moved past it, so that no change is made on top of
one its author did not see; an edit of a live draft is not checked so
(`live.edit`).

r[akt.revision.atomic]
An akt's current state, its new akt revision and its change event MUST be
written in one transaction, so that none is ever stored without the others.

## Batches

r[akt.batch]
The API MUST take a batch: a list of operations, each one of the akt
mutations (`createAkt`, `editAkt`, `performTransition`, `addToContainer`,
`removeFromContainer`, `transferMember`, `addLink`, `removeLink`,
`moveAkt` and `draftField`) with that mutation's input, and MUST answer the
akt each operation answers, in order. Each operation MUST be checked,
authorized, recorded and given change events exactly as the mutation it
stands for would be.

r[akt.batch.atomic]
A batch's operations MUST run in order in one transaction: all of them, or
none. The first operation refused MUST roll the batch back and refuse it
with that operation's code and message, and its position, counting from 1,
as the extension `operation`.

r[akt.batch.refer]
Wherever an operation takes an akt's number, `@<n>` MUST name the akt that
the batch's operation `n`, a `createAkt` before it, created. A batch whose
`@<n>` names an operation that isn't an earlier `createAkt` MUST be refused
as `BAD_REQUEST` before any operation runs.

r[akt.batch.revision]
An edit or a transition in a batch MUST name the revision it expects as the
akt will be after the batch's operations before it, and is refused as
`STALE` against that (`akt.revision.concurrency`).

r[akt.batch.locks]
A batch MUST take its locks before running any operation, in the order every
write takes them: the tenant's order lock, if any operation creates or moves
an akt, then every existing akt any operation names, by id. It MUST then
never wait on a lock that a write waiting on one of its own could hold, so
that batches and single writes never deadlock.

r[akt.batch.limit]
A batch MUST hold at least 1 and at most 200 operations, and MUST otherwise
be refused as `BAD_REQUEST`.

r[akt.batch.published]
A batch's change events MUST be published once it commits, and never for
a batch rolled back.

## Change events

r[akt.change-event]
Every akt revision MUST append a change event naming the tenant, the akt and
the revision, numbered from 1 per tenant in the order the changes committed.
Moves and releases append change events too (`order.not-recorded`,
`version.history`), numbered in the same sequence.
