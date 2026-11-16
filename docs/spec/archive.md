# Archiving

Every akt can be archived: kept, read-only, and out of the way. Archived is
a flag beside an akt's state, not a state, the same for every akt type.
Akt types may archive their akter by themselves, by rules they declare.

## Archived

r[archive.flag]
Every akt MUST be archived or not, beside its state. An archived akt MUST
record when it was archived, by whom, a subject or a rule
(`archive.rules`), and the reason given, if any. Archiving MUST NOT change
an akt's state, fields or outcome.

r[archive.act]
`archiveAkt(input: { number, reason })` MUST archive a live akt, and
`unarchiveAkt(input: { number, reason })` MUST make an archived akt live
again; each MUST be refused as `BAD_REQUEST` for an akt already so. The
reason, if given, MUST be one line of at most 200 characters.

r[archive.authz]
Archiving and unarchiving an akt MUST need Konklav's `kuria:akter:archive`
at its akt type, besides reading it (`authz.akt-read`); without it they
MUST be refused as `FORBIDDEN`.

r[archive.history]
Archiving and unarchiving MUST each append an akt revision with the
transition `archive` or `unarchive`, its reason and no field changes, and a
change event (`akt.change-event`), so that the akt's history and the
`changes` subscription show it. An archive by a rule MUST be authored by
Kuria and name the rule.

r[archive.readonly]
Every write to an archived akt MUST be refused as `ARCHIVED`, naming it,
until it is unarchived: an edit, a batch's operation, a transition, a link
or unlink from or to it, a change to its containers or members, a move, a
release, a verdict, a draft, a live-draft update and a push from a clone.
Reading it, its revisions, versions and PDFs, and resolving references to
it MUST work as for a live akt.

## Storage

r[archive.storage]
Akter MUST be kept in a table partitioned on whether they are archived,
live and archived akter in partitions of their own, and archiving or
unarchiving an akt MUST move it, its revisions, its live-draft updates and
its change events to the other partitions in the same transaction. Akt
numbers MUST stay unique in a tenant across both.

r[archive.storage.live]
Every read of a list of akter (`akt.list`, search, a container's members,
a board's columns, counts) that asks for live akter only MUST read only
the live partitions.

## Reading

r[archive.read]
`Akt.archived` MUST answer, for an archived akt, when, by whom and why it
was archived, and `null` for a live one. `akter`, `members`, `board` and
every count MUST take `archived: ArchivedFilter`, `LIVE` (the default),
`ARCHIVED` or `ALL`, and answer only the akter it allows. Reading an akt by
its number or id MUST answer it whether archived or not.

r[archive.board]
A container's board MUST show only its live members, unless the container
is archived itself, when it MUST show all of them, or the board is asked
for `ALL`. A column's work-in-progress limit MUST count live members only.

## Rules

r[archive.rules]
An akt type document MAY declare `archive: { rules: [ … ] }`, each rule an
object of one of these kinds:

- `{ "newer": N, "in": [states] }`, optionally with `"within": <container
  type>`: archive an akt in one of `in` once at least `N` other akter of
  its type reached one of `in` after it did, counting, with `within`, only
  those sharing a container of that type with it;
- `{ "with_container": <container type> }`: archive an akt when a
  container of that type that holds it is archived;
- `{ "days_in": D, "in": [final states] }`: archive an akt that has been
  in one of `in` for at least `D` days.

The document MUST be refused naming the rule when it isn't exactly one of
these kinds, `N` or `D` is below 1, `in` is empty or names a state that
isn't the type's, or a `days_in` state isn't final. `with_container` and
`within` name akt types by key; a rule naming one that never contains the
akt archives nothing.

r[archive.rules.state-since]
Every akt MUST record when it reached its current state: when it was
created, else at its last transition.

r[archive.rules.on-change]
Rules MUST be applied in the same transaction as the changes that can make
them true: a transition, to the other live akter of its type that it may
have overtaken (`newer`); archiving or unarchiving a container, to its
members (`with_container`), and so on down to their members.

r[archive.rules.container]
Unarchiving a container MUST unarchive the members `with_container`
archived with it, and only those.

r[archive.rules.kept]
Unarchiving an akt a rule archived MUST mark it kept: no rule may archive
it again until it is archived by a subject. Archiving it by a subject MUST
clear the mark.

r[archive.sweep]
Kuria MUST apply every rule to every live akt of a type that declares any
at least every `KURIA_ARCHIVE_SWEEP_SECONDS` seconds (3600 by default), so
that `days_in` rules and rules added to a type apply to akter already
there. At most one server instance MUST sweep a tenant at a time.
