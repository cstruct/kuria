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

With `--dir <path>`, or `KURIA_DIR`, every command below works on an akt
directory instead of a server, by the same rules and with the same
answers. An akt directory has `kuria.toml`, the akt types in `types/`,
and the akter in `akter/`: each file named by its first akt's number and
title, such as `akter/PLAN-008-the-cli-in-local-mode.md`, holds that akt
and, after `<!-- akt: <NUMBER> -->` lines, every akt numbered after it.
Git is the history. Change akter through the commands, which keep files
in that shape, rather than editing the directory's files by hand.

- `kuria init --dir <path>` makes one; put the akt types in `types/`, or
  `kuria types define <file>`.
- After each write, commit what it changed: an answer to `--reason` ends
  "Commit with: <reason>", the message to use. `kuria history` reads the
  commits.
- People are written by email or name. `@me` is `KURIA_ME`, else git's
  `user.email`.
- After a merge or pull, run `kuria check`: it names each problem's file,
  such as two branches taking the same number, and exits 1 if any.
  `kuria check --fix` first moves akter into the files they belong in and
  renames files by their titles.
- Writes lock the directory, so agents may write at once.
- An attachment is a file `attachments/<number>/<name>`, and its field's
  value is that path. `kuria set IMG-1 file=@logo.png` copies a file in;
  `kuria check` names one that is missing, of a media type its field
  doesn't take, or not an SBOM where one belongs.
- Signing in, users, contacts, drafts, uploads and bundles aren't
  there (`LOCAL_UNSUPPORTED`, exit 1). Signed in, `kuria export --types
  <key>,… --to-dir <path>` makes a directory from a server's akter, and
  `kuria import --from-dir <path>` takes one to a server that lacks its
  types, keeping numbers, versions and attachments.

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
- A field naming several people is a list: `testers: [a@x.example, b@x.example]`.
  A sections field is a section whose body is YAML: a list of member
  numbers and sections, each `{title, text, collapsible, items}`.
- `verdict` (each assignee's `pass`, `fail` or `skip`) and `outcome` (what
  verdicts, or members' outcomes, add up to) are read-only: give verdicts
  with `kuria verdict`.
- A `markdown` field is a section of its own, Markdown whose headings keep
  ids (`## Install {#install}`) and whose links `kuria:<number>[@<version>][#<heading>]`
  are references to other akter. A release is refused while one is broken;
  `kuria show` says which.
- `revision` is the revision you read. If the akt changed since, `apply`
  is refused as `STALE` (exit 3) and says what changed: `cat` it again,
  merge, and apply again. Never force it.

### A whole plan as one file

```sh
kuria cat PLAN-007 --all-members > plan.md    # the plan, then each member
kuria cat PLAN-007 --members task,gap         # only these akt types
# edit any of its akter in place
kuria apply plan.md --reason "Why"            # each akt that changed
```

After the first akt, each member follows a line `<!-- akt: <NUMBER> -->`
and has no `number` of its own. `apply` checks every akt in the file before
changing any: one that is stale, or a changed read-only key, anywhere,
and nothing changes. Members are direct ones only, in the container's
order.

## A container as files: clones

`kuria docs` works on a clone, run in it; it needs a server, so not
`--dir`.

```sh
kuria docs clone <container> [<dir>]   # it and its members as files, the base in <dir>/.kuria/
cd <dir>                               # then edit the files with any editor
kuria docs status                      # staged, changed, removed, conflicted
kuria docs diff [<path>…]              # what isn't staged; --staged: what is
kuria docs add <path>…                 # stage files as they are now; `.` for all
kuria docs add -p [<path>…]            # choose hunks: y, n, q, a, d, ?
kuria docs push -m "Why"               # send what is staged, as one push
kuria docs pull                        # Kuria's changes merged into the files
kuria docs fmt                         # the files as Kuria writes them
```

- A clone is the container's file at its root, each member's in a
  directory per section of the container's `sections`, and the pictures
  its documents embed in `attachments/`, linked relatively. Files have no
  `revision`, `in`, `links` or `outcome`: the base in `.kuria/` (which
  ignores itself in git) records each akt's revision and, for an akt with
  a live draft, the draft's state.
- `push` fetches first and is refused (`STALE`, exit 3) unless every
  staged akt is still as the base has it: `pull`, then push again. A live
  draft's staged change goes as an update merged with what others type
  meanwhile, kept as one change with your message; any other akt's as an
  edit, a revision with your message as its reason. New or removed files
  and changed pictures are refused: akter and pictures are made in Kuria.
  A file not in normal form is refused too: `kuria docs fmt`, then add it
  again.
- `pull` merges three ways into the files, and where both changed the same
  text writes `<<<<<<< ours`, `=======`, `>>>>>>> kuria` around both and
  names the file. Resolve, then `kuria docs add`: a file holding markers
  can't be staged.
- git, if the clone is also a repository, is yours: Kuria sees only the
  files. To push part of your work, `git branch keep`, take the rest out
  of the working files (`git reset --hard <commit>`, or `git revert`),
  `kuria docs add` and `kuria docs push`; later bring it back
  (`git merge keep`, or revert the revert), `kuria docs add`,
  `kuria docs push`.

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
kuria verdict <test> pass|fail|skip|clear [--for <assignee>]
kuria copy <template> [--only <number>…] [--in <container>]   # with its members
kuria release <number> [--label 2.0]   # its current revision as its next version
kuria versions <number>                # its versions, latest first
kuria cat <number> --version 2         # as it was at version 2 (read-only)
kuria diff <number> 1 [2]              # what changed since version 1, or up to 2
kuria pdf <number> [--version 2] -o doc.pdf   # a markdown field as a PDF
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
