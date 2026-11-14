---
number: PLAN-010
type: plan
revision: 11
rank: b30
state: active
title: "A container with its members, through cat and apply"
owner: albin@mattsson.io
---

<!-- field: goal -->
`kuria cat <number> --members <type>,…` (or `--all-members`) answers an akt followed by its direct members of those types, in the container's order, as one multi-akt file: the same form a plan's file has in an akt directory, without `rank`. `kuria apply` takes such a file back, applying each akt in it, so a person or an agent can read a whole plan, edit its tasks in place, and write back what changed in one command.

Success: on a server and in an akt directory alike (the parity test), `cat PLAN-001 --all-members > plan.md`, an edit to two tasks, and `apply plan.md` change exactly those two; a stale akt anywhere in the file changes nothing.

<!-- field: design -->
## Where it starts

- `cat <number>…` prints each akt as its akt file (`cli.file`), one after another; `-o <dir>` writes each to its own file. `apply <file>…` reads one akt per file, refuses a stale one or a changed read-only key, and edits the rest; locally every change is saved at the end, but against a server the files before a stale one are already applied.
- An akt directory stores a plan and its tasks in one file, the tasks after `<!-- akt: <NUMBER> -->` lines (`cli.local.layout`), read by `items/cli/src/local/dir.rs`.

## Design

### `cat --members` and `--all-members`

- **Members, not nesting:** the akter whose `in` lists the akt, as a server holds them, so that it answers alike on a server and in a directory (decided: PLAN-010-D01).
- **Two flags:** `--members <type>,…` for those akt types, `--all-members` for all; one flag with an optional value would take a following number as its value (decided: PLAN-010-D02). Direct members only. An unknown type is refused as `UNKNOWN_AKT_TYPE`.
- **Order:** the akt first, then its members in the container's order, whatever their types.
- **JSON:** `akter`, in that order. `-o <dir>` writes each to its own file, as now.

### The multi-akt file

- `cat` of several akter, with members or several numbers, prints one multi-akt file: the first akt's akt file, then each other akt after a line `<!-- akt: <NUMBER> -->`, as its akt file without `number`, which the line gives (decided: PLAN-010-D03). It is an akt directory's file without `rank`. A single akt prints as now.
- The splitting is the directory's: one function, a line in a fenced code block being text.

### `apply` of a multi-akt file

- Each part is applied as a file of one akt is: its number from its line or its `number`, which must agree.
- **Checked first:** every akt of every file is read, and checked for staleness and read-only keys, before any is changed, on a server as in a directory (decided: PLAN-010-D04). A directory then saves every change at once; a server is sent one edit per akt (PLAN-010-G01).
- Answers as now: one line and one `applied` entry per akt.

## Constraints

- `cat` of one akt, `-o`, and `apply` of an akt file with one akt don't change.
- The same answers on a server and in a directory (the parity test).

<!-- field: verification -->
- Local-mode tests: `cat --members` and `--all-members` order and filter; a multi-akt file applies, a stale part changes nothing, a read-only key in a later part changes nothing.
- The parity test: `cat --all-members` of a box is the same on both, and an edited multi-akt file applies the same.
- The CLI e2e test: a multi-akt file against the server, with a stale part refused before anything changed.

<!-- akt: PLAN-010-D01 -->
---
type: decision
revision: 2
rank: b31
state: decided
in: [PLAN-010]
title: "Members, not nesting"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A plan file in an akt directory holds the akter numbered after the plan; a server has no files, only containers and members.

<!-- field: decision -->
`--members` answers the akter whose `in` lists the akt, as a server holds them.

<!-- field: why -->
The same command must answer alike on a server and in a directory. In `plans/` members and nested akter are almost always the same; an akt removed from its plan, which stays in its file, isn't a member.

<!-- akt: PLAN-010-D02 -->
---
type: decision
revision: 2
rank: b32
state: decided
in: [PLAN-010]
title: "`--members <type>,…` and `--all-members`"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Which members to include: some akt types, or all.

<!-- field: decision -->
Two flags: `--members <type>,…` and `--all-members`, direct members only.

<!-- field: why -->
One `--members` with an optional value would read `kuria cat --members PLAN-009` as members of type `PLAN-009`. "Task type" would name one example's akt type; the flags name akt types in general.

<!-- akt: PLAN-010-D03 -->
---
type: decision
revision: 2
rank: b33
state: decided
in: [PLAN-010]
title: "The multi-akt file is the directory's file without `rank`"
decided_by: albin@mattsson.io
---

<!-- field: context -->
`cat` of several akter prints each akt file one after another, which a reader can only split by guessing at `---` lines, and `apply` reads one akt per file.

<!-- field: decision -->
`cat` of several akter prints them as one multi-akt file: the first as its akt file, each other after `<!-- akt: <NUMBER> -->` without `number`. `apply` reads it with the directory's splitting.

<!-- field: why -->
One format to learn and one parser: `cat PLAN-009 --all-members` reads like the plan's file in `plans/`, and the marker splits unambiguously, a fenced block excepted.

<!-- akt: PLAN-010-D04 -->
---
type: decision
revision: 2
rank: b34
state: decided
in: [PLAN-010]
title: Check every akt before changing any
decided_by: albin@mattsson.io
---

<!-- field: context -->
Against a server, `apply` of several files edits each in turn, so a stale file applies those before it; a directory saves only at the end, so it applies none.

<!-- field: decision -->
`apply` reads and checks every akt it is given, for staleness and read-only keys, before changing any, on a server as in a directory.

<!-- field: why -->
A multi-akt file makes a stale part likely, since a whole plan is edited at once; checking first makes the common refusals change nothing on both, and the answers the same.

<!-- akt: PLAN-010-G01 -->
---
type: gap
revision: 3
rank: b35
state: open
in: [PLAN-010]
links: [Resolved by PLAN-010-T03, Resolved by PLAN-011-T05]
title: A server takes a multi-akt apply one edit at a time
---

<!-- field: description -->
Checked first (PLAN-010-D04), a stale part or a read-only key changes nothing; but a refusal only the server makes while editing, such as a value its rules refuse, comes after the parts before it were edited. A directory changes nothing then. One mutation editing several akter in one transaction would close it, if it becomes a problem.

<!-- akt: PLAN-010-T01 -->
---
type: task
revision: 7
rank: b36
state: done
in: [PLAN-010]
links: [Blocks PLAN-010-T02, Blocks PLAN-010-T03]
title: "Spec cat with members, the multi-akt file, and its apply"
assignee: albin@mattsson.io
---

<!-- field: description -->
In `docs/spec/cli.md`: `cli.read` names `cat --members` and `--all-members`; `cli.members` (new) says which members, in what order, refusing an unknown type, with JSON and `-o`; `cli.file.multi` (new) defines the multi-akt file, the directory's file without `rank`, which `cat` of several akter prints; `cli.apply` applies each akt of a multi-akt file, checking every akt first, and refuses an akt given twice. Lint passes.

<!-- field: acceptance -->
- The rules say all of PLAN-010's design.
- `mise run lint` passes.

<!-- akt: PLAN-010-T02 -->
---
type: task
revision: 7
rank: b37
state: done
in: [PLAN-010]
links: [Blocked by PLAN-010-T01, Blocks PLAN-010-T04]
title: cat --members and --all-members
assignee: albin@mattsson.io
---

<!-- field: description -->
`cat --members <type>,…` and `--all-members` (`args.rs`, `reads/akter.rs`): unknown types are refused first; each akt is followed by its direct members, from `members` paged on a server or `members_of` in a directory, in the container's order, filtered by type, each akt once. Several akter print as one multi-akt file (`file::write_all`): the first as its file, each other after its line without `number`. One akt prints as before, and `-o` writes each to its own file. `cat PLAN-009 --all-members` in `plans/` is exactly that plan's file without `rank`. A local-mode test covers order across types, the filter, an unknown type, the file's form against the stored file, one akt, and `-o`. The server's side is tested in T04's e2e.

<!-- field: acceptance -->
- Members of the given types, or all, in the container's order, after the akt.
- One akt prints as before; several print as a multi-akt file.
- Local-mode tests.

<!-- akt: PLAN-010-T03 -->
---
type: task
revision: 8
rank: b38
state: done
in: [PLAN-010]
links: [Blocked by PLAN-010-T01, Blocks PLAN-010-T04, Resolves PLAN-010-G01]
title: "Apply multi-akt files, checked first"
assignee: albin@mattsson.io
---

<!-- field: description -->
Multi-akt `apply`. `file::split` moved from the directory reader into `file.rs` and is shared with `git.rs`. `file::parse_all` reads a multi-akt file, each akt's number from its line or its `number`, which must agree. `writes::read_akter` reads every akt of every file and refuses one given twice. On a server, `apply` now fetches and checks every akt (stale, read-only keys, what changed) before sending any edit. Locally it already saved only at the end. A local-mode test covers an unchanged plan file (nothing changes), two tasks edited in place (exactly those change), a stale part and a read-only key in a later part (nothing changes, file byte for byte), an akt given twice, and a line and `number` that disagree. PLAN-010-G01 stays open: a server refusal made while editing still comes after earlier edits. Lint and the CLI tests pass.

<!-- field: acceptance -->
- A multi-akt file applies each part.
- A stale part or a changed read-only key anywhere changes nothing, on both.
- Local-mode tests.

<!-- akt: PLAN-010-T04 -->
---
type: task
revision: 7
rank: b39
state: done
in: [PLAN-010]
links: [Blocked by PLAN-010-T02, Blocked by PLAN-010-T03]
title: "Parity, e2e and docs"
assignee: albin@mattsson.io
---

<!-- field: description -->
The parity test reads box A with `--all-members`, box B with `--members`, and an unknown type, the same on both. On each side, it then reads B with its members as one file and edits a member's title and B's. With another member changed meanwhile, `apply` is refused as STALE and B is unchanged. Read again, it applies each part, changing only the edited title. This covers the server's paged `members` and its check-first `apply`, so the CLI e2e test needs no case of its own. Docs: `kuria guide` has "A whole plan as one file" (the loop, the form, checked first); the CLI README describes `cat --members`/`--all-members` and multi-akt `apply`; `.agents/plans/PLANS.md` adds the whole-plan lines and now says to run commands as `mise run plans --`, which PLAN-008-T08 had missed. Lint, `mise run test` and all 27 e2e tests pass.

<!-- field: acceptance -->
- `mise run e2e` passes.
- The guide shows the loop: cat a plan with its members, edit, apply.
