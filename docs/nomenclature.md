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
  - What values a field takes: `text`, `choice`, `choices`, `date`, `user`, `checkbox`, `reference`, `git`, `docker` or `attachment`, so far.

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
  - The record that an akt changed, numbered per tenant: either a new akt revision, or a new rank. Change events are what subscriptions and catching up deliver.

## Links

- `link`
  - A typed, directed relationship from one akt to another, such as a CAPA to the complaint it originated from.
  - A link can pin a specific akt revision of its target instead of following the latest.

- `link kind`
  - A runtime definition of one kind of link, such as `verifies` or `originated from`.
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
  - The part of an akt type that says how its akter are shown: which field is the title, which fields go on board cards and list columns, and the tone of each state. It changes how akter look, never what they hold.

- `title`
  - The name an akt is shown by beside its number, such as "Sign-in" for `EPIC-001`: the value of the field its akt type's presentation names.

- `tone`
  - How strongly, and in which sense, a state is shown: `neutral`, `active`, `success`, `warning` or `danger`.

- `draft`
  - Text a field is filled with from the akter around its akt, by the recipe its field declares, such as release notes from the items a release shipped. A draft is an ordinary edit.

- `parent` (in a number pattern)
  - The container an akt was created in, whose akt number `{parent}` renders, as in `RELEASE-2026-0001-RC-0001`.
