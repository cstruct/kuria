# Bundles

A bundle is what Kuria exports from a tenant and imports into one:
selected akt types, with or without their akter, or the whole tenant.
Akter in a bundle keep their numbers and their whole history, so a bundle
moves a tenant's records between instances, or backs them up, without
rewriting them. A bundle never crosses tenants on export: it holds one
tenant's data, and is imported into the caller's tenant.

## The bundle

r[bundle.format]
A bundle MUST be a directory holding `bundle.json` and, if any akt holds an
attachment, `attachments/`, one file per attachment's bytes named by its
SHA-256 in hex. `bundle.json` MUST say the bundle's format version, its
scope (`types` or `tenant`), the akt type keys a `types` bundle selected,
whether it holds akter, and when it was exported. Kuria MUST write format
3, and read formats 1, 2 and 3: format 2 is format 3 without akter's
archived state, and format 1 is format 2 without versions or the commits
revisions were pushed in. On the wire, to and from
the server, a bundle MUST travel as a tar archive of that directory.

r[bundle.contents]
A bundle that holds akter MUST hold, for them:

- every version of their akt types, as stored;
- every akt, with its number, the version it was created with, its state,
  fields, title, rank, and when it was created and last changed;
- every akt revision of each, as stored: its number, author, time,
  transition, reason, changes, resulting state, membership or link change,
  and hash;
- every version of each, as stored: its number, the revision it names, its
  label, who released it and when, and its pins (`version.pins`);
- whether each is archived, and if so when, by whom or which rule (and,
  for `with_container`, the container it went with) and why, and whether
  it is kept from rules (`archive.flag`, `archive.rules.kept`);
- the memberships and links among them, with any pinned revisions;
- for each copy among them, the akt it was copied from, if the bundle holds
  that one too (`akt.copy.record`);
- the tenant's number counters;
- the contacts their values name, and the people their revisions and
  values name: subject, recorded name and email;
- the attachments they hold, in fields and as diagrams, with their
  metadata and bytes, and which akt holds each diagram, for which field
  and source (`akt.field.markdown.diagrams`).

A bundle without akter MUST hold the latest version of each selected akt
type's document only.

## Exporting

r[bundle.export.snapshot]
`GET /export` MUST answer a bundle of the caller's tenant as one consistent
snapshot: as the tenant was at one moment, whatever is written meanwhile.

r[bundle.export.types]
An export of selected akt types MUST hold those types, and their akter if
asked. An akt in it that names an akt outside it, as a member, container,
link target or link source, MUST make the export refused as
`INCOMPLETE_SELECTION`, naming the akt types missing, so that an import
never holds a reference it can't keep.

r[bundle.export.tenant]
An export of the whole tenant MUST hold everything Kuria holds for it: every
akt type with all its versions, every akt with everything
`bundle.contents` lists, all its contacts, linked or not, with whom they
are linked to and who linked them, its number counters, and the users who
have signed in to it (`person.tenant`) with their recorded names.

## Importing

r[bundle.import.format]
`POST /import` MUST refuse a bundle whose format version it doesn't know,
and one that doesn't hold what its scope says it holds, as
`INVALID_BUNDLE`, before writing anything.

r[bundle.import.atomic]
An import MUST write everything in its bundle into the caller's tenant in
one transaction, or nothing.

r[bundle.import.hashes]
An import MUST check every akt revision's hash against the state and field
values it records, and refuse a bundle with one that doesn't match as
`INVALID_BUNDLE`, before writing anything.

r[bundle.import.clashes]
An import of selected akt types MUST refuse, as `CONFLICT`, a bundle with
an akt number, akt type key or contact email the tenant already has. A
bundle without akter MUST instead define each akt type it holds as
`defineAktType` does, making new versions of keys the tenant has.

r[bundle.import.tenant]
An import of a whole tenant MUST only go into a tenant that holds no akt
type, akt or contact yet, and MUST otherwise be refused as `CONFLICT`. It
MUST restore everything `bundle.export.tenant` exports.

r[bundle.import.request]
`POST /import` MUST take the bundle as a tar archive, which MAY also hold
`subjects.json`: an object mapping a subject the bundle names to one of the
importing tenant (`bundle.import.subjects`).

r[bundle.import.ids]
Imported akter and akt type versions MUST get identifiers of their own.
Contacts and attachments MUST keep theirs, since the values and revisions
that name them are kept as they are, so an import MUST refuse, as
`CONFLICT`, a contact or attachment this Kuria already holds.

r[bundle.import.order]
Akter imported with selected akt types MUST come after the tenant's own in
its order, in the order the bundle gave them. A whole tenant's akter MUST
keep their places.

r[bundle.import.history]
Imported akter MUST keep their numbers, their akt type versions, their
revisions and the commits kept with them, and their versions exactly as the
bundle records them, authors and times included; an import MUST NOT append
revisions or versions to them. An import MUST refuse, as `INVALID_BUNDLE`
and before writing anything, an akt whose versions aren't numbered from 1
on, in order, name a revision it lacks or another version names, or hold
pins that don't read.

r[bundle.import.archived]
Imported akter MUST be archived and kept as the bundle records them, with
their history moved as archiving moves it (`archive.storage`), and a
`with_container` archive MUST name the identifier its container got, so
that unarchiving the container brings them back (`archive.rules.container`).

r[bundle.import.counters]
After an import, the tenant's number counters MUST be past every imported
number, so that the next akt of a pattern is numbered after the imported
ones.

r[bundle.import.subjects]
Subjects in a bundle MAY be mapped to the importing tenant's: by the
mapping the import is given (`bundle.import.request`), else by email to a
user who has signed in to the tenant, else not at all. Revisions and values MUST keep naming the subject
they recorded, so that they and their hashes stay as they were; the tenant
MUST instead record each mapping as an alias, and an aliased subject MUST
be shown as the user it maps to (`person.name`), and match `me` for that
user (`akt.filter`). A contact linked to a mapped subject MUST be linked to
the user it maps to. An unmapped subject MUST be shown by the name and
email the bundle recorded for them, unless Kuria already holds a name for
that subject.

r[bundle.import.recorded]
An import MUST be recorded, with who imported it, when, its scope and how
many akt types and akter it wrote, and MUST append change events for the
akter it wrote so that subscribers learn of them.

## Bootstrapping

A server can start with a bundle already imported: a bootstrap bundle, for
a new deployment's first data or for an example to explore.

r[bundle.bootstrap.variables]
The server MUST bootstrap when `KURIA_BOOTSTRAP_BUNDLE` names a bundle: a
directory as `bundle.format` describes it, or a file holding its tar
archive. `KURIA_BOOTSTRAP_SUBJECTS` MAY name a JSON file mapping subjects as
`subjects.json` does (`bundle.import.request`). Without
`KURIA_BOOTSTRAP_BUNDLE` the server MUST NOT bootstrap, and
`KURIA_BOOTSTRAP_SUBJECTS` alone MUST make start-up fail.

r[bundle.bootstrap.tenant]
The bundle MUST go into the tenant `KURIA_BOOTSTRAP_TENANT` names, else into
`KURIA_DEFAULT_TENANT`'s. With neither set, start-up MUST fail.

r[bundle.bootstrap.import]
Bootstrapping MUST import the bundle as `POST /import` does, with every
check and refusal of `bundle.import.*`, and a refused import MUST make
start-up fail with the refusal's message.

r[bundle.bootstrap.fresh]
Bootstrapping MUST only import into a fresh tenant: one that holds no akt
type, akt or contact, whatever the bundle's scope, checked in the import's
transaction. Other tenants in the database MUST NOT count. A tenant that
isn't fresh, even one that holds this very bundle from an earlier start,
MUST make start-up fail, naming the tenant and `KURIA_BOOTSTRAP_BUNDLE`, and
MUST NOT be written to.

r[bundle.bootstrap.before-serving]
The server MUST bootstrap after applying its migrations and before it
listens, so that no request is answered before the import, nor at all if it
fails.

r[bundle.bootstrap.recorded]
A bootstrap MUST be recorded as an import (`bundle.import.recorded`) by the
nil subject, since no one asked for it; the akt types a bundle without
akter defines are defined by the nil subject too.

## The examples' bundles

r[bundle.examples]
Each example under `docs/examples` MUST hold `bundle/`: a `types` bundle of
all its akt types with akter, made through Kuria's API, whose akt types'
latest versions are the example's documents as its files hold them. Each
MUST import into a fresh tenant (`bundle.bootstrap.fresh`).
