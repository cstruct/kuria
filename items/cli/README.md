# kuria-cli

`kuria`, Kuria's command-line client, for people and LLM agents
([spec](../../docs/spec/cli.md)). It signs in with a DPoP-bound Konklav
session and talks to Kuria's GraphQL API. Like the app, it knows no akt
type.

## Responsibilities

- **Signing in by a link** (`kuria login`, `cli.login`): it asks Kuria
  where its Konklav is (`GET /cli/config`, `cli.discovery`), makes a P-256
  DPoP key, a PKCE verifier and a `state`, and prints Konklav's
  authorization URL for the public client `kuria-cli`, bound to the key
  by `dpop_jkt`. The link can be opened in any browser, on any machine;
  Konklav redirects to Kuria's relay (`/cli/authorized`), from which the
  CLI collects the code, polling once a second for up to two minutes. It
  exchanges the code with the verifier and a DPoP proof, saves the
  session, and records the URL and tenant in a profile.
- **Profiles** (`cli.profile`): `config.toml` in `KURIA_CONFIG_DIR`, else
  `$XDG_CONFIG_HOME/kuria`, else `~/.config/kuria`. `--url` and `--tenant`
  win over `KURIA_URL` and `KURIA_TENANT`, which win over the profile
  (`--profile`, else the default one: the first one signed in to).
- **Secrets** (`cli.secret`): the DPoP key, refresh token and access token
  go in the operating system's credential store (the macOS keychain,
  Windows' credential manager, the Secret Service on Linux), else in
  `secrets/<profile>.json`, `0600` in a `0700` directory.
  `KURIA_SECRET_STORE=file` keeps them in the file. They are never printed.
  The access token is refreshed when it has less than a minute left, and
  the rotated refresh token saved; a session Konklav ended is forgotten.
  `kuria logout` revokes the refresh token at Konklav, then forgets the
  key and tokens.
- **Software** (`cli.secret-key`): with `KURIA_KEY` set to a Konklav secret
  key, each command signs in with a fresh DPoP key at the issuer Kuria
  names, moves into the tenant (`--tenant`, `KURIA_TENANT`, the profile's,
  else Kuria's default), and signs out when done. Nothing is saved. Such
  tokens carry Konklav's own audience, which Kuria must accept
  (`KONKLAV_AUDIENCE=kuria,konklav`).
- **DPoP on every request** (`cli.dpop`): every request with a token, and
  every grant, carries a fresh ES256 proof for its method and URL; a
  server asking for a nonce gets one more try with it.
- **Answers** (`cli.format`): Markdown by default, JSON objects with
  `--format json`, on standard output. Links and notes for the person go
  to standard error.
- **Refusals and exit codes** (`cli.errors`): the server's code, message
  and field, as `error: …` or, with `--format json`,
  `{"error": {"code", "message", "field"}}` on standard error. Exit codes:
  0 done, 1 refused, 2 not found, 3 stale, 4 not signed in or not
  permitted, 5 unavailable.

- **Reads** (`cli.read`):
  - `types`; `schema <type>` (fields with kinds, required and options,
    states and tones, transitions with what they require and the outcomes
    they allow, and from the
    type's document what it contains and links to, its board and
    presentation);
  - `ls [<type>]` with `--state` (key or name), `--in <container>` (its
    members; a state is then filtered here, since members have no state
    to ask for), `--where <field>=<value>` (`cli.values`: options by key
    or name, people by email, name or `@me`, dates as `YYYY-MM-DD`,
    single-line text whole), `--outcome` (a type with a verdict field, or
    with `--in` one whose outcome comes from its members), `--search`,
    `--limit` and `--after`, or
    `--all` pages. With a type, its list fields are the columns;
  - `find <text>`; `cat <number>… [--members <type>,… | --all-members]
    [-o <dir>]` (`cli.file`, `cli.members`): several akter, or an akt with
    its direct members in the container's order, print as one multi-akt
    file (`cli.file.multi`), each after the first following its
    `<!-- akt: <NUMBER> -->` line without `number`; `show <number>
    [--history]` (fields, containers, members by board column, and those
    no column holds by type, as the app shows them,
    links with pins, the transitions its state allows); `history
    <number>`, said as the app says it (`app.history`); `users`,
    `contacts`.
- **The akt file** (`cli.file`, [`src/file.rs`](src/file.rs)): `number`,
  `type`, `revision`, `state`, `in` and `links` (read-only), then each
  single-line field by key; each multiline text in a section after
  `<!-- field: <key> -->`. Values are written as the file reads them back:
  options by key, users and contacts by email else name, a checkbox as
  `true` or `for:<someone>`, references, Git, Docker values and
  attachments as flow-style objects. Strings are quoted only where YAML
  would read them as something else.

- **Writes** (`cli.write`, `cli.apply`):
  - `new <type> [--in <container>]… [--set <field>=<value>]… [--file
    <file>]`: from flags, a file, or a file and flags (flags win). A file's
    `number`, `revision`, `state` and `links` are another akt's, so they're
    ignored; its `in` is added to `--in`;
  - `apply <file>… [--dry-run]`: reads every akt of every file (a
    multi-akt file holds several), refuses one given twice, and checks
    every one, stale or read-only, before changing any, and against a
    server sends every edit in one `batch`, so that a refusal of any
    changes none (naming the akt it refused). For each, it compares each
    field of the file with the
    akt as it is, as the inputs the server takes (so a user written by
    name or by email is the same user), and edits only those that differ.
    A field the file leaves out is cleared. A file read at an older
    revision is refused as `STALE` (exit 3), naming what changed since; a
    changed `type`, `state`, `outcome`, `in`, `links` or verdicts, and a key
    the type lacks, are refused with the key as the field;
  - `set <number> <field>=<value>…`; `do <number> <transition>` (by key or
    name) `[--set …]`; `draft <number> <field>`;
  - `verdict <number> pass|fail|skip|clear [--for <user>]`: one's own
    verdict, or one for another assignee, as an edit of the type's
    `verdict` field, answering the outcome after it;
  - `copy <number> [--only <number>…] [--in <container>…]`
    ([`src/writes/copy.rs`](src/writes/copy.rs)): `copyAkt`, then the
    copy's members walked, parents first, to answer each copy with its
    original;
  - `release <number> [--label]`, `versions <number>`, `diff <number>
    <from> [<to>]`, `pdf <number> [--field] [--version] -o <file>`, and
    `cat`/`show --version` ([`src/versions.rs`](src/versions.rs));
    `release` and version reads work in local mode too, from git
    ([`src/local/versions.rs`](src/local/versions.rs));
  - `add <container> <member>…`, `remove …`, `transfer <member> <from>
    <to>`, `move <number> --before|--after <number>`; in a clone, `add`
    stages files instead (below);
  - `link <source> <kind> <target> [--pin]` (the kind by key or name;
    `--pin` pins the target's current revision), `unlink …`;
  - `types export [<key>…] [-o <dir>]`, `types define <file>… [--check]`
    (a file holds a document or a list of them; `--check` validates each
    with `kuria-config`, offline, without signing in).

- **Clones** ([`src/clone.rs`](src/clone.rs), docs/spec/clones.md,
  `clone.*`): a container and its members as files, synced with Kuria.
  - `clone <number> [<dir>]` ([`src/clone/sync.rs`](src/clone/sync.rs)):
    refuses an akt that isn't there or readable (`NOT_FOUND`) and one
    whose type contains nothing (`NOT_A_CONTAINER`); writes into `<dir>`,
    by default a new directory named as the container's file, which must
    be new or empty. Files are laid out as `clone.tree` says
    ([`src/clone/layout.rs`](src/clone/layout.rs)), with pictures
    ([`src/clone/pictures.rs`](src/clone/pictures.rs)).
  - `.kuria/` ([`src/clone/base.rs`](src/clone/base.rs)): `.gitignore`
    (`*`, so git ignores the folder), `clone.json` (`{url, number}`),
    `base.json` (`{akter: {<number>: {path, revision, live?: {state,
    base}}}}`, the live draft's state and base base64, as `liveState`
    answers them), `base/<path>` (each file and picture as Kuria last had
    them), `staged/<path>` and `pictures/<id>` (pictures' bytes, fetched
    once).
  - `status`, `diff [--staged] [<path>…]`, `add <path>…` and `add -p
    [<path>…]` ([`src/clone/work.rs`](src/clone/work.rs)), offline: the
    working files against what is staged, else the base. `add` takes
    files or directories, refuses pictures, removed files, files the base
    lacks and files holding conflict markers, and never changes a working
    file; `-p` offers each hunk (`y`, `n`, `q`, `a`, `d`, `?`), reading a
    line of standard input per answer, a terminal or not, and asking on
    standard error. In a clone, `add` and `diff` take paths when given
    `-p` or `--staged`, nothing, or only paths that exist; otherwise
    they are the container and version commands.
  - `push -m <message>`: fetches each staged akt (its revision, live
    draft's base and type) and refuses (`STALE`, exit 3), changing
    nothing, unless each is as the base has it; refuses a new file, a
    changed picture, a changed `number`, `type` or `state`, verdicts, and
    a file not in normal form. For an akt with a live draft, it loads
    `kuria_engine::live::LiveDraft` from the base's state, writes each
    field the staged file changed from the base's file (a text merged into
    the draft's text from the base's by characters), and sends the update
    with the base; for any other akt an `editAkt` of those fields at the
    base's revision. All go as one `pushClone`. Then the base's files are
    what was pushed, its revisions what Kuria answered, its live drafts'
    states and bases the drafts after the update, and the staged files
    are cleared; the working files are left as they are.
  - `pull`: fetches the container and its members and merges each file
    three ways with `kuria_engine::merge::merge_marked`, base to Kuria
    into the working file, by lines then characters, writing
    `<<<<<<< ours` / `=======` / `>>>>>>> kuria` where both changed the
    same text; moves a file whose path changed; writes new members;
    removes members gone unless changed here (kept and named); takes
    Kuria's pictures unless changed here; restages what was staged,
    merged the same way, or unstages it where that conflicts; and makes
    Kuria's state the base.
  - `fmt` ([`src/clone/fmt.rs`](src/clone/fmt.rs)): the clone's files,
    every file under it but those in `.git/` and `.kuria/`, put in normal
    form, each where its container's sections put it, and the pictures
    of the images they embed. The clone is the nearest directory holding
    `.kuria/`, else the current one; its container the one
    `.kuria/clone.json` names, else the akt of the root's file that no
    other file there is a member of.
  - Push, pull and `fmt` use the clone's URL when neither `--url`,
    `KURIA_URL` nor the profile names one, and are refused against
    another Kuria.

- **Bundles** ([`src/writes/bundles.rs`](src/writes/bundles.rs)):
  `export (--types <key>,… [--akter] | --whole-tenant) -o <dir>` writes the
  bundle Kuria answers into a new directory (`bundle.json`,
  `attachments/<sha256>`); `import <dir> [--map-subjects <file>]` sends it
  back as a tar archive, with the mapping, a JSON object of the bundle's
  subjects to this tenant's, as `subjects.json`.

  Every one that makes revisions takes `--reason`. `move` makes none: it
  changes the tenant's order only.
- **Values** (`cli.values`, [`src/values.rs`](src/values.rs)): `--set`
  takes options by key or name (several comma-separated), users and
  contacts by email, name, `@me` or identifier, dates as `YYYY-MM-DD`,
  checkboxes as `true`, `false` or `for:<someone>`, references, Git and
  Docker values as JSON (a reference also as its URL), attachments by id,
  a multiline text as `@<file>` or `@-`, and an empty value to clear. A
  field naming several people takes a list, or them comma-separated, and a
  sections field YAML, as `@<file>` too. The file's values are read the
  same way.
- **The akt file's kinds** (`cli.file.kinds`): several people are a list;
  a sections field is a section of its own whose body is YAML, a list of
  member numbers and `{title, text, collapsible, items}`; verdicts are a
  mapping of each assignee to `pass`, `fail` or `skip`; and an akt whose
  type has outcomes has `outcome` after `state`. `show` gives the outcome,
  each member's, and the members in their sections, a nested list with
  each section's outcome; `ls` adds an Outcome column when any row has
  one. Sections are read as `SectionsValue.value`, the JSON `editAkt`
  takes, whatever their depth, and each section's outcome is worked out
  from its members' through `kuria-engine`.

- **The guide** (`cli.guide`, [`src/guide.md`](src/guide.md)): `kuria
  guide` prints, for agents, signing in, finding an akt type's fields,
  states and transitions, the akt file, the verbs and values, the exit
  codes, and how to work a plan in the planning example. A unit test fails
  if a command is missing from it.

### Local mode

With `--dir <path>`, or `KURIA_DIR`, the reads work on an akt directory
instead of a server ([`src/local.rs`](src/local.rs), `cli.local.*`):

- **The directory** ([`src/local/dir.rs`](src/local/dir.rs)): `kuria init
  --dir <path>` makes `kuria.toml` (format 1), `types/` and `akter/`. Each
  `types/<key>.json` is validated as `defineAktType` validates it and kept
  as a server stores it. Each file in `akter/` begins with an akt file plus
  `rank`, and holds its nested akter after `<!-- akt: <NUMBER> -->` lines
  (outside fenced code blocks), each without `number`. Each akt is read
  and checked against its type, found by its number whatever the file is
  named, with any problem naming the file, and the number of a nested akt
  (`BAD_FILE`). A file holds no `outcome`: it is derived when read, from
  the directory's verdicts, through `kuria-engine`'s `outcomes`; verdicts
  are kept by assignee, without who gave one for them or when, which git
  says.
- **The same answers** ([`src/local/answers.rs`](src/local/answers.rs)): the
  directory answers what Kuria's GraphQL would for each of the CLI's
  queries, so every read renders through the same code: `types`, `schema`,
  `types export`, `ls` (by state, container, `--where`, `--outcome`, search; pages with
  `local:<offset>` cursors), `find`, `cat` (the file, without `rank`),
  `show` (board columns through `kuria-engine`, links read back by name)
  and `whoami` (the directory and who `@me` is).
- **Git** ([`src/local/git.rs`](src/local/git.rs)): a file may hold
  several akter, so history is per akt. One `git log` of `akter/` and one
  `git cat-file --batch` read the revisions each commit's files held; an
  akt changed in the commit where it first reached a revision, whichever
  file held it, so a move between files keeps its history. `history`
  answers those commits with the revision after each; created and changed
  are the first and last of them, or when its file was written if its
  revision differs from `HEAD`'s. Outside git, a file's akter share its
  time.
- **Layout** ([`src/local/layout.rs`](src/local/layout.rs)): an akt
  numbered after its parent goes in its parent's file, any other begins a
  file named `<NUMBER>-<slug>.md` from its title. A write moves the akter of
  the files it touched to where they belong and writes each file whole,
  nested akter in the directory's order; a file left empty, as by a new
  title, is removed.
- **Writes** ([`src/local/write.rs`](src/local/write.rs),
  [`edits.rs`](src/local/edits.rs), [`arrange.rs`](src/local/arrange.rs)):
  every write the server takes, through `kuria-engine` (`create`, `edit`,
  `transition` and its `rules`), refused with the server's codes; sections
  checked against the akt's members, and a member that leaves taken out of
  them; `verdict`; and `copy` ([`copy.rs`](src/local/copy.rs)), keeping
  what `kuria_engine::copying` says, as the server does. A write
  loads the directory under `.kuria.lock` (waiting up to 10 seconds, then
  `LOCKED`, exit 5), changes it in memory, and writes the files it touched
  whole, through a file renamed over each. It raises revisions as a server
  does: a member's and its container's on a membership change, both
  akter's on a link. Numbers come from the files present
  (`numbering::next_from`); containment lives in each member's `in`, a
  link in both akter's `links`. `apply` gives the engine every field of the
  file, so it decides what changed as a server would. `--reason` is
  answered back to commit with (`reason` in JSON, "Commit with:" in
  Markdown). `types define` replaces a document, refusing one the
  directory's akter wouldn't fit.
- **Check** ([`src/local/check.rs`](src/local/check.rs), `cli.local.check`):
  `kuria check --dir <path>` reads every file, collecting rather than
  stopping at problems, and checks what spans files: types' references to
  each other, values that fit their fields, numbers their pattern gives
  and unique (ignoring case), containers that exist and contain the type,
  exclusive containers, cycles, links of a declared kind to a type it
  allows, pins the target reached, both sides listing each link, and
  unique ranks. It answers `INCONSISTENT` (exit 1) with each problem as
  `- <file>: <what>`; `mise run lint` runs it on `plans/` once that
  exists. A merge left unresolved shows as a file that doesn't read. It
  also names an akt stored twice, one outside its parent's file or nested
  where it isn't numbered after an akt of the file, and a file not named
  by its first akt. `check --fix` first moves every akt to where it
  belongs and names every file, through the writer, changing no akt.
- **Converting** ([`src/local/convert.rs`](src/local/convert.rs),
  `cli.local.convert`), signed in, by way of a bundle:
  `export --types <key>,… --to-dir <path>` writes the bundle of those types
  and their akter as a new directory (each type's latest document, links
  ordered as the server lists them, people by email or name, versions with
  the headings they had and whether an image had its picture, each
  attachment's bytes as `attachments/<number>/<name>`), then checks it.
  `import --from-dir <path>` sends
  a consistent directory as a `types` bundle: each akt with revisions
  numbered up to its file's `revision`, each as git's latest commit at that
  revision holds it, so that every hash matches, pins hold and each
  version reads as it was released; its versions; each file an akt holds,
  now or at an earlier revision (from git if it was since removed), as an
  attachment with its bytes, under an identifier new to that bundle; each
  person as a subject named by their email, which the import maps to a
  user who has signed in.
- **Attachments** ([`src/local/attachments.rs`](src/local/attachments.rs),
  `cli.local.attachments`): files `attachments/<number>/<name>`, an
  `attachment` field's value being that path (`cat` prints it so). A write
  takes such a path, or `@<file>`, copied in when the change is saved,
  under a name of its own (`logo-2.png`) if another file has the name;
  either is checked: the file is there, its media type by extension one
  its field takes, an SBOM where the field holds SBOMs (`kuria-engine`'s
  `sbom`). An attachment's identifier, an upload, is `LOCAL_UNSUPPORTED`.
  `check` names each that doesn't hold. An image with its file has its
  picture, for embeds and releases.
- **People** are text: `@me` is `KURIA_ME`, else git's `user.email`. On
  their way through the engine, each name or email stands in as a subject
  derived from its text ([`src/local/values.rs`](src/local/values.rs)); a
  checkbox records whom it is for (`true`, or `for:<someone>`), not who
  checked it, which git keeps.
- **Refused** as `LOCAL_UNSUPPORTED`: signing in, `users`, `contacts`,
  `draft`, uploading attachments, and bundles (convert instead).

### JSON answers

| Command | Object |
|---|---|
| `whoami` | `{url, tenant, subject, name, email, signedInWith, profile}`; in local mode `{dir, me, signedInWith: "local"}` |
| `types` | `{types: [{key, name, version, numberPattern, akterCount}]}` |
| `schema` | `{type: {key, name, version, numberPattern, fields: [{key, name, kind, required, multiline, description, options, format, draft}], states: [{key, name, initial, final, tone}], transitions: [{key, name, from, to, requires, action}], contains, createdIn, links, board, presentation}}` |
| `ls`, `find` | `{akter: [{number, type, title, state, outcome, updatedAt, fields}], endCursor, hasNextPage}` |
| `cat` | `{akter: [{number, type, revision, state, outcome, in, links: [{kind, name, outgoing, number, pinnedRevision}], fields}]}`, or `{written}` with `-o` |
| `show` | `{akt: {…cat's, title, typeName, stateName, createdAt, updatedAt, containers, links: [{kind, name, outgoing, akt, pinnedRevision}], members: {columns?, akter, more, sections?}, transitions, history?}}` |
| `history` | `{number, revisions: [{revision, by, at, what, reason, changes: [{field, value}], memberships}]}`; in local mode also `tracked` and `uncommitted` |
| `users`, `contacts` | `{users: [{id, name, email}]}`, `{contacts: [{id, displayName, name, email, user}]}` |

| `new` | `{number, revision}` |
| `apply` | `{applied: [{file, number, revision, changed, dryRun?}]}` |
| `set`, `do`, `draft` | `{number, revision, changed \| state, transition \| field}` |
| `verdict` | `{number, revision, verdict, for, outcome}` |
| `copy` | `{number, from, members: [{from, number}]}` |
| `add`, `remove`, `transfer`, `move`, `link`, `unlink` | what was asked, such as `{source, kind, target, pinnedRevision}` |
| `guide` | `{guide}` |
| `clone` | `{dir, number, files, members}` |
| `status` | `{number, staged, unstaged, removed, conflicted, untracked}` |
| `diff` in a clone, `add` in a clone | `{staged, files}`, `{staged}` |
| `push`, `pull` | `{pushed, message}`, `{merged, added, removed, kept, conflicted, unstaged}` |
| `fmt` | `{changed}` |
| `export`, `import` | `{dir, scope, aktTypes, akter, contacts, attachments, files}`; `{scope, aktTypes, akter, aliases}` |
| `export --to-dir` | `{dir, aktTypes, akter, attachments}` |
| `types export`, `types define` | `{types: [document]}` or `{written}`; `{defined: [{key, version}]}` or `{valid: [key]}` |

`fields` maps field keys to values in the akt file's form.

## Entry points

- [`src/main.rs`](src/main.rs): parses the command line and runs it on a
  single-threaded Tokio runtime.
- [`src/lib.rs`](src/lib.rs): `run` resolves the profile and runs a
  command, saying its answer or refusal.
- [`src/args.rs`](src/args.rs): the commands and global options (clap).
- [`src/account.rs`](src/account.rs): `login`, `logout`, `whoami`.
- [`src/session.rs`](src/session.rs): the session a command uses, saved or
  from `KURIA_KEY`, and refreshing it; [`src/graphql.rs`](src/graphql.rs):
  queries through it.
- [`src/konklav.rs`](src/konklav.rs): `/cli/config`, Konklav's discovery,
  token grants, revocation and the secret-key login.
- [`src/dpop.rs`](src/dpop.rs) and [`src/http.rs`](src/http.rs): the key,
  its proofs, and requests carrying them.
- [`src/reads.rs`](src/reads.rs): the reads, in `reads/catalog.rs`,
  `reads/list.rs` and `reads/akter.rs`; [`src/model.rs`](src/model.rs):
  Kuria's answers and the GraphQL fragments asking for them;
  [`src/values.rs`](src/values.rs), [`src/file.rs`](src/file.rs),
  [`src/history.rs`](src/history.rs), [`src/md.rs`](src/md.rs).
- [`src/writes.rs`](src/writes.rs): the writes, in `writes/edit.rs` (`new`,
  `apply`, `set`, `do`, `draft`, `verdict`), `writes/copy.rs`,
  `writes/arrange.rs` (membership, order, links) and `writes/types.rs`
  (documents).
- [`src/clone.rs`](src/clone.rs): clones, in `clone/sync.rs` (`clone`,
  `pull`, `push`), `clone/work.rs` (`status`, `diff`, `add`),
  `clone/base.rs` (`.kuria/`), `clone/fmt.rs`, `clone/layout.rs` and
  `clone/pictures.rs`.
- [`src/profile.rs`](src/profile.rs), [`src/secret.rs`](src/secret.rs),
  [`src/output.rs`](src/output.rs), [`src/error.rs`](src/error.rs).

## Running

```sh
mise run kuria -- login --url http://localhost:18090
mise run kuria -- whoami --format json
mise run cli:install    # kuria on your PATH
```

Kuria must run the relay, and its Konklav must have the `kuria-cli` client
([`items/server/README.md`](../server/README.md)). The local stack
`mise run e2e:serve` has both.

## Testing

```sh
cargo test -p kuria-cli
```

- Unit tests: profile resolution, the secret file's permissions, DPoP
  proofs checked with Konklav's own `verify_proof`, error codes and exit
  codes, the command line, values as text and in files, `--where` and
  `--set` values by kind, the akt file written and read back, YAML
  quoting, refused files, and the history's wording.
- Local mode ([`tests/local.rs`](tests/local.rs)), on temporary akt
  directories in git repositories with the planning example's types:
  `init` and the manifest, types and schemas, lists filtered, ordered and
  paged, `cat` as the file without `rank`, `show` with its board and links,
  history from commits, refusals of what local mode doesn't do, and files
  that don't fit, named; and every write: numbering in and across plans,
  revisions of containers, the server's refusals, `apply` with dry runs,
  stale files and read-only keys, links with pins on both files, order,
  transfers, redefining types, and eight writers at once; a protocol of
  tests in sections taking verdicts, its outcome adding up, its file
  applied unchanged and with read-only verdicts and outcome refused, a
  test leaving its section, and copies of the whole and of a part;
  attachments copied in or named by path, refused when missing, of a
  media type the field doesn't take or not an SBOM, a document embedding
  a picture released, and attachments through a bundle and back, one
  since removed read from git; and `check`, on a
  consistent directory and on one with eight kinds of problem, each named
  with its file.
- Clones ([`tests/clones.rs`](tests/clones.rs)), against a Kuria server
  on a local port with a fresh database and a relying host standing in
  for Konklav ([`tests/support/mod.rs`](tests/support/mod.rs)), with the
  documents example's types, documents with live drafts: a clone and its
  `.kuria/`, `fmt` in it, refusals of a non-container and a missing akt;
  a live draft's edit pushed as one change with its message, and the
  index's as a revision with it as its reason; `add -p` staging one hunk
  of two and pushing only it; a push behind a live edit refused, then a
  pull merging it and the push again; a conflict marked, refused by `add`,
  resolved and pushed; and git's branch workflow end to end. Needs
  `mise run db:up`.
- End to end ([`e2e/tests/clone.spec.ts`](../../e2e/tests/clone.spec.ts)):
  the binary clones an index while the app types in its document; a push
  behind it is refused, a pull merges, and the push shows in the browser.
- End to end ([`e2e/tests/cli.spec.ts`](../../e2e/tests/cli.spec.ts)): the
  built binary against the e2e stack. Playwright authorizes the link
  `kuria login` prints; then `whoami` from the profile, a refresh, `logout`
  revoking at Konklav, and `KURIA_KEY` saving nothing, with exit codes 4
  and 5; and every read against the release-management example: schema,
  filtered and paged lists, search, the akt file, `show` with a board and
  history, and refusals with exit codes 1 and 2; and every write, on akt
  types it defines: offline checks, the file round trip (unchanged,
  edited, cleared, stale, read-only and unknown keys), `set`, `do`,
  links with pins, order, membership, transfers, a draft, and `new
  --file`; and a bundle exported and imported into the second tenant
  `lab`, with the same history and numbering on, then clashing, a type
  alone as a new version, and a whole tenant refused there; selected types
  converted to an akt directory reading as on the server, changed there,
  and imported into `lab` with their numbers and revisions; and the
  planning example defined from `docs/examples/planning` and a plan
  worked as the guide says.
- Parity ([`e2e/tests/parity.spec.ts`](../../e2e/tests/parity.spec.ts)):
  one sequence of commands, each run against the e2e server and then
  with `--dir` against a new akt directory. Each answer must be the same
  (a refusal by its code, the reason to commit with set aside), then each
  akt's `cat` and their order; and a second for a protocol: sections, own
  verdicts, outcomes in files and lists, copies whole and in part, a test
  removed, and files applied unchanged and with a verdict changed, `@me`
  in the directory being whom the server names the client by. It is how
  local mode is held to "the same rules, through a shared engine".

## Traceability

- Spec: [`docs/spec/cli.md`](../../docs/spec/cli.md) (`cli.profile`,
  `login`, `secret`, `secret-key`, `dpop`, `discovery`, `format`,
  `errors`, `guide`, `read`, `file`, `apply`, `write`, `values`, and
  `local.*` for local mode); [`docs/spec/clones.md`](../../docs/spec/clones.md)
  (`clone.*`); `app.history` for the history's wording.
- Annotations: `r[impl cli.*]` and `r[impl clone.*]` in `src/`,
  `r[verify cli.*]` and `r[verify clone.*]` in the unit tests,
  `tests/local.rs`, `tests/clones.rs`, `e2e/tests/cli.spec.ts`,
  `e2e/tests/clone.spec.ts` and `e2e/tests/parity.spec.ts`.
