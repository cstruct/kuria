# Nomenclature

This document defines the preferred language for Kuria specs, code, API surfaces, and documentation.

Terms are introduced in dependency order: each definition only uses terms defined above it, or points explicitly to a later section. If a new term becomes important to the project, add it here before spreading it across specs or APIs.

## Systems and boundaries

- `Kuria`
  - The case and record management system defined by this repository: issues, nested work, test protocols and records, release records, and CAPA.
  - Kuria hardcodes as little as possible about what these things are. Their kinds and the rules they follow come from configuration at runtime (see Akter).

- `Konklav`
  - The identity and access service Kuria relies on. Kuria is a Konklav `relying system`: Konklav decides who someone is and what they may do, and Kuria enforces those decisions on its own data.
  - Terms Kuria borrows from Konklav, such as `tenant`, `subject`, `tenant token` and `service principal`, keep the meaning Konklav's nomenclature gives them.

- `viewer`
  - The subject making a request, in the tenant of its token, as Kuria's GraphQL API presents it.

- `browser session`
  - A browser's signed-in state in Kuria: the Konklav tokens Kuria holds for it on the server, named by the browser's session cookie.
  - Not to be confused with Konklav's `session`, which a browser session's tokens belong to and which can outlive it.

- `app`
  - Kuria's frontend: the single-page app the server serves from its own origin, which builds every view from the akt types it reads.

- `login page`
  - Kuria's page that acts as Konklav's login application: it signs a person in to Konklav and accepts the login request a sign-in to Kuria, or to another client, started.

- `login attempt`
  - A browser sign-in that Kuria has started at Konklav and not completed yet, named by its `state`.

## Akter

- `akt` (plural `akter`)
  - The one generic object in Kuria. A bug report, an epic, a test record, a release record, and a CAPA are each an akt.
  - From the Swedish *akt*, a case file: an akt is something that is opened, worked on, and kept as a record.
  - Kuria's core knows only akter. What distinguishes one akt from another is its `akt type`.
  - Prefer this term over `issue`, `ticket`, `item`, or `object` when speaking of the generic thing. Those words may name an akt type, never the generic concept.
  - Identifiers stay singular: tables `akt` and `akt_type`, paths `/akt` and `/akt/{id}`. The plural `akter` appears only in prose.

- `akt type`
  - A runtime definition of one kind of akt, such as `bug`, `epic`, or `test record`.
  - Every akt has exactly one akt type.
  - An akt type defines the akt's fields, its states, and the transitions between them.

- `akt type version`
  - One version of an akt type's definition. Changing an akt type creates a new version, and versions that have been used never change.
  - Every akt revision records the akt type version it was validated against.

- `akt number`
  - The readable reference people use for an akt, such as `CAPA-2026-0042`. It never changes, and is separate from the akt's internal identifier.

- `number pattern`
  - The part of an akt type that says how its akt numbers read, such as `CAPA-{year}-{seq:4}`.

- `field`
  - One named value an akt type defines, such as `severity`. Every field has a `field kind`.

- `field kind`
  - What values a field takes: `text`, `choice`, `choices`, `date`, `user`, `checkbox`, `reference`, `git`, `docker`, `attachment`, `sections`, `verdict` or `markdown`, so far.

- `person`
  - What Kuria knows about a subject to show them: the name and email recorded at their last browser sign-in.

- `contact`
  - A person a tenant names before, or without, them being a user (a subject who signs in to Kuria): a name and an optional email, kept per tenant. The subject who first signs in to the tenant with that email, verified, is linked to it, and from then on it is shown as them.
  - Values keep naming the contact after it is linked; only how they are shown changes.

- `user` (field kind)
  - A field naming a Konklav subject or a contact, shown by name. Prefer `user` in field and API names; `subject` stays the term for the identity itself. In GraphQL a value names `Someone`: a `User` or a `Contact`.

- `checked for`
  - Whom a checkbox was checked for, when someone checked it on another's behalf: "Approved: Bob, checked by Alice". Without it, a box is checked for whoever checked it.

- `attachment`
  - A file uploaded to Kuria, immutable, stored in object storage by its SHA-256 and held by akter in `attachment` fields.

- `SBOM`
  - A software bill of materials: an attachment Kuria recognised as SPDX or CycloneDX, with a summary of its format, version, name and component count.

- `state`
  - Where an akt is in its akt type's workflow, such as `open` or `closed`. One state of each akt type is `initial`; states may be `final`, and an akt in a final state can no longer be edited.

- `akt revision`
  - One recorded change to an akt: who made it, in what capacity, through which transition, why, and what changed.
  - Revisions are append only. An akt's current state is always the result of its revisions.

- `transition`
  - A change of an akt from one state to another, as its akt type allows. A transition can have conditions, required fields, permitted roles and required signatures.

- `change event`
  - The record that an akt changed, numbered per tenant: a new akt revision, a new rank, a released version, or a change to its live draft (at most one a second per server for edits made live). Change events are what subscriptions and catching up deliver.

## Documents and versions

- `markdown` (field kind)
  - A field holding Markdown whose headings keep ids and whose links can name other akter: a document's body. Prefer it over `rich text`; the app edits it with a rich-text editor, but what it holds is Markdown.

- `heading id`
  - The id a heading in a `markdown` field keeps, written `## Installing {#install}`, unique within its akt. Given from the heading's text when it is first written, and kept when the text changes, so links to it survive renaming.

- `reference`
  - A link in a `markdown` field's text to an akt, written `kuria:<NUMBER>`, optionally to one of its versions (`@<n>`) and one of its headings (`#<heading id>`), such as `kuria:DOCS-1-D04@2#install`. Recorded as a link of the kind the field names. Not to be confused with the `reference` field kind, a link to something outside Kuria.

- `deep link`
  - A reference to a heading of another akt.

- `footnote`
  - A note in a `markdown` field, referred to in its text as `[^label]` and defined anywhere in it as `[^label]: …`, as GitHub writes them. Numbered in the order it is first referred to, and shown at the end of the text, or at the foot of the page in a PDF. Not to be confused with a `reference`, which links to an akt.

- `broken reference`
  - A reference to an akt that doesn't exist or the reader can't read, a version it doesn't have, or a heading it doesn't have.

- `image` (akt)
  - An akt whose type names, as `presentation.image`, an attachment field taking only image media types: its picture is that field's value. Not a field kind.

- `embedded image`
  - An image in a `markdown` field whose destination is a reference, `![Alt text](kuria:IMG-1)`: the image akt's picture shown in the text. A reference like any other.

- `diagram block`
  - A fenced code block with the language `diagram`, in a `markdown` field that says `"diagrams": true`: typst markup drawing a fletcher diagram or a cetz plot, rendered to SVG when the field is written.

- `picture` (in a clone)
  - An embedded image's picture as a file of a clone, `attachments/IMG-1.png`, which the document links to relatively. Pictures are uploaded in Kuria, never pushed.

- `bundled package`
  - A typst package vendored in Kuria, unmodified with its license, the only kind a diagram or a PDF can import: fletcher, cetz and cetz-plot.

- `live draft`
  - The fields of an akt whose type says `"live_drafts": true`, edited together, live, with Yjs. Edits change the live draft only; a release records it as a revision. Not to be confused with a `draft`, which a recipe fills a field with.

- `page break`
  - A line holding only `<!-- pagebreak -->` in a `markdown` field: a new page in its PDF, a new sheet in the app.

- `sheet`
  - One A4 page of a `markdown` field as the app shows and edits it, white whatever the theme.

- `living version`
  - An akt as it is now, which edits change; as opposed to its versions.

- `version`
  - One revision of an akt, named by releasing it: numbered 1, 2, … per akt, with an optional label, recording who released it and when and what each of its references meant then. It never changes. Not to be confused with an `akt type version`.

- `release` (verb)
  - To name the current revision of an akt as its next version. Prefer `version` for the result: `release` is also an akt type in the release-management example.

- `pin` (of a version)
  - What a version records for each of its references without a version: the target's latest version when it was released, else the target's revision then, so that the version's links keep their meaning.

- `clone`
  - A container and its members as files on someone's machine, made with `kuria docs clone`, worked on with any editor and synced with Kuria by `kuria docs pull` and `kuria docs push`. It may also be a git repository, which is the person's own.

- `base`
  - What a clone last had from Kuria: each file, each akt's revision, and each live draft's state. Pushes are made against it; pulls merge from it.

- `change set`
  - What one push from a clone changed in a live draft, kept as one change with its author and message.

- `normal form`
  - The files exactly as Kuria writes a container and its members to a clone; `kuria docs fmt` puts them in it.

## Links

- `link`
  - A typed, directed relationship from one akt to another, such as a CAPA to the complaint it originated from.
  - A link can pin a specific akt revision of its target instead of following the latest.

- `link kind`
  - A runtime definition of one kind of link, such as `verifies` or `originated from`, declared by the akt type of the link's source in `links`, with a name as the source reads it ("Blocks"), an inverse name as the target reads it ("Blocked by"), and the akt types its targets may have.
  - Containment is the one link kind Kuria builds in, because boards, ranks and numbering depend on it. Prefer `container` and `member` over `parent` and `child` for it.

## Containment

- `container`
  - An akt whose akt type lets it contain other akter, such as a backlog, an epic or a release.

- `member`
  - An akt in a container. An akt may be a member of many containers at once, such as a backlog, an epic and a release.

- `membership`
  - One akt being a member of one container. Adding and ending a membership are recorded as akt revisions of both akter.

- `exclusive`
  - Said of a container akt type when an akt may be a member of at most one of its akter at a time, such as one backlog.

- `transfer`
  - Ending an akt's membership in one container and starting one in another, as one change, such as moving an item from one backlog to another.

- `rank`
  - An akt's place in its tenant's one priority order. Every board orders members by rank, so moving an akt on one board moves it on every board.

- `move`
  - Giving an akt a new rank, directly before or after another akt. A move is not an akt revision.

- `board`
  - A container shown as columns. The container's akt type configures the columns, naming which states of which member akt types fall in each.

- `presentation`
  - The part of an akt type that says how its akter are shown: which field is the title, which fields go on board cards and list columns, the tone of each state, and the type's icon and colour. It changes how akter look, never what they hold.

- `title`
  - The name an akt is shown by beside its number, such as "Sign-in" for `EPIC-001`: the value of the field its akt type's presentation names.

- `tone`
  - How strongly, and in which sense, a state is shown: `neutral`, `active`, `success`, `warning` or `danger`.

- `draft`
  - Text a field is filled with from the akter around its akt, by the recipe its field declares, such as release notes from the items a release shipped. A draft is an ordinary edit. Not to be confused with a `live draft`.

- `parent` (in a number pattern)
  - The container an akt was created in, whose akt number `{parent}` renders, as in `RELEASE-2026-0001-RC-0001`.

## Lists

- `page`
  - The part of a list of akter answered at once, at most a limit of them, with a `cursor` after its last akt and whether more follow.

- `cursor`
  - An opaque mark after the last akt of a page, which asks the same list for the akter that follow. A cursor belongs to the list that gave it.

- `filter`
  - A condition on a field's value that a list's akter must meet, such as an assignee being the viewer.

## Moving records

- `bundle`
  - What an export writes and an import reads: selected akt types, with or without their akter, or a whole tenant, as a directory (a tar archive on the wire). Akter in a bundle keep their numbers and whole history.

- `subject alias`
  - A record, made by an import, that a subject a bundle names is shown as, and filters as, a user of the importing tenant. Revisions keep naming the subject they recorded.

## Command-line client

- `kuria`
  - Kuria's command-line client, for people and agents. Not to be confused with `kuria-server`, the server binary.

- `profile`
  - A saved Kuria URL and tenant that `kuria login` signed in to, which commands use when not told otherwise.

- `akt file`
  - An akt as `kuria cat` writes it and `kuria apply` reads it: Markdown with the akt's single-line fields in YAML front matter and each multiline field as its own section.

- `akt directory`
  - A directory of Markdown files that local mode reads and writes instead of a Kuria server: `kuria.toml`, akt type documents in `types/`, and akter in `akter/`, each file holding an akt and its nested akter, named by number and title, with git as their history.

- `template`
  - An akt kept to be copied, such as a test template copied into each test protocol. Nothing marks it but how it is used; an example may give it a state of its own.
- `copy`
  - A new akt made from another of the same akt type, in its initial state, with its field values and the part of its member tree chosen, each member copied likewise.
- `section`
  - A part of an akt's `sections` field: an optional title and text, whether it is collapsible, and the members and sections it holds, in order.
- `assignee`
  - A user or contact named in the `multiple` `user` field a `verdict` field names: someone who is to give the akt a verdict.
- `verdict`
  - An assignee's judgement of an akt: `pass`, `fail` or `skip`, recorded with who gave it and when.
- `outcome`
  - What an akt's verdicts, or its members' outcomes, add up to: `pass`, `fail`, `skip` or `pending`. Derived when read, never stored.
- `batch`
  - A list of akt writes, operations, that the API runs in order in one transaction: all of them, or none.
- `operation`
  - One akt write in a batch, counted from 1; `@<n>` names the akt that operation `n` created.
- `nested akt`
  - An akt stored in its parent's file in an akt directory, after the line `<!-- akt: <NUMBER> -->`, because its number is made from its parent's.
- `local mode`
  - The client working on an akt directory (`--dir`, `KURIA_DIR`) instead of a server, with the same commands, answers and rules.

- `relay`
  - Kuria's `/cli/authorized` page, which holds the authorization code Konklav redirects to it until the client that asked for it collects it.
