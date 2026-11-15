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

r[git.fetch]
A fetch MUST ask Kuria for the change events after the last one the
clone has, and add a commit for each revision of the container and its
members among them.

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
