# Requirements

This document lists what Kuria has to support, as capabilities rather than features of any particular process. It is drawn from the kind of procedures a regulated quality management system (for example under ISO 13485, ISO 14971, IEC 62304 and EU MDR) runs today, much of it on general-purpose issue trackers and document stores.

Nothing here names a specific organization's process. Each capability is stated so that a process can be configured from it, and the examples show the processes it has to cover.

Terms follow [nomenclature.md](nomenclature.md). Terms that are not defined there yet are written in plain words.

## Coverage so far

What Kuria implements today, with the specs that hold the rules. Everything
else in this document is still to come.

| Area | Covered | Specs |
|---|---|---|
| Akt types and fields | Akt types as runtime documents, versioned; the field kinds `text`, `choice`, `choices`, `date`, `user` (one person, or several), `checkbox` (who checked it, and for whom), `reference`, `git`, `docker`, `attachment`, `sections` (members laid out in nested sections) and `verdict` (each assignee's pass, fail or skip, with outcomes derived from them); required fields | [akter](spec/akter.md) |
| Attachments | Uploads in S3-compatible storage, immutable and stored once per tenant; SPDX and CycloneDX SBOMs recognised and summarised; downloads to readers of the akter holding them | [attachments](spec/attachments.md) |
| People | Users shown by the names recorded at sign-in; contacts named before they are users, linked to them at sign-in by a verified email | [people](spec/people.md), [contacts](spec/contacts.md) |
| Workflow | States, transitions with required fields, final states, and an extra permission per transition | [akter](spec/akter.md), [authorization](spec/authorization.md) |
| Links and nesting | Containment: many containers per akt, exclusivity, no cycles, frozen final containers, accepting members only in some states, creating inside containers, transfers | [containers](spec/containers.md) |
| Ordering and boards | One priority order per tenant, moves, boards with configured columns | [ordering](spec/ordering.md), [boards](spec/boards.md) |
| Numbering | Number patterns with `{year}`, `{month}`, `{parent}` and a sequence | [akter](spec/akter.md) |
| Records and integrity | Append-only revisions with author, reason and a state hash; membership history; change events | [akter](spec/akter.md), [containers](spec/containers.md) |
| Templates | Any akt copied, with the part of its members chosen, into new akter of the same types | [akter](spec/akter.md) |
| Drafting | Fields drafted from the akter around them, such as release notes, also the newest akt per value of a field, leaving out exclusions, such as the artifacts a release ships | [drafts](spec/drafts.md) |
| Live updates | Change events published through Valkey, a `changes` subscription over WebSocket, catching up from a cursor | [changes](spec/changes.md) |
| Identity and access | Konklav authentication, browser sessions, authorization per akt type and transition, erasure of sessions, names and linked contacts | [authentication](spec/authentication.md), [browser sessions](spec/browser-sessions.md), [authorization](spec/authorization.md), [erasure](spec/erasure.md) |

The release, backlog and epic processes run on this today, with build
artifacts, SBOMs and a go for release given by someone who need not be a
user: see [the release-management example](examples/release-management/README.md).
Test protocols run on it too, kept as templates and copied for each run,
with testers' verdicts adding up to each test's, section's and protocol's
outcome: see [the test-protocols example](examples/test-protocols/README.md).
Not yet: other link kinds and traceability, signatures, structured tables,
computed fields, deadlines, obligations, reporting and the frontend.

## Processes to cover

Kuria must be configurable, without code changes, to run at least these processes as akt types:

| Process | Typical akt types | What makes it hard |
|---|---|---|
| Problem resolution | problem report, anomaly, residual anomaly | Routing by classification; "won't fix" needs a reason from a fixed list; residual anomalies stay open across releases |
| Issue classification | (part of any akt that reports a problem) | A screening question, then severity and probability give a risk category; classification is incomplete until linked to a risk record |
| CAPA | CAPA, action | Several actions of different kinds under one CAPA; effectiveness check before closure; closure outcomes that reopen; due dates that may not be moved |
| Change management | change, impact analysis, design review | One impact assessment per area (safety, clinical, usability, technical, regulatory, post-market), each with a rationale; activities split by release scope |
| Verification and validation | V&V plan, test protocol, test record, V&V report | A protocol is a template of test cases; a record is one run of it against one version; findings become anomalies |
| Software development | backlog item, release candidate | Each work item must trace to the akt that justifies it; review evidence tightens stage by stage as code nears a release; most evidence lives in the code host |
| Release and deployment | release, deployment | Gates on everything above; approval by named roles with a fallback chain; links to build artifacts outside Kuria |
| Operations | maintenance task, incident | Acceptance criteria fixed before work starts and verified in each environment in turn; partial success accepted only after classification |
| Complaints and support | support ticket, complaint, feedback | Triage into complaint, feedback or potential vigilance event; response SLAs; escalation to problem report, CAPA or vigilance |
| Vigilance | vigilance case, report, field safety notice | A Day 0 that cannot be moved; reporting deadlines chosen by classification; several submissions per case |
| Risk management | risk plan, hazard, hazardous situation, risk control, risk report | Rows that nest; scores computed through a configured matrix; some outcomes can never be accepted; a control can introduce new risks |
| Requirements | use, system, software, security and risk-control requirements | Each must be testable and trace to a source; coverage by verification must be provable per release |
| Threat modeling | threat model, threat | A register per version; "no change" conclusions recorded per release |
| Usability | task, use error, evaluation plan, evaluation record | Rows that link to risks and scenarios; partial acceptance per workflow |
| Internal audit | audit program, audit, finding, action | Every process covered within a cycle; deadlines by finding class; auditors may not audit their own work |
| Management review | review, action | Inputs aggregated from every other process; a pinned version of the documents reviewed |
| Supplier evaluation | supplier, evaluation, supplier nonconformity | A weighted scorecard; outcome from score thresholds; re-evaluation due by tier; conditional approvals that expire |
| Post-market surveillance | PMS plan, PMS review, periodic report | Metrics against thresholds; periodic reports on a regulatory schedule |
| Competence | role, appointment, training record | Appointments and competence gate who may sign, review or audit |

## Akt types and fields

- An akt type defines its fields at runtime. Field kinds needed so far:
  - text and rich text (descriptions, rationales, root cause)
  - a single choice from a list (severity, priority, finding class)
  - several choices from a list (labels, affected areas)
  - an ordered scale whose levels carry both a rank and a number (severity 0–4; probability levels each mapped to a numeric probability)
  - number, and score with a weight (supplier scorecards)
  - date and due date
  - a subject or a role (reporter, owner, tester, approver)
  - a reference to an external artifact by URL or identifier (pull request, commit SHA, image digest, pipeline run, document, submission)
  - attachments (screenshots, logs, datasets, signed PDFs)
  - a version of something (software version tested, version a release ships)
  - a structured table: repeated rows with their own fields (test case results, per-area impact assessments, documents affected)
- Fields can be required, optional, or required only in some states ("root cause is required before leaving investigation").
- Fields can be filled automatically: identifier, creation date, reporter.
- Fields can be derived from other fields:
  - a matrix lookup: severity and probability give a score and a category (acceptable, investigate, unacceptable)
  - a formula: a risk priority number from several probabilities and a severity; a weighted total from a scorecard
  - a threshold: a supplier outcome from its weighted score and any failed critical gate
  - a rule over several fields: hotfix eligibility from severity, risk acceptability and priority
- Matrices, scales and thresholds are configuration of their own. They can differ per product or scope, they are versioned, and changing one needs approval.
- A field can require a rationale whenever it takes certain values: "not applicable" needs a justification, "won't fix" needs one of a fixed list of reasons plus free text, and "accepted" or "no action" always needs a stated reason. Silence never counts as acceptance.
- An akt type can be a template for others. A test protocol defines the cases and expected outcomes, and each test record copies them and adds the results.
- Every akt type has its own stable, human-readable identifier scheme.
- Some akt types hold reference data rather than cases: suppliers, roles, KPIs, plans. They use the same machinery.

## Workflow

- Each akt type has its own states and allowed transitions, defined at runtime.
- A transition can have:
  - **conditions**: required fields filled; linked akter in certain states (no release-blocking anomaly open, every action under the CAPA done, all included issues closed, documents signed); a link of a given kind present (classification is incomplete until linked to a risk record)
  - **prohibitions** that no rationale overrides (an unacceptable risk cannot be accepted on benefit-risk grounds alone)
  - **permissions**: only certain roles may perform it (QA/RA confirms whether a CAPA is needed and approves its closure)
  - **separation of duties**: the reviewer and the approver must be different subjects; an auditor may not audit their own work; when one person holds several roles, a documented check is needed
  - **signatures**: one or more electronic signatures from given roles, each with a stated meaning (reviewed, approved, released), bound to the exact version of the akt that was signed; boards where several parties must approve
  - **a required outcome**: closing a CAPA picks one of completed; not completed by due date and reopened; ineffective and reopened; superseded by a linked CAPA
- Reopening is a normal transition and keeps the full history. A reopen can require its own fields (reason, remaining actions, revised target, impact of the delay).
- Some states change what an akt means without closing it: a problem report can become a residual anomaly, accepted into a release with a risk evaluation.
- States can be time-bound: a conditional approval or a temporary deviation expires on a date and then escalates or reverts.
- Partial acceptance: each row of a structured table can be dispositioned on its own (one workflow of a usability evaluation passes, another is retested).
- Routing: a value set in one step decides which path the akt takes next, or which other akt type has to be opened (Major or Critical with unacceptable risk → CAPA; complaint with suspected serious incident → vigilance). Opening that akt carries fields over and links it back.
- When the route is unclear, the configuration can say which route is the safe default.
- Retrospective completion: an akt can pass a gate early (an urgent correction is deployed first) on condition that the skipped steps are completed and signed afterwards. The outstanding obligation stays visible until then.
- Obligations created by a transition: deploying a hotfix creates the obligation to bring the fix back into the main line; accepting "no negative impact observed" creates a follow-up akt to review the metrics later, with its own due date.
- Fields can be locked from a given state onward (acceptance criteria are fixed before implementation starts). Changing a locked field is still possible where the configuration allows it, but needs a recorded reason.
- Repeated steps: the same verification can be required once per row of a list, such as once per environment in a configured order (operations, development, staging, each production environment), and the akt advances only when each row is verified or dispositioned.
- Evidence requirements can depend on a stage the akt is in, and grow stricter as it nears release: review by one other person early on; links to change, risk and problem records later; formal approval at release.
- "No impact" is an answer that has to be given: for each area a change could affect (architecture, security, labelling, risk), either a linked update or an explicit no-impact conclusion is required, never a blank.
- A work akt must link to the akt that justifies it (a change, a problem report, a CAPA, a planned initiative), and that governing akt cannot close until the work linked to it is complete or dispositioned.

## Links and nesting

- Akter link to each other with typed links defined at runtime. Kinds needed so far:
  - parent and child, nested to any depth (CAPA → actions; epic → stories; hazard → hazardous situation → sequence of events; V&V report → test records)
  - originated from (CAPA from complaint, problem report from a test record finding, new risk from a risk control)
  - escalated to (support ticket → complaint → vigilance case → CAPA)
  - verifies (test case → requirement; test record → release)
  - implements control (requirement → risk; requirement → threat)
  - traces to source (requirement → intended use, regulation, risk)
  - included in (change or problem fix → release)
  - superseded by (CAPA → CAPA; requirement revision → its successor)
  - affects version (anomaly → released version)
- Links can be required by the workflow, and the target's state can gate the source's transitions.
- Links can cycle (a control introduces a new risk that needs its own control) and can cross products (a workflow shared between systems).
- A link can point to a specific version of its target rather than the latest (the version of a register a review used, the requirements a release shipped).
- Links to things outside Kuria (pull requests, commits, documents in another system) are first class, so that evidence can be cited rather than copied.

## Development tools and other systems

- Much of the evidence lives in the code host and the delivery pipeline: pull requests and their reviews, commits, CI results, release candidates, built artifacts, image digests, software bills of materials, deployment logs. Kuria references this evidence and does not copy it, and it must stay readable for audits after the source system changes.
- Events from those systems can drive akter: a merged pull request, a passing pipeline or a finished deployment can satisfy a condition or perform a transition. A review approval in the code host can count as evidence, but a signature that the configuration requires is still given in Kuria.
- A release akt records the identifiers that make it reproducible: product version, source commit or tag, release candidate, deployed artifacts, dependency evidence and deployment records.
- Other systems can open akter through an API: monitoring alerts open incidents, dependency scanners open vulnerability findings, a support inbox opens tickets. Duplicate events must not open duplicate akter.
- Reference akter describe what records are about: products and their versions, software items, environments, third-party components, suppliers. Other akter link to them instead of using free-text labels.

## Traceability

- Traceability must be answerable from links alone, as matrices and as gap reports:
  - requirement → test case → test record that passed it, per release
  - risk → control → requirement → verification
  - threat → risk → requirement
  - usability task → risk → evaluation scenario
  - release → every change, fix and anomaly accepted into it, and the evidence behind each
  - pull request → backlog item → the change, problem report or CAPA that justified it
- A gap (a requirement in scope with no passing record, a risk with no verified control) can block a transition.

## Deadlines and schedules

- An akt can have due dates that are set once and then tracked. Moving one is a recorded event with a reason, and the configuration can forbid it.
- A deadline can be relative to an anchor (Day 0 + 2, 10 or 15 days, depending on the classification). An anchor that has been set cannot be moved later without a recorded reason.
- Deadlines count calendar days or business days, as configured.
- A deadline can send reminders before it falls due, escalate to a role when it is missed, or perform a transition (auto-close a support ticket after N days without a reply).
- Overdue is a visible condition that transitions can depend on.
- Recurring schedules create or flag work: annual reviews, re-evaluation every 12 or 24 months by tier, every process audited within a 36-month cycle, periodic reports at a regulatory frequency. An ad hoc trigger (a signal, a change, an incident) can advance a schedule.

## Records and integrity

- Every change to an akt is kept: who, when, what changed, in which transition, and the reason. The history is append only.
- Once an akt reaches a state the configuration marks final (a signed test record, an approved release), its fields cannot change. Corrections happen through a new version or a linked akt, never by editing in place.
- Records are retained for a configured period (ten years is typical), including superseded versions. They can be exported in full, with history, signatures and links, for an audit or a regulatory submission.
- Backups and exports must be possible independently of Kuria's own availability.
- Some records contain health information or personal data and need access restricted beyond the akt type's general permissions.

## Reporting

- Queries over akter by any field, state, link and date, with saved views and boards.
- Metrics per akt type: volume and severity over time, distribution by component or label, time to first response and to resolution, escalation rate, recurrence. Thresholds on a metric can raise a signal that opens an akt.
- Periodic reports generated from akter and links: a release's accepted anomalies and verification coverage, a post-market surveillance summary, the input pack for a management review, the status of an audit program.

## Identity and access

- Authentication and authorization come from Konklav. Roles named in the configuration (QA/RA, product management, approver) map to Konklav roles.
- Tenants come from Konklav. Akt types and their configuration belong to a tenant.
- Whether someone may perform a transition can depend on an appointment or competence record, not only on a role.
- A role can be held by a stand-in when the usual holder is absent, through a configured fallback chain, and the record shows who acted in which capacity.
