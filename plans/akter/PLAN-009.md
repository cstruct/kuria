---
number: PLAN-009
type: plan
revision: 12
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
