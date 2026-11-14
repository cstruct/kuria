---
number: PLAN-002
type: plan
revision: 37
rank: ar
state: done
title: Field Kinds and Release Artifacts
---

<!-- field: goal -->
After this milestone, an akt type can declare:

- `user`: a person, shown by name, as in "Release manager: Bob". Replaces the
  `subject` kind.
- `checkbox`: checked or not, recording who checked it and when, as in
  "Approved for release: ✓ by Alice, 2026-04-01".
- `git`: a repository and a commit SHA, a tag, or both.
- `docker`: an image repository, its digest and optionally a tag.
- `attachment`: an uploaded file; with `"format": "sbom"`, only an SPDX or
  CycloneDX document, which Kuria validates and summarises.

And the release example gains:

- an `artifact` akt type (component, version, change, git source, Docker
  image, SBOM), which release candidates and hotfixes contain;
- a release manager (`user`) and a go/no-go `checkbox` that releasing
  requires;
- release notes with an artifacts section drafted from the release's
  candidates and hotfixes: per component, the newest artifact, leaving out
  components whose newest artifact is a removal.

Success is the release walk-through extended to upload an SBOM, build up
artifacts across two release candidates (one removing a component), and
draft notes that list exactly what ships.

<!-- field: design -->
## Constraints And Assumptions

- Builds on the field model in `docs/spec/akter.md`: fields are typed by
  kind, values are validated by `kuria-engine` before they are written, and
  stored as JSONB in `akt.fields`, revisions recording what changed.
- `kuria-engine` stays free of I/O. A value that needs the database (an
  attachment's existence) or the caller (who checked a box) gets that from
  the server, which passes it in.
- Konklav keeps no profile data. Names come from the ID token and userinfo
  claims at browser sign-in, kept in Kuria and erased with the subject.
- Attachments live in S3-compatible object storage, through the
  `object_store` crate: in development a maintained S3 server
  (`versitygw`) in `dev/compose.yaml`, in tests an in-memory store, and any
  S3 in production. Postgres keeps their metadata.
- Specs first: each task starts with its Tracey rules and terms.

## Design

### Field kinds

| Kind | Stored value | Input | Shown |
|---|---|---|---|
| `user` | subject id | subject id | `UserValue { user { id name email } }` |
| `checkbox` | `{ "checked_by": id, "checked_at": time }`, or absent | `true` or `false` | `CheckboxValue { checked by at }` |
| `git` | `{ "repository", "sha"?, "tag"? }` | the same | `GitValue`, `repository@tag (sha12)` |
| `docker` | `{ "repository", "digest", "tag"? }` | the same | `DockerValue`, `repository:tag@sha256:…` |
| `attachment` | the attachment's id, as a string | an uploaded attachment's id | `AttachmentValue { attachment { name size sha256 sbom } }` |

- `user` replaces `subject`. The kind name changes; the value doesn't.
- `checkbox`: checking records the caller and the time, which the server
  supplies, never the client. Unchecking clears it. A transition that
  `requires` a checkbox needs it checked.
- `git`: `repository` is an `https` URL or an `ssh` remote; `sha` is 40 or 64
  lowercase hex digits; at least one of `sha` and `tag`.
- `docker`: `repository` such as `registry.example/team/backend`; `digest`
  `sha256:` and 64 lowercase hex digits; `tag` optional.
- `attachment`: a field may say `"format": "sbom"`. Setting it requires an
  attachment of the tenant, and for `sbom`, one Kuria recognised as SPDX or
  CycloneDX.

### People

- A `person` table: subject id, name, email, updated. Filled at browser
  sign-in from the ID token or the userinfo endpoint (scopes `profile email`
  requested), and updated on each sign-in.
- `User { id name email }` in GraphQL; `name` falls back to the email, then
  to the id, for subjects that never signed in (service principals).
- Erasing a subject deletes its person row (`erasure.*`).

### Attachments

- `POST /attachments` with the file as the body and its name in a header;
  authenticated like `/graphql`, with the origin check for cookies. Answers
  the attachment's id, size and SHA-256.
- Stored once under its SHA-256 per tenant; metadata in `attachment`:
  tenant, id, name, media type, size, SHA-256, uploaded by and when, and an
  SBOM summary if it is one.
- SBOM detection at upload: JSON with `spdxVersion` (SPDX) or
  `"bomFormat": "CycloneDX"`. The summary keeps format, spec version,
  document name and component count. Anything else isn't an SBOM.
- `GET /attachments/{id}` streams it back to anyone who may read an akt that
  references it, or to its uploader while nothing references it yet. The
  write path keeps `akt_attachment` (akt, field, attachment) current so
  this is one query.
- A size limit from configuration; attachments are immutable.

### Drafting artifacts

Two additions to draft recipes:

- `latest_by`: a field key. Of the listed akter with the same value in that
  field, only the most recently created one is kept. So a component's newest
  artifact replaces the older ones.
- `exclude`: per field key, option keys. Listed akter whose field holds one
  of them are left out, after `latest_by`. So a removal artifact, being the
  newest, makes its component disappear.

The release notes' artifacts field: `through: [release_candidate, hotfix]`,
`types: [artifact]`, `latest_by: component`, `exclude: { change: [removed] }`,
a line such as `- {field:component} {field:version}: {field:image}`.

<!-- field: verification -->
- `mise run test` with Postgres, Valkey and `versitygw` running; attachment
  tests that don't need S3 itself use the in-memory store.
- `tracey query untested` shows only `auth.service-principal`.
- By hand against a running server: upload a CycloneDX file, set it on an
  artifact, see its summary; check a box and see who checked it.

<!-- akt: PLAN-002-T01 -->
---
type: task
revision: 3
rank: as
state: done
in: [PLAN-002]
title: T1 Spec and nomenclature
---

<!-- field: description -->
Spec and nomenclature: the new kinds in `akt.field.kinds` with
`akt.field.checkbox` and `akt.field.attachment`; `docs/spec/attachments.md`
(7 rules); `docs/spec/people.md` (3 rules); `draft.latest` and the text of
every kind in `draft.render`; and the terms user, person, attachment and
SBOM (`7b7cc80`).

<!-- akt: PLAN-002-T02 -->
---
type: task
revision: 3
rank: at
state: done
in: [PLAN-002]
title: "T2 People and the `user` kind"
---

<!-- field: description -->
People and the `user` kind: `subject` renamed to `user`; `person`
filled from Konklav's userinfo endpoint at sign-in (`profile email`
requested, best effort); `User { id name email }` in GraphQL, a revision's
`by`, names in drafts; erased with the subject (`95eda36`).

<!-- akt: PLAN-002-T03 -->
---
type: task
revision: 3
rank: au
state: done
in: [PLAN-002]
title: "T3 Checkbox, git and Docker"
---

<!-- field: description -->
Checkbox, git and Docker: a `Change` (who, when) the engine takes
for values the server supplies; `checkbox` recording the caller and keeping
the first record when checked again; `git` and `docker` values with their
validation, text and GraphQL types. T5's value types for these came along
(`839aa17`).

<!-- akt: PLAN-002-T04 -->
---
type: task
revision: 3
rank: av
state: done
in: [PLAN-002]
title: T4 Attachments
---

<!-- field: description -->
Attachments: `versitygw` in development; `object_store` (S3, in
memory in tests); `POST /attachments` and `GET /attachments/{id}` with the
size limit, percent-encoded names, and downloads always as attachments;
content-addressed storage per tenant; SBOM detection in the engine; the
`attachment` kind with `format`, checked by the write path (the tenant's,
readable by the caller, an SBOM for `sbom`), and `akt_attachment` kept on
every write that touches an attachment field; `AttachmentValue` in
GraphQL; attachment names in drafts. T5 is complete with it
(`037b891`).

<!-- akt: PLAN-002-T05 -->
---
type: task
revision: 3
rank: aw
state: done
in: [PLAN-002]
title: T6 Drafting artifacts
---

<!-- field: description -->
Drafting artifacts: `latest_by` and `exclude` in recipes, their
keys checked in the document, and the fields and options they name
checked against the listed akt types in both directions
(`check_field_references`); `draft::select` applied after the listed
akt types are chosen and before rank order (`0d81326`).

<!-- akt: PLAN-002-T06 -->
---
type: task
revision: 3
rank: ax
state: done
in: [PLAN-002]
title: T7 The release example
---

<!-- field: description -->
The release example: `artifact.json` (component, change built or
removed, version, git source, Docker image, SBOM); candidates and hotfixes
contain artifacts; the release has a release manager and a go checkbox
that releasing requires; the notes draft their artifacts. The walkthrough
has a pipeline (another subject) upload a CycloneDX SBOM and register
artifacts across two candidates and a hotfix, with one removal
(`a9ccdd6`).

<!-- akt: PLAN-002-T07 -->
---
type: task
revision: 3
rank: ay
state: done
in: [PLAN-002]
title: T8 Docs
---

<!-- field: description -->
Docs: the requirements' coverage table (field kinds, attachments,
people and contacts, drafting the newest per value), the architecture's
GraphQL sketch, mutations and identity, and the server README's types and
error codes, done together with the contacts milestone's last task.

<!-- akt: PLAN-002-T08 -->
---
type: task
revision: 3
rank: az
state: done
in: [PLAN-002]
title: "T5.1 `UserValue`, `CheckboxValue`, `GitValue`, `DockerValue`, `AttachmentValue`; drafts showing each as…"
---

<!-- field: description -->
`UserValue`, `CheckboxValue`, `GitValue`, `DockerValue`,
`AttachmentValue`; drafts showing each as text.

<!-- akt: PLAN-002-G01 -->
---
type: gap
revision: 1
rank: b00
state: open
in: [PLAN-002]
title: "G1 A `user` field accepts any subject id; whether it must be a member of the tenant isn't checked, sin…"
---

<!-- field: description -->
A `user` field accepts any subject id; whether it must be a member of
the tenant isn't checked, since Kuria can't list memberships cheaply.

<!-- akt: PLAN-002-G02 -->
---
type: gap
revision: 1
rank: b01
state: open
in: [PLAN-002]
title: "G2 Attachments aren't scanned for malware"
---

<!-- field: description -->
Attachments aren't scanned for malware.

<!-- akt: PLAN-002-G03 -->
---
type: gap
revision: 1
rank: b02
state: open
in: [PLAN-002]
title: "G3 An attachment's bytes are kept when its uploader is erased; they are records, not profile data"
---

<!-- field: description -->
An attachment's bytes are kept when its uploader is erased; they are
records, not profile data. Whether some must go is a retention question.

<!-- akt: PLAN-002-G04 -->
---
type: gap
revision: 1
rank: b03
state: open
in: [PLAN-002]
title: "G4 Exports don't include attachments yet"
---

<!-- field: description -->
Exports don't include attachments yet.

<!-- akt: PLAN-002-G05 -->
---
type: gap
revision: 2
rank: b04
state: closed
in: [PLAN-002]
title: "G6 The attachment branch of draft text (an attachment shows by name) had no test of its own; T6's mani…"
---

<!-- field: description -->
The attachment branch of draft text (an attachment shows by name)
had no test of its own; T6's manifest test covers it.

<!-- field: resolution -->
The attachment branch of draft text (an attachment shows by name)
had no test of its own; T6's manifest test covers it.

<!-- akt: PLAN-002-G06 -->
---
type: gap
revision: 1
rank: b05
state: open
in: [PLAN-002]
title: "G7 The origin check on `POST /attachments` is the session layer's, tested for `/graphql` only"
---

<!-- field: description -->
The origin check on `POST /attachments` is the session layer's,
tested for `/graphql` only.

<!-- akt: PLAN-002-G07 -->
---
type: gap
revision: 1
rank: b06
state: open
in: [PLAN-002]
title: "G8 Uploads are read into memory whole (up to the limit) to hash and detect SBOMs before storing; strea…"
---

<!-- field: description -->
Uploads are read into memory whole (up to the limit) to hash and
detect SBOMs before storing; streaming them is for when the limit grows.

<!-- akt: PLAN-002-G08 -->
---
type: gap
revision: 1
rank: b07
state: open
in: [PLAN-002]
title: "G9 Uploads no akt ever holds stay stored; sweeping old unheld ones is open"
---

<!-- field: description -->
Uploads no akt ever holds stay stored; sweeping old unheld ones is
open.

<!-- akt: PLAN-002-G09 -->
---
type: gap
revision: 1
rank: b08
state: open
in: [PLAN-002]
title: "G5 One full test run during T3 failed once and couldn't be reproduced in nine further runs; it came ri…"
---

<!-- field: description -->
One full test run during T3 failed once and couldn't be reproduced
in nine further runs; it came right after a rebuild. Likely a timing-bound
test (subscriptions without a cursor, or token expiry) on a cold first
connection. Watch for it; if it recurs, capture the log.

<!-- akt: PLAN-002-D01 -->
---
type: decision
revision: 2
rank: b09
state: decided
in: [PLAN-002]
title: Names from sign-in claims
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Names from sign-in claims**, not a Kuria directory or a Konklav change:
people name themselves once by signing in, and no one maintains a second
list. The price is that someone who never signed in shows by email or id.

<!-- field: why -->
, not a Kuria directory or a Konklav change:
people name themselves once by signing in, and no one maintains a second
list. The price is that someone who never signed in shows by email or id.

<!-- akt: PLAN-002-D02 -->
---
type: decision
revision: 2
rank: b0A
state: decided
in: [PLAN-002]
title: "S3-compatible storage through `object_store`"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**S3-compatible storage through `object_store`**, with `versitygw` in
development rather than MinIO, and an in-memory store in tests.

<!-- field: why -->
, with `versitygw` in
development rather than MinIO, and an in-memory store in tests.

<!-- akt: PLAN-002-D03 -->
---
type: decision
revision: 2
rank: b0B
state: decided
in: [PLAN-002]
title: A removal is an artifact
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**A removal is an artifact**, the newest for its component, so the
release notes build up the artifact set across all of a release's
candidates and hotfixes in order. They don't filter by candidate state: a
rejected RC1 followed by an RC2 that rebuilds only the backend still ships
RC1's frontend.
Confirmed after T7, knowing that a rejected candidate's new build then
ships unless a later candidate rebuilds or removes it.

<!-- field: why -->
, the newest for its component, so the
release notes build up the artifact set across all of a release's
candidates and hotfixes in order. They don't filter by candidate state: a
rejected RC1 followed by an RC2 that rebuilds only the backend still ships
RC1's frontend.
Confirmed after T7, knowing that a rejected candidate's new build then
ships unless a later candidate rebuilds or removes it.

<!-- akt: PLAN-002-D04 -->
---
type: decision
revision: 2
rank: b0C
state: decided
in: [PLAN-002]
title: SBOMs are validated and summarised
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**SBOMs are validated and summarised**, not indexed: format, spec version,
name and component count. Indexing components can come with vulnerability
work.

<!-- field: why -->
, not indexed: format, spec version,
name and component count. Indexing components can come with vulnerability
work.

<!-- akt: PLAN-002-D05 -->
---
type: decision
revision: 2
rank: b0D
state: decided
in: [PLAN-002]
title: The server supplies who checked a box
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**The server supplies who checked a box**, never the client.

<!-- field: why -->
, never the client.

<!-- akt: PLAN-002-D06 -->
---
type: decision
revision: 2
rank: b0E
state: decided
in: [PLAN-002]
title: Upload names travel in a header
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Upload names travel in a header**, `X-Attachment-Name`, with the bytes as
the whole body, rather than multipart: one code path, streamable, and the
same from scripts and browsers (T1).

<!-- field: why -->
, `X-Attachment-Name`, with the bytes as
the whole body, rather than multipart: one code path, streamable, and the
same from scripts and browsers (T1).

<!-- akt: PLAN-002-D07 -->
---
type: decision
revision: 2
rank: b0F
state: decided
in: [PLAN-002]
title: "`subject` stays the term for the identity; `user` is the field kind"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**`subject` stays the term for the identity; `user` is the field kind**,
since most users read "user" where the API says subject (T1).

<!-- field: why -->
,
since most users read "user" where the API says subject (T1).

<!-- akt: PLAN-002-D08 -->
---
type: decision
revision: 2
rank: b0G
state: decided
in: [PLAN-002]
title: Names come from the userinfo endpoint
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Names come from the userinfo endpoint**, not the ID token: an
authenticated call Kuria already makes the right way, no JWT to parse, and
the `sub` it answers is checked against the verified access token. A
sign-in doesn't fail if it can't get a name (T2).

<!-- field: why -->
, not the ID token: an
authenticated call Kuria already makes the right way, no JWT to parse, and
the `sub` it answers is checked against the verified access token. A
sign-in doesn't fail if it can't get a name (T2).

<!-- akt: PLAN-002-D09 -->
---
type: decision
revision: 2
rank: b0H
state: decided
in: [PLAN-002]
title: Subscription tests start from a cursor
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Subscription tests start from a cursor** (`after: 0`) wherever they can,
so they don't depend on how soon a subscription joins its hub; one
intermittent failure under a heavily parallel run showed that they did
(T2).

<!-- field: why -->
(`after: 0`) wherever they can,
so they don't depend on how soon a subscription joins its hub; one
intermittent failure under a heavily parallel run showed that they did
(T2).

<!-- akt: PLAN-002-D10 -->
---
type: decision
revision: 2
rank: b0I
state: decided
in: [PLAN-002]
title: Input and stored values are read apart
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Input and stored values are read apart** (`Value::from_input` and
`Value::from_json`), since they differ exactly where the server supplies
part of a value; the client can't send a checkbox's record (T3).

<!-- field: why -->
(`Value::from_input` and
`Value::from_json`), since they differ exactly where the server supplies
part of a value; the client can't send a checkbox's record (T3).

<!-- akt: PLAN-002-D11 -->
---
type: decision
revision: 2
rank: b0J
state: decided
in: [PLAN-002]
title: Checking a checked box changes nothing
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Checking a checked box changes nothing**, so the first record stays until
the box is cleared (T3).

<!-- field: why -->
, so the first record stays until
the box is cleared (T3).

<!-- akt: PLAN-002-D12 -->
---
type: decision
revision: 2
rank: b0K
state: decided
in: [PLAN-002]
title: An attachment value is its id as a plain string
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**An attachment value is its id as a plain string**, in input and in
storage, like a `user`; the metadata is joined in where it is shown (T4).

<!-- field: why -->
, in input and in
storage, like a `user`; the metadata is joined in where it is shown (T4).

<!-- akt: PLAN-002-D13 -->
---
type: decision
revision: 2
rank: b0L
state: decided
in: [PLAN-002]
title: Taking an attachment into a field needs the right to download it
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Taking an attachment into a field needs the right to download it**, so
no one can attach, and thereby read, someone else's unheld upload or an
attachment of an akt type they can't read (T4).

<!-- field: why -->
, so
no one can attach, and thereby read, someone else's unheld upload or an
attachment of an akt type they can't read (T4).

<!-- akt: PLAN-002-D14 -->
---
type: decision
revision: 2
rank: b0M
state: decided
in: [PLAN-002]
title: "Uploads are stored once by SHA-256 per tenant, but each upload is its own attachment, with its own…"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Uploads are stored once by SHA-256 per tenant, but each upload is its own
attachment**, with its own name and uploader (T4).

<!-- field: why -->
**Uploads are stored once by SHA-256 per tenant, but each upload is its own
attachment**, with its own name and uploader (T4).

<!-- akt: PLAN-002-D15 -->
---
type: decision
revision: 2
rank: b0N
state: decided
in: [PLAN-002]
title: "Downloads are always `Content-Disposition: attachment`"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**Downloads are always `Content-Disposition: attachment`** with `nosniff`
and a sandboxing CSP, so uploaded HTML can't run on Kuria's origin (T4).

<!-- field: why -->
with `nosniff`
and a sandboxing CSP, so uploaded HTML can't run on Kuria's origin (T4).

<!-- akt: PLAN-002-D16 -->
---
type: decision
revision: 2
rank: b0O
state: decided
in: [PLAN-002]
title: "`latest_by` compares stored values"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**`latest_by` compares stored values**, so two akter share a component
when their field's JSON is equal; "newest" is creation time, then id.
Akter without a value are each listed (T6).

<!-- field: why -->
, so two akter share a component
when their field's JSON is equal; "newest" is creation time, then id.
Akter without a value are each listed (T6).

<!-- akt: PLAN-002-D17 -->
---
type: decision
revision: 2
rank: b0P
state: decided
in: [PLAN-002]
title: "A recipe's field names hold if any listed akt type has them"
---

<!-- field: context -->
Recorded in the plan "Field Kinds and Release Artifacts", migrated from `.agents/plans/field-kinds-and-artifacts.md`.

<!-- field: decision -->
**A recipe's field names hold if any listed akt type has them**, as the
spec says; a listed akt type not yet defined doesn't count against it,
so documents can still be defined in any order (T6).

<!-- field: why -->
, as the
spec says; a listed akt type not yet defined doesn't count against it,
so documents can still be defined in any order (T6).
