---
number: PLAN-009
type: plan
revision: 13
rank: b2p
state: active
title: "Fewer, readable files: akter nested in their parent's file, named by title"
owner: albin@mattsson.io
---

<!-- field: goal -->
An akt directory stores an akt numbered after its parent inside its parent's file, and names each file by its number and title: `plans/akter/PLAN-008-the-cli-in-local-mode.md` holds PLAN-008 and every task, gap and decision of it. Kuria's 237 plan files become 8, each reading top to bottom like the Markdown plans that came before, and a person browsing `plans/akter/` sees what each file is about.

Nothing else changes: every command answers as before, `cat`, `cat -o` and `apply` still work on one akt in the akt file format, and the parity test still passes.

Success: `plans/` holds one file per plan, named by its title; `kuria check --dir plans` passes; the parity test and the local-mode tests pass with the new layout; and a title changed through the CLI renames the file, which git records as a rename.

<!-- field: design -->
## Where it starts

- An akt directory (`cli.local.layout`) holds each akt as `akter/<number>.md`: the akt file `cat` prints, plus `rank`. `items/cli/src/local/dir.rs` reads them, `write.rs` writes each touched akt whole under the directory's lock, `check.rs` names each problem's file, and `convert.rs` writes a directory from a bundle.
- Number patterns say when an akt is numbered after its parent (`{parent}-T{seq:2}`), from the container it is created in (`created_in`).

## Constraints

- **Storage, not exchange.** The akt file format (`cli.file`) stays the unit of `cat`, `cat -o` and `apply`; only how a directory stores akter changes.
- **The number decides.** An akt is found by the number in its front matter, never by its file name.
- **Same answers.** Every read and write answers exactly as now, on a server and in a directory (the parity test).
- **One change, one lock.** A write still writes every file it touched whole, through a file beside it and a rename.

## Design

### Nesting

- **Where an akt is stored:** an akt whose number is made from its parent's is stored in the file of that parent, after the parent's own fields, in the directory's order. Any other akt has a file of its own. The rule comes from the number pattern, so no akt type or setting changes (decided: PLAN-009-D01).
- **The marker:** each nested akt begins with `<!-- akt: <NUMBER> -->` on a line of its own, followed by its front matter and its field sections, as its akt file would be, without `number` (the marker says it). A marker inside a fenced code block is text, as a field marker is.
- **Nested deeper:** an akt numbered after a nested akt is stored in the same file, after its parent.
- **Containers:** which file holds an akt doesn't say what it is in: `in` lists every container, the parent too, as in an akt file. One removed from its parent, or moved to another container, stays in its parent's file, since its number doesn't change either.

### File names

- **`<NUMBER>-<slug>.md`**, where the slug is the title in lower case, words joined by `-`, ASCII letters and digits only, cut at a word to at most 40 characters; `<NUMBER>.md` when the title gives no slug (decided: PLAN-009-D02).
- **The name follows the title:** a write that changes the title of the akt a file is named by renames the file in the same change. Links and `in` name akter by number, so nothing else changes.
- **`check`** warns, as a problem, of a file whose name doesn't start with the number of the akt it holds first.

### Moving a directory to the new layout

- See PLAN-009-G01.

## Verification

- Unit and local-mode tests: nested files read back as separate akter; writes rewrite only the touched files; a title change renames; a marker in a fenced code block is text; `check` names problems in nested akter by file and number.
- The parity test (`e2e/tests/parity.spec.ts`) and the conversion e2e test pass unchanged.
- `plans/` moved to the new layout: `kuria check` passes, every akt's `cat` is the same before and after, and the file count is 8.

<!-- akt: PLAN-009-G02 -->
---
type: gap
revision: 4
rank: b2t
state: closed
in: [PLAN-009]
links: [Resolved by PLAN-009-T03]
title: Branches changing one plan meet in one file
---

<!-- field: description -->
With a plan's tasks in one file, two branches that each change a task of the same plan edit the same file. Git merges different hunks, but two branches adding tasks both change the plan's `revision` and append near each other, so they conflict where today they only clash on numbers. Decide whether to accept this, with `check` after a merge, or to lessen it, for example by keeping a container's revision out of the hunk that adding a member touches.

<!-- field: resolution -->
Accepted. Two branches changing different akter of one plan edit different parts of its file, which git merges. Two branches adding to the same plan conflict on the plan's `revision` line and near each other at its end, where today they already clash on the new numbers. Resolve by hand, then run `kuria check`, which names what is left. Keeping a container's revision out of the hunk adding a member touches would mean not bumping it, which the server does (`cli.local.revision`), so it stays.

<!-- akt: PLAN-009-T03 -->
---
type: task
revision: 9
rank: b2w
state: done
in: [PLAN-009]
links: [Blocked by PLAN-009-T02, Blocks PLAN-009-T05, Resolves PLAN-009-G02, Resolves PLAN-009-G03]
title: Write nested akter and rename on a title change
assignee: albin@mattsson.io
---

<!-- field: description -->
**Layout** (`items/cli/src/local/layout.rs`, new):
- `parent` finds the akt a number was made from, with `NumberPattern::parent_of`.
- `home` gives the file an akt belongs in: its parent's, else `<NUMBER>-<slug>.md` from its title (`slug` follows `cli.local.names`).
- `relocate` moves the akter of the given files to where they belong, parents first.
- `file_text` writes a file: its first akt, then its nested akter after their lines, in the directory's order, each after its parent.

**Writes** (`write.rs`): `save` takes the files of the akter a change touched, moves every akt in them to where it belongs, and writes each resulting file whole. A file that no akt is left in is removed afterwards: one renamed for a new title, or an old-layout file whose akt moved into its parent's. So a new task lands in its plan's file, a plan's new title renames its file along with its nested akter, and a nested akt's title names no file. An akt removed from its plan stays in its file, in nothing. T02's guard is gone. `convert::to_dir` writes the new layout through the same functions.

**Per-akt history** (PLAN-009-G03): `git.rs` now follows each akt by its revision across commits and files. `history` answers the commits that changed that akt, not its file, with its revision after each; created and changed come from those commits. `cli.local.history` says so, and that outside git a file's akter share its time.

**`check`** names a nested akt's problems with its number (one part of T04).

**Tests:** in `tests/local.rs`, writes into the plan's file, ordering after a move, renaming on a title change, removal from a parent, an old-layout akt moving in, an exact rewrite of a nested file, and history across a move. The round trip compares `cat` of every akt and finds one file. The parity test compares order within each box.

Lint, `mise run test` and all 27 e2e tests pass.

Until T05, `plans/` is mixed: recording through the CLI moves the akter it touches into their plan's file. PLAN-009's own file is now named by its title.

<!-- field: acceptance -->
- A change rewrites only the files it touched, still under the lock and through a rename.
- A title change renames the file in the same change.
- The parity test and the conversion e2e test pass.

<!-- akt: PLAN-009-G03 -->
---
type: gap
revision: 4
rank: b2z
state: closed
in: [PLAN-009]
links: [Resolved by PLAN-009-T03]
title: "An akt's times and history when its file holds others"
---

<!-- field: description -->
Local mode takes when an akt was created and changed, and `history <number>`, from git per file (`items/cli/src/local/git.rs`), or from the file's modification time outside git. With nesting, every akt of a plan's file shares those. `ls <type>`, which orders by most recently changed, then differs from the server, and the parity test fails on it. `show` gives the file's times, and `history PLAN-009-T01` would list every commit to the plan's file.

Options:
- **Per akt from git:** an akt changes exactly when its `revision` does (a move makes no revision, as on a server). Walk the commits that touched `akter/`, read each changed file once, and follow each akt by its number into whichever file holds it. Its history is the commits where its revision changed, it was created in the first commit that holds it, and an akt whose revision differs from HEAD's has changes not committed yet. This follows akter across files, so the move into the new layout keeps their history. Outside git, the file's modification time stays shared.
- **Times in the front matter** (`changed:`): duplicates git (PLAN-008-D01), and two branches changing one akt always conflict.

<!-- field: resolution -->
Built in PLAN-009-T03, per akt from git, as you chose. `items/cli/src/local/git.rs` reads one `git log` of `akter/` and the files each commit left, in one `git cat-file --batch`. An akt changed in the commit where it first reached a revision, whichever file held it, so a move between files keeps its history, and a merge seeing it again adds nothing. Revisions at HEAD come from `git ls-tree`, and an akt whose revision differs from HEAD's has changes not committed yet. `history` answers each commit with the akt's revision after it. Outside git, a file's akter share its modification time (`cli.local.history`), so the parity test compares order within each box rather than by most recently changed. Every command still reads the whole log; a cache keyed by HEAD would help once plans have thousands of commits.
