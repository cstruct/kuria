---
number: PLAN-023
type: plan
revision: 7
rank: b5f
state: done
title: "Creating akter from boards, with parent boards"
owner: albin@mattsson.io
---

<!-- field: goal -->
A person working a board creates akter from it directly: each column offers to create a member of the akt types whose initial state it shows, inside the container. A board may have parent boards, other akter it links to by the link kind its type names as `board.parent`; an akt created from the board is created in each parent board that contains its type as well. In the release-management example a release candidate's parent board is a backlog, so a task created on a candidate's board lands in the backlog too. You know it works when, in `mise run example release-management`, the "+" on a candidate's "To do" column creates a task that shows in both the candidate and its backlog.

<!-- field: design -->
## Creating from a column

A column offers "+" for each akt type the container contains whose initial state the column names for it (`app.board-create`). With one such type it links to the new-akt page, `/types/<key>/new?in=<container>`; with several it is a menu. The page's Add menu keeps offering every contained type.

## Parent boards

`board.parent` in an akt type document names one of the link kinds the document declares (`board.parent`, refused otherwise). The akter an akt links to by that kind are its board's parent boards: set one by adding a link, as any link, from the akt's page or `kuria link`. The relationship is per akt, picked in the app, not derived. Creating an akt from the container's page, from a column or the Add menu, adds `?in=<parent>` for each parent board whose akt type contains the new akt's type, so the server creates it in all of them in one change (`container.create-inside`); the container stays the first `in`, and so its parent for `{parent}` numbering.

The server knows nothing of parent boards beyond validating `board.parent`: an API or CLI client that creates an akt says every container itself.

## The example

- `backlog_item` becomes `task` ("Task", `TASK-{seq:4}`).
- `release_candidate` declares a link kind `parent_board` ("Parent board" / "Parent board of") to `backlog`, and `board.parent: "parent_board"`.
- `task`, `release_candidate` and `release_notes` say `presentation.navigation: "more"`.

<!-- field: verification -->
- `items/config` unit tests: `board.parent` naming a declared link kind is accepted, any other refused.
- App unit tests for which types a column offers and which parent boards a creation adds.
- `mise run test` (the example bundles import, with the renamed type), `mise run lint`, and the end-to-end tests touching the example.

<!-- akt: PLAN-023-T01 -->
---
type: task
revision: 4
rank: b5g
state: done
in: [PLAN-023]
title: Spec board.parent and app.board-create
assignee: albin@mattsson.io
---

<!-- akt: PLAN-023-T02 -->
---
type: task
revision: 4
rank: b5h
state: done
in: [PLAN-023]
title: Validate board.parent in akt type documents
assignee: albin@mattsson.io
---

<!-- akt: PLAN-023-T03 -->
---
type: task
revision: 4
rank: b5i
state: done
in: [PLAN-023]
title: "Offer creating from board columns, into parent boards"
assignee: albin@mattsson.io
---

<!-- akt: PLAN-023-T04 -->
---
type: task
revision: 4
rank: b5j
state: done
in: [PLAN-023]
title: Rename backlog_item to task and give candidates a parent board in the example
assignee: albin@mattsson.io
---
