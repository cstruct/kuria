---
number: PLAN-011
type: plan
revision: 11
rank: b3A
state: active
title: "Batches: several akt writes in one transaction"
owner: albin@mattsson.io
---

<!-- field: goal -->
Kuria's GraphQL API takes a `batch` of akt writes and runs them in order in one transaction: all of them, or none. Any write the API offers one at a time can be in a batch, and a later operation can name an akt an earlier one created (`@2`). A client can then make a change that spans akter, such as editing a whole plan, or creating a task and linking it, without leaving half of it behind when one part is refused.

Success: a batch whose last operation is refused changes nothing, and its answer names that operation; a batch that creates a task in a plan, links it and edits another task commits as one, its change events published after it; batches and single writes running at once never deadlock; and `kuria apply` of a multi-akt file sends one batch, so a refusal anywhere changes nothing on a server, as in a directory (PLAN-010-G01).

<!-- field: design -->
## Where it starts

- `items/server/src/akter.rs` is the one write path. Each write (`create`, `edit`, `transition`, `add_member`, `remove_member`, `transfer`, `add_link`, `remove_link`, `move_akt`, `draft`) opens its own transaction, takes its locks, asks the engine, writes the akt, its revision and its change event (`akt.revision.atomic`), and wakes the relay after it commits.
- Locks are taken in one order, so no two changes deadlock: the tenant's order lock, then akt rows (by id), then number counters, then the change-event counter. Every change takes the change-event counter, a row per tenant, and holds it until it commits.
- Every akt mutation in `items/server/src/graphql/akter.rs` answers the `Akt` it changed.
- The CLI's `apply` checks every akt first, then edits each with its own `editAkt` (PLAN-010-D04), so a refusal only the server makes while editing comes after earlier edits (PLAN-010-G01).

## Constraints

- **The same rules:** each operation is checked, authorized and recorded exactly as the mutation it stands for, with its own revisions and change events.
- **No deadlocks:** a batch takes the module's locks in its order, though its operations would take them one after another.
- **No behaviour change** for single writes.

## Design

### The mutation

- `batch(input: { operations: [Operation!]! }): [Akt!]!`, `Operation` a one-of input with one member per akt mutation, each taking that mutation's input: `createAkt`, `editAkt`, `performTransition`, `addToContainer`, `removeFromContainer`, `transferMember`, `addLink`, `removeLink`, `moveAkt`, `draftField` (decided: PLAN-011-D01).
- It answers each operation's `Akt`, in order.
- At most 200 operations; more is refused as `BAD_REQUEST`.

### All or nothing

- The operations run in order in one transaction. The first refusal rolls it all back, and the error says which operation, by its position from 1 (`extensions.operation`), with that operation's code and message.
- Each operation's change events are written with it; the relay is woken once, after the batch commits.

### Naming what an earlier operation created

- Wherever an operation takes an akt's number, `@<n>` names the akt that operation `n`, an earlier `createAkt`, created (decided: PLAN-011-D02). `@<n>` naming anything else, or a later operation, is refused before anything runs.
- An edit or transition names the revision it expects as it will be after the operations before it (decided: PLAN-011-D04).

### Locks

- Before any operation runs, the batch takes the tenant's order lock if any operation creates or moves, then locks every existing akt any operation names, by id. Then it runs the operations, whose own locks are already held, except for akter it created, which no one else can be waiting on (decided: PLAN-011-D03).

### Inside the write path

- Each write's body moves into a function that runs in a transaction it is given, its locks unchanged; the public method opens the transaction around it, as before. The batch runs the bodies in one.

<!-- field: verification -->
- Server tests: the existing write tests pass unchanged after the bodies move; a batch refused at its last operation changes nothing and names it; `@n` references, and their refusals; each operation authorized as alone; the limit; change events in order after the commit; batches and single writes at once, from several connections, finishing without a deadlock.
- The CLI's e2e and parity tests pass with `apply` sending one batch, and a server refusal mid-file changes nothing.

<!-- akt: PLAN-011-D01 -->
---
type: decision
revision: 2
rank: b3B
state: decided
in: [PLAN-011]
title: Every akt write can be in a batch
decided_by: albin@mattsson.io
---

<!-- field: context -->
A change that spans akter, such as editing a whole plan or creating a task and wiring it up, takes several mutations, any of which can be refused after the others committed.

<!-- field: decision -->
`batch` takes any of the akt mutations, each with its own input, as a one-of `Operation`, and answers their `Akt`s in order.

<!-- field: why -->
Every akt mutation already answers an `Akt` and goes through the one write path, so a batch of all of them costs little more than one of edits and transitions, and later needs don't make a second mechanism. Defining akt types and contacts aren't akt writes and stay out.

<!-- akt: PLAN-011-D02 -->
---
type: decision
revision: 2
rank: b3C
state: decided
in: [PLAN-011]
title: "`@<n>` names what operation n created"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A batch that creates an akt and then links, moves or edits it can't name it, since its number is only known once it is created.

<!-- field: decision -->
Wherever an operation takes an akt's number, `@<n>` names the akt that an earlier `createAkt`, operation `n` counting from 1, created. Anything else `@<n>` could mean is refused before anything runs.

<!-- field: why -->
Positions are what the client knows when it builds the batch. Counting from 1 matches how the error names a refused operation. A number can't start with `@`, so the two never clash.

<!-- akt: PLAN-011-D03 -->
---
type: decision
revision: 2
rank: b3D
state: decided
in: [PLAN-011]
title: A batch takes its locks before its operations
decided_by: albin@mattsson.io
---

<!-- field: context -->
Every change takes the tenant's change-event counter, after the akt rows it locks, and holds it to the commit. A batch locking as its operations go would hold that counter while waiting for a later operation's row, which a single write holding the row and waiting for the counter would deadlock with.

<!-- field: decision -->
Before any operation, the batch takes the order lock if it creates or moves, then locks every existing akt it names by id, so that its operations only re-take locks it holds.

<!-- field: why -->
It keeps the module's one lock order, so batches and single writes never wait on each other in a circle. The akter a batch creates are new rows that no one else can hold.

<!-- akt: PLAN-011-D04 -->
---
type: decision
revision: 2
rank: b3E
state: decided
in: [PLAN-011]
title: Expected revisions count earlier operations
decided_by: albin@mattsson.io
---

<!-- field: context -->
An edit or transition names the revision it is based on, which earlier operations of the same batch may have raised, such as adding the akt to a container.

<!-- field: decision -->
Each edit and transition names the revision it expects after the operations before it in the batch, as if they had committed one by one.

<!-- field: why -->
It keeps each operation's input exactly the mutation's, and its check the same. A client building a batch knows what its own operations change.

<!-- akt: PLAN-011-T01 -->
---
type: task
revision: 6
rank: b3F
state: done
in: [PLAN-011]
links: [Blocks PLAN-011-T02]
title: Spec batches
assignee: albin@mattsson.io
---

<!-- field: description -->
A "Batches" section in `docs/spec/akter.md`, before "Change events":
- `akt.batch`: the ten akt mutations as operations, each answered and checked as itself.
- `akt.batch.atomic`: in order, in one transaction; the first refused rolls back, with its code, message and position as the extension `operation`.
- `akt.batch.refer`: `@<n>` for an earlier `createAkt`'s akt, refused as `BAD_REQUEST` before anything runs otherwise.
- `akt.batch.revision`: expected revisions after the earlier operations.
- `akt.batch.locks`: the order lock if creating or moving, then every existing akt named, by id, before any operation.
- `akt.batch.limit`: 1 to 200 operations.
- `akt.batch.published`: change events published only once it commits.

`docs/nomenclature.md` gains `batch` and `operation`. Lint passes.

<!-- field: acceptance -->
- The rules say all of PLAN-011's design.
- `mise run lint` passes.

<!-- akt: PLAN-011-T02 -->
---
type: task
revision: 7
rank: b3G
state: done
in: [PLAN-011]
links: [Blocked by PLAN-011-T01, Blocks PLAN-011-T03]
title: "Each write's body runs in a transaction it is given"
assignee: albin@mattsson.io
---

<!-- field: description -->
Each akt write's body now runs in a transaction it is given, as a free function next to its method:
- `create_in`;
- `edit_in` and `transition_in`, through the shared `change_in`;
- `add_member_in`, `remove_member_in` and `transfer_member_in`;
- `add_link_in` and `remove_link_in`;
- `move_akt_in`;
- `draft_field_in`.

Each public method opens the transaction around its body and wakes the relay after the commit, as before. Their locks are unchanged. Checks that came before the transaction, such as a transfer to the same container or a move next to itself, now run first thing inside it. `draft` reads on the given connection instead of one of its own, so in a batch it sees the batch's earlier changes. Seven of the writes were split by a script from one pattern, and `edit`, `transition` and `draft_field` by hand. No test changed: `mise run test` (313 Rust tests), lint and all 27 e2e tests pass.

<!-- field: acceptance -->
- No behaviour change: every server test passes unchanged.

<!-- akt: PLAN-011-T03 -->
---
type: task
revision: 8
rank: b3H
state: done
in: [PLAN-011]
links: [Blocked by PLAN-011-T02, Blocks PLAN-011-T04, Blocks PLAN-011-T05]
title: "The batch: locks, operations, references"
assignee: albin@mattsson.io
---

<!-- field: description -->
**The write path** (`items/server/src/akter/batch.rs`, new):
- `Operation` has one variant per akt write, with `numbers_mut` listing every akt number it names.
- `check` refuses an empty batch, more than 200 operations, or an `@<n>` naming anything but an earlier `createAkt` as `BAD_REQUEST`, with the operation's position, before anything runs.
- `Akter::batch` runs in one transaction. `lock` takes the order lock if any operation creates or moves, then `lock_akter` on every existing akt named, by id. Each operation then runs through its body function (`create_in`, `edit_in`, …), its `@<n>` replaced by that operation's number.
- The first refusal rolls back as a `BatchError` naming its position. The relay is woken once, only after a commit.
- `AktError::BadRequest` is new, answered as `BAD_REQUEST`.

**GraphQL** (`graphql/akter.rs`): `batch(input: { operations: [OperationInput!]! }): [Akt!]!`. `OperationInput` is `@oneOf` over the ten mutations' own input types, and a refusal carries the extension `operation`. `items/web/schema.graphql` is regenerated.

**Tests:** a unit test of `check`'s references and limits, and `items/server/tests/batches.rs` with a smoke test against Postgres. It creates a plan, two tasks in it through `@1`, a link and a transition in one batch, then shows a batch refused at its second operation (STALE, `operation: 2`) undoes its first. Clippy and lint pass; T04 adds the rest of the server tests.

<!-- field: acceptance -->
- The mutation runs every akt write, all or nothing.
- The OpenAPI and GraphQL schema documents, if any, are regenerated.

<!-- akt: PLAN-011-T04 -->
---
type: task
revision: 6
rank: b3I
state: done
in: [PLAN-011]
links: [Blocked by PLAN-011-T03]
title: Server tests for batches
assignee: albin@mattsson.io
---

<!-- field: description -->
`items/server/tests/batches.rs`, against Postgres, six tests:
- **Every write or none:** a plan, two tasks in it through `@1`, a link and a transition commit in one batch; a refusal at operation 2 undoes operation 1.
- **No trace:** a create, a link and an edit before a stale edit leave nothing, not even the number the create took, which the next create gets. A refusal in the middle stops there.
- **Checked before running:** `@<n>` naming nothing, or an edit, is refused as `BAD_REQUEST` with its position before any operation runs. 0 or 201 operations are refused without a position. 200 edits of one task, each on the revision the one before left, all commit.
- **Authorized as alone:** a task editor's batch editing a task and a plan is `FORBIDDEN` at 2, and the task is unchanged.
- **Revisions and change events:** expected revisions count earlier operations. The change events are the link's two revisions, then the edit's and the transition's, with consecutive `seq`s. A rolled-back batch leaves none.
- **No deadlocks:** 4 workers sending batches (create, link both ways, move, add to a second plan) and 4 sending single add, move and remove on the same akter, 6 rounds each. Every batch commits, and the only refusals are the expected ALREADY_MEMBER and NOT_MEMBER.

The deadlock test was checked by turning off the batch's up-front row locks: it then failed 5 runs of 5 with INTERNAL, Postgres's deadlock. With them, it passed every run. Lint, `mise run test` (320 Rust tests) and all 27 e2e tests pass.

<!-- field: acceptance -->
- The tests pass in `mise run test`.

<!-- akt: PLAN-011-T05 -->
---
type: task
revision: 3
rank: b3J
state: todo
in: [PLAN-011]
links: [Blocked by PLAN-011-T03, Resolves PLAN-010-G01]
title: apply sends one batch
---

<!-- field: description -->
The CLI's `apply`, against a server, sends every edit as one batch after its checks, so a server refusal anywhere changes nothing (PLAN-010-G01). The server README, the CLI README and the API docs describe batches.

<!-- field: acceptance -->
- An e2e test: a multi-akt file whose second edit the server refuses changes nothing.
- The parity test passes.
