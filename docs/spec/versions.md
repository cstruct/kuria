# Versions

A version is one revision of an akt, named by releasing it, such as a
document's version 2. The akt itself stays its living version, which edits
change; its versions never change. Any akt type may release versions;
none is special.

## Declaring

r[version.declare]
An akt type document MAY declare `versions`, an object whose optional
`action` names the Konklav action a release needs (`authz.release`). The
action MUST follow `authz.transition-action`, and `versions` MUST NOT hold
any other member; otherwise the document MUST be refused. Releasing an
akt whose type version doesn't declare `versions` MUST be refused as
`BAD_REQUEST`.

## Releasing

r[version.release]
`releaseVersion` MUST name the akt's current revision as its next
version, with an optional label, a single line of at most 100
characters. It MUST take the revision the caller read, and refuse a
revision that isn't the akt's current one as `STALE`
(`akt.revision.concurrency`). It MUST refuse a revision that is already a
version as `BAD_REQUEST`. An akt in a final state MAY be released.

r[version.number]
An akt's versions MUST be numbered 1, 2, … in the order they were
released, with no gaps. A version MUST record its number, its label, the
revision it names, who released it and when.

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
  reference names one, else in the target's current revision.

r[version.pins]
A version MUST record, for each of its references that names no version,
what the reference meant when it was released: the target's latest
version, if it has one, else the target's revision then.

r[version.history]
A release MUST be shown in the akt's history, and MUST append a change
event (`akt.change-event`) naming the akt and the version. It MUST NOT
make an akt revision.

## Reading

r[version.list]
An akt MUST be served with its versions, latest first, each with what
`version.number` records.

r[version.read]
An akt MUST be readable at one of its versions: its fields, state and
title as they were at the revision the version names, by a subject who
may read the akt. Its references MUST be served as the version reads
them: one that names a version, at that version; one the version pinned,
at its pin.

r[version.diff]
Two versions of an akt, or a version and its living version, MUST be
comparable field by field, a `markdown` field line by line.
