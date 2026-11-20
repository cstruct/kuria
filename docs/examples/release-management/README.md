# Release management

An example configuration: thirteen akt types for running a product's
backlog and releases in Kuria, the test protocols its release candidates
are accepted on, and the problem reports and CAPAs (corrective and
preventive actions) that keep its quality. Nothing in Kuria knows these names; they are
ordinary akt type documents, and a tenant can change them or start from
others.

| Akt type | Number | Contains | Created in |
|---|---|---|---|
| [`backlog_item`](backlog_item.json), named Task | `TASK-0001` | | |
| [`epic`](epic.json) | `EPIC-001` | tasks, each in one epic | |
| [`backlog`](backlog.json) | `BACKLOG-001` | tasks, each in one backlog, and epics | |
| [`release`](release.json) | `RELEASE-2026-0001` | backlogs, release candidates, hotfixes, one set of release notes | |
| [`release_candidate`](release_candidate.json) | `RELEASE-2026-0001-RC-0001` | epics, tasks, artifacts and test protocols | a release |
| [`hotfix`](hotfix.json) | `RELEASE-2026-0001-HOTFIX-0001` | tasks and artifacts | a release |
| [`artifact`](artifact.json) | `ARTIFACT-00001` | | |
| [`release_notes`](release_notes.json) | `RELEASE-2026-0001-NOTES-1` | | a release |
| [`test_protocol`](test_protocol.json) | `TP-001` | test cases | |
| [`test_case`](test_case.json) | `TP-001-C01` | | a test protocol |
| [`problem_report`](problem_report.json) | `PR-0001` | | |
| [`capa`](capa.json), named CAPA | `CAPA-2026-0001` | its actions | |
| [`capa_action`](capa_action.json) | `CAPA-2026-0001-A01` | | a CAPA |

## How it fits together

- A task lives in one backlog and at most one epic, and in as many
  release candidates and hotfixes as ship it. (Its key stays
  `backlog_item`, since the [planning example](../planning/README.md)
  keys its own tasks `task`.) Every board it is on shows it
  in the same relative order, because they all order by rank: moving it in
  the backlog moves it in its epic too.
- A release candidate's parent board is a backlog. The candidate links to
  it as *Ships from*, which the form creating a candidate in a release
  asks for, offering the release's backlog. A task or epic created from
  the candidate's board, by its *To do* column, is created in the backlog
  too ([parent boards](../../spec/boards.md)). *Add from* the backlog adds
  several of its tasks to the candidate at once.
- A release is planned, tested, released, and finally closed when it is no
  longer maintained. Its release candidates and hotfixes are created inside
  it and numbered after it. It takes candidates only while planning or
  testing, and hotfixes only once released; once it is closed, its members
  are frozen.
- Replanning an item into another backlog is a transfer: one step, recorded
  on the item and both backlogs.
- Release candidates and hotfixes are accepted or deployed with evidence:
  the build, as a reference. A candidate is accepted only once its result,
  what its test runs add up to, is Passed; its page says why Accept waits,
  and how many runs have passed.
- A hotfix is a correction that can't wait: it is deployed first, with why
  it is low risk (how isolated the change is, what it could break, how it
  is rolled back) and without approval, and reviewed once it is stable,
  with `release:approve`. Until then it reads as Deployed, a warning; a
  release ends its maintenance only once every hotfix in it is reviewed.
- A candidate's test protocols live in it: each is a run, a copy of a
  template made in the candidate, shown on its board from draft to done
  beside the work it tests. A candidate takes and loses protocols only
  while it is testing; once accepted or rejected, its runs stay as they
  were ([test protocols](#test-protocols)).
- An artifact is what a build produced for one component: its version, the
  git commit it was built from, its container image by digest, and its SBOM
  (SPDX or CycloneDX, uploaded to `POST /attachments`). The build pipeline
  registers artifacts, and the release manager puts them into candidates
  and hotfixes. An artifact whose change is `removed` says that a candidate
  drops a component.
- A release names its release manager, and can only be released once
  *Go for release* is checked, which records who checked it and when. The
  release manager can check it for whoever gave the go, such as a head of
  product, who may be a contact rather than a user: "Go for release: Hedda,
  checked by Alice". Once that person signs in with the contact's email,
  verified, the release shows them as the user they became.
- The release notes are drafted from what the release's accepted candidates
  and deployed or reviewed hotfixes contain, including the items inside their epics,
  but not from its backlogs, whose items may not have shipped, nor from
  rejected candidates. The draft is then edited and approved.
- The release notes' *Artifacts* are drafted from the artifacts in the
  release's candidates and hotfixes: each component's newest artifact, and
  none for a component whose newest artifact is a removal. Every candidate
  counts, rejected ones too, so a component built once early and never
  rebuilt still ships; a candidate that must not ship a component removes
  it.

- Old work archives itself ([archiving](../../spec/archive.md)): a
  release once two newer releases are closed, no longer maintained, its
  candidates, hotfixes and notes with it, each candidate's test protocols
  and their test cases with the candidate, and a task 90 days after it is
  done. Templates, in no candidate, stay.
  Archived akter are read-only and left out of lists, boards and search
  until **Show archived**; unarchiving a release brings back what it took,
  and an akt someone unarchives by hand stays until archived by hand again.

- A document's `presentation` says how its akter are shown: an icon and a
  colour for the type (a rocket for a release, a flask for a candidate,
  a wrench for a hotfix, a package for an artifact, a purple clipboard
  for a test protocol, a blue flask for a test case, a red bug for a
  problem report, a teal shield for a CAPA), the
  fields on board cards (a task's kind and assignee, a candidate's
  build) and in lists (an artifact's change, version and image, a
  protocol's owner, a test case's testers), and the
  tone of each state (in progress is active, in review a warning, done,
  accepted, reviewed and approved a success, a deployed hotfix still to
  review a warning, rejected a danger). Titles
  come from each type's first required single-line text field, a
  candidate's from its label; a hotfix and release notes have none, and
  show their number. The sidebar lists the types by job, under Planning,
  Releasing, Testing and Quality (`group`). Release candidates and release
  notes are reached from inside releases, and CAPA actions from inside
  CAPAs, so it lists them under **Show more** (`navigation`). Pages offer each state's next step first (`primary`), a
  release's manager, a protocol's, CAPA's or action's owner and a problem
  report's reporter default to whoever creates them
  (`default`), and fields such as a release's date or a task's pull
  request are asked for only in the states they belong to (`states`).

## Problem reports and CAPAs

A problem report is a problem found in the product. A CAPA is the
corrective and preventive action taken when a problem is serious,
systemic or recurring: what happened or could happen, why, what is done
about it, and whether that worked. Their types use rules any akt type can
use ([spec](../../spec/akter.md)): transitions that wait for members and
links, fields required only for some values, locked fields, due dates and
reopening.

- **A problem report** lives in a backlog, on its board beside the tasks.
  - It is classified with the answer to "Could it lead to a hazardous
    situation?", its severity and its priority, and the risk record it is
    assessed against when the answer is yes or uncertain.
  - It is fixed by tasks, linked as *Fixed by*: *Mark fixed* needs at least
    one, and waits until every one is done.
  - *Won't fix* needs one of five reasons (cannot be reproduced, by design,
    unsupported version, risk acceptable, workaround documented) and a
    rationale.
  - *Accept as residual anomaly* needs a rationale and the risk record. The
    report stays open across releases, to be fixed later.
- **A CAPA** goes through *Open*, *Investigating*, *Planning*, *Implementing*
  and *Verifying*, and ends *Closed*, *Superseded* or *Not required*.
  - **Confirming it**, or deciding that none is needed (with why), is
    QA/RA's: `capa:confirm`. Confirming needs its severity, impact and risk
    record.
  - **Planning** needs the kinds of action it takes (correction, corrective,
    preventive), what is affected, and whether vigilance reporting and
    customer communication are needed. It also needs:
    - the root cause, for a corrective action;
    - the causal hypothesis and its trigger, for a preventive one;
    - the record of each report or notice, where one is needed.
  - **Starting implementation** needs the effectiveness check and its
    acceptance criteria. From then on they are locked: changing them needs a
    reason, which the history keeps.
  - **Verifying, and closing,** wait until every one of its actions is done
    or dropped; the page names those still open. Closing is QA/RA's
    (`capa:approve_closure`), and needs the effectiveness results, the
    adverse impact check, and an outcome:
    - completed;
    - not completed by its due date, and reopened;
    - ineffective, and reopened.
  - **Superseding** needs a *Superseded by* link to the CAPA that takes over.
  - **Reopening** a closed CAPA, back to *Implementing*, is QA/RA's. It needs
    the reason, the remaining actions, a revised target, the owner's
    confirmation and the impact of the delay. Closed and reopened, a CAPA
    keeps both in its history. A second reopening finds the first's values
    still there: change them to say what is true now.
  - **Its links:** *Originated from* the problem report it came from (*Led
    to* on the report), and *Verified by* the test cases or protocols that
    show it worked.
- **A CAPA's actions** are created in it, numbered after it, and shown on its
  board: *Planned*, *In progress*, *Done* or *Dropped* (with why).
  - Each is a correction, a corrective or preventive action, or a
    verification, with an owner, a due date and the evidence it must leave.
  - Its due date, once passed while the action is open, shows as overdue on
    cards, in lists and on its page, and the list of CAPA actions filters to
    those overdue. Once the action has started, the due date moves only with
    a reason.
  - An action is *Implemented by* the tasks, hotfixes or release candidates
    that do it, and finishes, with its evidence, only once they are done.

### A CAPA, in the app

1. On the problem report, choose **New CAPA** under its links. The new
   CAPA's page starts with what the report says, for each field both have,
   and is linked to it once created.
2. QA/RA chooses **Confirm CAPA**, and the owner plans it: **Plan actions**
   asks for the root cause only when it is corrective.
3. Under the CAPA's board, create its actions, and on each action, link what
   implements it, or choose **New Task** under its links.
4. **Start implementing**, then **Verify effectiveness** once every action
   is done. **Close** says which actions are still open while it waits.

### A CAPA, from the command line

```sh
kuria new capa --link originated_from:PR-0001 --carry \
  --set title="Sessions lost at every deploy" --set problem=@problem.md
kuria do CAPA-2026-0001 confirm --set severity=major \
  --set impact="No safety impact" --set risk_record=https://qms.shop.example/risks/RA-12
kuria new capa_action --in CAPA-2026-0001 --set title="Hotfix 4.2.1" \
  --set kind=correction --set due=2026-04-15
kuria link CAPA-2026-0001-A01 implemented_by RELEASE-2026-0001-HOTFIX-0001
kuria ls capa_action --overdue                      # actions past their due date
kuria set CAPA-2026-0001-A02 due=2026-07-15 --reason "Waiting on the pipeline team"
kuria show CAPA-2026-0001                           # what Verify and Close wait for
kuria do CAPA-2026-0001 reopen --set reopen_reason=@why.md …
```

## Test protocols

A test protocol is a set of test cases laid out in sections. It is kept as
a template and copied into each release candidate for a run, where testers
record their verdicts and the outcome follows. They use the `sections`,
multiple `user` and `verdict` field kinds and copying, which any akt type
can use ([spec](../../spec/akter.md)).

- **A test case** says what to do (steps) and what should happen (expected
  result), and who runs it (testers, any number of users or contacts).
  Each tester gives it a verdict, `pass`, `fail` or `skip`, for
  themselves, with a description of what they saw, which a fail or a skip
  must have; anyone who may edit it can give one for a tester.
- **Its outcome** follows from its testers' verdicts: it fails if any
  tester failed it; otherwise it is pending until every tester has
  answered (or while it has none); it is skipped if all skipped it, and
  passes otherwise. Outcomes are worked out when read, never stored, so
  they never make a revision.
- **A test protocol** holds its test cases, numbered after it, and lays
  them out in sections (its layout): each section with an optional title,
  Markdown text, and whether it is collapsible, nested as deep as needed.
  A section's outcome, and the protocol's, follow from the test cases
  they hold by the same rules: fail if any fails, pending while any is
  pending or there are none, skip if all skip, else pass.
- **Templates:** a protocol is drafted and then kept as a template, in no
  candidate. A run is a copy of it made in a release candidate: a new
  protocol in draft, with copies of the test cases chosen (the sections
  that hold them come along, and empty ones are left out), each with its
  steps, expected result and testers, and no verdicts. The copy says what
  it was copied from. A run is started, and finished with its conclusion
  once its outcome is `pass`, `fail` or `skip`: while any test case is
  pending, it can't be finished (`"outcomes"` on the `finish`
  transition). It can be withdrawn before then.
- **In a candidate:** a copy is made where it is told to be, else where
  the akt copied is. Copying a template with `--in` a candidate makes the
  run there; in the app, **Make a copy** of a template makes the run
  outside any candidate, to be added to one as a member. A protocol can
  also be written in a candidate from scratch, with the "+" on its
  board's *To do* column.
- **A tester's work:** test cases naming a tester among their testers are
  in the tester's "My work" on Home, and `kuria ls test_case --where
  testers=@me` lists them. The test cases still to run are those whose
  outcome is pending: the list of test cases filters by outcome, as does
  `kuria ls test_case --outcome pending`.
- **Changing a run:** add a test case to it and place it in a section,
  move test cases between sections, or remove one, as with any akt;
  verdicts belong to the test case, not to its section.

### A run, in the app

1. Open the template and choose **Make a copy**, the page's first
   action on a template. Choose the candidate it is for under **Copy
   into**, the most recently changed open one at first. Every section and
   test case is chosen at first; clear the ones this run leaves out, then
   **Copy**. The copy opens, in the candidate, named after it, such as
   "Shop acceptance test: 4.2.0-rc.1".
2. **Start** it. Its test cases are listed in their sections, collapsible
   ones closed until opened and saying how many they hold, each with its
   testers, their verdicts and its outcome. A template, or a run not yet
   started, takes no verdicts.
3. Each tester gives their verdict in place, beside the test case:
   **Pass** at once, or **Fail** or **Skip** with a description of what
   they saw, which a fail or a skip needs before it is saved. Choosing
   again changes it, and **Clear** takes it back. The outcome of the test
   case, its sections and the protocol follow.
4. **Finish** it with a conclusion. While a test case is pending,
   **Finish** is unavailable, and says why beside it.

A tester finds their test cases under **My work** on Home, and those
still pending on the test cases' list, with **Outcome** set to Pending.

### A run, from the command line

```sh
kuria copy TP-001 --in RELEASE-2026-0001-RC-0002 --reason "Run for 4.2.0-rc.2"
kuria copy TP-001 --only TP-001-C01 --only TP-001-C03   # only some test cases
kuria do TP-002 start
kuria verdict TP-002-C01 pass                         # my own verdict
kuria verdict TP-002-C02 fail --description "The total is wrong"
kuria verdict TP-002-C01 skip --description "Away" --for ben@shop.example
kuria show TP-002                                     # sections and outcomes
kuria ls test_protocol --in RELEASE-2026-0001-RC-0002 # the candidate's runs
kuria ls test_case --in TP-002                        # with an Outcome column
kuria ls test_case --where testers=@me --outcome pending  # what I have left
kuria do TP-002 finish --set conclusion=@conclusion.md
```

`--only` names the test cases to keep; the sections holding them come
along. In a file (`kuria cat`), the layout is a section of YAML, testers a
list, and verdicts and the outcome are read-only:

```markdown
---
number: TP-002
type: test_protocol
revision: 7
state: running
outcome: fail
title: Checkout regression
---

<!-- field: layout -->
- title: Signing in
  items:
  - TP-002-C01
- title: Paying
  text: Use the test card 4242 4242 4242 4242.
  items:
  - TP-002-C03
```

The same works in an akt directory, without a server (`--dir`).

## Trying it in a browser

```sh
mise run e2e
```

walks this example in Kuria's app ([`e2e/`](../../../e2e/README.md)): a
developer and a release manager sign in, plan a release, work two
candidates and a hotfix, give the go for a contact, and draft the notes
and the artifacts that ship; then releases go on until the first archives
itself with its candidates. Another walk copies a test protocol from its
template, runs it as two testers and finishes it. A third escalates a
problem report to a CAPA, and works the CAPA from its confirmation, through
its actions and the hotfix that is its correction, to its closure.

## Defining it

Submit each document with `defineAktType`. Their order doesn't matter:
akt types name each other by key.

```graphql
mutation($document: JSON!) {
  defineAktType(document: $document) { key version }
}
```

## With example data

[`bundle/`](bundle/) is an example bundle of these akt types with akter:
two releases of a web shop: spring's, worked through a rejected and an
accepted candidate with the pipeline's artifacts and an SBOM, each with
its run of the acceptance test, copied from the template, one failed and
one passed, with verdicts from two testers, approved notes, the go given
for the head of product and a deployed and reviewed hotfix, then closed;
and summer's, being planned, with a wish-list epic under way and a test
protocol in draft. Two CAPAs came of spring:

- one for the sessions lost at every deploy, from a problem report fixed by
  the hotfix's task. The hotfix is its correction, a summer task its
  corrective action, overdue, and a new acceptance test case verifies it;
  it is still being implemented.
- one for slow searches, preventive, closed as ineffective and reopened.

Devi Larsen (`dev@shop.example`), Rami Moberg (`rm@shop.example`), Hedda
Lund (`hedda@shop.example`) and Ines Berg (`qa@shop.example`), QA/RA, wrote
them, the people the end-to-end seed signs in as. To start a server with it, set

```sh
KURIA_BOOTSTRAP_BUNDLE=docs/examples/release-management/bundle
```

and the server imports it into its default tenant (or
`KURIA_BOOTSTRAP_TENANT`) before it serves; the tenant must hold nothing
yet ([`items/server`](../../../items/server/README.md#running)).
`kuria import docs/examples/release-management/bundle` takes it to a running server's
tenant that hasn't defined these types.

```sh
mise run example release-management
```

serves it on <http://localhost:18290> to its people, who sign in as
themselves, with the app hot-reloading ([`example/`](../../../example/README.md)).
[`docs/examples`](../README.md) says how the bundles are made.

## Konklav roles

Kuria's actions and its scope kind must be in Konklav's catalog (see the
server's README), together with the action these documents name:

- `release:approve`, which releasing, reviewing a hotfix and approving
  release notes require;
- `capa:confirm`, which confirming a CAPA, deciding none is needed, and
  reopening one require;
- `capa:approve_closure`, which closing and superseding a CAPA require.

Two roles cover the usual split:

- **Developer**, tenant-wide: `kuria:akter:read`; and at `backlog`, `epic`,
  `backlog_item` and `problem_report`: `kuria:akter:create`,
  `kuria:akter:edit` and `kuria:akter:transition`.
- **Release manager**, tenant-wide: `kuria:akter:read` and
  `kuria:contacts:manage`, to name approvers who aren't users yet; and at
  `release`,
  `release_candidate`, `hotfix` and `release_notes`: `kuria:akter:create`,
  `kuria:akter:edit`, `kuria:akter:transition`, `kuria:akter:archive`, and
  `release:approve`; and the developer's at `backlog`, `epic` and
  `backlog_item`, to plan candidates from their backlogs.
- **Test lead**, tenant-wide: `kuria:akter:read`; and at `test_protocol`
  and `test_case`: `kuria:akter:create`, `kuria:akter:edit` and
  `kuria:akter:transition`, to keep templates and run them; and
  `kuria:akter:edit` at `release_candidate`, to put runs into candidates.
  A release manager with the test lead's at `test_protocol` and
  `test_case` needs nothing more.
- **QA/RA**, tenant-wide: `kuria:akter:read`; and at `capa`, `capa_action`,
  `problem_report` and `test_case`: `kuria:akter:create`,
  `kuria:akter:edit`, `kuria:akter:transition`, `kuria:akter:archive`,
  `capa:confirm` and `capa:approve_closure`. A CAPA's owner who isn't QA/RA
  needs create, edit and transition at `capa` and `capa_action` to work it,
  and links to tasks need `kuria:akter:read` at `backlog_item`.
- **Tester**: `kuria:akter:read`, and `kuria:akter:edit` at `test_case`,
  to give verdicts. A verdict is an edit of the test case: only its
  testers give their own, and anyone who may edit it can give one for a
  tester.

And one for software:

- **Build pipeline**, at `artifact`: `kuria:akter:read` and
  `kuria:akter:create`. It uploads SBOMs and registers artifacts holding
  them; only it can attach its uploads until an artifact holds them, and
  then whoever reads artifacts can download them.

A release manager can put tasks into release candidates and hotfixes,
since that needs `edit` at the container and `read` at the task, and
create them from a candidate's board, which needs `create` at the task
and `edit` at the candidate and its backlog. A developer can't create a
release candidate.
