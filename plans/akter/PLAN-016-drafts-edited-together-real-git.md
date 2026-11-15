---
number: PLAN-016
type: plan
revision: 21
rank: b4O
state: active
title: "Drafts edited together, real git repositories, pages, and attachments in akt directories"
owner: albin@mattsson.io
---

<!-- field: goal -->
Documents written together, released as they are, and worked on with real git.

- **A living draft, edited together:** an akt type may opt in to live drafts. Its akter then have one draft, all their fields, edited live by everyone allowed, with [Yjs](https://github.com/yjs/yjs). Edits never make revisions: each revision is a release, which records the draft as it is.
- **Real git:** each container that opts in, such as a document index, is a git repository Kuria hosts, its objects in object storage (S3) and its refs in Postgres. `git clone https://kuria.example/git/DOCS-1.git` gives the index and all its documents. Branches, merges and tags are kept as pushed. `main` is the drafts: the app's edits become commits on it, and a push to it flows into the drafts.
- **The branch workflow works:** make many edits, branch to keep them, revert on `main` what isn't ready, release, then merge the branch back to recover the rest.
- **Pages:** a document is shown and edited as white A4 pages, whatever the theme, with page breaks written `<!-- pagebreak -->`, as its PDF lays them out.
- **Releases at hand:** an akt's page shows the draft, and each release's PDF opens in a new tab.
- **Attachments in akt directories:** `kuria export --to-dir` and `import --from-dir` carry every attachment field as files, as clones carry pictures.

Success: the documents walkthrough runs in `mise run e2e` with two people editing a draft at once, a release, the branch workflow done with plain `git` over HTTPS, and an exported directory holding images that imports back.

<!-- field: design -->
## Where it starts

- **PLAN-014 and PLAN-015:** documents with `markdown` bodies, references, versions (named revisions, pins), PDFs through `kuria-typst` in a limited process, image akter, diagram blocks, and clones through `git-remote-kuria`, which rebuilds a straight history from revisions, refuses merges, and keeps pictures in `attachments/`.
- **Revisions** are made by every write; versions name some of them.
- **Akt directories** (local mode, `export --to-dir`, `import --from-dir`) refuse attachments.
- **Storage:** Postgres for akter, object storage (S3, or memory in tests) for attachments, Valkey for change events and sessions.

## Attachments in akt directories (PLAN-016-D01)

- An attachment is a file under `attachments/<number>/<name>` in the directory, and an akt file names it by that relative path, for every `attachment` field.
- Local mode checks the file exists, its media type against `media_types`, and an SBOM field's SBOM, with the engine's SBOM reading. A local release resolves pictures from these files.
- `export --to-dir` writes each attachment's bytes there; `import --from-dir` puts them in the bundle it imports.

## Live drafts (PLAN-016-D02, D03)

- **Opting in:** an akt type says `"live_drafts": true`. Every field of its akter is in the draft. Other types are as now.
- **Revisions are releases:** for such an akt, edits change its draft only. Releasing records the draft's fields as a revision and names it as the next version, as versions do now, with its reference checks and pins. A transition or membership change is a revision too, carrying the last release's fields. Reading an akt answers its draft, and its revisions as before.
- **The draft is a Yjs document** the server reads and writes with `yrs`: each `markdown` and multiline `text` field a `Y.Text` of its Markdown or text, every other field a value in a `Y.Map`, last writer wins. Rich-text structure isn't in the CRDT, so the server can read, check, render and merge it (PLAN-016-D03).
- **Kept** as `akt_draft` (its encoded state) and `akt_draft_update` (each update, with who sent it and when), compacted into the state now and then; the akt's history shows the draft's changes by author, between releases.
- **Collaboration:** the y-websocket sync and awareness protocols at `GET /akter/{number}/live` over WebSocket, authenticated as `/graphql` is, read-only to readers. Updates between server instances go through Valkey, as change events do.
- **The editor** binds Lexical to the field's `Y.Text`: a local change becomes the minimal text edit of its Markdown; a remote change re-imports the blocks it touched, keeping the selection where it was. Other fields bind to the map. Who else is editing shows as presence.

## Git repositories (PLAN-016-D04, D05)

- **Opting in:** a container type says `"git": true`; the documents example's index does. Each such akt is a repository.
- **Storage:** objects as packfiles in object storage, under `git/<tenant>/<akt>/`, content-addressed and never changed; refs in Postgres (`git_ref`), changed by compare-and-swap in the same transaction as the akter a push writes. A server keeps a local cache of each repository it serves, filled from storage.
- **Serving:** smart HTTP at `/git/<number>.git`, by the `git` binary (`upload-pack`, `receive-pack`, stateless), authenticated as the API is; `kuria git-credential` gives git the CLI's sign-in. A push's new objects are checked before they are kept, then packed and stored, and refs moved.
- **`main` is the drafts:** its tip's tree is the container's file and each member's, laid out by its sections, with pictures (`git.tree`, `git.pictures`). A push to `main` must be in normal form; each document whose file changed has the change merged into its draft, as the edit from the commit's parent to it, applied to the `Y.Text` as text operations, so live edits made meanwhile survive. Members of types without drafts are edited as now.
- **The app's edits become commits** on `main`: when a draft has been quiet for a minute, or on release, the server commits the drafts' changes since the last commit, authored by whoever made them.
- **Branches** other than `main`, merges and tags are kept as pushed and change nothing in Kuria. A release tags its commit `<number>/v<n>`.
- **Moving over:** each repository starts from the history `git-remote-kuria` builds, so existing clones' commits stay valid; the helper is retired once clones move to HTTPS (PLAN-016-G03).

## Pages (PLAN-016-D06)

- A `markdown` field is shown and edited as A4 sheets, 210 mm wide, scaled to fit, white with dark text in either theme, with the PDF's margins and fonts.
- `<!-- pagebreak -->` alone on a line starts a new page: a new sheet in the app, `#pagebreak()` in the PDF. The editor offers it as Page break. Pages without one end where a sheet would, shown by a faint guide; only explicit breaks are exact.

## Releases

- An akt's page shows its draft. Its versions list each release with Open PDF, opening the PDF in a new tab; the PDF route answers inline unless `download` is asked for.

## The example

- `document` says `"live_drafts": true`; `document_index` says `"git": true`.

<!-- field: verification -->
- `mise run lint` and `mise run test`: local mode's attachments; the draft's `yrs` reading and writing, releases making revisions, pushes merged into drafts with concurrent edits; refs' compare-and-swap; the editor's binding to `Y.Text` both ways.
- `mise run e2e`: two browsers editing one draft at once, presence, a release and its PDF in a new tab, pages and page breaks with axe, the branch workflow over HTTPS with plain `git`, and an exported directory with images imported back.

<!-- akt: PLAN-016-D01 -->
---
type: decision
revision: 2
rank: b4P
state: decided
in: [PLAN-016]
title: Attachments are files in akt directories
decided_by: albin@mattsson.io
---

<!-- field: context -->
Akt directories refuse attachments, so an exported index loses its images and SBOMs. Clones already keep pictures as files.

<!-- field: decision -->
Every attachment field's value is a file under `attachments/<number>/<name>` in an akt directory, named by that relative path in the akt file. Export writes them; import uploads them; local mode checks them.

<!-- field: why -->
It is how clones keep pictures, so both kinds of directory read alike, and it serves SBOMs as well as images with one mechanism.

<!-- akt: PLAN-016-D02 -->
---
type: decision
revision: 3
rank: b4Q
state: decided
in: [PLAN-016]
title: "Drafts are opt-in, hold all fields, and only releases make revisions"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents should be edited by several people at once, and a revision should mean something: what was released.

<!-- field: decision -->
An akt type may say `"live_drafts": true`. Its akter have one draft holding all their fields, edited live; edits change only the draft, and a release records it as a revision and names it as the next version. Transitions and membership changes are revisions carrying the last release's fields. Other types keep a revision per write.

<!-- field: why -->
Opting in keeps planning and release management as they are. Keeping versions, now made by every release, keeps references, pins, PDFs and tags working unchanged. All fields, not only the body, so that a title edited live is in the same draft.

<!-- akt: PLAN-016-D03 -->
---
type: decision
revision: 2
rank: b4R
state: decided
in: [PLAN-016]
title: "A draft holds text the server can read, not Lexical's tree"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Lexical's own Yjs binding keeps its rich node tree in the CRDT, in a form only JavaScript reads. The server must read the draft to release it, check its references, render its PDF and merge pushes into it.

<!-- field: decision -->
The draft holds `markdown` and multiline `text` fields as `Y.Text` of their text, and other fields as values in a `Y.Map`. The editor binds to the `Y.Text` by text diffs: its own changes as minimal edits of the Markdown, others' by re-importing the blocks they touched.

<!-- field: why -->
The server, with `yrs`, can then read, check and merge every draft, and a push's diff applies as text operations. Concurrent prose edits merge well as text. The cost is the editor's binding, which must keep the selection across remote changes (PLAN-016-G01).

<!-- akt: PLAN-016-D04 -->
---
type: decision
revision: 2
rank: b4S
state: decided
in: [PLAN-016]
title: Git repositories with packs in object storage and refs in Postgres
decided_by: albin@mattsson.io
---

<!-- field: context -->
Branches and merges can't be rebuilt from revisions, which form one line, so `git-remote-kuria` refuses merges. The branch workflow needs commits kept as pushed.

<!-- field: decision -->
A container type may say `"git": true`; each such akt is a git repository Kuria hosts, served over smart HTTP by the `git` binary. Objects are packfiles in object storage; refs are in Postgres, moved by compare-and-swap in the transaction that writes the push's akter.

<!-- field: why -->
Packfiles are content-addressed and never change, which suits object storage; refs must change atomically with Kuria's data, which Postgres does. The `git` binary is the reference implementation of the protocol and its checks. Plain `git clone https://…` replaces the helper, and branches, merges and tags work with no rebuilding.

<!-- akt: PLAN-016-D05 -->
---
type: decision
revision: 2
rank: b4T
state: decided
in: [PLAN-016]
title: "main is the drafts; pushes merge in, app edits become commits"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A repository and the drafts both hold the documents' text; one must be the other.

<!-- field: decision -->
`main` is the drafts. A push to it merges each changed file into its draft, as the edit from the commit's parent, so live edits survive. The server commits the app's edits on `main` after a minute's quiet or on release, authored by whoever made them. Other branches, merges and tags are kept and change nothing in Kuria; a release tags its commit.

<!-- field: why -->
Merging pushes as edits keeps people editing live and people pushing from git in step, with no locking. Committing the app's edits keeps `main` a true history of the drafts, so a branch made from it, reverted and merged back behaves as git users expect.

<!-- akt: PLAN-016-D06 -->
---
type: decision
revision: 2
rank: b4U
state: decided
in: [PLAN-016]
title: Documents are white A4 sheets with explicit page breaks
decided_by: albin@mattsson.io
---

<!-- field: context -->
A document should look like what it prints as, in either theme, with page breaks the author places.

<!-- field: decision -->
A `markdown` field is shown and edited as white A4 sheets, with the PDF's margins and fonts, in either theme. `<!-- pagebreak -->` alone on a line starts a new sheet, and `#pagebreak()` in the PDF. Pages end otherwise where a sheet would, shown by a guide; only explicit breaks are exact.

<!-- field: why -->
HTML pages stay editable in place. An HTML comment keeps the Markdown readable everywhere and invisible in other renderers. Paginating exactly as typst does would mean typst laying out the page live, which an editor can't do.

<!-- akt: PLAN-016-T01 -->
---
type: task
revision: 9
rank: b4V
state: done
in: [PLAN-016]
links: [Blocks PLAN-016-T02, Blocks PLAN-016-T03, Blocks PLAN-016-T04, Blocks PLAN-016-T06]
title: "Spec drafts, repositories, pages and directory attachments"
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec says what PLAN-016 builds. No implementation yet.

- **Naming:** `draft` already names a field filled by its recipe (`drafts.md`), so the collaborative draft is a `live draft`, declared `"live_drafts": true`, with its rules in a new `docs/spec/live-drafts.md` (`live.declare`, `live.fields`, `live.edit`, `live.read`, `live.release`, `live.revisions`, `live.history`, `live.sync` at `GET /akter/{number}/live`, `live.instances`, `live.presence`). The plan's design and D02 say so too.
- `docs/nomenclature.md`: `live draft`, `repository`, `main`, `page break`, `sheet`; `draft` points at `live draft`.
- `akter.md`: `akt.revision.record` excepts live drafts; validation lists `live_drafts` and `git`; a page break line. `versions.md`: `version.release` for live drafts.
- `git.md`: a Repositories section (`git.repo.declare`, `.storage`, `.serve`, `.push`, `.main`, `.commits`, `.branches`, `.seed`).
- `cli.md`: `cli.local.attachments`; `cli.local.unsupported` and `cli.local.convert` take attachments.
- `pdf.md`: page breaks, a live draft's PDF, inline unless `&download`. `frontend.md`: `app.pdf` opens versions in new tabs; new `app.sheets`, `app.live`.
- `requirements.md`, `architecture.md`.

`mise run lint` passes.

<!-- field: acceptance -->
- `mise run lint` passes, tracey listing the new rules as uncovered.

<!-- akt: PLAN-016-T02 -->
---
type: task
revision: 3
rank: b4W
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T08]
title: Attachments in akt directories
---

<!-- field: description -->
Every attachment field in akt directories (D01): local mode reads, checks and releases with them; `export --to-dir` writes them; `import --from-dir` uploads them.

<!-- field: acceptance -->
- Local tests: a directory with an image and an SBOM checks, releases a document embedding the image, and round-trips through a bundle.
- An e2e test: an index exported with its images, imported into another tenant, the pictures shown.

<!-- akt: PLAN-016-T03 -->
---
type: task
revision: 3
rank: b4X
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T08]
title: "Pages, page breaks and releases' PDFs in new tabs"
---

<!-- field: description -->
Pages and releases' PDFs (D06): A4 sheets in the app for showing and editing, white in either theme; `<!-- pagebreak -->` in the engine, the editor (Page break) and typst; guides where sheets end; Open PDF in a new tab, the route answering inline.

<!-- field: acceptance -->
- Engine and editor round trips with page breaks; a PDF with two pages from one break.
- e2e: sheets in dark mode stay white, a page break inserted, a release's PDF opened in a new tab; axe.

<!-- akt: PLAN-016-T04 -->
---
type: task
revision: 4
rank: b4Y
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T05, Blocks PLAN-016-T07]
title: Drafts that only releases make revisions of
---

<!-- field: description -->
Drafts (D02, D03): `"drafts": true`, validated; `akt_draft` and `akt_draft_update`; `yrs` reading and writing the draft's fields; edits to such akter change the draft; a release records it as a revision and version, with its checks; reading answers the draft; history shows the draft's changes by author.

<!-- field: acceptance -->
- Server tests: edits leave revisions alone, a release makes one, transitions carry the last release, references are checked on the draft, the draft's history.

<!-- akt: PLAN-016-T05 -->
---
type: task
revision: 3
rank: b4Z
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T04, Blocks PLAN-016-T08]
title: Editing a draft together with Yjs
---

<!-- field: description -->
Editing together: the y-websocket sync and awareness protocols at `/akter/{number}/draft`, read-only to readers, across instances through Valkey; the editor bound to `Y.Text` both ways, other fields to the map; presence.

<!-- field: acceptance -->
- Unit tests of the binding: local edits as minimal text operations, remote edits re-imported with the selection kept.
- e2e: two people editing one document at once, each seeing the other's edits and presence.

<!-- akt: PLAN-016-T06 -->
---
type: task
revision: 3
rank: b4a
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T07]
title: Git repositories on object storage
---

<!-- field: description -->
Git repositories (D04): `"git": true`; packfiles in object storage and `git_ref` in Postgres; smart HTTP at `/git/<number>.git` by the `git` binary with a pushed pack checked, stored and refs moved by compare-and-swap; `kuria git-credential`; each repository seeded from the helper's history.

<!-- field: acceptance -->
- Server tests: clone, fetch and push over HTTP against in-memory storage; a racing push refused; a seeded repository's commits equal the helper's.

<!-- akt: PLAN-016-T07 -->
---
type: task
revision: 4
rank: b4b
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T04, Blocked by PLAN-016-T06, Blocks PLAN-016-T08]
title: "main as the drafts: pushes, commits, branches, tags"
---

<!-- field: description -->
`main` and the drafts (D05): a push to `main` merged into drafts as text operations; the app's edits committed on `main` after a quiet minute or a release, by their authors; branches, merges and tags kept; a release tagged.

<!-- field: acceptance -->
- Server tests: a push merged with a concurrent live edit; app edits becoming commits; a merge commit pushed to `main`.
- e2e: the branch workflow with plain `git` over HTTPS.

<!-- akt: PLAN-016-T08 -->
---
type: task
revision: 5
rank: b4c
state: todo
in: [PLAN-016]
links: [Blocked by PLAN-016-T02, Blocked by PLAN-016-T03, Blocked by PLAN-016-T05, Blocked by PLAN-016-T07]
title: The example and its walkthrough
---

<!-- field: description -->
The example and its walkthrough: `document` with drafts, `document_index` with git; the walkthrough with two editors, a release, the branch workflow and an exported directory; docs; `git-remote-kuria` retired or kept for moving over (G03).

<!-- field: acceptance -->
- The walkthrough passes in `mise run e2e`, with lint and the tests.

<!-- akt: PLAN-016-G01 -->
---
type: gap
revision: 1
rank: b4d
state: open
in: [PLAN-016]
title: "The editor's binding to Y.Text"
---

<!-- field: description -->
The editor binds Lexical to a `Y.Text` of Markdown by diffing (D03). Re-importing the blocks a remote change touched must keep the selection, and structures that span blocks (lists, tables) may jump. Is block-level re-import good enough, or does the editor need a finer mapping from Markdown offsets to Lexical nodes?

<!-- akt: PLAN-016-G02 -->
---
type: gap
revision: 1
rank: b4e
state: open
in: [PLAN-016]
title: Repacking repositories in object storage
---

<!-- field: description -->
Every push adds a packfile to object storage. Fetches slow as packs accumulate. When and where are a repository's packs repacked into one, and how are unreachable objects (from deleted branches) collected without racing pushes?

<!-- akt: PLAN-016-G03 -->
---
type: gap
revision: 1
rank: b4f
state: open
in: [PLAN-016]
title: Moving clones off git-remote-kuria
---

<!-- field: description -->
Clones made with `git-remote-kuria` have the helper's commits. Seeding each repository from the same history keeps them valid, but their remote must change to `https://…/git/<number>.git`. Does the helper stay as a shim that forwards to the repository, or is it retired with a note on moving?

<!-- akt: PLAN-016-G04 -->
---
type: gap
revision: 1
rank: b4g
state: open
in: [PLAN-016]
title: Auditing a draft between releases
---

<!-- field: description -->
With drafts, the record of who changed what between releases is the draft's updates, not revisions. Is the update log, compacted, enough for an audit, or must each author's changes be kept as named snapshots?
