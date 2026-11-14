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
