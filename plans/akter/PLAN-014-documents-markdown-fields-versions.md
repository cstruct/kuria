---
number: PLAN-014
type: plan
revision: 24
rank: b3k
state: active
title: "Documents: markdown fields, versions, references, PDFs and a git remote"
owner: albin@mattsson.io
---

<!-- field: goal -->
Documents kept in Kuria, with a living version anyone allowed can edit and versions released by hand that never change. On a server and through git alike:

- **Write in the app:** a document's body is edited in a rich-text editor built on [Lexical](https://github.com/facebook/lexical), and stored as Markdown.
- **Work locally with git:** `git clone kuria::https://kuria.example/DOCS-1` clones a document index as a folder tree of Markdown files. You edit, commit, `git pull --rebase` and `git diff` as with any repository, and `git push` sends the commits back as revisions.
- **Link precisely:** a document links to other documents and to a heading within them. A release is refused while any link points at a missing document or heading.
- **Release:** releasing a document names a version of its current revision (version 1, 2, … with an optional label). A released version can be read, linked to and diffed, and never changes.
- **PDF:** the living version and any released version can be downloaded as a PDF of the body.

Nothing in Kuria knows these names. They are generic features any akt type can use: a `markdown` field kind, versions, references between akter, PDFs, and the git remote. An example, `docs/examples/documents`, puts them together as two akt types, `document_index` and `document`.

Success: the example's walkthrough runs in `mise run e2e`. A document is written in the editor with a deep link, its release is refused while the link is broken and taken once it isn't, and its PDF downloads. The same index is cloned with git, edited and rebased onto a change made in the app, then pushed, and the app shows the pushed revision.

<!-- field: design -->
## Where it starts

- **Akter and revisions:** every change to an akt is an immutable revision with its author, time and reason, and appends a change event numbered per tenant (`akt.revision.record`, `akt.change-event`). Final akter can't be edited.
- **Sections:** a `sections` field (PLAN-012) is a nested outline of a container's members, with titles, Markdown text and collapsible sections. Each member is placed at most once.
- **Links:** typed links declared by the source's akt type (`link.kinds`), which may pin a revision of their target (`link.pin`).
- **Multiline text** is rendered as Markdown in the app, with `marked` and DOMPurify (`app.fields`). There is no Markdown parser on the Rust side yet.
- **The CLI** reads and writes akt files, Markdown with front matter and one section per field (`cli.file`). It applies them back with revision checks (`cli.apply`), in batches (PLAN-011), and works in local mode on an akt directory kept in git (PLAN-008). It already runs `git` for history (`items/cli/src/local/git.rs`).
- **Attachments** are immutable files with a download route (`attachment.*`).

## The example

- **`document_index`:** a title, an owner, and a `sections` field placing its documents. It contains `document`. Its sections are the folders (PLAN-014-D01), so a folder is a section with a title, and documents sit in sections nested as deep as needed.
- **`document`:** a title, an owner, a `body` of kind `markdown`, and versions, released with the Konklav action `documents:release`. Created in an index and numbered after it (`DOCS-1-D01`).

## A `markdown` field (PLAN-014-D02)

- **Value:** CommonMark text, with GitHub's tables and task lists, and two additions:
  - **Heading ids:** `## Installing {#install}`. A heading without an id is given one when it is written: a slug of its text, unique within the field. It keeps that id when its text changes, so links survive renaming.
  - **References:** links whose target is `kuria:<NUMBER>`, optionally with `@<version>` and `#<heading id>`. For example, `[setup](kuria:DOCS-1-D04#install)` or `[setup, as released](kuria:DOCS-1-D04@2#install)`.
- **Engine:** `kuria-engine` gains a `markdown` module, built on `pulldown-cmark` with heading attributes. It parses a value, gives missing heading ids, and lists the headings and the references. The server and local mode both write through it (`cli.local.rules`).
- **References as links:** a `markdown` field names the link kind its references are recorded as (`"references": "refers_to"`). Each write replaces the akt's links of that kind with those its fields reference, so "Referred to by" works on the target. `addLink` and `removeLink` refuse that kind, since the text owns it.
- **In the app:** rendered as today, plus anchors on headings. References become links into the app (`/akt/DOCS-1-D04#install`, `?version=2` for a version). A missing target is shown as broken.

## The editor (Lexical)

- A `markdown` field is edited with Lexical's core, mounted in a Svelte component. It uses only the nodes Markdown can hold: headings, paragraphs, emphasis, lists, quotes, code, links, tables and horizontal rules.
- Import and export go through `@lexical/markdown` transformers, with custom ones for heading ids and references. Heading ids ride on the heading node, so the editor keeps them.
- **Insert link:** a picker searches akter, then lists the chosen one's headings. Its living version is the default; a released version can be chosen instead.
- Saving sends the Markdown through `editAkt` as any field.
- A round-trip test checks that every construct the editor offers comes back from Markdown unchanged.

## Versions (PLAN-014-D03)

- **Declaring:** an akt type may say it releases versions:
  ```json
  "versions": { "action": "documents:release" }
  ```
  The action is the Konklav action a release needs, besides `kuria:akter:edit`.
- **Releasing:** `releaseVersion(number, revision, label?)` names a version of the akt's current revision, with `revision` guarding against a stale page. It is refused if `revision` isn't current.
- **A version** is numbered 1, 2, … per akt, with an optional label, and records who released it and when.
  - It isn't a revision. Releasing changes no field, but it appears in the akt's history and appends a change event.
  - It is never changed or removed. A final akt can still be read at its versions, but no longer released.
- **Reading:**
  - `akt(number:, version:)` reads the akt at a version, as a pinned link reads a revision.
  - `versions` lists an akt's versions.
  - The CLI has `kuria release <number> [--label]` and `kuria versions <number>`. `cat`, `show` and `pdf` take `--version`.
  - The app has a Release dialog, a version picker on the akt page, and a diff between two versions or a version and now.
- **Checks at release (PLAN-014-D05):** every reference in the akt's `markdown` fields must name an akt the releaser may read. A heading it names must exist:
  - in that akt's living version, for an unversioned reference;
  - in the named version, for a versioned one.
  Otherwise the release is refused, naming each broken reference.
- **Pinning at release:** the version records what each unversioned reference meant: the target's latest version, if it has one, else the revision it was at. Reading a released version follows those pins, so its links never change meaning.

## PDFs (PLAN-014-D06)

- **Contents:** a `markdown` field's value rendered as a PDF, body only: its headings, text, lists, tables and code.
  - References become the target's title, linked to the app.
  - Headings become the PDF's bookmarks.
- **Rendering:** on the server, converting the Markdown to [typst](https://typst.app) markup through the engine's parser, and compiling it with the `typst` crate, in Rust with fonts bundled.
- **Where to get one:**
  - Server: `GET /akter/<number>/pdf?field=body[&version=2]`.
  - CLI: `kuria pdf <number> [--version] -o <file>`.
  - App: a Download PDF action.
- A version's PDF is rendered from the version's content and pins, so it is the same each time.

## The git remote (PLAN-014-D04)

- **Installing and cloning:** `git-remote-kuria` is a second binary of the CLI crate, installed with `kuria`. Git runs it for `kuria::<url>` remotes, so `git clone kuria::https://kuria.example/DOCS-1` clones a container and its members. It signs in as the CLI does (`kuria login`).
- **The tree:**
  - The container's akt file sits at the root.
  - Each member's akt file (`cli.file`) sits in folders that mirror the container's `sections`, one directory per section, named by a slug of its title.
  - Members in no section sit at the root.
  - Files are named as `cli.local.names` says.
- **History:**
  - **Commits:** one commit per revision of the container or any member, or one per batch, in change-event order. The author and time are the revision's, and the message is its reason. A version is a tag, `<NUMBER>/v<n>`.
  - **Same commits for everyone:** commits are built in the helper from what the server answers, the same way for everyone, so every clone has the same commits.
  - **Pushed commits keep their identity:** a pushed commit's author, committer, times, message and parent are stored with the revisions it made. Every clone then builds that very commit, not a look-alike.
- **Fetching:** asks for the change events after the last one fetched.
- **Pushing:**
  - Each new commit is applied as one batch, checked by the same rules as any write.
    - An edited file is an edit.
    - A file moved between folders is a move between sections.
    - A section directory renamed changes its title.
    - A deleted file is removed from the container.
  - **Normal form:** the tree after the push must be exactly the one the server would write. Otherwise the push is refused, saying what differs, for example a heading without an id. `kuria fmt` in the clone puts files in that form, so the commit can be amended.
  - **Fast-forward only:** a push not on top of the server's latest is refused as non-fast-forward, and `git pull --rebase` resolves it.
  - Merge commits are refused, so the history stays linear.

## Constraints

- **Same rules everywhere:** the server, local mode and the git remote all write through `kuria-engine`.
- **Released versions never change:** no write changes a version or what it pins.
- **Generic:** none of these features names `document`. The example is ordinary akt type documents, as for test protocols.
- **Same app conventions:** keyboard reachable, checked by axe, light and dark.

## Order

1. Spec and nomenclature.
2. The `markdown` field, with heading ids and references.
3. Then, in parallel:
   - versions, with the release check;
   - the Lexical editor;
   - PDFs.
4. The git remote, once versions exist for its tags.
5. The example and its walkthrough.

<!-- field: verification -->
- **`kuria-engine`:**
  - The Markdown parser on a table of cases: heading ids given, kept and made unique, and references parsed with and without a version or heading.
  - The release check: broken akt, broken heading, versioned and unversioned references, and pins.
  - The typst conversion on the same cases.
- **`kuria-config`:** `markdown` fields, `references` naming a declared link kind, and `versions` with a valid action.
- **Server:**
  - Writes give heading ids and replace reference links.
  - `addLink` and `removeLink` refuse a references kind.
  - `releaseVersion`: refused when stale, without the action, or with a broken link; taken otherwise.
  - Reading at a version follows its pins. Change events for versions.
  - The PDF route for the living version and a version: a PDF with the body's text and bookmarks.
- **CLI:**
  - Local tests for `markdown` fields and versions in an akt directory.
  - The parity test extended with references, a refused and a taken release, and `--version` reads.
  - `git-remote-kuria` tested against a server:
    - clone gives the expected tree and commits;
    - two clones get the same commit ids;
    - a push applies a commit as a batch;
    - a non-fast-forward push is refused and works after `pull --rebase`;
    - a push not in normal form is refused;
    - versions appear as tags.
- **App:**
  - Unit tests for the Lexical Markdown round trip and for reference rendering.
  - The documents walkthrough in `mise run e2e`: edit with the editor, deep link, refused and then taken release, PDF download, git clone, rebase onto an app change, push. Axe on each view.
- `mise run lint`, `mise run test` and `mise run e2e` pass.

<!-- akt: PLAN-014-D01 -->
---
type: decision
revision: 2
rank: b3l
state: decided
in: [PLAN-014]
title: "Folders are the index's sections"
decided_by: albin@mattsson.io
---

<!-- field: context -->
A document index is a tree of folders holding documents. Kuria already has a nested outline of a container's members: a `sections` field (PLAN-012).

<!-- field: decision -->
A folder is a section of the index's `sections` field. There is no folder akt type.

<!-- field: why -->
Sections already nest, have titles and place each member once, and the app, the CLI and copying handle them. A folder akt type would give folders their own permissions, links and history, which nothing asks for, at the cost of a number per folder.

<!-- akt: PLAN-014-D02 -->
---
type: decision
revision: 2
rank: b3m
state: decided
in: [PLAN-014]
title: Documents are Markdown
decided_by: albin@mattsson.io
---

<!-- field: context -->
A document must be editable in the app with Lexical, readable and diffable in git, and rendered as a PDF. Lexical's own format is a JSON editor state.

<!-- field: decision -->
A `markdown` field holds CommonMark with tables, heading ids and `kuria:` references. Lexical edits it through Markdown, offering only what Markdown can hold.

<!-- field: why -->
Markdown is what the akt file and git diffs already are, and what the CLI, the server and the PDF renderer can all parse with one parser. Lexical's JSON would round-trip the editor exactly, but would leave git, the CLI and the server without a readable form.

<!-- akt: PLAN-014-D03 -->
---
type: decision
revision: 2
rank: b3n
state: decided
in: [PLAN-014]
title: A released version names a revision
decided_by: albin@mattsson.io
---

<!-- field: context -->
A document has a living version and versions released by hand that never change. Revisions are already immutable. A release could instead be a separate akt holding a copy.

<!-- field: decision -->
A version names one revision of an akt: numbered 1, 2, … per akt, with an optional label, by whom and when. Any akt type may release versions, declaring the Konklav action a release needs (`versions.action`). A version isn't a revision, but is in the akt's history and appends a change event.

<!-- field: why -->
Revisions are already what a version needs to be: immutable and readable at any time, as pinned links read them. Naming one duplicates nothing. Numbering by Kuria, with a free label beside it, and making versions generic were proposed in discussion as defaults, and can be revisited here before T03.

<!-- akt: PLAN-014-D04 -->
---
type: decision
revision: 2
rank: b3o
state: decided
in: [PLAN-014]
title: Git through a remote helper in the CLI
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents should be cloneable with git, edited locally and pushed back, with rebase and diff out of the box.

<!-- field: decision -->
`git-remote-kuria`, a binary of the CLI crate, makes `kuria::<url>` a git remote. It builds commits from revisions and change events, and applies pushed commits as batches. Kuria itself serves no git protocol and stores no git objects; it keeps a pushed commit's metadata with the revisions it made.

<!-- field: why -->
Git's remote-helper protocol gives clone, fetch, pull --rebase, diff and push without a git server in Kuria. A git server over HTTPS would work without the CLI, but would mean storing git objects and keeping them consistent with revisions.

<!-- akt: PLAN-014-D05 -->
---
type: decision
revision: 2
rank: b3p
state: decided
in: [PLAN-014]
title: References are checked and pinned at release
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents link to each other and to headings within them, and a release must not have broken links. A link to a living version can break after the release, when its target changes.

<!-- field: decision -->
A release is refused while any reference in its `markdown` fields names an akt the releaser can't read, or a heading the target (or the version named) doesn't have. The version records what each unversioned reference meant: the target's latest version if it has one, else its revision then.

<!-- field: why -->
Checking makes "no broken links" true at release. Pinning keeps it true afterwards: a released version reads as it was released, whatever its targets do later.

<!-- akt: PLAN-014-D06 -->
---
type: decision
revision: 2
rank: b3q
state: decided
in: [PLAN-014]
title: "PDFs of the body, rendered with typst"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents, the living version and released versions, must be exportable as PDFs.

<!-- field: decision -->
The server renders a `markdown` field as a PDF, body only, through typst (the `typst` crate), with bundled fonts and the headings as bookmarks. A version's PDF follows its pins.

<!-- field: why -->
Rendering on the server gives the app and the CLI the same PDF. typst is in Rust and needs no headless browser. A title page, versions and approvals can come later.

<!-- akt: PLAN-014-T01 -->
---
type: task
revision: 6
rank: b3r
state: done
in: [PLAN-014]
links: [Blocks PLAN-014-T02]
title: "Spec documents: markdown fields, versions, references, PDFs and the git remote"
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec says what PLAN-014 builds. No implementation yet.

**`docs/nomenclature.md`:** a "Documents and versions" section with these terms:
- `markdown` (field kind), `heading id`, `reference`, `deep link`, `broken reference`;
- `living version`, `version` (not an akt type version), `release` (the verb; `release` is also an example's akt type), `pin`;
- `clone`, `normal form`.

The field kinds list now includes `sections`, `verdict` and `markdown`.

**`docs/spec/akter.md`:**
- `akt.field.markdown`: CommonMark with GitHub's tables, task lists and strikethrough.
  - Heading ids `{#id}`: given from the heading's text when missing, kept when the text changes, and unique within the akt.
  - A value giving one id to two headings is refused.
- `akt.field.markdown.references`: `kuria:<number>[@<version>][#<heading id>]`. A field's `references` link kind is kept as the akter referenced that exist and fit the kind's `to`. A write is never refused for a broken reference; a release is.
- `akt.type.validation` checks `references` and `versions`. `akt.change-event` adds releases.

**`docs/spec/links.md`:** `link.referenced`.
- A references kind is kept by writes alone: `addLink` and `removeLink` refuse it.
- Its changes are recorded in the source's revision only, never as a revision of the target, which may be final.
- `link.revisions` excepts it.

**`docs/spec/versions.md` (new):**
- `version.declare`: `versions` with an optional `action`.
- `version.release`: the current revision only (`STALE` otherwise), an optional label, also of a final akt.
- `version.number`, `version.immutable`.
- `version.references`: `BROKEN_REFERENCES` for a missing or unreadable akt, version or heading.
- `version.pins`: the target's latest version, else its revision then.
- `version.history`: in history, with a change event, not a revision.
- `version.list`, `version.read` (references followed as pinned), `version.diff`.

**`docs/spec/pdf.md` (new):**
- `pdf.render`: body only, headings as bookmarks, references as links.
- `pdf.route`: `GET /akter/{number}/pdf?field=&version=`, 404 as if absent.
- `pdf.repeatable`.

**`docs/spec/git.md` (new):**
- `git.remote`: `kuria::<url>/<number>`.
- `git.tree`: section directories, with members in no section at the root.
- `git.history`: a commit per revision or batch, in change-event order; versions as `<number>/v<n>` tags.
- `git.identity`: the same commits in every clone; a pushed commit's metadata kept with its revisions.
- `git.fetch`.
- `git.push`: a commit as a batch; moving a file is a move between sections; renaming a directory changes its section's title; removing a file removes the member.
- `git.push.normal`, `git.push.fast-forward` (merge commits refused), `git.fmt`.

**`docs/spec/authorization.md`:** `authz.release` requires edit, plus the type's `versions.action`.

**`docs/spec/cli.md`:**
- `cli.read`: `--version` for `cat`, `show` and `pdf`; `versions`; `diff`; `pdf`.
- `cli.file.kinds`: a `markdown` field as a section, and a read-only `version` key.
- `cli.write`: `release` and `fmt`.
- `cli.local.versions` (new): versions in front matter, a release refused while uncommitted, versions read from git.
- `cli.local.unsupported` adds `pdf`.

**`docs/spec/frontend.md`:**
- `app.fields`; `app.editor` (new: the rich-text editor's constructs, the unchanged round trip, the reference picker).
- `app.references` (new), `app.versions` (new), `app.pdf` (new).

`docs/spec/README.md` lists the new files. `mise run lint` passes, Tracey included.

<!-- field: acceptance -->
- `mise run lint` passes, Tracey included.
- Every rule PLAN-014's design relies on has an id.

<!-- akt: PLAN-014-T02 -->
---
type: task
revision: 10
rank: b3s
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T01, Blocks PLAN-014-T03, Blocks PLAN-014-T04, Blocks PLAN-014-T05, Blocks PLAN-014-T06]
title: "A markdown field kind, with heading ids and references"
assignee: albin@mattsson.io
---

<!-- field: description -->
A `markdown` field kind: its headings keep ids, and its references to akter keep links of a kind the field names.

- **Config:** `FieldKind::Markdown`, and `references` on a field, refused unless it is a `markdown` field naming a link kind the document declares.
- **Engine:**
  - `markdown` module on `pulldown-cmark`, with tables, task lists, strikethrough and heading attributes. `headings`, `references`, `Reference::parse`, `slug`, `give_ids`, `heading_ids`, `referenced`, and `broken` with `Target`.
  - Ids are put into the text in place, so the author's formatting is kept, before an ATX heading's closing `#`s too.
  - `apply` gives ids across all of an akt's `markdown` fields after the input is applied. It counts a field as changed only if it differs from before, so text sent without the ids it already had changes nothing.
  - An id that isn't one, or one id on two headings, is refused, naming the field.
  - `rules::links::check_by_hand` makes `to_add` and `to_remove` refuse a references kind as `INVALID_LINK`. `to_remove` now returns a `Result`.
- **Server:**
  - `MarkdownValue` gives `markdown`, `headings` and `references`. Each reference comes with its `akt` if readable, and `broken` saying why.
  - `Link.byReference`; `Field.references`.
  - `akter/references.rs` keeps the links on every create and edit, copies included. What changed is recorded in the source's revision (`membership`: `linked`, `unlinked`). The target gets no revision.
- **CLI:**
  - A `markdown` field is a section of the akt file, and `--set body=@file` works.
  - `links` in files and in `cat`'s JSON leave out links references keep, so a target's file doesn't change without a revision.
  - `show` lists broken references, and its JSON has `references` and each link's `byReference`.
  - `schema` says the references kind.
- **Local mode:** reference links are never stored. `answers` works them out from the directory's text, both ways, and works out broken references.
- **App:**
  - `references.ts` renders a `markdown` field with heading ids as anchors and references as links to `/akt/<number>[?version=][#heading]`. A broken reference shows its text and "(broken reference: why)".
  - `markdown.ts` shares one sanitizer, allowing app links and heading ids only for this.
  - A textarea edits the field until T04.
  - The links card shows reference links as "From the text", with no remove button, and leaves references kinds out of Add link.
  - The akt page scrolls to `#heading` once the akt loads.
- **Spec changes found while building:**
  - `cli.file.kinds`: files leave out reference links.
  - `cli.local.references` (new).
  - `link.read`: links say whether a reference keeps them.
  - `cli.read`: `show` gives broken references.

**Tests:**
- Config: `checks_the_link_kind_a_markdown_field_keeps_references_as`.
- Engine:
  - `headings_are_given_ids_and_keep_them`: ATX, closing `#`s, setext, digits, non-ASCII, kept ids, uniqueness, code blocks, idempotence.
  - `refuses_ids_that_clash_or_arent_ids`, `reads_references`, `says_why_a_reference_is_broken`.
  - `markdown_fields_get_heading_ids_unique_across_the_akt`.
- Server: `tests/documents.rs`.
  - `headings_get_ids_that_stay`.
  - `references_keep_links_and_say_when_broken`: links read from both sides; no revision of the target; `addLink` and `removeLink` refused; another kind still linked by hand; a link taken away from a final target.
- Local: `markdown_fields_get_heading_ids_and_keep_reference_links`.
- App: `references.test.ts`, `values.test.ts`, `document.test.ts`.

Lint and `mise run test` (358 Rust, 69 app) pass. The parity, links and akt-page e2e tests pass.

Not yet in a browser test: rendering and the textarea. The editor (T04) and the example (T07) cover them.

<!-- field: acceptance -->
- Engine table tests: ids given, kept and made unique; references parsed with and without a version or heading.
- Server, local and parity tests.
- App unit tests for rendering references.

<!-- akt: PLAN-014-T03 -->
---
type: task
revision: 8
rank: b3t
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T06, Blocks PLAN-014-T07]
title: "Versions: release a revision, check and pin references"
assignee: albin@mattsson.io
---

<!-- field: description -->
Versions: a release names the akt's current revision as its next version, once none of its references is broken, and pins what each unversioned reference meant.

- **Config:** `versions` in the akt type document, with an optional `action` checked like a transition's.
- **Store:** migration `000014_versions`.
  - `akt_version` is keyed by akt and number, unique by revision, and refers to the revision it names.
  - Its label is one line of at most 100 characters.
  - A trigger refuses any update or delete.
  - Change events gain the kind `version`.
  - `kuria_store::versions`: `insert` (with its change event), `of`, `one`.
- **Engine:**
  - `markdown::Target` holds each version's headings, so a versioned reference's heading is checked in that version.
  - `versions::check` gives the pins, or every broken reference.
  - `versions::as_pinned` says how a version reads a reference.
- **Server:**
  - `releaseVersion` refuses a stale revision, a type without `versions`, a revision already released and a bad label.
  - It needs `kuria:akter:edit` plus the type's action (`Permission::Release`).
  - Broken references are refused as `BROKEN_REFERENCES`, with `extensions.references`.
  - `Akt.versions` and `Akt.version`. `akt(number:, version:)` reads the revision's fields, state and title, and its references follow the pins (`pinnedVersion`, `pinnedRevision`).
  - The changes subscription gains `VERSION`.
- **CLI:**
  - `release [--label]`, `versions`, `diff <n> <from> [<to>]` (line diff of each changed field), and `cat` and `show` with `--version`.
  - A file read at a version has `version: <n>`, and `apply` refuses it as `READ_ONLY` before any staleness check.
  - `show` lists versions; `history` says "released as version n".
  - `guide` covers `markdown` fields and versions.
- **Local mode:**
  - Versions are in the directory file's front matter, with their pins and headings, and `cat` leaves them out.
  - A release refuses uncommitted changes as `UNCOMMITTED` and makes no revision.
  - Reading at a version takes the file from the commit that first held that revision (`Times::text_at`).
- **App:**
  - The akt cache key includes the version, so an akt read at a version never overwrites the living one.
  - A Versions card: the list, links to `?version=n`, back to the living version, Release with a label (broken references listed), and Compare.
  - `DiffDialog` with `lineDiff`.
  - At a version, the page says so and offers nothing to change.
  - History says releases. References follow pins.
- **Spec changes found while building:** an akt file never lists versions, which change without a revision (`cli.file.kinds`). `cli.local.versions` keeps them in the directory file like `rank`.
- **New gap G04:** versions in bundles and in directory conversion.

**Tests:**
- Config: `checks_the_action_a_release_needs`.
- Engine: `a_release_pins_what_its_references_meant`, `a_release_is_refused_while_a_reference_is_broken`, and the version-heading case in `says_why_a_reference_is_broken`.
- Server (`tests/documents.rs`):
  - `a_release_names_the_current_revision_once_nothing_is_broken`: broken, stale, forbidden without the action, labelled, pinned at a revision, released once, listed latest first.
  - `a_version_reads_as_it_was_released`: pinned to a version, still unbroken after the target loses the heading; reading at a missing version; change events; no update or delete.
- Local: `documents_are_released_and_read_at_their_versions`.
- App: `diff.test.ts`, `keys.test.ts`.

Lint and `mise run test` (364 Rust, 70 app) pass. The parity, akt-page, links, test-protocols and accessibility e2e tests pass. No browser test of the Versions card yet: T07's walkthrough covers it.

<!-- field: acceptance -->
- Engine tests for the check and pins.
- Server tests: refused when stale, without the action or with a broken link; taken otherwise; reads follow pins.
- Local and parity tests.
- App unit tests.

<!-- akt: PLAN-014-T04 -->
---
type: task
revision: 8
rank: b3u
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T07, Resolves PLAN-014-G02]
title: Edit markdown fields with Lexical
assignee: albin@mattsson.io
---

<!-- field: description -->
A `markdown` field is edited in a rich-text editor on Lexical 0.53.0 (core, no React), which offers only what the field's Markdown holds.

- **`editor/markdown.ts`:**
  - **Nodes:** headings, quotes, lists and task lists, code, links, tables and horizontal rules.
  - **Transformers:**
    - Lexical's own, with a heading transformer that writes `{#id}`.
    - Ours for horizontal rules.
    - Ours for GitHub tables: the header row, the divider, `\|` in cells.
  - **Heading ids:** kept in Lexical node state (`headingId`), taken off the heading's text on import, so the editor keeps them through edits.
  - **Setext headings:** made ATX before import (`atx`, through `marked`'s lexer).
- **`editor/MarkdownEditor.svelte`:**
  - Registers rich text, history, lists, check lists, tables, Markdown shortcuts and horizontal rules.
  - **Toolbar:** block type, Bold, Italic, Strikethrough, Code, the three lists, Table, Rule, Link, Reference.
  - **Reference dialog:** finds an akt through `pickable`, then lists its headings and versions (`AktReferable`).
  - A link inserted where nothing is selected is its target's heading or title, and typing goes on after it.
  - **Unchanged value:** if what the editor would write is what it wrote on opening, the field keeps its original text, so opening and saving changes nothing.
  - `FieldEditor` uses it for `markdown` fields.
- **Example:** the documents example's akt types are written early (`docs/examples/documents/document_index.json`, `document.json`), since the e2e tests' grants are per akt type key.
  - `e2e/scripts/setup.ts`: the `documents:release` action, a `document-release` role, and grants for the developer and release manager.
  - `e2e/support/example.ts`: `defineDocuments`.
  - T07 adds the README and the walkthrough.

**Tests:**
- `editor/markdown.test.ts`, in a headless editor: every construct the editor offers round-trips unchanged (headings with ids, emphasis, lists, task lists, quotes, code, links and references, tables, rules); setext headings are read.
- `e2e/tests/markdown-editor.spec.ts`:
  - writes in the editor, inserts a reference to another document's heading through the picker, formats bold, and saves;
  - checks the saved Markdown (`First steps. See [Installing](kuria:…#installing)**Now.**`) and the rendered link to the heading;
  - follows the link to the heading;
  - axe passes on the picker and the editor.

Lint and `mise run test` (364 Rust, 80 app) pass.

G02 (images, footnotes, merged cells) stays open: none of them is offered.

<!-- field: acceptance -->
- Round-trip unit tests for every construct the editor offers.
- An e2e check that edits, inserts a deep link and saves, with axe passing.

<!-- akt: PLAN-014-T05 -->
---
type: task
revision: 7
rank: b3v
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T07]
title: PDFs of markdown fields
assignee: albin@mattsson.io
---

<!-- field: description -->
A `markdown` field is a PDF of its body, of the living version or of a version, rendered by the server with typst.

- **Engine (`typst.rs`):** `markup` turns the field's Markdown into typst markup through the same parser.
  - Headings become typst headings, so they are the PDF's bookmarks.
  - Emphasis, strong, strikethrough, inline code, code blocks with their language, lists, ordered lists, task lists (☐ ☒), quotes, tables (header row strong) and rules.
  - Links, and references to the address `link_of` gives.
  - Text is escaped for typst.
  - A small writer keeps its methods under the length lint.
- **Server (`pdf.rs`):**
  - `GET /akter/{number}/pdf?field=&version=`, authenticated as `/graphql` is.
  - 404 for an akt the subject may not read, a field that isn't `markdown`, or a missing version. Without `field`, it takes the type's only `markdown` field.
  - At a version, references follow its pins.
  - Rendered with typst 0.14.0 (the newest on Rust 1.88), typst-pdf and typst-assets' bundled fonts, on a blocking thread. Links point to `KURIA_PUBLIC_URL`.
  - Answers `application/pdf` as `<number>[-v<n>].pdf`.
- **Browser sessions:** the session cookie now authenticates `/akter/`, like `/graphql` and `/attachments`, so the app's link works. Without it, a download was refused.
- **CLI:** `kuria pdf <number> [--field] [--version] -o <file>`, refused in local mode as `LOCAL_UNSUPPORTED`. In `guide`.
- **App:** Download PDF in the akt page's actions, one per `markdown` field ("<Field> as PDF" if there are several), at the version being read.

**Tests:**
- Engine: `markdown_becomes_typst` on ten cases.
- Server: `a_markdown_field_is_read_as_a_pdf`.
  - Type and file name; text and bookmarks read back with `lopdf`; the reference's link to the app's heading.
  - A version's PDF is of the version, and byte-identical twice.
  - 404s.
- e2e: `markdown-editor.spec.ts` downloads the PDF and checks it starts with `%PDF`.

Lint and `mise run test` (366 Rust, 80 app) pass.

<!-- field: acceptance -->
- Engine tests of the conversion.
- Server tests: a PDF with the body's text and bookmarks, for the living version and a version.
- An e2e download.

<!-- akt: PLAN-014-T06 -->
---
type: task
revision: 10
rank: b3w
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T03, Blocked by PLAN-014-T02, Blocks PLAN-014-T07, Resolves PLAN-014-G01, Resolves PLAN-014-G03]
title: "git-remote-kuria: clone, fetch and push a container"
assignee: albin@mattsson.io
---

<!-- field: description -->
`git-remote-kuria` makes `kuria::<url>/<number>` a git remote: a container and its members are cloned, fetched, diffed, rebased and pushed with git, and Kuria serves no git protocol.

- **Spec changes found while building (`docs/spec/git.md`):**
  - A clone's file is the akt file without `revision`, `in`, `links` and `outcome`. A pushed file can't know the revision its push makes, and the others change without the akt's revision. Fast-forward-only pushes take over staleness.
  - Folders follow the container's `sections` field: a move is an edit of the sections, which `kuria fmt` then follows. A new file is refused.
- **Server:**
  - Migration `000015_akt_commits`: `akt_commit` keeps a pushed commit's `{author, committer, message}` beside each revision its batch made. Revisions are append-only; the table refuses updates and deletes.
  - `batch(input: { commit })`; `AktRevision.commit`; `akt(number:, revision:)` reads an akt at a revision.
- **CLI:**
  - `src/remote.rs`, and the binary `src/bin/git-remote-kuria.rs` (`default-run = "kuria"`). It speaks git's remote-helper protocol with `import`, `export` and a refspec onto `refs/kuria/<remote>/heads/*`.
  - **History:** from the container's and every member's revisions, sorted by time. One commit per pushed commit (its kept metadata), else per change: one author, one second, one reason.
  - **Trees:** whole each commit, the container's file at the root and members in their sections' directories (`tree_of`). Versions are tags `<number>/v<n>`.
  - **Pushes:**
    - Each commit is refused unless Kuria is unchanged since the last fetch and the commit is a fast-forward of the private ref, with no merges.
    - Its tree must equal its normal form: every file re-rendered, heading ids given, in place.
    - Then it is applied as one batch with the commit kept: edits of the fields it changed, through `apply`'s `changes`, and removals for deleted files.
  - `kuria fmt` puts a clone's files in normal form.
  - `open_remote` signs in as `kuria` does. `values::input_of` now takes `markdown` fields: before, `kuria apply` of a document's file was refused.
- **Workspace:** `linker_messages` allowed. macOS's linker notes the size of the unwind tables in binaries that include typst, which `-D warnings` made an error.
- **Gaps:** G01 and G03 closed. New G05: fetching rebuilds the whole history.

**Tests:**
- Unit (`remote.rs`): `reads_a_remote_s_url`; `places_members_where_the_sections_put_them` (nested, untitled and repeated titles); `reads_what_git_exports`. That last one found that the optional newline after data was read as a commit's end.
- e2e (`git-remote.spec.ts`), against the real stack:
  - two clones have the same tree and commits;
  - a push becomes a revision, and the other clone fetches the very same commit;
  - a push after a change in Kuria is refused until `pull --rebase`;
  - a heading without an id is refused, fixed by `kuria fmt` and an amend;
  - a version becomes a tag, and both clones end on the same commit.

Lint and `mise run test` (369 Rust, 80 app) pass.

<!-- field: acceptance -->
- Tests against a server: clone tree and commits; two clones agree on commit ids; a push becomes revisions; a non-fast-forward push is refused and works after `pull --rebase`; a push not in normal form is refused; versions are tags.

<!-- akt: PLAN-014-T07 -->
---
type: task
revision: 9
rank: b3x
state: done
in: [PLAN-014]
links: [Blocked by PLAN-014-T03, Blocked by PLAN-014-T04, Blocked by PLAN-014-T05, Blocked by PLAN-014-T06]
title: The documents example and its walkthrough
assignee: albin@mattsson.io
---

<!-- field: description -->
The documents example, its walkthrough, the parity test and the docs.

- **Example:** `docs/examples/documents`: `document_index.json` and `document.json` (written in T04 for the e2e grants), and a README covering how it fits together, the app, the command line, git and the Konklav roles.
- **Walkthrough** (`e2e/tests/documents.spec.ts`):
  - the developer writes a document in the editor with a reference to another's heading;
  - the release manager's release is refused while a reference to a missing heading is broken, then taken once the heading exists;
  - version 1 reads as released, read-only, its links pinned;
  - its PDF downloads, and it is compared with the living version;
  - the index is cloned with git, rebased onto a change made in the app and pushed, and the app shows the pushed text and the commit's message in the history;
  - axe on the refused release, the version and the comparison.
- **Parity** (`parity.spec.ts`, a third test): stamped document types with versions and references, in a git-tracked directory. Both sides answer alike for `cat`, a refused release (broken), a refused hand link, a fixed body, `release --label`, `cat --version 1` and `2`, and `diff`.
- **Shared e2e helper:** `e2e/support/git.ts`. `e2e:serve` defines the documents example too.
- **Fixes found by the walkthrough and parity:**
  - The editor's dialogs gave focus back to the toolbar button, so typing after Insert reopened the dialog. `Dialog` now takes `onCloseAutoFocus`, and the editor's dialogs focus the editor.
  - Links in rendered Markdown are underlined (axe: link-in-text-block).
  - `kuria diff` compares texts with one final newline, as local mode and the server keep them differently.
- **Docs:** `docs/requirements.md`; `docs/architecture.md` (a Documents section); the e2e, CLI and server READMEs.

Lint, `mise run test` (369 Rust, 80 app) and `mise run e2e` (33 tests) pass.

<!-- field: acceptance -->
- The walkthrough and the parity test pass in `mise run e2e`.

<!-- akt: PLAN-014-G01 -->
---
type: gap
revision: 3
rank: b3y
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-014-T06]
title: New documents made in a clone
---

<!-- field: description -->
A file added in a clone has no number, and the server names files by number, so the pushed tree wouldn't be in normal form. Should `kuria new` in a clone reserve a number on the server first, or should a push allow new files and answer with a commit that renames them?

<!-- field: resolution -->
Refused (T06, `git.push`). A new file in a clone has no number, and only Kuria gives one. Create the document with `kuria new document --in <index>` (or in the app), fetch, then edit it in the clone.

<!-- akt: PLAN-014-G02 -->
---
type: gap
revision: 4
rank: b3z
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-014-T04, Resolved by PLAN-014-T10]
title: What Markdown and the editor leave out
---

<!-- field: description -->
Images and other files in a body need attachments, which local mode refuses (`cli.local.unsupported`). Footnotes, merged table cells and comments aren't in the Markdown chosen. Which of these do documents need, and how?

<!-- field: resolution -->
Resolved (T10). Footnotes are in: the Markdown, the app, the editor and PDFs. Images need attachments a document holds, which local mode and clones can't carry yet; that is G06. Merged table cells and comments have no Markdown, so documents don't have them.

<!-- akt: PLAN-014-G03 -->
---
type: gap
revision: 3
rank: b40
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-014-T06]
title: A document in several indexes
---

<!-- field: description -->
A document may be a member of several containers, but sits in one folder per index. Each index's clone shows it in its own sections. Does a push from one clone that moves it need to say which index it moves in, and is that only ever the cloned one?

<!-- field: resolution -->
A clone is of one container: a document shows in the directories that container's sections put it in, and a push edits its fields or removes it from that container only (T06). In the example, an index contains its documents exclusively, so a document is in one index.

<!-- akt: PLAN-014-G04 -->
---
type: gap
revision: 3
rank: b41
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-014-T08]
title: Versions in bundles and directory conversion
---

<!-- field: description -->
Bundles (`bundle.contents`) and converting between a server and an akt directory (`cli.local.convert`) carry akter, revisions, links and pins, but not versions (`akt_version`, or a directory file's `versions`). An exported tenant, or a directory imported to a server, loses its releases. Each format needs versions, with their pins, and an import must keep their numbers and the revisions they name.

<!-- field: resolution -->
Resolved (T08). Bundles are format 2: each akt carries its versions with their pins, and each revision the git commit it was pushed in; format 1 still imports. Converting to a directory writes versions into the files with the headings they had; converting from one builds revisions from git's commits, so a version reads on the server as it was released.

<!-- akt: PLAN-014-G05 -->
---
type: gap
revision: 3
rank: b42
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-014-T09]
title: Fetching rebuilds the whole history
---

<!-- field: description -->
`git-remote-kuria` rebuilds a container's whole history on every fetch and every push check. It reads each akt's revisions, and each akt at each revision, one GraphQL query apiece. That is fine for an index of tens of documents with hundreds of revisions, and slow beyond. Building only the commits after the last fetch needs the clone to keep where it got to (the change events seen, and the files at the tip), and a way to ask Kuria for the revisions of many akter at once.

<!-- field: resolution -->
Resolved for now (T09). A clone keeps every file it read, by number and revision, so a fetch reads only new revisions, and the helper asks for many akter to a request. Each fetch still lists every akt's revisions, in 1 + members/25 requests; building only the commits after the last fetch waits until an index is big enough to need it.

<!-- akt: PLAN-014-T08 -->
---
type: task
revision: 6
rank: b43
state: done
in: [PLAN-014]
links: [Resolves PLAN-014-G04]
title: Versions and commits in bundles and directory conversion
assignee: albin@mattsson.io
---

<!-- field: description -->
Bundles and conversion between a server and an akt directory keep versions, and the git commits pushes came from (PLAN-014-G04).

- **Bundles (format 2):** each bundle akt holds its versions (number, revision, label, who released it and when, pins), and each revision the commit it was pushed in (`git.identity`), so a clone of an imported tenant builds the same commits. Kuria reads formats 1 and 2: format 1 is format 2 without either. An import refuses, as `INVALID_BUNDLE`, versions not numbered from 1 in order, naming a revision the akt lacks or another version names, or with pins that don't read. It writes them as recorded, without change events of their own.
- **To a directory:** each akt's versions go into its file's `versions`, with the headings each had at its revision, replayed from the bundle's revisions, so references to a version still check. A version's text stays with the server: git has none of the revisions before the directory's.
- **From a directory:** each revision now records the akt as git's latest commit at that revision holds it, else as at the next revision git has, the first whole and each later one what changed. Before, every revision after the first changed nothing, so a version would have read on the server as the akt is now. A version naming a revision git lacks refuses the import as `NOT_FOUND`.
- **Code:** `kuria_store::versions::{of_akter, commits_of_akter, insert_imported}`; `bundles/format.rs` (`BundleAktVersion`, `BundleRevision.commit`, `READS`), `export.rs` (`history_of`), `import.rs` (`check_versions`, `insert_versions`); `local/convert.rs` (`versions_in`, `snapshots`, `history_of`, `bundled_versions`); `Times::revision_at`.
- **Spec:** `bundle.format`, `bundle.contents`, `bundle.import.history`, `cli.local.convert`. Docs: the CLI README and guide, `docs/architecture.md`.
- **Tests:** `versions_and_commits_move_with_their_akter` and `versions_that_dont_fit_their_akt_are_refused` (server bundles), `versions_go_to_a_bundle_and_back` (local).

Lint, 372 Rust and 80 app tests pass.

<!-- akt: PLAN-014-T09 -->
---
type: task
revision: 7
rank: b44
state: done
in: [PLAN-014]
links: [Resolves PLAN-014-G05]
title: Fetch with a cache of files and batched queries
assignee: albin@mattsson.io
---

<!-- field: description -->
`git-remote-kuria` fetches faster without changing the server (PLAN-014-G05, a first step).

- **A cache in the clone (`git.fetch.cache`):** an akt's file at a revision never changes, so the helper keeps each one it read (its file, title and sections) in `.git/kuria/<remote>.files.json`, by `<number>@<revision>`, and a later fetch or push reads only the ones it lacks. The cache is for the client version and the Kuria URL that wrote it; any other starts afresh, since another client might write files otherwise.
- **Batched queries (`git.fetch.batched`):** revisions are asked for 25 akter to a request, and akter at revisions 10 to a request, by GraphQL aliases. Measured against the server's complexity limit of 2,000: 10 akter at revisions fit and 25 don't, while 25 revision lists fit. A request Kuria refuses as too complex is split in half until it is taken.
- **The same history:** the builder first works out each commit's revisions and members, reads the files it lacks, then builds the commits as before.
- **What stays:** each fetch still asks for every akt's revisions, now in 1 + members/25 requests rather than one per akt. Building only the commits after the last fetch would need a server query for change events of many akter, and isn't needed at this size.
- **Code:** `items/cli/src/remote.rs` (`Filed`, `Kept`, `Remote::akter`, `events_of`, `filed`, `moments`).
- **Spec:** `git.fetch` reworded to what it does; `git.fetch.cache`, `git.fetch.batched` new. Docs: the CLI README, `docs/architecture.md`.
- **Tests:** `keeps_files_for_the_client_and_kuria_that_read_them`, `knows_a_request_kuria_finds_too_big` (unit), and `git-remote.spec.ts` now clones afresh at the end and checks the commit matches that of a clone that kept its files over several fetches.

Lint, 374 Rust, 80 app and 33 browser tests pass.

<!-- akt: PLAN-014-T10 -->
---
type: task
revision: 6
rank: b45
state: done
in: [PLAN-014]
links: [Resolves PLAN-014-G02]
title: Footnotes in markdown fields
assignee: albin@mattsson.io
---

<!-- field: description -->
Footnotes in `markdown` fields, everywhere a field is read or written (PLAN-014-G02).

- **Markdown:** GitHub's footnotes, `text[^note]` and a definition `[^note]: …` anywhere in the field (`pulldown-cmark`'s `ENABLE_FOOTNOTES`). Numbered in the order first referred to; a definition nothing refers to isn't shown, and a reference to none is its text. A reference inside a footnote is a reference like any other.
- **App (`app.references`):** each reference to a footnote a superscript number linking to `#fn:<n>`, the first with the id `fnref:<n>`, and the notes listed after the text in a "Footnotes" region, each linking back. A marked extension in `references.ts`; the sanitizer lets through those ids and in-page links, and only those.
- **Editor (`app.editor`):** `FootnoteReferenceNode` (a token text node, `[^label]`) and `FootnoteDefinitionNode` (a paragraph saying its label), with their transformers. The Footnote button puts the next unused number at the cursor and its definition at the end, where typing goes on, starting plain: the browser test caught bold carrying over into the note. One-paragraph footnotes come back unchanged.
- **PDF (`pdf.render`):** the typst writer reads the footnotes first, then writes each as `#footnote[…]` where first referred to and repeats its number with `#footnote(<label>)` after.
- **Left out:** images need attachments in documents, local mode and clones (G06). Merged table cells and comments have no Markdown.
- **Spec:** `akt.field.markdown`, `pdf.render`, `app.editor`, `app.references`; `footnote` in the nomenclature. Docs: `docs/architecture.md`, the documents example's README.
- **Tests:** engine (typst footnotes, references in footnotes), the server's PDF test, `references.test.ts`, the editor's round trip, and `markdown-editor.spec.ts`, which inserts a footnote with the button and checks it rendered and with axe.

Lint, 375 Rust, 83 app and 33 browser tests pass.

<!-- akt: PLAN-014-G06 -->
---
type: gap
revision: 3
rank: b46
state: closed
in: [PLAN-014]
links: [Resolved by PLAN-015-D02]
title: Images in documents
---

<!-- field: description -->
Images in a document's body need it to hold attachments by reference, `![alt](attachment:<id>)`: kept as held by the akt (so bundles and erasure see them), uploaded from the editor, shown in the app through the download route, and given to typst for the PDF. Local mode and clones refuse attachments (`cli.local.unsupported`), so a directory and a clone would need the image files beside the akt files, and a push would need to upload them. Is that worth it, or do documents link to images kept elsewhere?

<!-- field: resolution -->
Resolved by PLAN-015: images are akter of their own, embedded in a body as references (`![alt](kuria:IMG-1)`), shown in the app, the editor and PDFs, and pinned by versions. Images in local mode and clones stay open as PLAN-015-G01.
