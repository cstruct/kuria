---
number: PLAN-024
type: plan
revision: 7
rank: b5k
state: done
title: Verdicts reported with a description
owner: albin@mattsson.io
---

<!-- field: goal -->
A tester reports their verdict on a test case with what they saw, rather than pressing Pass, Fail or Skip in passing: **Report** opens a dialog with a status (pass, fail or skip) and a description, which a fail or a skip must have. Once reported, **Edit** opens the same dialog to change the status or the description, or to delete the verdict. The description is part of the verdict, kept per assignee, shown beside it, and given from the CLI too. It works when a fail without a description is refused by the API, the app and the CLI alike, and the test-protocols end-to-end test reports, edits and deletes verdicts through the dialogs.

<!-- field: design -->
## The verdict

A verdict gains an optional `description`: text saying what the assignee saw. `akt.field.verdict` requires one, not blank, for `fail` and `skip`; `pass` may have one. Giving a verdict takes `pass` alone, or `{"verdict": …, "description": …, "for": …}`; the input says the whole verdict, so giving `pass` over a pass with a description drops the description. Giving the very same verdict and description again changes nothing, and keeps who gave it first; anything else records who changed it and when. Taking one back is unchanged (`{"clear": true}`).

Stored verdicts hold `description` only when there is one, so verdicts given before read as they were, a fail without a description included: the requirement is on giving, in `engine` (`verdicts::Giving`), which the server and the CLI's local mode share.

## API and CLI

`GivenVerdict.description: String`. `kuria verdict <number> pass|fail|skip|clear [--description <text>] [--for <user>]`. An akt file's read-only verdict mapping gives an assignee with a description as `{verdict, description}`.

## The app

`VerdictButtons` shows each assignee's verdict and description, and for the viewer, if an assignee, one button: **Report** while they have none, **Edit** once they do. Both open `VerdictDialog`: status as three radio buttons, a description, required for fail and skip, and, when editing, **Delete**. Saving is the same field edit as before.

## The example

Test cases in the release-management example lose their `notes` field: what was seen is the verdict's description.

<!-- field: verification -->
- `items/engine` unit tests: fail and skip refused without a description, a description changed, the same verdict given again a no-op, older verdicts read.
- `items/server/tests/templates.rs` through GraphQL; `items/cli/tests/local.rs` for `kuria verdict --description`.
- App unit tests for the input a report makes; `e2e/tests/test-protocols.spec.ts` reports, edits and deletes verdicts in the dialogs, with axe on the dialog.
- `mise run lint`, `mise run test`, `mise run e2e`; the example bundle written again.

<!-- akt: PLAN-024-T01 -->
---
type: task
revision: 4
rank: b5l
state: done
in: [PLAN-024]
title: "Spec, engine and API: a verdict's description, required for fail and skip"
assignee: albin@mattsson.io
---

<!-- akt: PLAN-024-T02 -->
---
type: task
revision: 4
rank: b5m
state: done
in: [PLAN-024]
title: "CLI: kuria verdict --description, and descriptions in akt files"
assignee: albin@mattsson.io
---

<!-- akt: PLAN-024-T03 -->
---
type: task
revision: 4
rank: b5n
state: done
in: [PLAN-024]
title: "App: Report and Edit dialogs in place of the verdict buttons"
assignee: albin@mattsson.io
---

<!-- akt: PLAN-024-T04 -->
---
type: task
revision: 4
rank: b5o
state: done
in: [PLAN-024]
title: "Example, bundle, end-to-end test and docs"
assignee: albin@mattsson.io
---
