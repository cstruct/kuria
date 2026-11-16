# Planning

An example configuration: four akt types for planning work in Kuria
instead of in Markdown files, as Kuria's own development does. People and
agents read and update them with the `kuria` command-line client
([spec](../../spec/cli.md)). Nothing in Kuria knows these names; they are
ordinary akt type documents.

| Akt type | Number | Contains | Created in | Links |
|---|---|---|---|---|
| `plan` | `PLAN-001` | tasks, gaps and decisions | | |
| `task` | `PLAN-001-T01` | | a plan | blocks a task |
| `gap` | `PLAN-001-G01` | | a plan | resolved by a task or decision |
| `decision` | `PLAN-001-D01` | | a plan | supersedes a decision |

The documents are [`plan.json`](plan.json), [`task.json`](task.json),
[`gap.json`](gap.json) and [`decision.json`](decision.json). Define them
with tasks, gaps and decisions before plans, since a plan's board names
task states:

```sh
kuria types define docs/examples/planning/{task,gap,decision,plan}.json
```

## How it fits together

- **A plan** says what becomes possible and how anyone will know (goal),
  how it will be built (design), and how it is checked (verification), as
  Markdown, and has an owner. It is drafted, made active (which needs its
  goal), and done or dropped.
- **Tasks** are created in their plan and numbered after it. Each has a
  description and acceptance criteria, and an assignee, who may be a
  person or an agent's service principal, and the change (a pull request
  or commit) that does it. A task is to do, doing, in review, done or
  dropped; it can be finished from doing or from review. The plan's board
  shows its tasks in those columns, dropped ones with the done.
- **Order:** a task may block others, and is worked in the plan's rank
  order.
- **Gaps** are what a plan found missing or left open. Closing one
  requires its resolution, and it may be resolved by a task.
- **Decisions** record the context, what was decided and why, and who
  decided, so that a later reader doesn't undo them without knowing.
  Deciding requires the first three; a later decision may supersede one.
- **Archiving:** a plan is archived by hand once it is no longer read
  ([archiving](../../spec/archive.md)), and its tasks, gaps and decisions
  go with it (`with_container: plan`), coming back if it is unarchived. In
  an akt directory, archiving removes the plan's file, git's history and
  `akter/archived.md` keeping it: `kuria archive PLAN-001`.
- **Presentation:** each type has an icon and a colour; cards show a task's
  assignee; tones show what is active, done or dropped.

## Working a plan from the command line

```sh
kuria ls plan --state active                     # what's being worked
kuria show PLAN-007                              # its tasks by column, gaps, decisions
kuria ls task --in PLAN-007 --where assignee=@me # mine
kuria cat PLAN-007-T03 > t03.md                  # edit it as a file…
kuria apply t03.md --reason "Narrowed the scope" # …and write back what changed
kuria do PLAN-007-T03 finish --reason "Merged"   # move it on
kuria link PLAN-007-T03 blocks PLAN-007-T04      # say what comes first
```

`kuria guide` says the same for agents.

## Without a server

The same types work in an akt directory, Markdown files in a git
repository, with no server at all; Kuria's own plans are kept that way.
Every command above takes `--dir <path>` (or `KURIA_DIR`), and git is the
history: commit with the reason each write gives back.

```sh
kuria init --dir plans
cp docs/examples/planning/*.json plans/types/
kuria --dir plans new plan --set "title=Local mode"   # PLAN-001
kuria --dir plans new task --in PLAN-001 --set "title=Spec it"
kuria --dir plans check                                # after a merge
```

`kuria export --types plan,task,gap,decision --to-dir plans` makes such a
directory from a server's plans, and `kuria import --from-dir plans` takes
it to a server that hasn't defined the types yet, keeping numbers.

## With example data

[`bundle/`](bundle/) is an example bundle of these akt types with akter:
an active plan with tasks in every state, `blocks` links, an open gap and one closed by a decision, and a decision superseded by a later one; a finished plan and a dropped one. Devi Larsen (`dev@shop.example`), Rami Moberg (`rm@shop.example`)
and Hedda Lund (`hedda@shop.example`) wrote them, the people the end-to-end
seed signs in as. To start a server with it, set

```sh
KURIA_BOOTSTRAP_BUNDLE=docs/examples/planning/bundle
```

and the server imports it into its default tenant (or
`KURIA_BOOTSTRAP_TENANT`) before it serves; the tenant must hold nothing
yet ([`items/server`](../../../items/server/README.md#running)).
`kuria import docs/examples/planning/bundle` takes it to a running server's
tenant that hasn't defined these types. Against the end-to-end Konklav,

```sh
mise run e2e:example -- planning
```

serves it on <http://localhost:18090> to the seeded people, as themselves.
[`docs/examples`](../README.md) says how the bundles are made.

## Konklav roles

- **Planner**, tenant-wide: `kuria:akter:read`; and at `plan`, `task`, `gap`
  and `decision`: `kuria:akter:create`, `kuria:akter:edit` and
  `kuria:akter:transition`.
- **Agent**, a service principal with the planner's grants, signing in with
  a secret key (`KURIA_KEY`).
