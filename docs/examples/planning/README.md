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
| `gap` | `PLAN-001-G01` | | a plan | resolved by a task |
| `decision` | `PLAN-001-D01` | | a plan | supersedes a decision |

The documents come with the work that builds this example; this README
says what they hold.

## How it fits together

- **A plan** says what becomes possible and how anyone will know (goal),
  how it will be built (design), and how it is checked (verification), as
  Markdown. It is drafted, made active, and done or dropped.
- **Tasks** are created in their plan and numbered after it. Each has a
  description and acceptance criteria, and an assignee, who may be a
  person or an agent's service principal. A task is to do, doing, in
  review, done or dropped. The plan's board shows its tasks in those
  columns.
- **Order:** a task may block others, and is worked in the plan's rank
  order.
- **Gaps** are what a plan found missing or left open. Closing one
  requires its resolution, and it may be resolved by a task.
- **Decisions** record the context, what was decided and why, so that a
  later reader doesn't undo them without knowing. Deciding requires all
  three; a later decision may supersede one.
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
```

`kuria guide` says the same for agents.

## Konklav roles

- **Planner**, tenant-wide: `kuria:akter:read`; and at `plan`, `task`, `gap`
  and `decision`: `kuria:akter:create`, `kuria:akter:edit` and
  `kuria:akter:transition`.
- **Agent**, a service principal with the planner's grants, signing in with
  a secret key (`KURIA_KEY`).
