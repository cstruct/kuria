---
number: PLAN-015
type: plan
revision: 16
rank: b47
state: active
title: "Images and diagrams in documents, and types under Show more"
owner: albin@mattsson.io
---

<!-- field: goal -->
Documents with pictures: images that are akter of their own, embedded in a body and shown in the app, the editor and PDFs, and diagrams and plots written as typst in the body itself, rendered when saved. Types nobody browses on their own, such as images, stay out of the way in the sidebar.

- **Hidden types:** an akt type may say it belongs under "Show more" in the sidebar.
- **Images:** an image is an akt whose type shows one of its attachment fields as its picture. A body embeds it with `![Alt text](kuria:IMG-1)`, which is a reference like any other: kept as a link, checked before a release, and pinned by a version.
- **Diagrams:** a `diagram` code block in a `markdown` field that allows them holds a fletcher diagram or a cetz plot, with the packages already imported. Saving renders each block to SVG, refusing one that doesn't compile. The app shows the picture; the editor shows the picture or its code, toggled per block; the PDF draws it from its source.

Nothing in Kuria knows images or documents. These are generic: a `navigation` presentation member, an attachment field's media types, a type's `image` presentation member, embedded references, and diagram blocks in `markdown` fields. The documents example uses them all, adding an `image` type.

Success: the example's walkthrough runs in `mise run e2e`. An image is uploaded from the editor and embedded; a fletcher diagram is written, toggled between code and picture, and saved; the document is released and its PDF holds both; the image type sits under "Show more".

<!-- field: design -->
## Where it starts

- **PLAN-014** made `markdown` fields, references (`kuria:<number>[@<version>][#<heading>]`) kept as links of the kind the field names, versions with pins, the Lexical editor, PDFs through typst 0.14, and the git remote.
- **Attachments** (`attachment.*`) are immutable files, content-addressed, held by an akt's `attachment` field (one per field, `akt_attachment`), downloaded at `/attachments/{id}` by anyone who may read an akt that holds them.
- **The sidebar** lists every akt type with its icon, colour and count (`app.akt-types`, `app.type-identity`).
- **The app's sanitizer** forbids `img` and `svg`; the CSP allows `img-src 'self' data:`.
- **typst** runs in the server for PDFs, in a `World` that reads no file and no package.

## Hidden akt types (PLAN-015-D01)

- `presentation.navigation`: `main` (the default) or `more`. Anything else is refused.
- The sidebar lists `more` types after the others, under a "Show more" disclosure with how many there are, closed by default. Whether it is open is remembered per browser. A `more` type's page, search results and pickers are as before; only the sidebar hides it.
- Home lists every type as now.

## Images as akter (PLAN-015-D02)

- **Media types:** an `attachment` field may list the media types it takes, `"media_types": ["image/png", "image/jpeg", "image/svg+xml", "image/webp"]`. An upload stored in it with another type is refused.
- **An image type:** `presentation.image` names an `attachment` field whose media types are all images. An akt of such a type is an image, and its picture is that field's value. Its cards and its page show the picture.
- **Embedding:** `![Alt text](kuria:IMG-1)` and `![Alt text](kuria:IMG-1@2)`; a heading means nothing to an image, so `#…` is refused as broken. It is a reference: kept as the field's references link, listed with the others, and pinned by a version like any reference without one. Broken if the akt doesn't exist, the reader can't read it, it isn't an image, or it has no picture at the revision it is read at.
- **What a reader is given:** `MarkdownValue.images`, each embedded image's destination with the attachment it shows at the revision read (its pin, at a version) and its media type, or why it is broken.
- **In the app:** an embedded image is an `<img>` of `/attachments/{id}`, with its alt text, in a figure captioned by the alt text. The sanitizer allows `img` with a `src` of exactly `/attachments/<uuid>` and nothing else. An SVG is only ever an `<img>`, never inlined, so its scripts never run.
- **In the editor:** an image node, shown as the picture. The Image button picks an image akt, or uploads a file, creating an image akt of a type the person chooses among those that are images, at the top level, titled by the file's name.
- **In the PDF:** the server reads each embedded picture's bytes before rendering, and the `World` answers them as files; typst draws PNG, JPEG, SVG and WebP.

## Diagrams (PLAN-015-D03, D04)

- **Opt-in:** a `markdown` field says `"diagrams": true` to allow diagram blocks; elsewhere a `diagram` block is an ordinary code block.
- **A block:** a fenced code block whose language is `diagram`, holding typst markup with these already imported, and nothing else importable:
  - `fletcher` 0.5.8, as `fletcher`, with `diagram`, `node` and `edge`;
  - `cetz` 0.5.2, as `cetz`, with `canvas` and `draw`;
  - `cetz-plot` 0.1.4, with `plot` and `chart`.
- **The packages** (PLAN-015-D05) are vendored unmodified under `items/typst/packages/preview/<name>/<version>/` with their licenses, fletcher's own cetz 0.3.4 included, and compiled into the binary. The `World` resolves `@preview/<name>:<version>` to them alone, and refuses any other package and every file.
- **A crate of its own:** `kuria-typst` holds the `World`, the bundled fonts and packages, the PDF and the diagram rendering, used by the server. It moves `pdf::render` out of the server.
- **Rendering on write:** writing a field that allows diagrams renders each block whose source is new to SVG, with the page sized to the diagram. A block that doesn't compile refuses the write as `INVALID_DIAGRAM`, naming the field, the block and typst's message.
- **Kept as attachments (PLAN-015-D04):** each SVG is an attachment, content-addressed like any other, recorded in `akt_diagram (akt, field, source hash, attachment)`. Rows are never removed, so a version shows the diagrams it had. `attachment.download` also answers an attachment an akt holds as a diagram. Bundles and erasure treat them as attachments.
- **What a reader is given:** `MarkdownValue.diagrams`, each block's source hash and its SVG's attachment.
- **In the app:** each block an `<img>` of its SVG, its source as the alt text.
- **In the editor:** a diagram block shows the picture, rendered through `renderDiagram(source)`, a query that compiles without storing, with a toggle per block between the picture and the code. The code is a code block like any other, and typing in it re-renders after a pause, showing typst's error in place of the picture until it compiles.
- **In the PDF:** the block's source, drawn in place with the same imports, so it stays vector.

## The example

- `image`: a title, a `file` (`attachment`, the four image types), a caption; `presentation`: `"image": "file"`, `"navigation": "more"`, icon and colour. Numbered `IMG-{seq}`, at the top level, so a clone of an index never holds one.
- `document`: its `body` gets `"diagrams": true`, and its references kind's `to` includes `image`.

## Out of scope

- Images and diagrams in local mode and clones beyond their text (PLAN-015-G01).
- Limits on how long a diagram may take to compile (PLAN-015-G02).

<!-- field: verification -->
- `mise run lint`: clippy, tracey, the plans check, schema drift.
- `mise run test`: config validation of `navigation`, `media_types` and `image`; the engine's embedded references; the server's images, diagrams, `INVALID_DIAGRAM`, `renderDiagram`, version reads, downloads and PDFs; the app's rendering, sanitizer and editor round trip.
- `mise run e2e`: the sidebar's "Show more", the editor's Image button and diagram toggle, and the documents walkthrough with a release and its PDF, checked with axe.

<!-- akt: PLAN-015-D01 -->
---
type: decision
revision: 2
rank: b48
state: decided
in: [PLAN-015]
title: Hidden akt types are said per type
decided_by: albin@mattsson.io
---

<!-- field: context -->
Some akt types are rarely browsed on their own: images, embedded in documents, would crowd the sidebar.

<!-- field: decision -->
An akt type says `"navigation": "more"` in its `presentation`, and the sidebar lists it under a "Show more" disclosure. It is decided per type, for everyone.

<!-- field: why -->
Per type is one member of a document that already says how a type is shown, and needs no per-person settings. Per-person pinning and hiding can come later on top of it.

<!-- akt: PLAN-015-D02 -->
---
type: decision
revision: 3
rank: b49
state: decided
in: [PLAN-015]
links: [Resolves PLAN-014-G06]
title: "Images are akter, embedded as references"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents need images (PLAN-014-G06). An image could be a file held inside the document's body, or an akt of its own that bodies embed.

<!-- field: decision -->
An image is an akt: its type names, as `presentation.image`, an attachment field taking only image media types. A body embeds it as `![Alt text](kuria:IMG-1)`, a reference like any other.

<!-- field: why -->
References already do what embedded images need: links kept from the text, checks before a release, pins at a version, "Referred to by" on the image. An image is reused and replaced in one place, with its own history. And a body stays text, so clones and local mode keep working without carrying files. A body holding its own files would need several attachments per field and files in every clone.

<!-- akt: PLAN-015-D03 -->
---
type: decision
revision: 2
rank: b4A
state: decided
in: [PLAN-015]
title: "Diagrams are typst blocks in the body, rendered on write"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Diagrams and plots in a document should be written as text, rendered, and edited either as the picture or as code.

<!-- field: decision -->
A `diagram` code block in a `markdown` field that says `"diagrams": true` holds typst markup with fletcher, cetz and cetz-plot already imported: just the diagram, no imports or page setup. The server renders it to SVG when the field is written, and refuses a block that doesn't compile.

<!-- field: why -->
Just the diagram is short to write and lets Kuria decide what can be imported, so a block can't reach a package or file it shouldn't. Rendering on write means reading never compiles, and a broken diagram is found when it is written, not when someone reads it. Opting in per field keeps the cost to the fields that want it.

<!-- akt: PLAN-015-D04 -->
---
type: decision
revision: 2
rank: b4B
state: decided
in: [PLAN-015]
title: Rendered diagrams are attachments the akt holds
decided_by: albin@mattsson.io
---

<!-- field: context -->
A rendered diagram has to be kept somewhere readers can fetch it, with the same access as the akt, for as long as any version shows it.

<!-- field: decision -->
Each SVG is stored as an attachment, and `akt_diagram` records which akt holds it, for which field and source. Rows are never removed. `attachment.download` answers an attachment an akt holds as a diagram as it answers one held by a field.

<!-- field: why -->
Attachments are already content-addressed, immutable, access-checked, carried by bundles and handled by erasure. A table of SVGs of its own would repeat all of that. Keeping every row means a version reads its diagrams without rendering them again.

<!-- akt: PLAN-015-D05 -->
---
type: decision
revision: 3
rank: b4C
state: decided
in: [PLAN-015]
title: "Typst packages are vendored, unmodified, with their licenses"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Diagrams need fletcher and cetz-plot. typst fetches packages from the network, which the server must not do. fletcher 0.5.8 imports cetz 0.3.4; cetz-plot 0.1.4 imports cetz 0.5.2; cetz 0.3.4 and 0.5.2 import oxifmt 0.2.1 and 1.0.0. cetz and cetz-plot are LGPL-3.0-or-later, fletcher MIT, oxifmt MIT-0 and MIT or Apache-2.0; Kuria is MIT.

<!-- field: decision -->
The six packages are vendored unmodified, with their license files, under `items/typst/packages/preview/<name>/<version>/`, compiled into the binary, and resolved by the `World` alone. Updating one is a change to the repository.

<!-- field: why -->
Vendoring keeps rendering offline and every server rendering the same diagram the same way, which the content-addressed SVGs and PDFs rely on. Keeping the LGPL packages unmodified, as their own files with their licenses, in a directory of their own, keeps them separable from Kuria's code, as the LGPL asks. That reading should be confirmed before a release that ships them.

<!-- akt: PLAN-015-T01 -->
---
type: task
revision: 8
rank: b4D
state: done
in: [PLAN-015]
links: [Blocks PLAN-015-T02, Blocks PLAN-015-T03, Blocks PLAN-015-T05]
title: "Spec images, diagrams and types under Show more"
assignee: albin@mattsson.io
---

<!-- field: description -->
The spec says what PLAN-015 builds. No implementation yet.

- `docs/nomenclature.md`: `image` (akt), `embedded image`, `diagram block`, `bundled package`.
- `docs/spec/akter.md`: `akt.type.presentation` gains `navigation` and `image`, validated; `akt.field.attachment` gains `media_types`; `akt.field.markdown.references` says an image with a reference embeds it; new `akt.field.markdown.images`, `akt.field.markdown.diagrams` (with `INVALID_DIAGRAM`) and `diagram.preview`; `akt.type.validation` lists `media_types`.
- `docs/spec/attachments.md`: new `attachment.media-types` (refused as `REFUSED`, as other values that don't fit are); `attachment.download` covers diagrams.
- `docs/spec/versions.md`: `version.references` covers embedded images; `version.read` shows images and diagrams as they were.
- `docs/spec/pdf.md`: `pdf.render` draws images and diagrams; new `pdf.packages`.
- `docs/spec/frontend.md`: new `app.navigation`, `app.images`, `app.diagrams`; `app.editor` gains images and diagram blocks.
- `docs/spec/bundles.md`: `bundle.contents` carries diagrams and which akt holds them.
- `docs/requirements.md`, `docs/architecture.md`: images and diagrams.

`mise run lint` passes.

<!-- field: acceptance -->
- `mise run lint` passes, tracey listing the new rules as uncovered.

<!-- akt: PLAN-015-T02 -->
---
type: task
revision: 7
rank: b4E
state: done
in: [PLAN-015]
links: [Blocked by PLAN-015-T01, Blocks PLAN-015-T07]
title: Akt types under Show more in the sidebar
assignee: albin@mattsson.io
---

<!-- field: description -->
`presentation.navigation` (`main` or `more`) in `kuria-config` (`Navigation`), refused otherwise by the document's reading, answered by GraphQL as `Presentation.navigation` (`AktTypeNavigation`). The sidebar lists `more` types after the others under a "Show more" button (`aria-expanded`, with how many), closed by default; whether it is open is kept in `localStorage`, and a browser that refuses storage starts closed (`shell.ts`: `splitTypes`, `moreWasOpen`, `rememberMore`).

The documents example gains `image.json` (title, file, caption, under "Show more"); T03 gives it media types and its picture. The e2e developer may work on `image`.

Tests: config (`checks_the_presentation`), `shell.test.ts`, and `shell.spec.ts`'s Show more test (open, axe, follow, remembered across reloads, Home unchanged). Lint, 375 Rust and 85 app tests pass.

<!-- field: acceptance -->
- Config tests refuse another value.
- An app test and an e2e test: a `more` type is under "Show more", the disclosure opens and closes and is remembered, and its page opens from there. Checked with axe.

<!-- akt: PLAN-015-T03 -->
---
type: task
revision: 7
rank: b4F
state: done
in: [PLAN-015]
links: [Blocked by PLAN-015-T01, Blocks PLAN-015-T04]
title: "Image akter, embedded and read"
assignee: albin@mattsson.io
---

<!-- field: description -->
Image akter, embedded and read.

- **Config:** `media_types` on `attachment` fields (each a media type, once; only on attachment fields), and `presentation.image` naming an attachment field whose media types are all of `IMAGE_MEDIA_TYPES` (PNG, JPEG, SVG, WebP).
- **Writes:** an attachment of a media type its field doesn't take is refused as `REFUSED`, naming both (`attachment.media-types`).
- **Held for good:** `akt_held` (migration `000016`) records every attachment an akt has held, in a field at any revision; `set_held` adds to it and never removes, and downloads are decided by it. So a version or revision read later still shows the picture it had, even after the image's file is replaced. A bundle import records the attachments each akt held in any revision. This changes `attachment.download`: an attachment cleared from a field stays readable by the akt's readers, as its history shows it; the attachments test now says so.
- **Engine:** `markdown::images` reads `![alt](kuria:…)`; `references` includes them, so they are kept as the field's references links and pinned. `Target` gains `image` and `picture` (and `TargetVersion.picture`); `broken_image` says why one is broken; `versions::check` uses it for embedded images.
- **Server:** `versions::target` fills `image`/`picture`; `picture_in`, `picture_at`; `MarkdownValue.images` (`MarkdownImage`: destination, number, version, attachment at the revision read or its pin, broken).
- **App:** `renderReferenced` takes the images: an image is an `<img>` of its attachment's download, one alone in its paragraph a figure captioned by its alt text, a broken one or an image of anything else its alt text and why. The sanitizer lets `img` through only in markdown fields and only with a `src` of `/attachments/<uuid>`. `pictureOf`: an image attachment is shown as its picture on an akt's page (`FieldShown`), on cards (`Members`) and in lists (the type's page).
- **Example:** `image.json` gets its media types, `image: "file"`, and lists `file` and `caption`; a document's `refers_to` may name images.
- **Spec:** `app.images` and `attachment.download` say what was built.

Tests: config (`checks_the_presentation`), engine (`reads_embedded_images_as_references`, `embedded_images_are_checked_and_pinned_as_references`), server `tests/images.rs` (media types, broken cases, links, release refused, a version's picture after the file is replaced, and its download), app (`references.test.ts`, `members.test.ts`). Lint, 380 Rust and 88 app tests, and the documents, editor, parity, git and shell browser tests pass.

<!-- field: acceptance -->
- Server tests: embedding, broken cases, pins at a version, downloads, the refused media type.
- App tests: rendering, and the sanitizer refusing any other `img` and inline SVG.

<!-- akt: PLAN-015-T04 -->
---
type: task
revision: 7
rank: b4G
state: done
in: [PLAN-015]
links: [Blocked by PLAN-015-T03, Blocks PLAN-015-T07]
title: Images in the editor and PDFs
assignee: albin@mattsson.io
---

<!-- field: description -->
Images in the editor and PDFs.

- **Editor:** `ImageNode`, an inline decorator showing the picture (or its alt text and destination while the editor doesn't know it), with its transformer `![alt](kuria:…)` (alt text with `[`, `]` and `\` escaped). The Markdown draft carries each embedded image's picture from `MarkdownValue.images`, so an opened field shows them. The Image button opens a dialog: find an image akt (`app.pickers`, limited to image types, `ImageTypes`, `ImagePicture`), or upload a file, which creates an image akt of a chosen image type at the top level, titled by the file's name; alt text defaults to the title or the file's name.
- **GraphQL:** `Presentation.image`.
- **PDF:** the typst writer draws an embedded image as `#box(image(…))`, one alone in its paragraph as a figure captioned by its alt text, and one without a picture as its alt text in italics. The PDF route reads each embedded picture as the PDF's version reads it (pins included), only where the reader may read the image, from storage; the `World` answers those files and nothing else.

Tests: the editor's round trip with images and a picture test; engine typst cases; `a_pdf_draws_its_embedded_pictures` (a PNG and an SVG); `markdown-editor.spec.ts`'s new test uploads a PNG from the Image dialog, embeds it, saves, sees the figure and the image akt's picture, with axe. Lint, 381 Rust, 90 app tests and the editor browser tests pass.

<!-- field: acceptance -->
- Editor round trip with images.
- Server test: a PDF holding a PNG and an SVG.

<!-- akt: PLAN-015-T05 -->
---
type: task
revision: 7
rank: b4H
state: done
in: [PLAN-015]
links: [Blocked by PLAN-015-T01, Blocks PLAN-015-T06]
title: "kuria-typst, bundled packages and diagrams rendered on write"
assignee: albin@mattsson.io
---

<!-- field: description -->
`kuria-typst`, bundled packages and diagrams rendered on write.

- **The crate** (`items/typst`): a `World` with typst's fonts, the bundled packages and the files it is given, reading no network or file system; `pdf(markup, files)` and `diagram(source)` (SVG sized to the diagram, `typst-svg`), and `IMPORTS`. `build.rs` embeds every package file. The server's PDFs now go through it, and the server no longer depends on typst directly.
- **The packages:** fletcher 0.5.8, cetz 0.3.4 and 0.5.2, cetz-plot 0.1.4, and oxifmt 0.2.1 and 1.0.0, which cetz imports and the plan hadn't foreseen (D05 updated), vendored unmodified with their licenses, `packages/README.md` saying where each came from and why. Checked first: fletcher on cetz 0.3.4 compiles on typst 0.14, and cetz 0.5.2's WASM plugin runs.
- **Config:** `diagrams` on `markdown` fields only.
- **Engine:** `markdown::diagrams` reads the `diagram` blocks.
- **On write** (`akter/diagrams.rs`, from `create_in` and `change_in`): each changed field allowing diagrams has each block whose source hash (with the imports) the akt doesn't hold rendered off the runtime, stored as an SVG attachment and recorded in `akt_diagram` (migration `000017`, immutable) and `akt_held`. A block that doesn't compile refuses the write as `INVALID_DIAGRAM`, with `field` and `block`. `Caller` and `Akter` carry the storage.
- **GraphQL:** `MarkdownValue.diagrams` (source and attachment, in fields that allow them); `renderDiagram(source)` (SVG or error, at most 64 KiB of source, storing nothing); `Field.diagrams` and `Field.mediaTypes`.
- **Bundles:** each akt's diagrams, their attachments included, exported and imported.

Tests: `kuria-typst` (every package imports; a fletcher diagram and a plot render, the same each time; another package, a file, a URL and broken source are refused; a PDF reads only what it is given), the engine's `reads_diagram_blocks`, config, and the server's `tests/diagrams.rs` (rendering and download, code in a field without diagrams, `INVALID_DIAGRAM`, a version's diagrams after an edit, the preview, a bundle round trip). Lint, 391 Rust and 90 app tests pass.

<!-- field: acceptance -->
- Unit tests: each package imports, a fletcher diagram and a plot render, another package and any file are refused.
- Server tests: rendering on write, refusal with typst's message, a version's diagrams, downloads, a bundle round trip.

<!-- akt: PLAN-015-T06 -->
---
type: task
revision: 3
rank: b4I
state: todo
in: [PLAN-015]
links: [Blocked by PLAN-015-T05, Blocks PLAN-015-T07]
title: "Diagrams in the app, the editor and PDFs"
---

<!-- field: description -->
Diagrams in the app, the editor and PDFs.

- App: each diagram block shown as its SVG.
- Editor: a diagram block shows its picture through `renderDiagram`, with a toggle per block to its code; typing re-renders after a pause, showing typst's error until it compiles. The round trip keeps the block as written.
- PDF: each block drawn from its source, with the same imports.

<!-- field: acceptance -->
- Editor round trip; an app test of the toggle.
- Server test: a PDF with a diagram.

<!-- akt: PLAN-015-T07 -->
---
type: task
revision: 4
rank: b4J
state: todo
in: [PLAN-015]
links: [Blocked by PLAN-015-T02, Blocked by PLAN-015-T04, Blocked by PLAN-015-T06]
title: The example and its walkthrough
---

<!-- field: description -->
The example and its walkthrough.

- `docs/examples/documents`: the `image` type; `document.body` with `"diagrams": true` and references to images; its README.
- `e2e`: an image uploaded from the editor and embedded, a fletcher diagram written, toggled and saved, the document released, its PDF downloaded; the image type under "Show more". Axe on each step.
- Docs: the CLI and server READMEs, `docs/architecture.md`.

<!-- field: acceptance -->
- The walkthrough passes in `mise run e2e`, with lint and the tests.

<!-- akt: PLAN-015-G01 -->
---
type: gap
revision: 1
rank: b4K
state: open
in: [PLAN-015]
title: Images and diagrams in local mode and clones
---

<!-- field: description -->
Local mode refuses attachments (`cli.local.unsupported`), so a directory can't hold image akter, and a local release of a document embedding one finds the reference broken. Local mode also has no typst, so it can't render or check diagram blocks; they are checked when the server takes them, by apply or push. Clones show both as text. Should local mode learn attachments, or treat images and diagrams as references it can't check?

<!-- akt: PLAN-015-G02 -->
---
type: gap
revision: 1
rank: b4L
state: open
in: [PLAN-015]
title: Limits on compiling a diagram
---

<!-- field: description -->
typst can't be interrupted, so a diagram that loops or allocates without end would hold a server thread. Rendering runs off the async runtime, but a write waits for it. Does rendering need its own worker with a time and memory limit, such as a separate process?
