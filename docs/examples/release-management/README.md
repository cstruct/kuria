# Release management

An example configuration: eight akt types for running a product's backlog
and releases in Kuria. Nothing in Kuria knows these names; they are
ordinary akt type documents, and a tenant can change them or start from
others.

| Akt type | Number | Contains | Created in |
|---|---|---|---|
| [`backlog_item`](backlog_item.json) | `ITEM-0001` | | |
| [`epic`](epic.json) | `EPIC-001` | backlog items, each in one epic | |
| [`backlog`](backlog.json) | `BACKLOG-001` | backlog items, each in one backlog, and epics | |
| [`release`](release.json) | `RELEASE-2026-0001` | backlogs, release candidates, hotfixes, one set of release notes | |
| [`release_candidate`](release_candidate.json) | `RELEASE-2026-0001-RC-0001` | epics, backlog items and artifacts | a release |
| [`hotfix`](hotfix.json) | `RELEASE-2026-0001-HOTFIX-0001` | backlog items and artifacts | a release |
| [`artifact`](artifact.json) | `ARTIFACT-00001` | | |
| [`release_notes`](release_notes.json) | `RELEASE-2026-0001-NOTES-1` | | a release |

## How it fits together

- A backlog item lives in one backlog and at most one epic, and in as many
  release candidates and hotfixes as ship it. Every board it is on shows it
  in the same relative order, because they all order by rank: moving it in
  the backlog moves it in its epic too.
- A release is planned, tested, released, and finally closed when it is no
  longer maintained. Its release candidates and hotfixes are created inside
  it and numbered after it. It takes candidates only while planning or
  testing, and hotfixes only once released; once it is closed, its members
  are frozen.
- Replanning an item into another backlog is a transfer: one step, recorded
  on the item and both backlogs.
- Release candidates and hotfixes are accepted or deployed with evidence:
  the build, and for a candidate its test record, as references.
- An artifact is what a build produced for one component: its version, the
  git commit it was built from, its container image by digest, and its SBOM
  (SPDX or CycloneDX, uploaded to `POST /attachments`). The build pipeline
  registers artifacts, and the release manager puts them into candidates
  and hotfixes. An artifact whose change is `removed` says that a candidate
  drops a component.
- A release names its release manager, and can only be released once
  *Go for release* is checked, which records who checked it and when. The
  release manager can check it for whoever gave the go, such as a head of
  product, who may be a contact rather than a user: "Go for release: Hedda,
  checked by Alice". Once that person signs in with the contact's email,
  verified, the release shows them as the user they became.
- The release notes are drafted from what the release's accepted candidates
  and deployed hotfixes contain, including the items inside their epics,
  but not from its backlogs, whose items may not have shipped, nor from
  rejected candidates. The draft is then edited and approved.
- The release notes' *Artifacts* are drafted from the artifacts in the
  release's candidates and hotfixes: each component's newest artifact, and
  none for a component whose newest artifact is a removal. Every candidate
  counts, rejected ones too, so a component built once early and never
  rebuilt still ships; a candidate that must not ship a component removes
  it.

- Old work archives itself ([archiving](../../spec/archive.md)): a
  release once two newer releases are closed, no longer maintained, its
  candidates, hotfixes and notes with it, and a backlog item 90 days after it is done.
  Archived akter are read-only and left out of lists, boards and search
  until **Show archived**; unarchiving a release brings back what it took,
  and an akt someone unarchives by hand stays until archived by hand again.

- A document's `presentation` says how its akter are shown: an icon and a
  colour for the type (a rocket for a release, a flask for a candidate,
  a wrench for a hotfix, a package for an artifact), the
  fields on board cards (a backlog item's kind and assignee, a candidate's
  build) and in lists (an artifact's change, version and image), and the
  tone of each state (in progress is active, in review a warning, done,
  accepted, deployed and approved a success, rejected a danger). Titles
  come from each type's first required single-line text field; a
  candidate, a hotfix and release notes have none, and show their number.

## Trying it in a browser

```sh
mise run e2e
```

walks this example in Kuria's app ([`e2e/`](../../../e2e/README.md)): a
developer and a release manager sign in, plan a release, work two
candidates and a hotfix, give the go for a contact, and draft the notes
and the artifacts that ship; then releases go on until the first archives
itself with its candidates.

## Defining it

Submit each document with `defineAktType`. Their order doesn't matter:
akt types name each other by key.

```graphql
mutation($document: JSON!) {
  defineAktType(document: $document) { key version }
}
```

## Konklav roles

Kuria's actions and its scope kind must be in Konklav's catalog (see the
server's README), together with the action these documents name:

- `release:approve`, which releasing, deploying a hotfix and approving
  release notes require.

Two roles cover the usual split:

- **Developer**, tenant-wide: `kuria:akter:read`; and at `backlog`, `epic`
  and `backlog_item`: `kuria:akter:create`, `kuria:akter:edit` and
  `kuria:akter:transition`.
- **Release manager**, tenant-wide: `kuria:akter:read` and
  `kuria:contacts:manage`, to name approvers who aren't users yet; and at
  `release`,
  `release_candidate`, `hotfix` and `release_notes`: `kuria:akter:create`,
  `kuria:akter:edit`, `kuria:akter:transition`, `kuria:akter:archive`, and
  `release:approve`.

And one for software:

- **Build pipeline**, at `artifact`: `kuria:akter:read` and
  `kuria:akter:create`. It uploads SBOMs and registers artifacts holding
  them; only it can attach its uploads until an artifact holds them, and
  then whoever reads artifacts can download them.

A release manager can put items into release candidates and hotfixes,
since that needs `edit` at the container and `read` at the item. A
developer can't create a release candidate.
