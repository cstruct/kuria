# Kuria Plan Standard

This file defines the shared standard for planning work in this repository. Plans live in Kuria itself, as akter of the planning example's akt types ([docs/examples/planning](../../docs/examples/planning/README.md)), read and written with the `kuria` command-line client. This file is the rubric; it is not a plan.

## Where Plans Live

- **Locally:** the dogfooding stack keeps its data between runs ([dogfood/README.md](../../dogfood/README.md)). Start it with `mise run dogfood:serve`; agents then run `mise run dogfood:kuria -- <command>`, which signs in as the seeded agent. `mise run dogfood:kuria -- guide` explains the commands, the akt file and the exit codes.
- **What a plan is:** a `plan` akt (`PLAN-007`) containing `task`, `gap` and `decision` akter (`PLAN-007-T03`, `PLAN-007-G01`, `PLAN-007-D01`), each created in it.
- **The Markdown plans** that came before (`.agents/plans/<topic>.md`) were moved into Kuria by `dogfood/scripts/migrate.ts`. Don't start new ones.

## Purpose

A plan should let a new contributor, or a coding agent with no prior context, understand why the work matters, what the intended end state is, how to implement it safely, and how to prove it works.

Plans are required for work that is likely to span multiple commits, multiple sessions, or multiple design and implementation steps. Small, obvious, single-file edits do not need a plan unless the user asks for one.

## Core Principles

Every plan must be:

- Self-contained enough that a newcomer can follow it without needing hidden context from chat history.
- Outcome-focused, not just a list of code edits.
- Living: updated as decisions, discoveries, and progress change.
- Spec-first: requirements and design should be clarified before implementation is treated as complete.
- Verifiable: it must say how to observe the intended behavior or otherwise prove the change works.

When a plan depends on prior work, restate the important assumptions instead of relying on memory, and link to the relevant repository files or other plans by number.

## When To Use A Plan

Create or update a plan when the work involves any of the following:

- A complex feature or substantial refactor.
- A migration that spans multiple files, layers, or milestones.
- Design uncertainty that should be resolved before or during implementation.
- Work that is expected to continue across multiple sessions.
- User-visible behavior, data model, persistence, or API changes that need explicit acceptance criteria.

If there are several concurrent efforts, prefer several focused plans over one overloaded one.

## What A Plan Holds

| Concern | Where it goes |
|---|---|
| Title | the plan's `title` |
| Purpose or goal: what becomes possible, and how anyone will know | `goal`; activating a plan requires it |
| Current direction, constraints, design | `design`, in Markdown, with its own headings |
| Verification or testing strategy | `verification` |
| Tasks or milestones | `task` akter in the plan, worked in its order; `blocks` links say what comes first |
| Progress | each task's state (to do, doing, in review, done, dropped), on the plan's board |
| Known gaps and open questions | `gap` akter, closed with their resolution |
| Decision notes | `decision` akter, decided with their context, the decision and why; a later one supersedes an earlier one |
| Updating the plan | edits, transitions and their reasons, kept in each akt's history |

## Writing Guidance

Start by explaining what becomes possible after the work is done and how someone will know it succeeded. Prefer concrete outcomes over internal implementation language.

Name repository paths, modules, commands, and important data objects precisely. If a term is project-specific or easy to misread, define it in plain language the first time it appears, and add it to [docs/nomenclature.md](../../docs/nomenclature.md).

Resolve ambiguity in the plan when possible. Do not leave major design choices implicit if those choices affect behavior, safety, migration order, or acceptance criteria. An open one is a gap.

Keep the plan safe to resume: someone should be able to stop midway, come back later, and continue from the plan without guessing what changed.

## Task Workflow

Every task should follow this workflow unless there is a documented reason not to:

- Define or update requirements first.
- Write or update design notes before committing to implementation details.
- Implement the code.
- Add or update tests.
- Update docs.
- Run verification and compare requirements, design, code, tests, and docs.
- If any mismatch appears, loop back to requirements and design before continuing.
- Land one coherent changeset where practical.
- Update the plan to reflect the new reality.

If something is unclear and the answer would materially change the implementation, record it as a gap and resolve it before marking the work done.

## Working A Plan

```sh
kuria ls plan --state active                   # the plans being worked
kuria show PLAN-007                            # its board, gaps, decisions
kuria set PLAN-007-T03 assignee=@me            # take a task
kuria do PLAN-007-T03 start --reason "…"
kuria cat PLAN-007-T03 > t03.md                # record progress as a file…
kuria apply t03.md --reason "…"                # …and write back what changed
kuria do PLAN-007-T03 finish --reason "…"      # or `review` first, with its change
kuria new gap --in PLAN-007 --set title="…"    # something found missing
kuria new decision --in PLAN-007 --set title="…"
```

(Agents prefix each with `mise run dogfood:kuria --`.)

When a task completes:

1. Record what was done, and how it was verified, in its description.
2. Finish it, with a reason.
3. Add the follow-up work it found as tasks or gaps.
4. Record decisions the work made, and supersede those it reversed.

Akt numbers are stable references: cite `PLAN-007-T03` in commits, conversation and other plans. Never reuse one; drop a task instead of deleting it (nothing is deleted).

## Verification Standard

A plan must describe how the reader can validate the intended behavior. Good verification includes one or more of:

- Specific tests to run.
- Expected observable behavior.
- Migration checks.
- Restart or persistence checks where relevant.
- Documentation updates that match the implemented system.

Avoid acceptance criteria that only restate internal code structure. Prefer statements that a human can verify directly.

## Decision Notes

Record important design decisions, tradeoffs, and reversals as decisions, so a later reader understands why the current direction was chosen, not just what it is. When the plan changes materially, decide a new one and let it supersede the old (`kuria link <new> supersedes <old>`).

## Anti-Patterns

Avoid plans that:

- only list code edits without stating the user-visible goal
- assume unstated background knowledge
- become stale after implementation starts
- mix unrelated initiatives into one plan
- treat tests or docs as optional afterthoughts
- mark work done without describing verification

## Starting A Plan

```sh
cat > plan.md <<'MD'
---
type: plan
title: <what becomes possible>
---

<!-- field: goal -->
The outcome, and how success will be recognized.

<!-- field: design -->
## Constraints And Assumptions

- …

## Design

…

<!-- field: verification -->
- …
MD
kuria new plan --file plan.md --reason "…"     # answers its number
kuria do PLAN-008 activate
kuria new task --in PLAN-008 --set title="Spec" --set acceptance=@acceptance.md
```
