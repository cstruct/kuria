# Kuria Specs

This directory holds the Tracey-managed requirements for Kuria.

Use the vocabulary in [`../nomenclature.md`](../nomenclature.md).

Rules are defined with standalone `r[...]` markers. Implementation and tests
reference them from code comments as `r[impl ...]` and `r[verify ...]`, and
`mise run lint` checks that every reference names a rule.

Write a rule for behavior that other software items or people depend on and
that should stay true across refactors. Rationale and design choices belong in
[`../architecture.md`](../architecture.md), which is not normative.

Specs:

- [authentication](authentication.md): Kuria as a Konklav relying system, and
  how requests are authenticated
- [browser sessions](browser-sessions.md): signing browsers in through
  Konklav, session cookies, refreshing and cross-site requests
- [erasure](erasure.md): deleting what Kuria holds about a subject Konklav
  erases, without missing an erasure
- [akter](akter.md): akt types, fields, akt numbers, creating, editing and
  transitioning akter, akt revisions and change events
- [authorization](authorization.md): Kuria's actions in Konklav's catalog,
  and what each operation on akt types and akter requires
- [containers](containers.md): akter containing akter, memberships and
  their history, and creating an akt inside a container
- [ordering](ordering.md): one rank per akt, and moves
- [boards](boards.md): a container's members in configured columns
- [links](links.md): typed links between akter, declared by akt types
- [drafts](drafts.md): drafting a field, such as release notes, from the
  akter around it
- [changes](changes.md): publishing change events through Valkey, the
  `changes` subscription, catching up, and WebSocket connections
- [attachments](attachments.md): uploading and reading files, their storage,
  and SBOMs
- [live drafts](live-drafts.md): an akt's fields edited together, live,
  with Yjs, and revisions made only by releasing
- [versions](versions.md): releasing an akt's revision as a version that
  never changes, the release's reference check, and reading versions
- [PDFs](pdf.md): `markdown` fields as PDFs, of the living version or a
  version
- [clones](clones.md): a container and its members as files, cloned,
  staged, pulled and pushed with `kuria`, synced with live drafts
- [people](people.md): the names Kuria shows for users, from sign-in
- [contacts](contacts.md): people a tenant names before they are users,
  and linking them to the users they become
- [login page](login.md): Kuria's login page as Konklav's login
  application, and the proxy to Konklav it needs
- [frontend](frontend.md): serving the app, signing in from it, and views
  built from akt types
- [bundles](bundles.md): exporting and importing selected akt types, with
  or without akter, or a whole tenant, keeping numbers and history
- [command-line client](cli.md): `kuria`, signing in by a link, reading
  and editing akter as files, and the relay its sign-in uses
