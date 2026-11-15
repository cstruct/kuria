# The git remote

`git-remote-kuria` is a git remote helper, installed with `kuria`. It
makes `kuria::<url>` a git remote, so that a container and its members
can be cloned, fetched, diffed, rebased and pushed with git. Kuria serves
no git protocol: the helper speaks GraphQL to Kuria.

## Cloning

r[git.remote]
`git clone kuria::<url>/<number>` MUST clone the container `number` on
the Kuria at `url`, signed in as `kuria` is (`cli.profile`,
`cli.secret`). A number that isn't a container the subject may read MUST
fail the clone, saying why.

r[git.tree]
A clone's tree MUST hold the container's file at its root, and each
member's file in directories that mirror the container's `sections`
field: one directory per section, named by its title as
`cli.local.names` names files, with `section` for one without a title,
and `-2`, `-3`, … for siblings named alike. Members in no section, and
every member of a container without a `sections` field, MUST be at the
root. Files MUST be named as `cli.local.names` says. A clone's file MUST
be an akt file (`cli.file`) without `revision`, `in`, `links` and
`outcome`, which change without the akt's revision changing, or, for
`revision`, are git's own history: an akt's file changes in a commit
exactly when the akt has a revision.

r[git.history]
A clone's history MUST be one commit per akt revision of the container or
any member, or per batch (`akt.batch`) for revisions a batch made, in
change-event order (`akt.change-event`), linear. A commit's author and
time MUST be the revision's, and its message the revision's reason, else
what changed. A version (`version.number`) MUST be the tag
`<number>/v<n>` on the commit of the revision it names.

r[git.identity]
Every clone of a container MUST have the same commits, by id: the helper
MUST build them only from what Kuria answers. A commit pushed to Kuria
(`git.push`) MUST be built again, by every clone, as the very commit that
was pushed: Kuria MUST keep its author, committer, times, message and
parent with the revisions it made.

r[git.pictures]
A clone MUST hold, as `attachments/<number>.<extension>`, the picture of
each image its files embed (`akt.field.markdown.images`) that names no
version or heading and isn't one of the clone's own files, as Kuria has
it at each commit, and each such embedded image MUST be linked to its
picture by a path relative to its file. An image's revisions MUST be
commits of the history too, so every clone has the same commits. A push
MUST turn such links back into `kuria:<number>`, and MUST be refused unless
its pictures are exactly those of the images it embeds, as Kuria has
them: pictures are uploaded in Kuria. `kuria fmt` MUST write those
pictures, and remove any other.

r[git.fetch]
A fetch MUST build the container's history from Kuria's revisions of it
and its members (`git.history`), so that the commits the clone has stay
as they were and those after them are added.

r[git.fetch.cache]
A revision never changes, so the clone MUST keep each akt's file at each
revision it read, in its git directory, and a later fetch or push MUST NOT
ask Kuria for one it kept. What it keeps MUST be for the client version and
the Kuria that wrote it, and read afresh by any other.

r[git.fetch.batched]
The helper MUST ask for the revisions of many akter, and for many akter at
revisions, in one request, and split a request Kuria refuses as too
complex until it is taken.

## Pushing

r[git.push]
A push MUST apply each new commit, in order, as one batch (`akt.batch`)
with the commit's message as its reason:

- a changed file, as an edit of the fields it changed (`cli.apply`), the
  container's `sections` field included, which says where its members'
  files go;
- a file removed, as removing the member from the container.

A new file MUST be refused: an akt's number comes from Kuria.

A refusal of any commit's batch MUST stop the push there, saying which
commit and why; the commits before it stay pushed.

r[git.push.normal]
A commit MUST be refused unless its tree is in normal form: the tree the
helper would build for the container and its members as the commit leaves
them, such as every heading with an id (`akt.field.markdown`).

r[git.push.fast-forward]
A push MUST be refused as not a fast-forward unless its first new
commit's parent is the clone's latest commit from Kuria and Kuria has no
revision of the container or its members since, so that `git pull
--rebase` resolves it. A merge commit MUST be refused.

r[git.fmt]
`kuria fmt` in a clone MUST put its files in normal form, changing no
value but the heading ids it gives, and moving each file to where the
container's `sections` field puts it.

## Repositories

A container may instead be a git repository Kuria hosts: cloned, fetched
and pushed over HTTPS with plain git, its commits, branches, merges and
tags kept as pushed.

r[git.repo.declare]
An akt type document that contains akt types MAY say `"git": true`; each
akt of it is then a repository, of it and its members, at
`/git/<number>.git`.

r[git.repo.storage]
A repository's objects MUST be kept in object storage as packfiles, never
changed once kept, and its refs in Kuria's database, each moved only by a
compare-and-swap in the transaction that writes what its push changed, so
that two pushes racing to one ref can't both succeed.

r[git.repo.serve]
Kuria MUST serve each repository over git's smart HTTP protocol,
authenticated as `/graphql` is: fetching to a subject who may read its
akt, pushing to one who may edit it. `kuria git-credential` MUST give git
the credentials of the CLI's sign-in.

r[git.repo.push]
A push MUST be checked before any of it is kept: a push to `main` as
`git.repo.main` says, any other ref only for being a branch or tag. A
refused push MUST keep nothing.

r[git.repo.main]
`main` MUST hold the container and its members as `git.tree` and
`git.pictures` lay them out, in normal form (`git.push.normal`). A push to
`main` MUST change in Kuria what its commits changed: an akt with a live
draft by merging into its live draft, as text, the change from each
commit's parent to it, so that edits made meanwhile are kept; any other
akt as `git.push` does. Merge commits MUST be taken.

r[git.repo.commits]
Changes made in Kuria to a repository's akter, through the app or the
API, MUST become commits on `main`, by whoever made them: a live draft's
after a minute without changes, or when it is released; any other change
as it is made. Each release MUST be tagged `<number>/v<n>` at its commit.

r[git.repo.branches]
Branches other than `main`, and tags, MUST be kept as pushed, and MUST
change nothing in Kuria.

r[git.repo.seed]
A repository MUST start from the history `git-remote-kuria` builds for its
container (`git.history`), so that clones made through the helper have the
same commits.
