# kuria: a guide for agents

`kuria` reads and writes a Kuria tenant's akter (records: tasks, plans,
CAPAs…) for people and agents. It knows no akt type: ask the tenant what it
has, then work with what it answers.

## Signing in

- As software: set `KURIA_URL`, `KURIA_KEY` (a Konklav secret key) and, if
  Kuria has no default tenant, `KURIA_TENANT`. Nothing is saved.
- As a person: `kuria login --url <Kuria>` prints a link to open in any
  browser. `kuria whoami` says who you are; `kuria logout` ends it.

## Local mode

With `--dir <path>`, or `KURIA_DIR`, every command works on an akt
directory instead of a server: `kuria.toml`, the akt types in `types/`,
and each akt as an akt file in `akter/<number>.md`, with git as their
history. `kuria init --dir <path>` makes one, and `kuria check --dir
<path>` checks all of it as a server would, naming each problem's file
(run it after a merge). People are written by email
or name; `@me` is `KURIA_ME`, else git's `user.email`. Signing in, users,
contacts, drafts and bundles aren't there (`LOCAL_UNSUPPORTED`, exit 1).
Signed in, `kuria export --types <key>,… --to-dir <path>` makes a
directory from a server's akter, and `kuria import --from-dir <path>`
takes a directory's akter to a server that lacks its types, keeping
numbers.

## Finding your way

```sh
kuria types                    # the akt types, with how many akter each has
kuria schema <type>            # its fields, states, transitions, links, board
kuria ls <type> --state <s>    # akter, a page at a time (--all for every page)
kuria ls <type> --where assignee=@me   # by a field's value
kuria ls --in <container>      # a container's members
kuria find <text>              # by number or title words
kuria show <number> --history  # everything about one akt
kuria history <number>         # who changed what, and when
kuria users                    # whom user fields can name
kuria contacts                 # and the contacts they can name
```

`schema` is the source of truth for field keys, option keys, states and
what each transition requires. Read it before writing.

Add `--format json` to any command for a stable JSON object instead of
Markdown; refusals then come as `{"error": {"code", "message", "field"}}`
on standard error.

## Editing as a file

```sh
kuria cat <number> > akt.md            # the akt as an akt file
# edit akt.md
kuria apply akt.md --dry-run           # what would change
kuria apply akt.md --reason "Why"      # writes the fields that changed
```

An akt file is Markdown with YAML front matter:

```markdown
---
number: PLAN-007-T03
type: task
revision: 4
state: doing
in: [PLAN-007]
links: [Blocks PLAN-007-T04]
title: Export and import with history
assignee: someone@example.com
---

<!-- field: description -->
Markdown, as long as it needs.
```

- `number`, `type`, `revision`, `state`, `in` and `links` are read-only:
  change state with `do`, containers with `add`/`remove`/`transfer`, links
  with `link`/`unlink`.
- Single-line fields are front-matter keys; each multiline field is a
  section after `<!-- field: <key> -->`. Leaving a field out clears it.
- `revision` is the revision you read. If the akt changed since, `apply`
  is refused as `STALE` (exit 3) and says what changed: `cat` it again,
  merge, and apply again. Never force it.

## Changing things with verbs

```sh
kuria new <type> --in <container> --set title="…" --set description=@notes.md
kuria set <number> assignee=@me
kuria do <number> <transition> --set field=value --reason "…"
kuria add <container> <member>…    kuria remove <container> <member>…
kuria transfer <member> <from> <to>
kuria move <number> --before <other>
kuria link <number> blocks <other>      kuria unlink <number> blocks <other>
kuria draft <number> <field>
```

Values: options by key or name; users and contacts by email, name or
`@me`; dates as `YYYY-MM-DD`; checkboxes as `true`, `false` or
`for:<someone>`; references, Git and Docker values as JSON (a reference
also as its URL); a multiline text as `@<file>` or `@-`. An empty value
clears a field. Give `--reason` whenever the history should say why.

Akt types: `kuria types export [<key>…] -o <dir>`, and
`kuria types define <file>… [--check]`. Whole data sets move with
`kuria export … -o <dir>` and `kuria import <dir>`. `kuria guide` prints
this guide.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | Refused for what was asked: a bad value, an invalid change, a clash |
| 2 | Not found: no such akt, akt type or field |
| 3 | Stale: the akt changed since the revision you named |
| 4 | Not signed in, or not permitted |
| 5 | Kuria or Konklav unavailable: try again later |

## Planning work in Kuria

Where a tenant uses the planning example (`kuria schema plan`):

- A **plan** (`PLAN-007`) holds its goal, design and verification, and
  contains **tasks** (`PLAN-007-T03`), **gaps** (`PLAN-007-G01`) and
  **decisions** (`PLAN-007-D01`), each created in it:
  `kuria new task --in PLAN-007 --set title="…"`.
- To work: `kuria show PLAN-007` for its board, pick a task in `todo` that
  nothing unfinished blocks, `kuria set <task> assignee=@me`,
  `kuria do <task> start`, and record progress in its description with
  `cat` and `apply`.
- To finish: `kuria do <task> review` with its `change` set, then
  `finish`; or `finish` directly from `doing`.
- Something missing or open is a gap; closing one needs its resolution.
  A choice worth remembering is a decision: deciding needs its context,
  the decision and why.
- Order: tasks are worked in the plan's order (`move`), and `blocks`
  links say what must come first.
