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

<!-- akt: PLAN-001-T01 -->
---
type: task
revision: 3
rank: a1
state: done
in: [PLAN-001]
title: T1 Spec and nomenclature
---

<!-- field: description -->
Spec and nomenclature: `docs/spec/containers.md`, `ordering.md`,
`boards.md`, additions to `akter.md` and `authorization.md` (25 new rules,
none implemented yet), and the containment terms in the nomenclature
(`6f59069`).

<!-- akt: PLAN-001-T02 -->
---
type: task
revision: 3
rank: a2
state: done
in: [PLAN-001]
title: T2 Configuration
---

<!-- field: description -->
Configuration: `contains` with `exclusive`, `created_in`, `board`
columns, and `{parent}` in number patterns through `NumberContext`, with
their validation and tests in `kuria-config`. The server passes no parent
yet, which T4.3 adds (`829eadc`).

<!-- akt: PLAN-001-T03 -->
---
type: task
revision: 3
rank: a3
state: done
in: [PLAN-001]
title: "T3 Storage, with T4.1 brought forward"
---

<!-- field: description -->
Storage, with T4.1 brought forward: rank keys in `kuria-engine`
(`rank_between`, `nth_appended`), the containment migration (ranks
backfilled in creation order, `akt_membership`, membership revisions, rank
change events), the queries in `kuria_store::containers`, and every new
akt appended last under the tenant's order lock (`9d74a88`).

<!-- akt: PLAN-001-T04 -->
---
type: task
revision: 3
rank: a4
state: done
in: [PLAN-001]
title: T4 Write path and T5 Authorization
---

<!-- field: description -->
Write path and T5 Authorization: adding and removing members,
creating inside containers with `{parent}` numbers, and moves, in
`items/server/src/akter/{create,membership,ordering}.rs`, with their
permissions. T6.2's mutations and T6.1's `members` and `containers` came
along, to test through GraphQL; boards remain (`b46b5e0`).

<!-- akt: PLAN-001-T05 -->
---
type: task
revision: 3
rank: a5
state: done
in: [PLAN-001]
title: T6 GraphQL
---

<!-- field: description -->
GraphQL: `Akt.board` with configured columns and `unmapped`,
placement in `kuria_engine::board`, and members loaded in a fixed number
of queries. Every rule of the milestone's specs is implemented and
verified (`6b6fad5`).

<!-- akt: PLAN-001-T06 -->
---
type: task
revision: 3
rank: a6
state: done
in: [PLAN-001]
title: T7 Drafting
---

<!-- field: description -->
Drafting: `docs/spec/drafts.md`, `draft` recipes on multiline text
fields in `kuria-config`, rendering in `kuria_engine::draft`, and
`draftField`, which saves the draft as an ordinary edit (`db020b5`).

<!-- akt: PLAN-001-T07 -->
---
type: task
revision: 3
rank: a7
state: done
in: [PLAN-001]
title: "T8 The example configuration in `docs/examples/release-management` (seven akt types and a README with…"
---

<!-- field: description -->
The example configuration in `docs/examples/release-management`
(seven akt types and a README with the roles to grant), and
`items/server/tests/release_flow.rs`, which walks a release from planning
to closed with it (`9ea48e6`).

<!-- akt: PLAN-001-T08 -->
---
type: task
revision: 3
rank: a8
state: done
in: [PLAN-001]
title: T9 Docs
---

<!-- field: description -->
Docs: `docs/architecture.md` brought in line with the code (crates,
write path, schema, storage, a section on containers, order and boards,
decided and open questions), a coverage table in `docs/requirements.md`,
and the item READMEs checked.

<!-- akt: PLAN-001-T09 -->
---
type: task
revision: 3
rank: a9
state: done
in: [PLAN-001]
title: "T5.1 Permission variants for membership, create-in and moves"
---

<!-- field: description -->
Permission variants for membership, create-in and moves.

<!-- akt: PLAN-001-T10 -->
---
type: task
revision: 3
rank: aA
state: done
in: [PLAN-001]
title: T5.2 Tests with the fake decider
---

<!-- field: description -->
Tests with the fake decider: refused without each action,
members you can't read left out, nobody adding an akt they can't see.

<!-- akt: PLAN-001-G01 -->
---
type: gap
revision: 2
rank: aB
state: closed
in: [PLAN-001]
title: "G1 Change events are not yet published to Valkey, so boards can't update live"
---

<!-- field: description -->
Change events are not yet published to Valkey, so boards can't
update live. Rank events are written to the outbox like the others and
will be delivered once publishing lands; that is a separate milestone.

<!-- field: resolution -->
Change events are not yet published to Valkey, so boards can't
update live. Rank events are written to the outbox like the others and
will be delivered once publishing lands; that is a separate milestone.

<!-- akt: PLAN-001-G02 -->
---
type: gap
revision: 2
rank: aC
state: closed
in: [PLAN-001]
title: "G2 Akter keep their akt type version, but board columns name states of *other* akt types, whose newer…"
---

<!-- field: description -->
Akter keep their akt type version, but board columns name states of
*other* akt types, whose newer versions may rename states. Unmapped
members show in an extra column; whether columns should name akt type
versions is left open.

<!-- field: resolution -->
Akter keep their akt type version, but board columns name states of
*other* akt types, whose newer versions may rename states. Unmapped
members show in an extra column; whether columns should name akt type
versions is left open.

<!-- akt: PLAN-001-G03 -->
---
type: gap
revision: 2
rank: aD
state: closed
in: [PLAN-001]
title: G3 Decided in T4
---

<!-- field: description -->
Decided in T4: a release candidate's members need not be in its
release. See decision notes.

<!-- field: resolution -->
Decided in T4: a release candidate's members need not be in its
release. See decision notes.

<!-- akt: PLAN-001-G04 -->
---
type: gap
revision: 2
rank: aE
state: closed
in: [PLAN-001]
title: "G4 Moving an akt between two containers of an exclusive type (from one backlog to another) is a remove…"
---

<!-- field: description -->
Moving an akt between two containers of an exclusive type (from
one backlog to another) is a remove and an add. A single `moveTo` may be
nicer; not in this milestone.

<!-- field: resolution -->
Moving an akt between two containers of an exclusive type (from
one backlog to another) is a remove and an add. A single `moveTo` may be
nicer; not in this milestone.

<!-- akt: PLAN-001-G05 -->
---
type: gap
revision: 2
rank: aF
state: closed
in: [PLAN-001]
title: G5 Settled in T7
---

<!-- field: description -->
Settled in T7: `through`, `depth`, `types`, `line` and
`group_by`. See decision notes.

<!-- field: resolution -->
Settled in T7: `through`, `depth`, `types`, `line` and
`group_by`. See decision notes.

<!-- akt: PLAN-001-G06 -->
---
type: gap
revision: 2
rank: aG
state: closed
in: [PLAN-001]
title: "G6 A draft recipe can't filter by state, so release notes include what a rejected release candidate co…"
---

<!-- field: description -->
A draft recipe can't filter by state, so release notes include
what a rejected release candidate contained. The walk-through's rejected
candidate held nothing that didn't ship in the next one, but a filter such
as `"states": { "release_candidate": ["accepted"] }` in `through` is
likely wanted.

<!-- field: resolution -->
A draft recipe can't filter by state, so release notes include
what a rejected release candidate contained. The walk-through's rejected
candidate held nothing that didn't ship in the next one, but a filter such
as `"states": { "release_candidate": ["accepted"] }` in `through` is
likely wanted.

<!-- akt: PLAN-001-G07 -->
---
type: gap
revision: 2
rank: aH
state: closed
in: [PLAN-001]
title: "G7 A container can't say in which of its states it accepts members, so a released release still takes…"
---

<!-- field: description -->
A container can't say in which of its states it accepts members, so
a released release still takes new release candidates, not only hotfixes.
A per-`contains` `"while": ["planning", "testing"]` would close it.

<!-- field: resolution -->
A container can't say in which of its states it accepts members, so
a released release still takes new release candidates, not only hotfixes.
A per-`contains` `"while": ["planning", "testing"]` would close it.

<!-- akt: PLAN-001-D01 -->
---
type: decision
revision: 2
rank: aI
state: decided
in: [PLAN-001]
title: One shared priority
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**One shared priority.** Every akt has one rank in its tenant, and every
board sorts by it. Moving anywhere moves everywhere, so boards never
disagree on relative order. The cost is that two boards can't hold
conflicting orders, which is the point. Moves place the akt relative to a
neighbour, so other boards are disturbed as little as possible.

<!-- field: why -->
Every akt has one rank in its tenant, and every
board sorts by it. Moving anywhere moves everywhere, so boards never
disagree on relative order. The cost is that two boards can't hold
conflicting orders, which is the point. Moves place the akt relative to a
neighbour, so other boards are disturbed as little as possible.

<!-- akt: PLAN-001-D02 -->
---
type: decision
revision: 2
rank: aJ
state: decided
in: [PLAN-001]
title: "Membership is a record, order is not"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Membership is a record, order is not.** Being in a release, an RC or a
hotfix is part of what a release record must show, so it is an akt
revision, on both the member and the container. Reordering is planning
noise and would flood histories, so it is only a change event.

<!-- field: why -->
Being in a release, an RC or a
hotfix is part of what a release record must show, so it is an akt
revision, on both the member and the container. Reordering is planning
noise and would flood histories, so it is only a change event.

<!-- akt: PLAN-001-D03 -->
---
type: decision
revision: 2
rank: aK
state: decided
in: [PLAN-001]
title: Release notes are an akt drafted from members
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Release notes are an akt drafted from members.** This keeps them under
the same revision, approval and (later) signature rules as any record,
rather than inventing a special document.

<!-- field: why -->
This keeps them under
the same revision, approval and (later) signature rules as any record,
rather than inventing a special document.

<!-- akt: PLAN-001-D04 -->
---
type: decision
revision: 2
rank: aL
state: decided
in: [PLAN-001]
title: "Board columns live on the container's akt type"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Board columns live on the container's akt type.** A board mixes akt
types whose states differ, so only the container can say how they line
up.

<!-- field: why -->
A board mixes akt
types whose states differ, so only the container can say how they line
up.

<!-- akt: PLAN-001-D05 -->
---
type: decision
revision: 2
rank: aM
state: decided
in: [PLAN-001]
title: "Release, RC and hotfix are configuration, not code"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Release, RC and hotfix are configuration, not code.** The numbering
`RELEASE-2026-0001-RC-0001` comes from the generic `{parent}` placeholder
and `created_in`, so other hierarchies (a test protocol and its runs, say)
get the same mechanism.

<!-- field: why -->
The numbering
`RELEASE-2026-0001-RC-0001` comes from the generic `{parent}` placeholder
and `created_in`, so other hierarchies (a test protocol and its runs, say)
get the same mechanism.

<!-- akt: PLAN-001-D06 -->
---
type: decision
revision: 2
rank: aN
state: decided
in: [PLAN-001]
title: Membership changes take no expected revision
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Membership changes take no expected revision.** A board drag shouldn't
fail because someone edited the item's text. Row locks on both akter keep
the change consistent, and the revisions record it.

<!-- field: why -->
A board drag shouldn't
fail because someone edited the item's text. Row locks on both akter keep
the change consistent, and the revisions record it.

<!-- akt: PLAN-001-D07 -->
---
type: decision
revision: 2
rank: aO
state: decided
in: [PLAN-001]
title: Containment is the one built-in link kind
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Containment is the one built-in link kind.** The nomenclature used to
say nesting is just the link kind `child of`. Boards, ranks and `{parent}`
numbering depend on it, so it is built in, with `container` and `member`
as its terms. Other link kinds stay runtime configuration (T1).

<!-- field: why -->
The nomenclature used to
say nesting is just the link kind `child of`. Boards, ranks and `{parent}`
numbering depend on it, so it is built in, with `container` and `member`
as its terms. Other link kinds stay runtime configuration (T1).

<!-- akt: PLAN-001-D08 -->
---
type: decision
revision: 2
rank: aP
state: decided
in: [PLAN-001]
title: A state falls in at most one column
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A state falls in at most one column.** `board.columns` refuses a state
of one akt type in two columns, so a member is never shown twice on one
board (T1).

<!-- field: why -->
`board.columns` refuses a state
of one akt type in two columns, so a member is never shown twice on one
board (T1).

<!-- akt: PLAN-001-D09 -->
---
type: decision
revision: 2
rank: aQ
state: decided
in: [PLAN-001]
title: "Unreadable containers and members are `NOT_FOUND`"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Unreadable containers and members are `NOT_FOUND`.** Arranging, moving
and creating inside follow `authz.read`: nobody learns of an akt they
can't read through a refusal (T1).

<!-- field: why -->
Arranging, moving
and creating inside follow `authz.read`: nobody learns of an akt they
can't read through a refusal (T1).

<!-- akt: PLAN-001-D10 -->
---
type: decision
revision: 2
rank: aR
state: decided
in: [PLAN-001]
title: "Column keys can't be `unmapped`"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Column keys can't be `unmapped`.** That key is the extra column for
members no column names, so `board.columns` reserves it (T2).

<!-- field: why -->
That key is the extra column for
members no column names, so `board.columns` reserves it (T2).

<!-- akt: PLAN-001-D11 -->
---
type: decision
revision: 2
rank: aS
state: decided
in: [PLAN-001]
title: "Numbers render from a `NumberContext`"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Numbers render from a `NumberContext`.** Year, month and parent travel
together, so adding a placeholder later changes one type, not every
caller (T2).

<!-- field: why -->
Year, month and parent travel
together, so adding a placeholder later changes one type, not every
caller (T2).

<!-- akt: PLAN-001-D12 -->
---
type: decision
revision: 2
rank: aT
state: decided
in: [PLAN-001]
title: Rank keys are fractional indexing with an integer part
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Rank keys are fractional indexing with an integer part.** Plain
midpoint keys grow about one character every six appends, and almost
every akt is appended. The integer-part scheme (as in the
`fractional-indexing` library) keeps 5,000 appends at four characters,
and still always finds a key between two others (T3).

<!-- field: why -->
Plain
midpoint keys grow about one character every six appends, and almost
every akt is appended. The integer-part scheme (as in the
`fractional-indexing` library) keeps 5,000 appends at four characters,
and still always finds a key between two others (T3).

<!-- akt: PLAN-001-D13 -->
---
type: decision
revision: 2
rank: aU
state: decided
in: [PLAN-001]
title: "Ranks sort in the \"C\" collation"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Ranks sort in the "C" collation.** Keys use `0-9A-Za-z` in ASCII order;
most collations sort `Zz` after `a0` (T3).

<!-- field: why -->
Keys use `0-9A-Za-z` in ASCII order;
most collations sort `Zz` after `a0` (T3).

<!-- akt: PLAN-001-D14 -->
---
type: decision
revision: 2
rank: aV
state: decided
in: [PLAN-001]
title: One order lock per tenant
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**One order lock per tenant.** Every rank change, whether a new akt or a
move, takes a counter row lock first, so two never compute the same key.
The unique index stays as a guard. This makes T4.4's retry unnecessary;
`order.concurrent` holds by the lock (T3).

<!-- field: why -->
Every rank change, whether a new akt or a
move, takes a counter row lock first, so two never compute the same key.
The unique index stays as a guard. This makes T4.4's retry unnecessary;
`order.concurrent` holds by the lock (T3).

<!-- akt: PLAN-001-D15 -->
---
type: decision
revision: 2
rank: aW
state: decided
in: [PLAN-001]
title: Membership revisions name the other akt
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Membership revisions name the other akt** as
`{"added": {"akt", "number"}}` on the member and `added_member` on the
container, in a nullable `membership` column, rather than as fields (T3).

<!-- field: why -->
as
`{"added": {"akt", "number"}}` on the member and `added_member` on the
container, in a nullable `membership` column, rather than as fields (T3).

<!-- akt: PLAN-001-D16 -->
---
type: decision
revision: 2
rank: aX
state: decided
in: [PLAN-001]
title: "Release candidates aren't limited to their release's members (G3)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Release candidates aren't limited to their release's members (G3).**
An RC often takes a hotfix's item or a late fix that was never in the
release's backlog, and a rule tying them would need a notion of "reachable
through which link kinds" that doesn't exist yet. It can come later as a
configured condition rather than a built-in rule (T4).

<!-- field: why -->
An RC often takes a hotfix's item or a late fix that was never in the
release's backlog, and a rule tying them would need a notion of "reachable
through which link kinds" that doesn't exist yet. It can come later as a
configured condition rather than a built-in rule (T4).

<!-- akt: PLAN-001-D17 -->
---
type: decision
revision: 2
rank: aY
state: decided
in: [PLAN-001]
title: Membership revisions hold lists
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Membership revisions hold lists**, `{"added": [{"akt", "number"}]}`, so
creating an akt inside several containers records them in one revision
(T4).

<!-- field: why -->
, `{"added": [{"akt", "number"}]}`, so
creating an akt inside several containers records them in one revision
(T4).

<!-- akt: PLAN-001-D18 -->
---
type: decision
revision: 2
rank: aZ
state: decided
in: [PLAN-001]
title: One lock order for every write path
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**One lock order for every write path**: the tenant's order lock, akt rows
by id, number counters, the change-event counter. Creating takes the order
lock before locking its containers, so it can't deadlock with a move (T4).

<!-- field: why -->
the tenant's order lock, akt rows
by id, number counters, the change-event counter. Creating takes the order
lock before locking its containers, so it can't deadlock with a move (T4).

<!-- akt: PLAN-001-D19 -->
---
type: decision
revision: 2
rank: aa
state: decided
in: [PLAN-001]
title: A move to where the akt already is changes nothing
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A move to where the akt already is changes nothing** and writes no
event (T4).

<!-- field: why -->
and writes no
event (T4).

<!-- akt: PLAN-001-D20 -->
---
type: decision
revision: 2
rank: ab
state: decided
in: [PLAN-001]
title: "The `unmapped` column is always there"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**The `unmapped` column is always there**, empty or not, so a client can
render a board without first asking whether anything is unmapped (T6).

<!-- field: why -->
, empty or not, so a client can
render a board without first asking whether anything is unmapped (T6).

<!-- akt: PLAN-001-D21 -->
---
type: decision
revision: 2
rank: ac
state: decided
in: [PLAN-001]
title: "A board shows at most 1,000 members"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A board shows at most 1,000 members** (500 by default). Larger
containers page through `members`; paging a board is left until a
frontend needs it (T6).

<!-- field: why -->
(500 by default). Larger
containers page through `members`; paging a board is left until a
frontend needs it (T6).

<!-- akt: PLAN-001-D22 -->
---
type: decision
revision: 2
rank: ad
state: decided
in: [PLAN-001]
title: "The draft recipe (G5)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**The draft recipe (G5)** names which members to look further inside
(`through`), how deep (`depth`), what to list (`types`), how each line
reads (`line`, with `{number}`, `{type}`, `{state}` and `{field:<key>}`),
and whether to group by akt type. Release notes look through release
candidates, hotfixes and epics, so a backlog's planned but unshipped items
stay out (T7).

<!-- field: why -->
names which members to look further inside
(`through`), how deep (`depth`), what to list (`types`), how each line
reads (`line`, with `{number}`, `{type}`, `{state}` and `{field:<key>}`),
and whether to group by akt type. Release notes look through release
candidates, hotfixes and epics, so a backlog's planned but unshipped items
stay out (T7).

<!-- akt: PLAN-001-D23 -->
---
type: decision
revision: 2
rank: ae
state: decided
in: [PLAN-001]
title: "The akt's own containers are always looked inside"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**The akt's own containers are always looked inside**; `through` applies
from there down. Otherwise release notes would need `release` in
`through` and would then also look inside other releases' members (T7).

<!-- field: why -->
; `through` applies
from there down. Otherwise release notes would need `release` in
`through` and would then also look inside other releases' members (T7).

<!-- akt: PLAN-001-D24 -->
---
type: decision
revision: 2
rank: af
state: decided
in: [PLAN-001]
title: A draft only shows what its drafter can read
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A draft only shows what its drafter can read**, and doesn't look inside
what they can't. It becomes part of a record others read, so it mustn't
carry what the drafter couldn't see (T7).

<!-- field: why -->
, and doesn't look inside
what they can't. It becomes part of a record others read, so it mustn't
carry what the drafter couldn't see (T7).

<!-- akt: PLAN-001-D25 -->
---
type: decision
revision: 2
rank: ag
state: decided
in: [PLAN-001]
title: "Drafting is computed, then saved through `edit`"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Drafting is computed, then saved through `edit`.** The draft is read
outside the edit's transaction; if a member changes in between, the
draft is as of the moment it was read, which is what a draft is (T7).

<!-- field: why -->
The draft is read
outside the edit's transaction; if a member changes in between, the
draft is as of the moment it was read, which is what a draft is (T7).

<!-- akt: PLAN-001-D26 -->
---
type: decision
revision: 2
rank: ah
state: decided
in: [PLAN-001]
title: "`released` isn't final; `closed` is"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**`released` isn't final; `closed` is.** A released release must still
take hotfixes, and a final container is frozen, so the example's release
goes planning, testing, released, then closed when maintenance ends. The
plan's walk-through had "release, check frozen, then hotfix", which can't
work (T8).

<!-- field: why -->
A released release must still
take hotfixes, and a final container is frozen, so the example's release
goes planning, testing, released, then closed when maintenance ends. The
plan's walk-through had "release, check frozen, then hotfix", which can't
work (T8).

<!-- akt: PLAN-001-D27 -->
---
type: decision
revision: 2
rank: ai
state: decided
in: [PLAN-001]
title: The walk-through reads the example files directly
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**The walk-through reads the example files directly** (`include_str!`), so
the example can't drift from what Kuria accepts without the test failing
(T8).

<!-- field: why -->
(`include_str!`), so
the example can't drift from what Kuria accepts without the test failing
(T8).

<!-- akt: PLAN-001-D28 -->
---
type: decision
revision: 2
rank: aj
state: decided
in: [PLAN-001]
title: Transfers are their own operation
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Transfers are their own operation**, not a remove and an add, so that an
exclusive membership can move without a moment in neither container, and
the akt's history shows one change rather than two (G4).

<!-- field: why -->
, not a remove and an add, so that an
exclusive membership can move without a moment in neither container, and
the akt's history shows one change rather than two (G4).

<!-- akt: PLAN-001-D29 -->
---
type: decision
revision: 2
rank: ak
state: decided
in: [PLAN-001]
title: "`while` restricts gaining members only"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**`while` restricts gaining members only.** Leaving a container is
governed by `frozen` alone: a sprint that has started may still drop an
item (G7).

<!-- field: why -->
Leaving a container is
governed by `frozen` alone: a sprint that has started may still drop an
item (G7).

<!-- akt: PLAN-001-D30 -->
---
type: decision
revision: 2
rank: al
state: decided
in: [PLAN-001]
title: "A draft's `states` also stop it looking inside"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A draft's `states` also stop it looking inside**, not only listing, so a
rejected candidate's members don't come in through it (G6).

<!-- field: why -->
, not only listing, so a
rejected candidate's members don't come in through it (G6).

<!-- akt: PLAN-001-D31 -->
---
type: decision
revision: 2
rank: am
state: decided
in: [PLAN-001]
title: "Valkey wakes, Postgres orders (G1)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Valkey wakes, Postgres orders (G1).** Instances may publish in a
different order than sequence numbers, so a stream entry is only a
wake-up; the hub reads the tenant's events after the last it broadcast,
in order. Sequence numbers follow commit order because taking one locks
the counter until commit, so nothing is skipped.

<!-- field: why -->
Instances may publish in a
different order than sequence numbers, so a stream entry is only a
wake-up; the hub reads the tenant's events after the last it broadcast,
in order. Sequence numbers follow commit order because taking one locks
the counter until commit, so nothing is skipped.

<!-- akt: PLAN-001-D32 -->
---
type: decision
revision: 2
rank: an
state: decided
in: [PLAN-001]
title: "Catching up reads Postgres, not the stream (G1)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Catching up reads Postgres, not the stream (G1).** It is exact, has no
window, and is one code path. The streams stay bounded, so a window of
recent events is there if something later wants it.

<!-- field: why -->
It is exact, has no
window, and is one code path. The streams stay bounded, so a window of
recent events is there if something later wants it.

<!-- akt: PLAN-001-D33 -->
---
type: decision
revision: 2
rank: ao
state: decided
in: [PLAN-001]
title: "Every instance relays, with `skip locked` (G1)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Every instance relays, with `skip locked` (G1)**, rather than one under
an advisory lock: an instance's own writes publish at once, and any
instance picks up what another left.

<!-- field: why -->
, rather than one under
an advisory lock: an instance's own writes publish at once, and any
instance picks up what another left.

<!-- akt: PLAN-001-D34 -->
---
type: decision
revision: 2
rank: ap
state: decided
in: [PLAN-001]
title: "A subscription ends at token expiry (G1)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**A subscription ends at token expiry (G1)**, so a socket never outlives
what authorized it; clients reconnect with `after`.

<!-- field: why -->
, so a socket never outlives
what authorized it; clients reconnect with `after`.

<!-- akt: PLAN-001-D35 -->
---
type: decision
revision: 2
rank: aq
state: decided
in: [PLAN-001]
title: "Refuse, rather than version, state references (G2)"
---

<!-- field: context -->
Recorded in the plan "Nesting, Boards and Releases", migrated from `.agents/plans/nesting-and-releases.md`.

<!-- field: decision -->
**Refuse, rather than version, state references (G2).** Board columns
could have named akt type versions, but that would have every container
pinned to old member versions. Checking both directions at definition
keeps references true for the latest versions, and the order to change
things is plain: stop naming a state, then drop it. Members of older
versions in a dropped state show as `unmapped`, which is honest (G2).

<!-- field: why -->
Board columns
could have named akt type versions, but that would have every container
pinned to old member versions. Checking both directions at definition
keeps references true for the latest versions, and the order to change
things is plain: stop naming a state, then drop it. Members of older
versions in a dropped state show as `unmapped`, which is honest (G2).
