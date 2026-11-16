# Live drafts

An akt type may give its akter a live draft: all their fields, edited by
everyone allowed at once, live, with [Yjs](https://github.com/yjs/yjs).
Edits change the live draft only; an akt's revisions are its releases, each
recording the live draft as it was. Not to be confused with a `draft`, text
a field is filled with by its recipe ([drafts](drafts.md)).

## Declaring

r[live.declare]
An akt type document MAY say `"live_drafts": true`. It MUST then also
declare `versions` (`version.declare`), and no `markdown` or multiline
`text` field of it may be keyed `fields`; otherwise the document MUST be
refused.

## The live draft

r[live.fields]
An akt with a live draft MUST have exactly one, a Yjs document holding
every field of its type: each `markdown` and multiline `text` field as a
`Y.Text` named by the field's key, holding its Markdown or text; every
other field as the field's value in the `Y.Map` named `fields`, in the
form GraphQL answers it (`Akt.fields`, a revision's `changes`), the last
write winning. A field without a value is an empty `Y.Text` or absent from
the map. Updates MUST be Yjs v1 updates, as y-websocket speaks them.

r[live.edit]
Writing the fields of an akt with a live draft, through `editAkt`, a batch
or a push from a clone (`clone.server`), MUST change its live draft only and MUST NOT
make a revision. Each write MUST be checked as a revision's would be
(`akt.field.kinds`), the live draft's values standing for the akt's, and
MUST change each text by the characters that differ, so that edits made
concurrently merge. The revision an edit names MUST NOT be checked
(`akt.revision.concurrency`): it doesn't move with the live draft, which
merges.

r[live.read]
Reading such an akt MUST answer its live draft's fields as its fields,
with its latest revision, its state and the rest as they are. Reading it at
a version MUST answer the version as before (`version.read`).

r[live.release]
Releasing such an akt MUST record its live draft's fields as a new
revision, checking them as a write would, and MUST name that revision as
its next version, with the checks and pins of a release
(`version.references`, `version.pins`). It MUST take the revision the
caller read, as `version.release` does, and MUST be refused as
`BAD_REQUEST` when the live draft is the same as its latest version.

r[live.revisions]
A transition, or a change to the akt's containers or links, MUST still
append a revision, recording the fields of its latest revision unchanged,
so that every revision's fields are a release's. A transition that would
set fields MUST be refused as `BAD_REQUEST`.

r[live.history]
Every change to a live draft MUST be kept, with who made it, when and, for
a push from a clone (`clone.server`), its message, so
that the akt's history shows its live draft's changes between releases, by
author: `Akt.draftChanges` MUST answer those since its latest release, or
all with `all`, each with who made it, when and the fields it changed.

r[live.state]
Reading an akt with a live draft MUST also answer, to a subject who may
read it, its live draft's state, encoded as a Yjs update, its state
vector, and its base, its Yjs snapshot, which also changes with a change
that only deletes, so that a clone can make updates against it and name
what it made them against (`clone.server`).

## Editing together

r[live.sync]
`GET /akter/{number}/live` MUST upgrade to a WebSocket speaking the
y-websocket sync and awareness protocols for the akt's live draft,
authenticated as `/graphql` is. A subject who may read the akt MUST get
its live draft and every change to it; only one who may edit it MUST have
its changes taken, each checked as `live.edit` says, a refused change
being undone for every client. Each diagram block a taken change writes
MUST be rendered as a write's is (`akt.field.markdown.diagrams`), but a
block that doesn't compile, being typed, MUST NOT refuse the change. An
akt without a live draft, or one the subject may not read, MUST be
refused as if it didn't exist.

r[live.instances]
Changes and awareness MUST reach every client of a live draft, whichever
of Kuria's servers each is connected to.

r[live.presence]
Awareness MUST say who each client is, as Kuria names them
(`person.name`), and where their cursor is.
