# Documents

An example configuration: document indexes, each a tree of folders holding
documents, where a document is a living version anyone allowed can edit and
versions released by hand that never change. Nothing in Kuria knows these
names: they are ordinary akt type documents, using `markdown` fields,
`sections`, versions, live drafts, references, image akter and diagram blocks, which any
akt type can use ([spec](../../spec/akter.md),
[versions](../../spec/versions.md), [PDFs](../../spec/pdf.md),
[clones](../../spec/clones.md),
[live drafts](../../spec/live-drafts.md)).

| Akt type | Number | Contains | Created in |
|---|---|---|---|
| `document_index` | `DOCS-1` | documents | |
| `document` | `DOCS-1-D01` | | a document index |
| `image` | `IMG-1` | | |

The documents are [`document_index.json`](document_index.json),
[`document.json`](document.json) and [`image.json`](image.json), images
first, since a document's references may name them:

```sh
kuria types define docs/examples/documents/{image,document,document_index}.json
```

## How it fits together

- **A document index** has a title, an owner, what it is about, and its
  folders: a `sections` field placing its documents in sections nested as
  deep as needed. A document is in one index.
- **A document's body** is a `markdown` field: Markdown whose headings keep
  ids (`## Installing {#install}`), given from the heading's text when it is
  first written and kept when the text changes, so links to it survive
  renaming, and footnotes (`[^1]` and `[^1]: …`). In the app it is written
  in a rich-text editor.
- **Images** are akter of their own: an `image` holds its picture, a PNG,
  JPEG, SVG or WebP, in its `file` field, which its type names as its
  picture. A body embeds one as `![The flow at a glance](kuria:IMG-1)`, a
  reference like any other, so a release checks it and a version keeps the
  picture it had. Images aren't in an index, and the sidebar lists them
  under "Show more".
- **Diagrams:** a body's ```` ```diagram ```` block holds typst, with
  fletcher (`diagram`, `node`, `edge`), cetz (`canvas`, `draw`) and
  cetz-plot (`plot`, `chart`) imported. Saving the body renders each block,
  and is refused if one doesn't compile:

  ````md
  ```diagram
  #diagram(node((0, 0), [Write]), edge("-|>"), node((1, 0), [Release]))
  ```
  ````
- **References:** a link `kuria:DOCS-1-D04#install` names another document
  and one of its headings; `kuria:DOCS-1-D04@2#install`, that document as it
  was at version 2. Each referenced document is linked as "Refers to", and
  reads "Referred to by" back. A reference to a document or heading that
  isn't there is shown as broken, saying why.
- **Live drafts:** a document says `"live_drafts": true`, so its fields
  are one live draft everyone allowed edits at once, seeing each other's
  text and cursors as they type ([live drafts](../../spec/live-drafts.md)).
  Edits are kept as they are made, with no Save; its revisions are its
  releases.
- **Versions:** releasing a document names its current revision as its next
  version, 1, 2, …, with an optional label such as `2.0`. It needs the
  Konklav action `documents:release`. A release is refused while any
  reference is broken, a link to another document being read as that
  document's latest version: one never released, or whose latest version
  lacks the heading, breaks it. Documents that link to each other's new
  text are **released together**, all or nothing, each link read as what
  the other is released as. A version never changes, and pins what each
  reference naming no version meant, so its links keep their meaning.
- **PDFs:** a document's body, of the living version or a version, its
  pictures and diagrams drawn in it.
- **Clones:** an index as files, its folders as directories, each document
  a Markdown file, to edit with any editor and sync with `kuria docs pull`
  and `kuria docs push` (below).

## In the app

1. Open a document and edit its **Body**, live: whoever else has it open
   sees your text as you type, and you theirs. **Done** closes the editor.
   **Reference** finds another document, then one of its headings, and its
   living version or a version.
   **Image** finds an image, or uploads a file as a new one. **Diagram**
   starts a diagram as code, previewed as you type; **Show diagram** and
   **Show code** switch between its picture and its code.
2. **Release a version** in its Versions card, with a label. If a reference
   is broken, the release is refused, listing each; where releasing another
   document with it would mend a link, it offers **Release together with**
   that document, chosen already: **Release** again releases both.
3. Each version opens read-only; **Compare** shows what changed since.
   **Download PDF** gives the body of what is open.

## From the command line

```sh
kuria new document --in DOCS-1 --set title=Setup --set body=@setup.md
kuria show DOCS-1-D02                 # its broken references, if any
kuria release DOCS-1-D02 --label 1.0
kuria release DOCS-1-D02 DOCS-1-D03   # together, linking to each other's new text
kuria versions DOCS-1-D02
kuria cat DOCS-1-D02 --version 1      # read-only
kuria diff DOCS-1-D02 1               # what changed since version 1
kuria pdf DOCS-1-D02 --version 1 -o guide.pdf
```

## With kuria docs

An index and its documents as files, synced with Kuria
([clones](../../spec/clones.md)), live drafts included:

```sh
kuria docs clone DOCS-1       # into DOCS-1-handbook/, the base in .kuria/
cd DOCS-1-handbook
# edit guides/DOCS-1-D02-setup.md with any editor
kuria docs status             # what changed, what is staged
kuria docs diff               # what isn't staged yet
kuria docs add guides/DOCS-1-D02-setup.md    # or `kuria docs add -p` for some hunks
kuria docs push -m "Explain the install"
kuria docs pull               # what others changed, merged in
```

The index's file is at the root, and each document's file is in the
directories its folders give; a file is the akt file without `revision`,
`in`, `links` and `outcome`. The pictures of the images the documents
embed are in `attachments/`, as `attachments/IMG-1.png`, and a document
links to its pictures relatively,
`![The flow at a glance](../attachments/IMG-1.png)`, so editors and git
hosts show them. Diagrams stay as their code. `.kuria/` holds the base:
each file as last pulled, each akt's revision and each live draft's
state. It ignores itself in git.

- **A push** fetches first, and is refused unless what it pushes is still
  as the base has it: `kuria docs pull`, then push again. A document's staged
  change goes to its live draft as one change, merged with whatever is
  being typed in the app meanwhile, and kept with your message in its
  history; the index's goes as a revision with the message as its reason.
  Documents and images are created and removed in Kuria, and pictures
  uploaded there: a new or removed file, or a changed picture, is refused.
- **A pull** merges Kuria's changes into your files. Where you and Kuria
  changed the same text, the file holds both, between `<<<<<<< ours`,
  `=======` and `>>>>>>> kuria`, and `kuria docs status` names it: edit it
  to what it should say, then `kuria docs add` it (a file still holding markers
  isn't staged). What you had staged is staged again, or unstaged where
  it conflicts.
- **With git:** a clone can also be a git repository of your own; Kuria
  sees only the files. To push part of your work and keep the rest:

  ```sh
  git init && git add -A && git commit -m "Clone"   # once
  # …work, committing as you go…
  git branch keep               # everything, kept on a branch
  git reset --hard HEAD~1       # the unready commits out of the working copy
                                # (or `git revert` them: Kuria sees only files)
  kuria docs add . && kuria docs push -m "Ready part"
  kuria release DOCS-1-D02 --label 1.0     # or release it in the app
  git merge keep                # the rest back (after a revert, revert the revert)
  kuria docs add . && kuria docs push -m "The rest"
  ```

`kuria docs fmt`, run in a clone, puts its files as Kuria writes them, as a
push needs them: it gives headings their ids, puts each file where its index's
folders say, and fetches the pictures of images newly embedded, putting
back any changed.

Exported as an akt directory, `kuria export --types
document_index,document,image --to-dir handbook`, each image keeps its
file as `attachments/IMG-1/<name>`, its `file` naming that path, and
`kuria import --from-dir handbook` takes the files along to another
tenant. There, `kuria --dir handbook set IMG-1 file=@flow.png` copies a
new picture in, and a release finds the pictures it embeds.

## With example data

[`bundle/`](bundle/) is an example bundle of these akt types with akter: a
quality manual of documents in folders, referring to each other and to a
diagram image, released (the release procedure twice) and one retired, and
an archived handbook. Devi Larsen (`dev@shop.example`), Rami Moberg
(`rm@shop.example`) and Hedda Lund (`hedda@shop.example`) wrote them, the
people the end-to-end seed signs in as. To start a server with it, set

```sh
KURIA_BOOTSTRAP_BUNDLE=docs/examples/documents/bundle
```

and the server imports it into its default tenant (or
`KURIA_BOOTSTRAP_TENANT`) before it serves; the tenant must hold nothing
yet ([`items/server`](../../../items/server/README.md#running)).
`kuria import docs/examples/documents/bundle` takes it to a running server's
tenant that hasn't defined these types.

```sh
mise run example documents
```

serves it on <http://localhost:18290> to its people, who sign in as
themselves, with the app hot-reloading ([`example/`](../../../example/README.md)).
[`docs/examples`](../README.md) says how the bundles are made.

## Konklav roles

- **Writer:** `kuria:akter:read`, and `kuria:akter:create`,
  `kuria:akter:edit` and `kuria:akter:transition` at `document_index`,
  `document` and `image`.
- **Releaser:** a writer with `documents:release` at `document`.
