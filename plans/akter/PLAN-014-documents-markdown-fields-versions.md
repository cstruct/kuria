---
number: PLAN-014
type: plan
revision: 18
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
revision: 2
rank: b3r
state: todo
in: [PLAN-014]
links: [Blocks PLAN-014-T02]
title: "Spec documents: markdown fields, versions, references, PDFs and the git remote"
---

<!-- field: description -->
Write the spec PLAN-014 builds, and the terms it needs: `docs/spec/akter.md` for the `markdown` kind, heading ids and references; a new `docs/spec/versions.md`; `links.md` for links recorded from references; a `pdf` section; `cli.md` for `release`, `versions`, `pdf`, `--version` and `fmt`; a new `docs/spec/git.md` for the remote; and `frontend.md` for the editor, versions and PDFs. Add `markdown field`, `heading id`, `reference`, `living version`, `version` and `release` to `docs/nomenclature.md`. No implementation.

<!-- field: acceptance -->
- `mise run lint` passes, Tracey included.
- Every rule PLAN-014's design relies on has an id.

<!-- akt: PLAN-014-T02 -->
---
type: task
revision: 6
rank: b3s
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T01, Blocks PLAN-014-T03, Blocks PLAN-014-T04, Blocks PLAN-014-T05, Blocks PLAN-014-T06]
title: "A markdown field kind, with heading ids and references"
---

<!-- field: description -->
`kuria-config`: the `markdown` kind and its `references` link kind. `kuria-engine`: a `markdown` module on `pulldown-cmark` that gives missing heading ids, keeps existing ones, and lists headings and references. The server and local mode write through it and replace the akt's reference links. `addLink` and `removeLink` refuse that kind. The app renders heading anchors and references, broken ones too. The CLI reads and writes the field as a section of the akt file.

<!-- field: acceptance -->
- Engine table tests: ids given, kept and made unique; references parsed with and without a version or heading.
- Server, local and parity tests.
- App unit tests for rendering references.

<!-- akt: PLAN-014-T03 -->
---
type: task
revision: 4
rank: b3t
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T06, Blocks PLAN-014-T07]
title: "Versions: release a revision, check and pin references"
---

<!-- field: description -->
`versions` in akt type documents. `releaseVersion`, `versions`, `akt(version:)` and a version's change event. The release check and pinning of PLAN-014-D05. The CLI's `release`, `versions` and `--version` reads, in local mode too. The app's Release dialog, version picker and diff.

<!-- field: acceptance -->
- Engine tests for the check and pins.
- Server tests: refused when stale, without the action or with a broken link; taken otherwise; reads follow pins.
- Local and parity tests.
- App unit tests.

<!-- akt: PLAN-014-T04 -->
---
type: task
revision: 4
rank: b3u
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T07, Resolves PLAN-014-G02]
title: Edit markdown fields with Lexical
---

<!-- field: description -->
A Svelte component around Lexical's core with the nodes Markdown holds, `@lexical/markdown` transformers with custom ones for heading ids and references, and an Insert link picker (akt, heading, living or released version). It replaces the textarea for `markdown` fields.

<!-- field: acceptance -->
- Round-trip unit tests for every construct the editor offers.
- An e2e check that edits, inserts a deep link and saves, with axe passing.

<!-- akt: PLAN-014-T05 -->
---
type: task
revision: 3
rank: b3v
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T02, Blocks PLAN-014-T07]
title: PDFs of markdown fields
---

<!-- field: description -->
Markdown to typst markup through the engine's parser. Compile with the `typst` crate and bundled fonts. `GET /akter/<number>/pdf` for the living version and versions, `kuria pdf`, and the app's Download PDF action.

<!-- field: acceptance -->
- Engine tests of the conversion.
- Server tests: a PDF with the body's text and bookmarks, for the living version and a version.
- An e2e download.

<!-- akt: PLAN-014-T06 -->
---
type: task
revision: 6
rank: b3w
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T03, Blocked by PLAN-014-T02, Blocks PLAN-014-T07, Resolves PLAN-014-G01, Resolves PLAN-014-G03]
title: "git-remote-kuria: clone, fetch and push a container"
---

<!-- field: description -->
The helper binary: the tree from sections, commits from revisions in change-event order, tags for versions, pushed commits' metadata kept with their revisions, pushes as batches, the normal-form and fast-forward checks, and `kuria fmt`.

<!-- field: acceptance -->
- Tests against a server: clone tree and commits; two clones agree on commit ids; a push becomes revisions; a non-fast-forward push is refused and works after `pull --rebase`; a push not in normal form is refused; versions are tags.

<!-- akt: PLAN-014-T07 -->
---
type: task
revision: 5
rank: b3x
state: todo
in: [PLAN-014]
links: [Blocked by PLAN-014-T03, Blocked by PLAN-014-T04, Blocked by PLAN-014-T05, Blocked by PLAN-014-T06]
title: The documents example and its walkthrough
---

<!-- field: description -->
`docs/examples/documents` with `document_index.json`, `document.json` and a README. An e2e walkthrough: edit with Lexical, a deep link, release refused and then taken, the PDF downloaded, git clone, rebase onto an app change, push, shown in the app. Extend the parity test.

<!-- field: acceptance -->
- The walkthrough and the parity test pass in `mise run e2e`.

<!-- akt: PLAN-014-G01 -->
---
type: gap
revision: 2
rank: b3y
state: open
in: [PLAN-014]
links: [Resolved by PLAN-014-T06]
title: New documents made in a clone
---

<!-- field: description -->
A file added in a clone has no number, and the server names files by number, so the pushed tree wouldn't be in normal form. Should `kuria new` in a clone reserve a number on the server first, or should a push allow new files and answer with a commit that renames them?

<!-- akt: PLAN-014-G02 -->
---
type: gap
revision: 2
rank: b3z
state: open
in: [PLAN-014]
links: [Resolved by PLAN-014-T04]
title: What Markdown and the editor leave out
---

<!-- field: description -->
Images and other files in a body need attachments, which local mode refuses (`cli.local.unsupported`). Footnotes, merged table cells and comments aren't in the Markdown chosen. Which of these do documents need, and how?

<!-- akt: PLAN-014-G03 -->
---
type: gap
revision: 2
rank: b40
state: open
in: [PLAN-014]
links: [Resolved by PLAN-014-T06]
title: A document in several indexes
---

<!-- field: description -->
A document may be a member of several containers, but sits in one folder per index. Each index's clone shows it in its own sections. Does a push from one clone that moves it need to say which index it moves in, and is that only ever the cloned one?
