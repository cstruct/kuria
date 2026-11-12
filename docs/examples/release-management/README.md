# Release management

An example configuration: seven akt types for running a product's backlog
and releases in Kuria. Nothing in Kuria knows these names; they are
ordinary akt type documents, and a tenant can change them or start from
others.

| Akt type | Number | Contains | Created in |
|---|---|---|---|
| [`backlog_item`](backlog_item.json) | `ITEM-0001` | | |
| [`epic`](epic.json) | `EPIC-001` | backlog items, each in one epic | |
| [`backlog`](backlog.json) | `BACKLOG-001` | backlog items, each in one backlog, and epics | |
| [`release`](release.json) | `RELEASE-2026-0001` | backlogs, release candidates, hotfixes, one set of release notes | |
| [`release_candidate`](release_candidate.json) | `RELEASE-2026-0001-RC-0001` | epics and backlog items | a release |
| [`hotfix`](hotfix.json) | `RELEASE-2026-0001-HOTFIX-0001` | backlog items | a release |
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
- The release notes are drafted from what the release's accepted candidates
  and deployed hotfixes contain, including the items inside their epics,
  but not from its backlogs, whose items may not have shipped, nor from
  rejected candidates. The draft is then edited and approved.

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
- **Release manager**, tenant-wide: `kuria:akter:read`; and at `release`,
  `release_candidate`, `hotfix` and `release_notes`: `kuria:akter:create`,
  `kuria:akter:edit`, `kuria:akter:transition`, and `release:approve`.

A release manager can put items into release candidates and hotfixes,
since that needs `edit` at the container and `read` at the item. A
developer can't create a release candidate.
