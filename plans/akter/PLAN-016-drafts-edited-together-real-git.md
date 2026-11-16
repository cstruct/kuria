---
number: PLAN-016
type: plan
revision: 30
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
revision: 3
rank: b4S
state: superseded
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
revision: 3
rank: b4T
state: superseded
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
revision: 6
rank: b4W
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T08]
title: Attachments in akt directories
assignee: albin@mattsson.io
---

<!-- field: description -->
Attachments in akt directories (D01), built in a worktree (`e50c08a`).

- **Files:** an attachment is `attachments/<number>/<name>`, named by that path in the akt file (`local/attachments.rs`: path rules, media types by extension, SBOMs through the engine's `sbom::detect`). `Ids` maps paths to stand-in identifiers, random per client, since imported attachment identifiers must be unique across a server.
- **Writing:** `@<file>` is checked and copied into `attachments/<number>/` on save, numbered rather than overwriting; a path in the directory is accepted; an upload's identifier is `LOCAL_UNSUPPORTED`. `kuria check` names a missing file, a wrong type or a missing SBOM.
- **Releases:** a local target is an image with a picture when its file exists; a release keeps `picture` per version.
- **Conversion:** `to_dir` writes each attachment's bytes; `bundle_of` carries each file's metadata and bytes, and every attachment any revision names, reading one since deleted from git (`git::last_bytes`).
- **Differences:** `cat` shows an attachment as its path in a directory and `{id, name}` on a server; diagram renderings aren't carried to directories; a file's changed bytes at one path aren't followed through history.
- **Tests:** `attachments_are_files_of_the_directory`, `a_document_embedding_a_picture_in_the_directory_releases`, `attachments_go_to_a_bundle_and_back`; an e2e test in `cli.spec.ts` (export to a directory and import into another tenant), run after merging.

<!-- field: acceptance -->
- Local tests: a directory with an image and an SBOM checks, releases a document embedding the image, and round-trips through a bundle.
- An e2e test: an index exported with its images, imported into another tenant, the pictures shown.

<!-- akt: PLAN-016-T03 -->
---
type: task
revision: 6
rank: b4X
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T08]
title: "Pages, page breaks and releases' PDFs in new tabs"
assignee: albin@mattsson.io
---

<!-- field: description -->
Sheets, page breaks and releases' PDFs in new tabs (D06), built in a worktree (`90860c8`).

- **Page breaks:** a top-level line holding only `<!-- pagebreak -->` is `#pagebreak()` in typst, a new sheet in the app (`sheets.ts`: `sheetsOf`, `guideOffsets`), and a `PageBreakNode` in the editor with a Page break button. A break inside a list, quote or footnote is dropped in both, since typst allows none inside containers; one in code or inline stays literal.
- **Sheets:** `sheets.css` lays a `markdown` field out as 210 mm sheets scaled to the column, 2 cm margins, a Libertinus-first serif at 11 pt; fixed `--sheet-*` tokens in `:root` only, the theme's tokens set to them inside a sheet so links, code, tables and footnotes stay legible on white in dark mode (contrast checked in `theme.test.ts`). Guides every A4 height, from a gradient when shown and an SVG overlay from measured breaks in the editor. Only sheets ended by a break are a full page high.
- **PDFs:** the route answers inline unless `&download`; the akt page's Download PDF adds it; the versions list has Open PDF per version, in a new tab.
- **Tests:** engine typst cases, the editor round trip, `sheets.test.ts`, `theme.test.ts`, the server's `a_page_break_starts_a_page_and_a_pdf_opens_unless_downloaded`; `e2e/tests/sheets.spec.ts` written, run after merging. Lint, 400 Rust and 100 app tests pass in its worktree.

<!-- field: acceptance -->
- Engine and editor round trips with page breaks; a PDF with two pages from one break.
- e2e: sheets in dark mode stay white, a page break inserted, a release's PDF opened in a new tab; axe.

<!-- akt: PLAN-016-T04 -->
---
type: task
revision: 7
rank: b4Y
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T05, Blocks PLAN-016-T07]
title: Drafts that only releases make revisions of
assignee: albin@mattsson.io
---

<!-- field: description -->
Live drafts on the server (D02, D03), built in a worktree (`4f9247e`).

- **Config:** `live_drafts`, requiring `versions`; a text field keyed `fields` is refused, as it would clash with the map.
- **Yjs** (`kuria_engine::live`, `yrs` 0.28): text fields as `Y.Text`, others in the `fields` map as GraphQL answers them (`Value::to_json`), not their input form, which can't carry who checked a checkbox or a verdict (`live.fields` says so). Edits are character diffs (`similar`) applied as Yjs operations.
- **Storage:** `akt_draft` (state, the fields it reads as, compaction and release marks) and `akt_draft_update` (each update, who and when, the fields changed; immutable), migration `000018`; compaction on release and every 64 updates.
- **Writes:** `editAkt`, batches and `draftField` on a live akt run the engine's checks on the draft's values, then sections, contacts, attachments and diagrams, and write one update; no revision. An edit doesn't check the revision it names; transitions and releases do (`live.edit`, `akt.revision.concurrency`). Creating makes revision 1 and the draft; copying copies it. A transition records the last revision's fields, and one that would set fields is refused.
- **Reading:** every read overlays the draft's fields and title; versions and revisions as before; a PDF without a version is of the draft.
- **Releasing:** checks references and pins on the draft, records it as a revision with a write's checks and its reference links, names it as the next version; refused when the draft equals the latest version.
- **GraphQL:** `AktType.liveDrafts`, `Akt.draftChanges(all)` (`DraftChange`: seq, by, at, fields).
- **Left for later:** no change events for draft edits (T05); bundles carry the latest revision, not the draft; `editAkt` sends whole values, so it overwrites another's concurrent change to that field, as only Yjs clients merge.
- **Tests:** `tests/live_drafts.rs` (6), engine Yjs tests (6), config (2).

<!-- field: acceptance -->
- Server tests: edits leave revisions alone, a release makes one, transitions carry the last release, references are checked on the draft, the draft's history.

<!-- akt: PLAN-016-T05 -->
---
type: task
revision: 6
rank: b4Z
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T04, Blocks PLAN-016-T08]
title: Editing a draft together with Yjs
assignee: albin@mattsson.io
---

<!-- field: description -->
Editing a live draft together (`live.sync`, `live.instances`, `live.presence`, `app.live`), built in a worktree (`85bdcde`, `2305b7c`).

- **Server** (`live.rs`, `live/{room,socket,presence}.rs`): `GET /akter/{number}/live` speaks y-websocket's sync and awareness protocols, authenticated as `/graphql/ws`; 404 for an akt without a live draft or one the subject can't read; readers are told they may read only. An update is taken under the akt's lock and checked (`kuria_engine::live::check`, then sections, contacts and attachments in a savepoint); accepted ones are stored by author, with a follow-up stamping checkboxes; a refused one is stored with the update undoing it (`LiveDraft::restore`), both sent to every client and the reason to the sender. One room per open draft per instance sends stored updates after the last it sent, woken by its own clients, a Valkey message (`kuria:live:<tenant>:<akt>`, identifiers only) or a one-second poll. Awareness stamped with the subject's `{id, name}`; a new change kind `draft` (migration `000021`).
- **App:** `editor/live.ts` binds Lexical to a `Y.Text` (local changes as the one differing span; remote ones re-import the changed top-level blocks, the selection mapped through Markdown offsets and the Yjs delta); `bindTextarea` for multiline text; `live.svelte.ts` wraps the y-websocket provider (cross-tab broadcast off) with saved/saving, refusals and read-only; `LivePresence.svelte`; remote carets with names. Text fields edited live with Done; other fields saved to the map; checkboxes and verdicts through `editAkt`. `yjs` 13.6.33, `y-protocols` 1.0.7, `y-websocket` 3.1.0, `lib0` 0.2.119.
- **Differences:** live updates check heading ids but don't add them (inserting while someone types breaks their typing), the next `editAkt` does; diagrams aren't rendered on live updates, a release still does; a verdict through the map is refused; edits through `editAkt` or a push reach live clients within about a second.
- **G01:** lists and tables are one block each, so a remote edit in one re-reads it and the caret is mapped by text offset; carets can drift where markup matches text; Markdown not in the editor's normal form is rewritten by the next local edit as one larger change.
- **Tests:** `tests/live_sync.rs` (6, two instances sharing a database and Valkey), engine (3 more), presence, the app's `editor/live.test.ts` (7) and `live.test.ts` (3); `e2e/tests/live.spec.ts` written, run with T08.

<!-- field: acceptance -->
- Unit tests of the binding: local edits as minimal text operations, remote edits re-imported with the selection kept.
- e2e: two people editing one document at once, each seeing the other's edits and presence.

<!-- akt: PLAN-016-T06 -->
---
type: task
revision: 6
rank: b4a
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T01, Blocks PLAN-016-T07]
title: Git repositories on object storage
assignee: albin@mattsson.io
---

<!-- field: description -->
Git repositories on object storage (D04), built in a worktree (`5a91797`, `a4de226`), merged in `f007f5c`.

- **Config:** `git` on a type that contains akt types.
- **Storage:** packfiles at `git/<tenant>/<akt>/pack-<sha>.{pack,idx}` in object storage; `git_ref` in Postgres (migration `000019`), moved by `move_ref` compare-and-swap.
- **Cache:** a bare repository per repository under `KURIA_GIT_CACHE`, locked per request, filled with packs it lacks and `packed-refs` from `git_ref`; throwaway.
- **Serving:** `info/refs` and `git-upload-pack` by the `git` binary; `git-receive-pack` answered by the server itself: commands parsed, the pack indexed into a quarantine, each update checked (branch or tag, from `git_ref`'s target, connected), `main` as below, then one transaction runs the batches (`akter::batch_in`), moves every ref by compare-and-swap and stores the pack; any refusal refuses every update, keeping nothing.
- **Auth:** routes authenticated as `/graphql`; git's Basic password taken as the bearer token; 401s ask for Basic so git asks its helper; `kuria git-credential get|store|erase`.
- **Shared code:** the server uses `kuria-cli` as a library: the helper's history builder and push logic run in-process through `kuria_cli::session::Api`, answered by the server's schema as the caller.
- **Seeding:** a repository without refs is seeded from the helper's history through `git fast-import`; commits equal the helper's, by id.
- **Until T07:** a push to `main` must be a linear fast-forward of the history Kuria would build now; merges on `main` refused; releases after seeding untagged.
- **The credential gap:** the CLI's tokens are DPoP-bound, which Konklav refuses as plain bearer tokens, so `kuria git-credential` authenticates only against the test host (PLAN-016-G05).
- **Tests:** `tests/repositories.rs` (6, real git over a local port), the store's CAS, config, the credential protocol, pkt-lines and Basic parsing.

<!-- field: acceptance -->
- Server tests: clone, fetch and push over HTTP against in-memory storage; a racing push refused; a seeded repository's commits equal the helper's.

<!-- akt: PLAN-016-T07 -->
---
type: task
revision: 7
rank: b4b
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T04, Blocked by PLAN-016-T06, Blocks PLAN-016-T08]
title: "main as the drafts: pushes, commits, branches, tags"
assignee: albin@mattsson.io
---

<!-- field: description -->
`main` as the drafts (D05), built in a worktree (`60c7125`).

- **Kuria's changes become commits** (`git/commits.rs`, `git_mark` in migration `000022`, marking which revision and draft update each akt's file on `main` reflects): a revision or membership change at the next pass; a release at once, from its revision, lightweight-tagged `<number>/v<n>`; a live draft's edits after `Timing.quiet` (a minute) without changes. The tree is rebuilt with the helper's code as Kuria (`Authorizer::kuria()`, read-only, never given to a request); author the latest changer, others `Co-authored-by:`, committer Kuria; pack stored, then `main` moved by compare-and-swap with the marks and tags. Passes run before every git request and every 5 s in the server.
- **Pushes to `main`:** a fast-forward of `git_ref`'s `main`; each commit along the first parents applied as its change from its first parent; merge commits taken; normal form still required. Live drafts get each changed text field three-way merged (`kuria_engine::merge::merge_text`: by lines, then characters where both changed nearby; overlapping changes go to the push) into the draft as it is; other fields the push's value.
- **Differences:** merged draft updates are recorded as the authenticated pusher, not the commit's claimed author; dropping unready work on `main` must be `git reset`, not `git revert`, since a reverted commit merged back is a no-op in git; "at once" is the next pass.
- **The helper** refuses a container with `"git": true`, giving its `…/git/<number>.git` URL and the `git remote set-url` to move (G03); `AktType.git` in GraphQL.
- **Tests:** `tests/repositories.rs` (12): quiet-period commits with co-authors, a release committed and tagged, a push merged with a concurrent app edit, a merge commit on `main`, the branch workflow end to end, a push behind Kuria's commit refused then taken after rebase, the helper's refusal; merge unit tests.

<!-- field: acceptance -->
- Server tests: a push merged with a concurrent live edit; app edits becoming commits; a merge commit pushed to `main`.
- e2e: the branch workflow with plain `git` over HTTPS.

<!-- akt: PLAN-016-T08 -->
---
type: task
revision: 10
rank: b4c
state: dropped
in: [PLAN-016]
links: [Blocked by PLAN-016-T02, Blocked by PLAN-016-T03, Blocked by PLAN-016-T05, Blocked by PLAN-016-T07]
title: The example and its walkthrough
assignee: albin@mattsson.io
---

<!-- field: description -->
Superseded by PLAN-016-T13 once D07 and D08 replaced hosted repositories with clones: the example (`document` live, `document_index` without git), the walkthrough with two editors (`live.spec.ts`), a release, the branch workflow through `kuria clone/pull/push` with local git, and an exported directory (`cli.spec.ts`) were done there. G03 went with D08.

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
revision: 2
rank: b4e
state: closed
in: [PLAN-016]
title: Repacking repositories in object storage
---

<!-- field: description -->
Every push adds a packfile to object storage. Fetches slow as packs accumulate. When and where are a repository's packs repacked into one, and how are unreachable objects (from deleted branches) collected without racing pushes?

<!-- field: resolution -->
Moot: hosted repositories are removed (PLAN-016-D08).

<!-- akt: PLAN-016-G03 -->
---
type: gap
revision: 2
rank: b4f
state: closed
in: [PLAN-016]
title: Moving clones off git-remote-kuria
---

<!-- field: description -->
Clones made with `git-remote-kuria` have the helper's commits. Seeding each repository from the same history keeps them valid, but their remote must change to `https://…/git/<number>.git`. Does the helper stay as a shim that forwards to the repository, or is it retired with a note on moving?

<!-- field: resolution -->
Resolved (T07): `git-remote-kuria` refuses a container whose type has `"git": true`, naming its `https://…/git/<number>.git` and the `git remote set-url` that moves a clone over; repositories are seeded from the helper's history, so the clone's commits stay valid.

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

<!-- akt: PLAN-016-G05 -->
---
type: gap
revision: 2
rank: b4h
state: closed
in: [PLAN-016]
title: "git's credentials against DPoP-bound tokens"
---

<!-- field: description -->
The CLI's access tokens are DPoP-bound: each request carries a proof signed with the CLI's key. git sends a fixed credential, so `kuria git-credential` hands it the token as a plain password, which Konklav refuses as a bearer token. Against real Konklav, cloning over HTTPS therefore fails; only the test host accepts it. Options: Kuria mints its own short-lived git tokens for a DPoP-authenticated request (`kuria git-credential` asks for one), kept hashed and scoped to the subject and tenant, deciding permissions as the subject; or Konklav issues an unbound token for git. Which?

<!-- field: resolution -->
Moot: hosted repositories are removed (PLAN-016-D08).

<!-- akt: PLAN-016-D07 -->
---
type: decision
revision: 2
rank: b4i
state: decided
in: [PLAN-016]
title: "Clones sync with live drafts through kuria; git stays local"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Hosted repositories (D04, D05) made Kuria invent commits from live drafts on a timer, keep every clone's commit ids equal, store and repack packs, and authenticate git over HTTPS, which DPoP-bound tokens can't do (G05). A CRDT has no commits: what means something is a release, a deliberate push, and live edits by author. What people want from git is a working copy: clone records a base, edit, stage what to send, push after fetching the current state, resolve conflicts.

<!-- field: decision -->
`kuria clone` writes a container and its members as files with a base in `.kuria/`; `kuria status`, `diff`, `add` (with `-p` for hunks), `pull` and `push` work against it. A push fetches first and is refused unless the base is current; it sends each live draft's staged change as a Yjs update made against the base, checked as a live edit and recorded as one change set with its author and message, and other akter's as edits. A pull merges the remote state into the working copy three ways, writing conflict markers where both changed the same text. git, if used, is local: branches, reverts and merges are the user's, and Kuria sees only files.

<!-- field: why -->
An update made against the base merges with live edits character by character, as Yjs does, with no commit to invent. The CLI's own signed session authenticates, so no git credentials are needed. Since no commit leaves the machine, nothing needs equal ids across clones, and a revert or a reset are alike to Kuria. Conflict markers are what git users know.

<!-- akt: PLAN-016-D08 -->
---
type: decision
revision: 2
rank: b4j
state: decided
in: [PLAN-016]
title: Hosted repositories and git-remote-kuria are removed
decided_by: albin@mattsson.io
---

<!-- field: context -->
With clones synced through `kuria` (D07), hosted repositories (T06, T07) and `git-remote-kuria` have no use left.

<!-- field: decision -->
They are removed: the `git` flag, smart HTTP, packs and refs, Kuria's commits, the credential helper, and the helper binary. Their layout code (files, sections as directories, pictures, normal form) and the three-way text merge stay, for clones. A migration drops `git_ref` and `git_mark`.

<!-- field: why -->
Less to run and secure, nothing to repack, and one way to work locally instead of three.

<!-- akt: PLAN-016-T09 -->
---
type: task
revision: 7
rank: b4k
state: done
in: [PLAN-016]
links: [Blocks PLAN-016-T10, Blocks PLAN-016-T11]
title: Spec clones synced with live drafts
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec of clones synced with live drafts (D07, D08). No implementation.

- New `docs/spec/clones.md`: `clone.tree`, `clone.pictures`, `clone.normal` (the layout `git.md` had), and `clone.clone`, `clone.status`, `clone.add` (with `-p`), `clone.push` (fetch first, refused unless the base is current; live drafts as updates against the base, others as edits at the base's revision; new and removed files refused), `clone.pull` (three-way, conflict markers `<<<<<<< ours` / `=======` / `>>>>>>> kuria`), `clone.server` (`pushClone`, all or nothing, `STALE` for a stale base, a change set with its message).
- `docs/spec/git.md` removed, with its repository and helper rules.
- `live-drafts.md`: `live.state` (the draft's Yjs state and state vector answered for clones); `live.edit` and `live.history` name pushes from clones and their messages.
- `cli.md`, `akter.md`, `bundles.md`, `README.md`: references to the removed rules gone.
- Nomenclature: `clone` redefined; `base`, `change set` new; `repository`, `main` removed. `architecture.md` and `requirements.md` describe clones.

Code still references the removed `git.*` rules until T10.

<!-- field: acceptance -->
- `mise run lint` passes, tracey listing the new rules as uncovered.

<!-- akt: PLAN-016-T10 -->
---
type: task
revision: 6
rank: b4l
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T09, Blocks PLAN-016-T12]
title: Remove hosted repositories and git-remote-kuria
assignee: albin@mattsson.io
---

<!-- field: description -->
Hosted repositories and `git-remote-kuria` removed (D08), built in a worktree (`28f332c`).

- **Gone:** the server's git module, routes, Basic-credential middleware, cache, timing and committer; `Authorizer::kuria()`; named storage; the server's dependency on `kuria-cli`; `AktType.git`; the `git` flag; `kuria_store::git`; `git_ref` and `git_mark` (dropped by migration `000023`, the old migrations kept); the helper binary, its history builder, cache and push; `kuria git-credential`; `Session::answered_by`; the batch's `commit` input; `git-remote.spec.ts` and the git part of the documents walkthrough.
- **Kept, moved to `items/cli/src/clone/`:** `CloneOf` (batched reads, pictures, normal tree), `layout.rs` (`tree_of`, `place`, `filed`, `normal`, `title_in`), `pictures.rs`, and `fmt.rs`: `kuria fmt` finds the clone's root by `.kuria/`, lists files without git, and reads the container's number from `.kuria/clone.json`, else the root's single akt file, else asks Kuria. Annotated `clone.tree`, `clone.pictures`, `clone.normal`.
- **Note:** stored type documents with `"git": true` no longer read (`deny_unknown_fields`).
- Lint (tracey with no `git.*` reference), 430 Rust and 110 app tests pass in its worktree.

<!-- field: acceptance -->
- Lint and tests pass with no `git.*` rule referenced and nothing left of the removed parts.

<!-- akt: PLAN-016-T11 -->
---
type: task
revision: 6
rank: b4m
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T09, Blocks PLAN-016-T12]
title: Pull and push on the server
assignee: albin@mattsson.io
---

<!-- field: description -->
Pull and push on the server (`clone.server`, `live.state`), built in a worktree (`6e7bc9f`).

- **`Akt.liveState`:** `state` (the draft as a Yjs v1 update), `stateVector` and `base`, base64; null without a live draft or read at a version or revision.
- **The base is a Yjs snapshot, not a state vector:** a delete-only change doesn't move a state vector, so comparing vectors would let a push slip past it; a snapshot (vector and delete set) moves with every change, compared decoded. The spec says so.
- **`pushClone(input: { message, drafts: [{ number, base, update }], operations })`:** a one-line message of 1 to 200 characters; all locks first, then each draft refused as `STALE` (naming `number`, `base`, `stateVector`, `draft`) unless its base is current, applied and checked as a WebSocket update is, diagrams rendered, kept as one `akt_draft_update` with the message (migration `000024`) and a `draft` change event; operations as a batch's with the message as reason. Any refusal names its `draft` or `operation` and keeps nothing. Answers `PushedClone { drafts, operations }`.
- **`DraftChange.message`.** `LiveDraft::of` builds with a fixed client so an update made against one read applies to the next; `base()`, `is_at()`.
- **Tests:** `tests/clones.rs` (5: a change set with author and message; stale bases, a delete-only one included; all or nothing across two drafts and an operation; operations' revisions and stale revisions; `liveState` round trip), engine and push unit tests.

<!-- field: acceptance -->
- Server tests: a push from a base merged with a concurrent live edit; a stale base refused; a refused field keeps nothing; the change set listed with its message.

<!-- akt: PLAN-016-T12 -->
---
type: task
revision: 8
rank: b4n
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T11, Blocked by PLAN-016-T10, Blocks PLAN-016-T13]
title: "The CLI's clone workflow"
assignee: albin@mattsson.io
---

<!-- field: description -->
The CLI's clone workflow (`clone.clone`, `.status`, `.add`, `.push`, `.pull`), built in a worktree (`406ac10`).

- **`.kuria/`:** `clone.json` (`url`, `number`), `base.json` (each akt's path, revision and, for a live draft, its state and base), `base/` and `staged/` files, and a `.gitignore` of `*`.
- **`push -m`:** fetches the staged akter first, refused as `STALE` (with a hint to pull) unless each is as the base has it; a live draft's change goes as a Yjs update made with `LiveDraft` against the base's state, texts merged by characters; other akter's as `editAkt` at the base's revision; all in one `pushClone`. New and removed files, changed pictures, changed read-only keys and files not in normal form are refused. The live state after a push is computed locally.
- **`pull`:** three-way merges each file with the engine's new `merge_marked`, writing `<<<<<<< ours` / `=======` / `>>>>>>> kuria` where both changed the same text; a file with markers can't be staged. Adds and removes members, restages or unstages.
- **`add` and `diff`:** inside a clone they take paths (`add -p` stages hunks); elsewhere a container and members, and an akt and versions.
- **Docs:** reverting unready edits with `git revert` means a later `git merge` won't bring them back; `git reset` is advised.
- **Tests:** `tests/clones.rs` against a server on a local port (clone, stage a hunk, push; stale push refused and merged by pull; conflict markers resolved and pushed; the branch workflow with local git), `merge_marked` unit tests, and `e2e/tests/clone.spec.ts` (run in T13). 449 Rust and 110 app tests pass after merging.

<!-- field: acceptance -->
- CLI tests against a test server: clone, edit, stage a hunk, push; a push behind a live edit refused, pull merging it; a conflict written as markers, resolved and pushed; the branch workflow with local git.

<!-- akt: PLAN-016-T13 -->
---
type: task
revision: 6
rank: b4o
state: done
in: [PLAN-016]
links: [Blocked by PLAN-016-T12]
title: "The walkthrough, the failing browser tests and docs"
assignee: albin@mattsson.io
---

<!-- field: description -->
The example's documents live, the clone walkthrough, and all of `mise run e2e` green (`959beb7`).

- **Example:** `document` says `"live_drafts": true`; its README says how live drafts work. The browser tests close the editor with **Done**; `live.spec.ts` and `clone.spec.ts` no longer redefine the type.
- **Walkthrough** (`documents.spec.ts`): after the release, the index cloned with `kuria clone` into a git repository of its own (`.kuria/` not committed, the picture linked relatively); a ready commit and an unready one; the unready one kept on a branch and reset away; the ready part pushed and released in the app; the branch merged back and pushed; the guide has it, version 1 doesn't; `kuria status` clean.
- **Fixed on the way:**
  - `kuria clone <number> <dir>` took the directory as the global `--dir` and refused as local mode; the positional is now `into`, with a parsing test.
  - A diagram typed into a live draft was never drawn: WebSocket edits now render each block that compiles and pass over one half written (`live.sync`, server test).
  - A release checked references to a live akt against its last revision, not its live draft (`live.read`, server test). Its pin still names that revision: PLAN-016-G06.
  - Tests pick the example's Image type when other image types exist, and wait for a transition's dialog before its state.
- Lint, 451 Rust, 110 app and 39 browser tests pass.

<!-- field: acceptance -->
- All of `mise run e2e` passes, with lint and the tests.

<!-- akt: PLAN-016-G06 -->
---
type: gap
revision: 1
rank: b4p
state: open
in: [PLAN-016]
title: "A release pins a live target's last revision"
---

<!-- field: description -->
A release checks a reference to an akt with a live draft against that draft (its living version), but pins the target's latest revision, which is its last release and may not hold the heading the reference names. Should a release of A pin B's live draft as it is, say by releasing B too, recording a snapshot, or refusing until B is released with the heading?
