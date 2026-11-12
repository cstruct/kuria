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
  - What values a field takes: `text`, `choice`, `choices`, `date`, `subject` or `reference`, so far.

- `state`
  - Where an akt is in its akt type's workflow, such as `open` or `closed`. One state of each akt type is `initial`; states may be `final`, and an akt in a final state can no longer be edited.

- `akt revision`
  - One recorded change to an akt: who made it, in what capacity, through which transition, why, and what changed.
  - Revisions are append only. An akt's current state is always the result of its revisions.

- `transition`
  - A move of an akt from one state to another, as its akt type allows. A transition can have conditions, required fields, permitted roles and required signatures.

- `change event`
  - The record that an akt changed: the tenant, the akt and its new akt revision, numbered per tenant. Change events are what subscriptions and catching up deliver.

## Links

- `link`
  - A typed, directed relationship from one akt to another, such as a CAPA to the complaint it originated from.
  - A link can pin a specific akt revision of its target instead of following the latest.

- `link kind`
  - A runtime definition of one kind of link, such as `child of`, `verifies` or `included in`. Nesting is the link kind `child of`, not a separate mechanism.
