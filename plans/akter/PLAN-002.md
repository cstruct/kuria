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
