# Versions

A version is one revision of an akt, named by releasing it, such as a
document's version 2. The akt itself stays its living version, which edits
change; its versions never change. Any akt type may release versions;
none is special.

## Declaring

r[version.declare]
An akt type document MAY declare `versions`, an object whose optional
`action` names the Konklav action a release needs (`authz.release`), and
whose optional `file` names the field each version is released with a
file in (`version.file`). The action MUST follow
`authz.transition-action`, and `versions` MUST NOT hold any other member;
otherwise the document MUST be refused. Releasing an
akt whose type version doesn't declare `versions` MUST be refused as
`BAD_REQUEST`.

r[version.file]
`versions.file` MUST name an `attachment` field of the type that takes
only `application/pdf` (`media_types`); otherwise the document MUST be
refused. A type without `markdown` fields is one whose akter are written
elsewhere, such as an external document, released as a PDF each; a type
with them, such as a document whose earlier versions were kept as PDFs,
renders a version from its text unless it was released with a PDF. Only a release MUST set the
field: a write that would change it otherwise MUST be refused as
`REFUSED`, and a copy (`akt.copy`) MUST NOT keep it.

## Releasing

r[version.release]
`releaseVersion` MUST name the akt's current revision as its next
version, or for an akt with a live draft record its live draft as a new
revision and name that (`live.release`), with an optional label, a single line of at most 100
characters, and an optional description, text of at most 2,000
characters that may run over several lines, such as what changed; either,
blank once trimmed, is none. It MUST take the revision the caller read, and refuse a
revision that isn't the akt's current one as `STALE`
(`akt.revision.concurrency`). It MUST refuse a revision that is already a
version as `BAD_REQUEST`. An akt in a final state MAY be released.

r[version.number]
An akt's versions MUST be numbered 1, 2, … in the order they were
released, with no gaps. A version MUST record its number, its label, its
description, the revision it names, who released it and when.

r[version.immutable]
A version MUST never change and never be removed, nor anything it records,
its pins included.

r[version.references]
A release MUST be refused as `BROKEN_REFERENCES`, naming each broken
reference by field and destination, while any reference in the revision's
`markdown` fields (`akt.field.markdown.references`) names:

- an akt that doesn't exist, or that the releaser may not read;
- a version the target doesn't have;
- a heading id the target doesn't have: in the version named, if the
  reference names one; else, if the target's type declares `versions`,
  in its latest version, the reference being broken while it has none;
  else in the target's current revision;
- for an embedded image, an akt that isn't an image or has no picture,
  or a heading (`akt.field.markdown.images`), read as a heading is.

The refusal MUST also name, under `together`, each target that releasing
together (`version.together`) would mend a reference to: one whose type
declares `versions`, which the reference names no version of, and whose
current revision, or live draft, has what the reference names.

r[version.pins]
A version MUST record, for each of its references that names no version,
what the reference meant when it was released: the target's latest
version, if its type declares `versions`, else the target's revision
then.

r[version.together]
`releaseVersions` MUST release several akter, each as `releaseVersion`
would, all or nothing, refusing an akt named twice as `BAD_REQUEST`. A
reference from one of them to another MUST be checked against what that
other is released as, at the version it gets, and pinned to that
version; every broken reference of every one of them MUST be named,
prefixed by the number of the akt it is in.

r[version.history]
A release MUST be shown in the akt's history, and MUST append a change
event (`akt.change-event`) naming the akt and the version. It MUST NOT
make an akt revision, except as `live.release` and `version.release-file`
say.

r[version.release-file]
Releasing an akt whose type declares `versions.file` MAY take a file, an
attachment (`attachment.upload`) the releaser may read, of the media type
`application/pdf`, and MUST take one if the type has no `markdown`
fields; it MUST be refused as `BAD_REQUEST` without one where one is
needed, or with one for a type that doesn't declare `versions.file`, and
as an edit's attachment is (`attachment.media-types`) with one that isn't
a PDF. A release with a file MUST record it in the field as a new akt
revision, checked as an edit's attachment is (`attachment.references`),
and name that revision as the version; a release without one, of an akt
whose field holds a file, MUST clear it so, so that each version either is
the PDF it was released with or has none. With a live draft
(`live.release`), the file is recorded with the live draft, in the same
revision. An akt in a final state MAY be released so too.

## Reading

r[version.list]
An akt MUST be served with its versions, latest first, each with what
`version.number` records.

r[version.read]
An akt MUST be readable at one of its versions: its fields, state and
title as they were at the revision the version names, by a subject who
may read the akt. Its references MUST be served as the version reads
them: one that names a version, at that version; one the version pinned,
at its pin. An embedded image MUST show its picture as it was there, and
each diagram block the rendering the akt held for it.

r[version.diff]
Two versions of an akt, or a version and its living version, MUST be
comparable field by field, a `markdown` field line by line.
