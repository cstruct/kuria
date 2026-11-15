# Documents

An example configuration: document indexes, each a tree of folders holding
documents, where a document is a living version anyone allowed can edit and
versions released by hand that never change. Nothing in Kuria knows these
names: they are ordinary akt type documents, using `markdown` fields,
`sections`, versions, references, image akter and diagram blocks, which any
akt type can use ([spec](../../spec/akter.md),
[versions](../../spec/versions.md), [PDFs](../../spec/pdf.md), [the git
remote](../../spec/git.md)).

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
- **Versions:** releasing a document names its current revision as its next
  version, 1, 2, …, with an optional label such as `2.0`. It needs the
  Konklav action `documents:release`. A release is refused while any
  reference is broken. A version never changes, and pins what each reference
  naming no version meant, so its links keep their meaning.
- **PDFs:** a document's body, of the living version or a version, its
  pictures and diagrams drawn in it.
- **Git:** an index is cloned with git, its folders as directories, each
  document a Markdown file, edited, committed, rebased and pushed back.

## In the app

1. Open a document and edit its **Body**. **Reference** finds another
   document, then one of its headings, and its living version or a version.
   **Image** finds an image, or uploads a file as a new one. **Diagram**
   starts a diagram as code, previewed as you type; **Show diagram** and
   **Show code** switch between its picture and its code.
2. **Release a version** in its Versions card, with a label. If a reference
   is broken, the release is refused, listing each.
3. Each version opens read-only; **Compare** shows what changed since.
   **Download PDF** gives the body of what is open.

## From the command line

```sh
kuria new document --in DOCS-1 --set title=Setup --set body=@setup.md
kuria show DOCS-1-D02                 # its broken references, if any
kuria release DOCS-1-D02 --label 1.0
kuria versions DOCS-1-D02
kuria cat DOCS-1-D02 --version 1      # read-only
kuria diff DOCS-1-D02 1               # what changed since version 1
kuria pdf DOCS-1-D02 --version 1 -o guide.pdf
```

## With git

`git-remote-kuria` is installed with `kuria`, and signs in as it does:

```sh
git clone kuria::https://kuria.example/DOCS-1 handbook
cd handbook
$EDITOR guides/DOCS-1-D02-guide.md
git commit -am "Explain the checks"
git pull --rebase                     # onto what changed in Kuria
git push                              # each commit, one batch of revisions
```

The index's file is at the root, and each document's file is in the
directories its folders give. A file is the akt file without `revision`,
`in`, `links` and `outcome`. Every clone has the same commits, the ones
pushed included, and each version is a tag `DOCS-1-D02/v1`. A push is
refused unless it is on top of Kuria's latest, and unless its files are as
Kuria writes them: `kuria fmt` gives headings their ids and puts each file
where its index's folders say. Move a document by editing the index's
`folders` section; a new document is made with `kuria new`, then fetched.

## Konklav roles

- **Writer:** `kuria:akter:read`, and `kuria:akter:create`,
  `kuria:akter:edit` and `kuria:akter:transition` at `document_index`,
  `document` and `image`.
- **Releaser:** a writer with `documents:release` at `document`.
