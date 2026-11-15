# Clones

A clone is a container and its members as files on someone's machine,
worked on with any editor and synced with Kuria through `kuria`: clone
records a base, edits are staged, a push sends them after fetching what
changed, and a pull brings others' changes in, marking conflicts. An akt
with a live draft ([live drafts](live-drafts.md)) is synced with its live
draft, so a push merges with whatever is being typed in the app meanwhile.
git, if the clone is also a git repository, is the person's own: Kuria sees
only the files.

## The files

r[clone.tree]
A clone MUST hold the container's file at its root, and each member's file
in directories that mirror the container's `sections` field: one
directory per section, named by its title as `cli.local.names` names
files, with `section` for one without a title, and `-2`, `-3`, … for
siblings named alike. Members in no section, and every member of a
container without a `sections` field, MUST be at the root. Files MUST be
named as `cli.local.names` says. A clone's file MUST be an akt file
(`cli.file`) without `revision`, `in`, `links` and `outcome`, and an akt
with a live draft's file MUST hold its live draft.

r[clone.pictures]
A clone MUST hold, as `attachments/<number>.<extension>`, the picture of
each image its files embed (`akt.field.markdown.images`) that names no
version or heading and isn't one of the clone's own files, and each such
embedded image MUST be linked to its picture by a path relative to its
file. A push MUST turn such links back into `kuria:<number>`, and MUST be
refused if it changes a picture: pictures are uploaded in Kuria.

r[clone.normal]
A clone's files are in normal form when they are as Kuria would write
them: every heading with an id (`akt.field.markdown`), each file where the
container's `sections` puts it, and the pictures of the images they embed.
`kuria fmt` MUST put them in normal form, changing no value but the
heading ids it gives, and writing or removing pictures.

## Working

r[clone.clone]
`kuria clone <number> [<dir>]` MUST write the container `number` and its
members as files in `<dir>`, by default a directory named as its file is,
and record in `<dir>/.kuria/` the base: each file as cloned, each akt's
revision, and, for each akt with a live draft, its live draft's state
(`live.state`). A number that isn't a container the subject may read MUST
be refused, saying why.

r[clone.status]
`kuria status` MUST name each file whose working text differs from the
base, staged or not, each file staged, and each file the base has that is
gone; `kuria diff` MUST show those differences, unstaged against staged,
or with `--staged` staged against the base.

r[clone.add]
`kuria add <path>…` MUST stage each file as it now is; `kuria add -p`
MUST offer each changed hunk of each file in turn and stage only the hunks
chosen. Staging MUST NOT change the working file. A file holding conflict
markers MUST NOT be staged.

r[clone.push]
`kuria push -m <message>` MUST first fetch the container and its members,
and MUST be refused, staging nothing away, unless the base is still Kuria's
current state, saying to pull. Otherwise it MUST send what is staged as one
push (`clone.server`): for an akt with a live draft, the change from the
base to the staged file as an update made against the base's state; for any
other akt, an edit of the fields the staged file changed, at the base's
revision. A new file, or a removed one, MUST be refused: akter are created
and removed in Kuria. When the push is taken, the base MUST become what was
pushed, and what wasn't staged stays as it is in the working files.

r[clone.pull]
`kuria pull` MUST fetch the container and its members and, for each file,
merge three ways: from the base to Kuria's current text, into the working
file. Where Kuria and the working file changed the same text differently,
it MUST write conflict markers (`<<<<<<< ours`, `=======`, `>>>>>>>
kuria`) around both and say which files hold them. Members added to the
container MUST appear and members removed MUST go, unless the working file
changed, which MUST be kept and named. The base MUST become Kuria's current
state, and anything staged MUST be staged again against it, or unstaged
where it conflicts.

r[clone.server]
`pushClone` MUST take a message, one line of 1 to 200 characters, and, all
or nothing: for each akt with a live draft, an update made against a base,
refused as `STALE` unless the base is the live draft's current base
(`live.state`), the refusal naming the akt and that base, applied and
checked as a live edit (`live.edit`); and for other akter, edits at the
revision the clone read (`akt.revision.concurrency`). Each live draft's update MUST be kept as one
change, by the subject, with the message (`live.history`); other akter's
edits MUST be revisions with the message as their reason.
