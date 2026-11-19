---
number: PLAN-021
type: plan
revision: 7
rank: b5W
state: done
title: "Importing a document from a folder, its old versions as PDFs"
owner: albin@mattsson.io
---

<!-- field: goal -->
A document kept elsewhere moves into Kuria in one command, history and all: `kuria docs import <folder> --type <key> [--in <container>]` reads `metadata.json` (field values by key, a `draft`, and `versions`, each a PDF with an optional label and description), creates the akt, releases each old version as its PDF, then makes the draft, converted to Markdown with pandoc, its living body.

Success: the folder in the request (a docx draft, two PDF versions) imports as one `document` whose versions 1 and 2 show their PDFs and whose body is the draft, on a server and in an akt directory alike; later releases render from the body as before.

<!-- field: design -->
## Where it starts

- PLAN-020 made `versions.file` for types written elsewhere: an attachment field taking only PDFs that only releases set, refused on a type with `markdown` fields or live drafts (`version.file`, `version.release-file`). The app shows its PDF, and `/akter/{n}/pdf` serves it.
- `documents`' `document` type has a `markdown` body and live drafts; `editAkt` writes the live draft (`live.edit`), and a release records the live draft as a revision (`live.release`).
- Pandoc reads docx, odt, html, rtf, epub, LaTeX and more, but not PDF.

## Versions with a PDF on a type with a body (`version.file`, `version.release-file`)

- A type with `markdown` fields, live drafts or not, may declare `versions.file`. A release of it MAY take a PDF; one with a PDF records it in the field, one without clears it, so each version either is its PDF or is rendered from its body. A type without `markdown` fields still needs one each time.
- The app shows the PDF of a version that has one in place of the body's sheets, and `Download PDF` and `Open PDF` give that file; the living version is its body, as now. `/akter/{n}/pdf?version=n` serves a version's file if it has one, else renders.
- The documents example's `document` gains an optional `pdf` field and `"file": "pdf"`.

## `kuria docs import` (`cli.docs-import`)

- `metadata.json`: `draft` (a path), `versions` (`[{label?, description?, file}]`, oldest first), every other key a field value, written as `--set` writes it (`owner` by email, …).
- The draft: `.md`/`.markdown` as it is; anything else `pandoc <draft> -t gfm --wrap=none`, refused as `PANDOC_MISSING` without pandoc on `PATH`. Its pictures are not imported yet (a gap).
- Steps: create the akt with the fields, in `--in`; release each version with its PDF, label and description; set the type's one `markdown` field to the draft. Not one transaction: a failure part way says what was made, so the rest can be done by hand.
- Local mode: the same through local mode's create, release and set.

<!-- field: verification -->
- Server: a document type with a body releases with and without a PDF; the PDF route and version reads follow (`items/server/tests/external_documents.rs`).
- CLI: `kuria docs import` of a folder with a docx draft and two PDFs, in local mode (pandoc on PATH in CI is required, else the test is skipped with a note) and in the e2e parity test against a server.
- App: e2e reads an imported document at version 1 (its PDF) and at its living version (its body).
- `mise run lint`, `mise run test`.

<!-- akt: PLAN-021-T01 -->
---
type: task
revision: 5
rank: b5X
state: done
in: [PLAN-021]
title: Versions with a PDF on a type with a body
assignee: albin@mattsson.io
---

<!-- field: description -->
Done. A type with `markdown` fields, live or not, may declare `versions.file` (`version.file`); its releases may take a PDF, which that version then is, and one without clears the field (`version.release-file`). The server's `prepare`/`record_version` work out whether a release records a revision (`records`), and a live draft's release records the file with the draft. `/akter/{n}/pdf?version=n` serves a version's PDF where it has one, else renders. The app shows a version with a PDF as that PDF in place of its sheets, and asks for an optional PDF when releasing. Local mode releases with or without `--file` alike. The documents example's `document` has an optional `pdf` field and `"file": "pdf"`; its bundle is written again.

Verified: `a_document_written_here_keeps_its_old_versions_as_pdfs` (with and without live drafts), the earlier external document tests, `mise run test`.

<!-- field: acceptance -->
version.file and version.release-file allow a markdown type, live or not, to release with or without a PDF; the app, the PDF route and version reads show a version's PDF where it has one; the documents example's document takes one; server tests.

<!-- akt: PLAN-021-T02 -->
---
type: task
revision: 5
rank: b5Y
state: done
in: [PLAN-021]
title: "kuria docs import <folder>"
assignee: albin@mattsson.io
---

<!-- field: description -->
Done. `kuria docs import <folder> --type <key> [--in <container>…] [--reason]` (`cli.docs-import`, `items/cli/src/docs_import.rs`): reads and checks `metadata.json` and its files first; converts the draft with `pandoc --to gfm --wrap none` (Markdown as it is; `PANDOC_MISSING`, `PANDOC_FAILED`); creates the akt with the other keys as `--set`s, releases each version with `release --file`, oldest first, and sets the type's one markdown field to the draft. A failure after the akt is made says what was made. In local mode it refuses an uncommitted directory and commits each step (`git::commit`), since versions are read from git. It says how many of the draft's pictures it left out (PLAN-021-G01).

Verified: `imports_a_document_from_a_folder_its_old_versions_as_pdfs` (local, a docx made by pandoc), e2e `a document is imported from a folder, its old versions as PDFs` (the binary against a server, then the app at version 1 and living), `mise run lint`.

<!-- field: acceptance -->
cli.docs-import specified; metadata.json read; draft converted with pandoc; akt created, versions released with their PDFs, body set; server and local mode; tests and docs.

<!-- akt: PLAN-021-G01 -->
---
type: gap
revision: 2
rank: b5Z
state: closed
in: [PLAN-021]
title: Pictures in an imported draft
---

<!-- field: resolution -->
Imported: pandoc extracts the draft's pictures (--extract-media, with a Lua filter that unwraps figures and drops sizes, so they stay Markdown); each becomes an image akt of --image-type or the one type whose presentation names an image field, titled by its alt text, and the draft embeds it as kuria:<number>. Pictures it can't make so are left as they are and counted (cli.docs-import).

<!-- akt: PLAN-021-D01 -->
---
type: decision
revision: 2
rank: b5a
state: decided
in: [PLAN-021]
title: "One document: old versions are PDFs, the draft its body"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Pandoc reads the draft but not PDFs; a document type renders versions from its body, an external document has none.

<!-- field: decision -->
A type with a body may declare versions.file; a release may take a PDF, which that version then is. The import releases the old PDFs, then sets the body to the converted draft.

<!-- field: why -->
The user's choice: one akt with its whole history, rather than two linked akter or attachments without versions.
