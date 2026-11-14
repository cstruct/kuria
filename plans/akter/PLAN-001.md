---
number: PLAN-001
type: plan
revision: 55
rank: a0
state: done
title: "Nesting, Boards and Releases"
---

<!-- field: goal -->
After this milestone a tenant can, with configuration alone:

- keep a **backlog** akt that contains backlog items and epics, and an
  **epic** that contains backlog items, with one backlog item in a backlog,
  an epic and a release at the same time;
- reorder a backlog item on any board and see the same relative order on
  every other board it is on: moving it in the backlog moves it in the epic,
  and the other way round;
- see each container as a **board** whose columns its akt type configures,
  mapping the states of its different child akt types into the same columns;
- run a **release** `RELEASE-2026-0001` that contains backlogs, release
  candidates `RELEASE-2026-0001-RC-0001`, hotfixes
  `RELEASE-2026-0001-HOTFIX-0001` and a release-notes akt, where release
  candidates and hotfixes in turn contain epics and backlog items;
- draft the release notes from what the release and its release candidates
  and hotfixes contain, then edit them like any akt;
- read from every akt's history when it was put into or taken out of which
  container, by whom and why.

Success is the end-to-end test of task T8 passing: it sets up the example
release-management configuration and walks a release from planning through
two release candidates, the release and a hotfix, checking numbers, boards,
ordering, history and permissions along the way.

<!-- field: design -->
## Constraints And Assumptions

- Builds on what exists (see `docs/spec/akter.md`, `docs/spec/authorization.md`,
  `docs/architecture.md`):
  - Akt types are whole documents submitted through `defineAktType`; each
    submission is an immutable akt type version, and an akt keeps the
    version it was created with.
  - Every change to an akt is an append-only akt revision with a change
    event, written in one transaction by `items/server/src/akter.rs`, the
    only code that writes akt tables.
  - Permissions are Kuria actions in Konklav's catalog, required
    tenant-wide or at the scope `kuria_akt_type:<key>`.
  - Akt numbers come from a number pattern, counting per tenant and per
    pattern as rendered without its sequence.
- Everything is generic. "Backlog", "epic", "release" and so on are akt types
  in an example configuration, never names in Kuria's code. The only new
  built-in ideas are containers, membership, rank and boards.
- Nesting is the link kind `child of` described in the nomenclature. This
  milestone implements containment only; other link kinds (`verifies`,
  `originated from`) come later and must fit beside it.
- Specs first, as `AGENTS.md` requires: each task starts with its Tracey
  rules in `docs/spec/`, and new terms go into `docs/nomenclature.md` first.

## Design

### Terms (to add to the nomenclature in T1)

- **container**: an akt whose akt type allows it to contain other akter.
- **member**: an akt in a container. An akt may be a member of many
  containers at once.
- **membership**: one akt being in one container; created and ended by
  mutations, and recorded in both akter's histories.
- **rank**: an akt's place in its tenant's single priority order. Every
  board orders its members by rank.
- **board**: a container shown as columns, which its akt type configures.

### Configuration

An akt type document gains three optional members, and number patterns gain
one placeholder.

```json
{
  "key": "release_candidate",
  "name": "Release candidate",
  "number": "{parent}-RC-{seq:4}",
  "created_in": ["release"],
  "contains": [
    { "type": "epic" },
    { "type": "backlog_item" }
  ],
  "board": {
    "columns": [
      { "key": "todo", "name": "To do",
        "states": { "backlog_item": ["open"], "epic": ["open"] } },
      { "key": "testing", "name": "In test",
        "states": { "backlog_item": ["in_test"] } },
      { "key": "done", "name": "Done",
        "states": { "backlog_item": ["done"], "epic": ["done"] } }
    ]
  }
}
```

- `contains`: the akt types whose akter this akt type's akter may contain.
  An entry may say `"exclusive": true`: then an akt can be in at most one
  container of this type at a time (an item in one backlog, say).
- `created_in`: the akt types an akt of this type must be created inside.
  Creating it then names the container, and it becomes a member at once.
- `{parent}` in the number pattern: the number of the container the akt was
  created in. It is only valid together with `created_in`. Because the
  pattern's scope (everything but the sequence) then includes the parent's
  number, `RELEASE-2026-0001-RC-0001` and `RELEASE-2026-0002-RC-0001` count
  separately, which is exactly the numbering asked for.
- `board.columns`: each column names, per child akt type, the states that
  fall in it. The document can only check that those akt types appear in
  `contains`. The states belong to other akt types, which can change, so a
  member in a state no column names shows in an extra "Unmapped" column
  rather than disappearing.
- A field may declare `"draft"`, a recipe for filling it from the
  container's members (task T7).

### Storage

- `akt_membership (tenant_id, container_id, member_id, added_at, added_by,
  primary key (container_id, member_id))`: current memberships only. Their
  history is in the revisions.
- `akt.rank text not null`, unique per tenant: a fractional index, a
  base-62 string that sorts the akter of a tenant. New akter go after the
  last.
- `akt_revision` gains `membership jsonb null`:
  `{"added": {"container": "<id>", "number": "EPIC-0003"}}` on the member and
  `{"added_member": {...}}` on the container (with `removed` and
  `removed_member` the same way). A membership revision changes no fields,
  so its state hash equals the previous revision's.
- `change_event` gains `kind` (`revision` or `rank`), and `revision`
  becomes nullable for rank events, which have no revision.

### Rules

- Containment is acyclic: an akt can't be put into itself or into any akt
  it contains, directly or not. Checked in the same transaction with a
  recursive query, under the container's row lock.
- The membership of a container in a final state is frozen: a released
  release can't gain or lose members. A member in a final state can still be
  added to or removed from an open container.
- Adding or removing a member locks both akter, appends one revision to
  each, and writes two change events, in one transaction. It does not
  take an expected revision, because a board drag shouldn't fail because
  someone edited the item. The locks keep it consistent.
- **Moving** an akt "before Y" or "after Y" gives it a rank directly before
  or after Y in the tenant-wide order, between Y and Y's neighbour in that
  order. Since every board sorts by the same rank, the akt lands next to Y
  on every board that shows both, and other boards change as little as
  possible. A move is not a revision. It writes a `rank` change event so
  that open boards update.
- Rank collisions from concurrent moves are prevented by the unique index:
  a move that collides retries with a fresh key.

### Authorization

New requirements, all at the akt type scope as before:

- Adding to or removing from a container: `kuria:akter:edit` at the
  container's type, and `kuria:akter:read` at the member's type. Nobody can
  add an akt they can't see.
- Creating an akt inside a container (`created_in`): `kuria:akter:create`
  at the new akt's type, and `kuria:akter:edit` at the container's type.
- Moving: `kuria:akter:edit` at the moved akt's type.
- Lists of members and containers leave out akter the subject can't read,
  as `akter` does now.

### GraphQL

- `Akt.members(type, state, first)`: in rank order.
- `Akt.containers`: the containers it is in.
- `Akt.board`: columns, each with its members in rank order, plus
  "Unmapped".
- `createAkt(input: { …, in: ["RELEASE-2026-0001"] })`: created as a member
  of the containers named. The first one counts as the parent for
  `created_in` and `{parent}`.
- `addToContainer(container, member, reason)` and
  `removeFromContainer(container, member, reason)`.
- `moveAkt(number, before | after)`.
- `draftField(number, revision, field)` (T7).

<!-- field: verification -->
- `mise run test` runs every unit test and database test, including
  `release_flow.rs`, against the development Postgres and Valkey.
- `mise run lint`, including `tracey query validate`. Every new rule in
  `containers.md`, `ordering.md` and `boards.md` must have an `r[impl]` and
  an `r[verify]`; check with `tracey query untested`.
- Behaviour a person can check through GraphQL against a running server
  with the example configuration:
  - create `RELEASE-<year>-0001`, then an RC inside it, and see the number
    `RELEASE-<year>-0001-RC-0001`;
  - put one backlog item in a backlog and an epic, move it to the top of the
    backlog, and see it at the top of the epic's board;
  - try putting an epic inside a backlog item it contains, and get `CYCLE`;
  - release the release, then try adding to it, and get `FROZEN`;
  - read an item's `revisions` and see the membership changes with author
    and reason.
