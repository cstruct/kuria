---
number: PLAN-008
type: plan
revision: 18
rank: b2Z
state: done
title: "The CLI in local mode: akter as Markdown files, without a server"
---

<!-- field: goal -->
`kuria` works on a directory of Markdown files as it works on a Kuria server: with `--dir <path>` (or `KURIA_DIR`), every command reads and writes akt files and akt type documents on disk, with the same answers, JSON, refusals and exit codes, and the same rules the server enforces. Git is the history.

After this, Kuria's own plans live in the repository (`plans/`), versioned and reviewed with the code they plan, and an agent works them with the same commands as on a server, without Docker, Konklav or Postgres. A directory can later be pushed to a server, and a server's akter written to a directory.

Success: an agent follows AGENTS.md to read a plan in `plans/`, take a task, record progress and finish it, with nothing running; `kuria check --dir plans` passes in `mise run lint`; and the same scripted changes made on a server and in a directory give the same `cat` of every akt.

<!-- field: design -->
## Where it starts

- The CLI talks to Kuria only through GraphQL (`items/cli`); its reads and writes build on `Session::graphql`.
- The akt file (`cli.file`) is already the on-disk form: `kuria cat` writes it, `apply` reads it, and `state`, `in` and `links` are in its front matter.
- `kuria-engine` holds pure rules: field values, create, edit and transition, hashes, ranks, board columns and draft rendering. The server's write path (`items/server/src/akter.rs` and `akter/{create,membership,links,ordering,draft}.rs`, about 1,100 lines) adds numbering, containment (`contains`, exclusive, `while`, `created_in`, cycles, frozen containers), link kinds, and the membership and link records, interleaved with database locks.
- `NumberPattern::read` (T9 of PLAN-007) reads a number back into its counter scope and sequence.

## Constraints

- **Same rules.** Local mode enforces what the server enforces, through the same code: the server's pure rules move into `kuria-engine` first, with no behaviour change on the server (decided: PLAN-008-D02).
- **Git is the history** (decided: PLAN-008-D01). Files hold the current state; `history` reads `git log` for the file. Nothing in the directory duplicates what git keeps.
- **The commands don't change.** Answers, JSON objects and exit codes are the server's; what local mode can't do is refused with a code and a message, never silently different.
- **Plans move into the repository** (decided: PLAN-008-D03); the dogfooding stack stays, optional.

## Design

### The directory

```text
plans/
  kuria.toml            # marks the directory; its format version
  types/<key>.json      # each akt type's document, as `types export` writes it
  akter/<NUMBER>.md     # each akt, as `kuria cat` prints it, plus `rank`
```

- **Akt files** are `cat`'s format exactly, with one more front-matter key, `rank`, for the tenant's order. Sorted by name, files group by plan (`PLAN-007-T03.md`).
- **`revision`** counts writes to the file, so `apply` still refuses a file read before another write (`STALE`, exit 3).
- **Numbers** come from the files present: the next number of a scope is one past the highest `NumberPattern::read` finds. No counter file, so nothing extra conflicts in a merge.
- **Users and contacts** are written by email or name, as in akt files; locally they are text, and `@me` is `git config user.email`.

### One backend, two stores

The CLI's commands call a backend rather than GraphQL directly: the server backend (today's code) and a local one. The local backend loads the directory, applies the engine's rules, and writes only the files a change touches, under a lock file so two agents in one checkout don't interleave.

### What local mode refuses

`login`, `logout`, `whoami` (it says the directory instead), `users`, `contacts`, attachments, `draft` (until drafting moves into the engine), and `export`/`import` of bundles, which instead convert (below). Each answers `LOCAL_UNSUPPORTED`, exit 1.

### Converting

- **Server → directory:** `kuria export --to-dir plans` writes a server's selected types and akter as a directory: `types export` and `cat`, with ranks in order.
- **Directory → server:** pushing keeps numbers but has no revisions to replay (PLAN-008-G02).

### `kuria check`

`kuria check --dir plans` loads the whole directory and checks it as the server would: valid documents, unique numbers, every `in` and `links` naming an akt that exists and a kind its type declares, containment rules, and states the type has. `mise run lint` runs it, so a merge of two branches that both took the next number fails before it lands (PLAN-008-G03).

<!-- field: verification -->
- **Engine:** the server's tests pass unchanged after the rules move (T2); the moved rules get unit tests of their own.
- **Local mode:** CLI integration tests on temporary directories for every command: reads, the file round trip, `STALE`, read-only keys, numbering in and across scopes, containment and link refusals, ordering, and `LOCAL_UNSUPPORTED`.
- **Parity:** one scripted sequence of commands run against the e2e server and against a directory gives the same `cat` of every akt, and the same refusals.
- **History:** `kuria history` in a git checkout lists the file's commits.
- **Check:** `kuria check` refuses a directory with a duplicate number, a dangling `in`, or a link of an undeclared kind.
- **Dogfooding:** PLAN-001 to PLAN-008 converted into `plans/`, `kuria check --dir plans` clean in `mise run lint`, and an agent working a task there as AGENTS.md says.

<!-- akt: PLAN-008-D01 -->
---
type: decision
revision: 2
rank: b2a
state: decided
in: [PLAN-008]
title: Git is the history in local mode
decided_by: albin@mattsson.io
---

<!-- field: context -->
A directory of akt files needs a history for `kuria history` and for review.

<!-- field: decision -->
Files hold only the current state; `kuria history` reads `git log` for the akt's file. No revision log is kept beside the files.

<!-- field: why -->
Chosen by the user: simple, reviewable in pull requests, and nothing duplicates what git keeps. The cost: no per-revision hashes or reasons unless commit messages carry them (PLAN-008-G01).

<!-- akt: PLAN-008-D02 -->
---
type: decision
revision: 2
rank: b2b
state: decided
in: [PLAN-008]
title: "Local mode enforces the server's rules through a shared engine"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Local mode could check only values and transitions, which the engine already does, or everything the server checks.

<!-- field: decision -->
Move the server's pure rules (numbering, containment, link kinds, the membership and link records) into kuria-engine, so the server and local mode enforce them through one piece of code, before local mode uses them.

<!-- field: why -->
Chosen by the user: a directory that accepts what the server would refuse can't be pushed to one, and two rule sets drift. The cost is untangling about 1,100 lines of the server's write path from its locks, without changing its behaviour.

<!-- akt: PLAN-008-D03 -->
---
type: decision
revision: 2
rank: b2c
state: decided
in: [PLAN-008]
title: "Kuria's plans move back into the repository"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Plans moved from Markdown into the dogfooding Kuria (PLAN-007-T12), which needs Docker, Konklav and Postgres running.

<!-- field: decision -->
Once local mode works, PLAN-001 onwards are converted into `plans/` in the repository, and AGENTS.md points at `kuria --dir plans`. The dogfooding stack stays, optional.

<!-- field: why -->
Chosen by the user: plans versioned and reviewed with the code they plan, worked with no stack running, in the same commands an agent uses on a server.

<!-- akt: PLAN-008-G01 -->
---
type: gap
revision: 3
rank: b2d
state: closed
in: [PLAN-008]
links: [Resolved by PLAN-008-T01]
title: Where a local --reason goes
---

<!-- field: description -->
With git as the history, a write's `--reason` has nowhere to go but the commit that follows. Options: print it as the suggested commit message; keep it in the akt file until the next write; or have `kuria` commit each change itself (`--commit`). To decide in T1.

<!-- field: resolution -->
Decided in PLAN-008-T01 (`cli.local.reason`): `--reason` is accepted and kept nowhere; the answer gives it back (`reason` in JSON, the message to commit with in Markdown), so it becomes the commit message and git keeps it. `kuria` doesn't commit; a `--commit` flag can come later if agents should commit each change.

<!-- akt: PLAN-008-G02 -->
---
type: gap
revision: 4
rank: b2e
state: closed
in: [PLAN-008]
links: [Resolved by PLAN-008-T06]
title: Pushing a directory to a server keeps numbers but has no revisions
---

<!-- field: description -->
A bundle import replays revisions and checks their hashes (`bundle.import.hashes`); a directory has only current states. Options: a bundle scope that starts each akt at one revision recording its state as it is (hashes then hold), or creating akter through the API with their numbers. Needed before plans can move to Fly.io (PLAN-007-T13).

<!-- field: resolution -->
Decided and built in PLAN-008-T06. A directory goes to a server as a `types` bundle that the CLI builds itself; no server change was needed. Each akt gets revisions numbered 1 up to its file's `revision`: the first records its state and values as they are, and the rest change nothing. Every hash then matches its state (`bundle.import.hashes`), the numbering from 1 holds, and pins in the files name revisions the server has. The server holds no history from before the import: git holds it. Creating akter through the API was rejected, since it would make new revisions and couldn't keep pins.

<!-- akt: PLAN-008-G03 -->
---
type: gap
revision: 3
rank: b2f
state: closed
in: [PLAN-008]
links: [Resolved by PLAN-008-T05]
title: Numbers taken in two branches clash on merge
---

<!-- field: description -->
Numbers come from the files present, so two branches each creating the next task of a plan both take, say, PLAN-008-T08. Git merges them as two files with the same number in front matter, or one file conflicts. `kuria check` catches it in lint; whether `kuria` should also offer to renumber the newer one is open.

<!-- field: resolution -->
Resolved by PLAN-008-T05: `kuria check`, run by `mise run lint` on `plans/`, refuses what a merge of two branches taking the same number leaves: one file with conflict markers (it doesn't read), or two files whose numbers differ only in case. Renumbering the newer akt is left out until it is needed; resolving the conflict by hand and running `kuria check` again is the way for now.

<!-- akt: PLAN-008-G04 -->
---
type: gap
revision: 3
rank: b2g
state: closed
in: [PLAN-008]
links: [Resolved by PLAN-008-T01]
title: Who @me is without a sign-in
---

<!-- field: description -->
Locally there is no subject: `@me` is `git config user.email`, and an agent working for someone writes their email. Whether agents need a name of their own here (like PLAN-007-G05 on a server) is open.

<!-- field: resolution -->
Decided in PLAN-008-T01 (`cli.local.people`): `@me` is `KURIA_ME`, else `git config user.email` in the directory, else refused as `BAD_REQUEST`. An agent working for someone sets `KURIA_ME` to their email; agents of their own stay text, like everyone locally.

<!-- akt: PLAN-008-T01 -->
---
type: task
revision: 9
rank: b2h
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T02, Blocks PLAN-008-T03, Resolves PLAN-008-G01, Resolves PLAN-008-G04]
title: Spec local mode
assignee: albin@mattsson.io
---

<!-- field: description -->
Done: thirteen rules in `docs/spec/cli.md` under "Local mode" (`cli.local.dir`, `layout`, `types`, `rules`, `revision`, `numbers`, `people`, `history`, `reason`, `lock`, `unsupported`, `check`, `convert`), and the terms `akt directory` and `local mode` in `docs/nomenclature.md`. G01 and G04 closed with their resolutions. `mise run lint` passes.

One choice the plan didn't name: an akt directory keeps one version per akt type (`cli.local.types`), so `types define` replaces a document, refusing one the directory's akter wouldn't fit, and converting from a server uses each type's latest document.

The task as planned:

Rules for local mode in `docs/spec/cli.md` (`cli.local.*`): the directory and `kuria.toml`, akt files with `rank`, `revision` and `STALE`, numbers from the files present, people as text and `@me`, history from git, what is refused (`LOCAL_UNSUPPORTED`), the lock, `kuria check`, and converting. Terms in `docs/nomenclature.md`: local mode, akt directory. Resolve PLAN-008-G01 and G04.

<!-- field: acceptance -->
- Every behaviour this plan names is a rule, verifiable from outside the CLI.
- PLAN-008-G01 and G04 are closed with their resolutions.
- `mise run lint` passes (Tracey).

<!-- akt: PLAN-008-T02 -->
---
type: task
revision: 7
rank: b2i
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T04, Blocked by PLAN-008-T01]
title: "Move the server's pure rules into kuria-engine"
assignee: albin@mattsson.io
---

<!-- field: description -->
Done: `kuria-engine` gains `rules` (`AktRef`, `Broken`) with `numbering` (`counter`, `render`, and `next_from` for local mode), `containment` (`check_created_in`, `check_distinct`, `check_create_inside`, `check_unfrozen`, `check_not_itself`, `check_joining` given what the store knows, and `contains`, a cycle search for local mode), `links` (`check_not_itself`, `to_add`, `check_new`, `to_remove`, `check_existed`) and `records`; and `placement` for moves. The server's write path finds the facts in Postgres, calls them, and maps `Broken` to its existing `AktError` variants, so codes and messages are unchanged.

Verified: every server test passes with no test file changed; 25/25 e2e; 36 engine tests, 4 of them new for the rules; lint (Tracey) clean.

The task as planned:

Numbering (the next number of a scope, from what exists), containment (`contains`, exclusive, `while`, `created_in`, cycles, frozen containers), link kinds and pins, and the membership and link records, as pure functions in `kuria-engine` over plain inputs. The server's write path keeps its locks and queries and calls them.

<!-- field: acceptance -->
- No behaviour change: every server test passes unchanged, and so does the e2e suite.
- The moved rules have unit tests of their own in `kuria-engine`.
- No I/O in `kuria-engine`.

<!-- akt: PLAN-008-T03 -->
---
type: task
revision: 7
rank: b2j
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T04, Blocked by PLAN-008-T01]
title: "A backend for the CLI, and local reads"
assignee: albin@mattsson.io
---

<!-- field: description -->
Done: `items/cli/src/local/` loads an akt directory (`dir.rs`), answers the same JSON Kuria's GraphQL gives for each of the CLI's reads (`answers.rs`), and reads git for times and history (`git.rs`). The reads take a `Source` (a server's session, or a directory), so they render through the same code: `types`, `schema`, `types export`, `ls`, `find`, `cat`, `show`, `history`, `whoami`. `--dir`, `KURIA_DIR` and `kuria init`; `users`, `contacts`, `draft`, signing in, bundles and, until T04, writes refuse with `LOCAL_UNSUPPORTED`.

Found and fixed: the global `--dir` collided with `kuria import`'s positional `dir`, sending imports into local mode; import's is now `bundle`, with a test.

Verified: 8 local-mode integration tests (`items/cli/tests/local.rs`), 24 CLI unit tests, lint, all tests, and 25/25 e2e (after one run with two known flakes, added to PLAN-007-G08).

The task as planned:

The commands call a backend: the server's (today's GraphQL code) or a directory's. The directory backend loads `kuria.toml`, `types/` and `akter/`, and answers `types`, `schema`, `ls` (state, in, where, search, pages), `find`, `cat`, `show` and `history` (from `git log`) with the server's answers. `--dir` and `KURIA_DIR` choose it.

<!-- field: acceptance -->
- Each read gives the same Markdown and JSON as on a server for the same akter.
- Commands local mode can't do refuse with `LOCAL_UNSUPPORTED`, exit 1.
- Unit and integration tests on temporary directories.

<!-- akt: PLAN-008-T04 -->
---
type: task
revision: 10
rank: b2k
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T05, Blocks PLAN-008-T06, Blocks PLAN-008-T07, Blocked by PLAN-008-T02, Blocked by PLAN-008-T03]
title: Local writes
assignee: albin@mattsson.io
---

<!-- field: description -->
Done: every write the server takes works on an akt directory, through `kuria-engine`: `new` (from flags or a file), `apply` (with `--dry-run`, `STALE`, read-only and unknown keys), `set`, `do`, `add`, `remove`, `transfer`, `move`, `link` and `unlink`, and `types define`. `items/cli/src/local/write.rs` loads the directory under `.kuria.lock` and writes the files a change touched whole; `edits.rs` and `arrange.rs` are the commands; `values.rs` maps values to and from the engine's, a name or email standing in as a subject derived from its text. Revisions rise as on a server (a container's on a membership change, both akter's on a link). `--reason` comes back to commit with. `init` adds a `.gitignore` for the lock.

One difference to know: a local checkbox records whom it is for, not who checked it (git does).

Verified: 14 local integration tests (6 new: writes and their refusals, apply, arranging and links, redefining types, eight concurrent writers), all tests, lint, and 25/25 e2e.

The task as planned:

`new` (from flags or a file), `apply` (with `--dry-run`, `STALE`, read-only and unknown keys), `set`, `do`, `add`, `remove`, `transfer`, `move` (`rank`), `link` and `unlink`, and `types define` and `export`, through the engine's rules, writing only the files a change touches, under a lock file.

<!-- field: acceptance -->
- Every write's refusals match the server's codes and exit codes.
- Two `kuria` processes writing at once neither lose a change nor corrupt a file.
- Integration tests for every write.

<!-- akt: PLAN-008-T05 -->
---
type: task
revision: 8
rank: b2l
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T08, Blocked by PLAN-008-T04, Resolves PLAN-008-G03]
title: kuria check
assignee: albin@mattsson.io
---

<!-- field: description -->
Done: `kuria check --dir <path>` (`items/cli/src/local/check.rs`) reads every type and akt file, collecting problems instead of stopping (reading is now per file in `dir.rs`), and checks what spans files: types' references, values, numbers their pattern gives and unique ignoring case, containers that exist and may contain the akt, exclusive containers, cycles, links of declared kinds and allowed types, pins, both sides listing each link, and unique ranks. It answers `INCONSISTENT`, exit 1, with `- <file>: <what>` per problem. `mise run lint` runs it on `plans/` when that exists; the guide mentions it.

Verified: 2 new tests (a clean directory; one with eight kinds of problem, each named with its file), 16 local tests in all, every test and lint. e2e not run: nothing here touches the server or the app.

The task as planned:

`kuria check --dir <path>`: valid documents, unique numbers, every `in` and `links` naming an akt that exists and a kind its type declares, containment rules, and states the type has, each problem said with its file. `mise run lint` runs it on `plans/`.

<!-- field: acceptance -->
- A duplicate number (PLAN-008-G03), a dangling `in`, an undeclared link kind and an unknown state are each refused, naming the file.
- A clean directory exits 0.

<!-- akt: PLAN-008-T06 -->
---
type: task
revision: 9
rank: b2m
state: done
in: [PLAN-008]
links: [Blocks PLAN-007-T13, Blocks PLAN-008-T08, Blocked by PLAN-008-T04, Resolves PLAN-008-G02]
title: Convert between a server and a directory
assignee: albin@mattsson.io
---

<!-- field: description -->
Both directions go by way of a bundle, so the server is unchanged (`cli.local.convert`, `items/cli/src/local/convert.rs`).

- `kuria export --types <key>,… --to-dir <path>` fetches the bundle of those types with their akter and writes it as a new akt directory: each type's latest document, each akt as it is now (revision, rank, `in`, links with pins, people by email or name), links ordered as the server lists them. It then runs `check` on what it wrote, and refuses an akt that doesn't fit its type's latest document (`NOT_LATEST`) or that holds an attachment.
- `kuria import --from-dir <path> [--map-subjects]` checks the directory, then posts it as a `types` bundle. Each akt gets revisions 1 to its file's `revision`: the first records it as it is, and the rest change nothing, so every hash matches and pins stay valid. Each person is a subject named by their email, which the import maps to a user who has signed in.
- The CLI's bundle transport is split into `fetch` and `post`, which both commands share.

Verified: all 237 akter of this dogfooding Kuria exported to a directory, which `check` finds consistent; each one's local `cat` matches the server's byte for byte. A Rust test takes a directory to a bundle and back, checking the revisions and hashes as the import does; the akt files come back unchanged. An e2e test exports to a directory, changes it, and imports it into `lab` with the same numbers, `cat` and revision count, then gets CONFLICT on a second import. Lint, `mise run test` and all 26 e2e tests pass.

<!-- field: acceptance -->
- A server's akter written to a directory read back the same through local `cat`.
- PLAN-008-G02 decided, and pushing done or split into its own task.

<!-- akt: PLAN-008-T07 -->
---
type: task
revision: 7
rank: b2n
state: done
in: [PLAN-008]
links: [Blocks PLAN-008-T08, Blocked by PLAN-008-T04]
title: Parity tests and docs
assignee: albin@mattsson.io
---

<!-- field: description -->
`e2e/tests/parity.spec.ts` defines two akt types for each run: a box that contains items exclusively and only while open, and an item created in a box, with a link kind, choices, dates, a checkbox and a transition that requires it. It then runs one sequence of some thirty commands, each against the e2e server and then with `--dir` against a new akt directory:
- defining the types, creating akter (numbered in their box), set, transitions and a required field;
- links with a pin, unlink, move, add, remove and transfer;
- a refusal for each rule: CREATED_IN, a bad choice, a cleared required field, a missing requirement, a transition from the wrong state, INVALID_LINK for a duplicate and for a self-link, NOT_FOUND, self-move, EXCLUSIVE, NOT_MEMBER, NOT_ACCEPTING and FROZEN;
- `cat -o`, edit and `apply`.

Every answer must be the same, with the reason to commit with set aside: a success by its Markdown, a refusal by its exit code and code. So must every akt's `cat` and the akter's order afterwards, and the directory must pass `check`. A temporary log confirmed each refusal is the one intended.

Docs:
- **`kuria guide`:** its local-mode section is rewritten as steps for agents: commit with the reason each write gives back, run `check` after a merge, writes lock the directory, and converting.
- **The planning example's README:** a "Without a server" section, whose commands I ran.
- **The CLI and e2e READMEs:** describe the parity test.

Lint, `mise run test` (the guide's unit test included) and all 27 e2e tests pass.

<!-- field: acceptance -->
- The parity test passes in `mise run e2e`.
- `kuria guide` explains local mode; its unit test still finds every command.

<!-- akt: PLAN-008-T08 -->
---
type: task
revision: 8
rank: b2o
state: done
in: [PLAN-008]
links: [Blocked by PLAN-008-T05, Blocked by PLAN-008-T06, Blocked by PLAN-008-T07]
title: "Kuria's plans in the repository"
assignee: albin@mattsson.io
---

<!-- field: description -->
`plans/` holds PLAN-001 to PLAN-008, every plan, task, gap and decision, exported from the dogfooding Kuria with `kuria export --types plan,task,gap,decision --to-dir plans`: 4 akt types and 237 akter. `kuria check` finds the directory consistent, and `mise run lint` runs that check.

- **`mise run plans -- <command>`** runs the CLI with `KURIA_DIR` set to `plans/`, building it if needed; paths are the caller's, and exit codes pass through.
- **People:** the seeded agent had no name, only its subject. Its 12 values (assignee, decided_by) are now git's `user.email`, which is `@me`, as chosen.
- **Docs:** AGENTS.md and `.agents/plans/PLANS.md` point at `plans/` and say how to work there: take, record, finish, commit with the reason given back, and check after a merge. The dogfooding README calls the stack optional and shows `import --from-dir` and `export --to-dir` for it.
- **Removed:** `dogfood/scripts/migrate.ts`, which is done with. The old `.agents/plans/<topic>.md` files were never committed (`.gitignore`), so they are left on disk rather than deleted.

This task was finished in `plans/` itself, with no server involved: read with `cat`, recorded with `apply`, finished with `do`, and committed with the reasons the CLI gave back. The dogfooding Kuria's copy of PLAN-008 is now behind.

<!-- field: acceptance -->
- `plans/` holds every plan, task, gap and decision, and `kuria check` passes.
- An agent reads a plan in `plans/`, takes a task, records progress and finishes it, with nothing running.
- AGENTS.md and PLANS.md say so.
