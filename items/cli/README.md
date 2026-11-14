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
    states and tones, transitions with what they require, and from the
    type's document what it contains and links to, its board and
    presentation);
  - `ls [<type>]` with `--state` (key or name), `--in <container>` (its
    members; a state is then filtered here, since members have no state
    to ask for), `--where <field>=<value>` (`cli.values`: options by key
    or name, people by email, name or `@me`, dates as `YYYY-MM-DD`,
    single-line text whole), `--search`, `--limit` and `--after`, or
    `--all` pages. With a type, its list fields are the columns;
  - `find <text>`; `cat <number>… [-o <dir>]` (`cli.file`); `show <number>
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
  - `apply <file>… [--dry-run]`: compares each field of the file with the
    akt as it is, as the inputs the server takes (so a user written by
    name or by email is the same user), and edits only those that differ.
    A field the file leaves out is cleared. A file read at an older
    revision is refused as `STALE` (exit 3), naming what changed since; a
    changed `type`, `state`, `in` or `links`, and a key the type lacks, are
    refused with the key as the field;
  - `set <number> <field>=<value>…`; `do <number> <transition>` (by key or
    name) `[--set …]`; `draft <number> <field>`;
  - `add <container> <member>…`, `remove …`, `transfer <member> <from>
    <to>`, `move <number> --before|--after <number>`;
  - `link <source> <kind> <target> [--pin]` (the kind by key or name;
    `--pin` pins the target's current revision), `unlink …`;
  - `types export [<key>…] [-o <dir>]`, `types define <file>… [--check]`
    (a file holds a document or a list of them; `--check` validates each
    with `kuria-config`, offline, without signing in).

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
  a multiline text as `@<file>` or `@-`, and an empty value to clear. The
  file's values are read the same way.

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
  as a server stores it; each `akter/<number>.md` is an akt file plus
  `rank`, read and checked against its type, with any problem naming the
  file (`BAD_FILE`).
- **The same answers** ([`src/local/answers.rs`](src/local/answers.rs)): the
  directory answers what Kuria's GraphQL would for each of the CLI's
  queries, so every read renders through the same code: `types`, `schema`,
  `types export`, `ls` (by state, container, `--where`, search; pages with
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
  `transition` and its `rules`), refused with the server's codes. A write
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
  exists. A merge left unresolved shows as a file that doesn't read.
- **Converting** ([`src/local/convert.rs`](src/local/convert.rs),
  `cli.local.convert`), signed in, by way of a bundle:
  `export --types <key>,… --to-dir <path>` writes the bundle of those types
  and their akter as a new directory (each type's latest document, links
  ordered as the server lists them, people by email or name), then checks
  it. `import --from-dir <path>` sends a consistent directory as a `types`
  bundle: each akt with revisions numbered up to its file's `revision`,
  the first recording it as it is and the rest changing nothing, so that
  every hash matches and pins hold; each person as a subject named by
  their email, which the import maps to a user who has signed in.
- **People** are text: `@me` is `KURIA_ME`, else git's `user.email`. On
  their way through the engine, each name or email stands in as a subject
  derived from its text ([`src/local/values.rs`](src/local/values.rs)); a
  checkbox records whom it is for (`true`, or `for:<someone>`), not who
  checked it, which git keeps.
- **Refused** as `LOCAL_UNSUPPORTED`: signing in, `users`, `contacts`,
  `draft`, attachments and bundles (convert instead).

### JSON answers

| Command | Object |
|---|---|
| `whoami` | `{url, tenant, subject, name, email, signedInWith, profile}`; in local mode `{dir, me, signedInWith: "local"}` |
| `types` | `{types: [{key, name, version, numberPattern, akterCount}]}` |
| `schema` | `{type: {key, name, version, numberPattern, fields: [{key, name, kind, required, multiline, description, options, format, draft}], states: [{key, name, initial, final, tone}], transitions: [{key, name, from, to, requires, action}], contains, createdIn, links, board, presentation}}` |
| `ls`, `find` | `{akter: [{number, type, title, state, updatedAt, fields}], endCursor, hasNextPage}` |
| `cat` | `{akter: [{number, type, revision, state, in, links: [{kind, name, outgoing, number, pinnedRevision}], fields}]}`, or `{written}` with `-o` |
| `show` | `{akt: {…cat's, title, typeName, stateName, createdAt, updatedAt, containers, links: [{kind, name, outgoing, akt, pinnedRevision}], members: {columns?, akter, more}, transitions, history?}}` |
| `history` | `{number, revisions: [{revision, by, at, what, reason, changes: [{field, value}], memberships}]}`; in local mode also `tracked` and `uncommitted` |
| `users`, `contacts` | `{users: [{id, name, email}]}`, `{contacts: [{id, displayName, name, email, user}]}` |

| `new` | `{number, revision}` |
| `apply` | `{applied: [{file, number, revision, changed, dryRun?}]}` |
| `set`, `do`, `draft` | `{number, revision, changed \| state, transition \| field}` |
| `add`, `remove`, `transfer`, `move`, `link`, `unlink` | what was asked, such as `{source, kind, target, pinnedRevision}` |
| `guide` | `{guide}` |
| `export`, `import` | `{dir, scope, aktTypes, akter, contacts, attachments, files}`; `{scope, aktTypes, akter, aliases}` |
| `export --to-dir` | `{dir, aktTypes, akter}` |
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
  `apply`, `set`, `do`, `draft`), `writes/arrange.rs` (membership, order,
  links) and `writes/types.rs` (documents).
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
  transfers, redefining types, and eight writers at once; and `check`, on a
  consistent directory and on one with eight kinds of problem, each named
  with its file.
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
  akt's `cat` and their order. It is how local mode is held to "the same
  rules, through a shared engine".

## Traceability

- Spec: [`docs/spec/cli.md`](../../docs/spec/cli.md) (`cli.profile`,
  `login`, `secret`, `secret-key`, `dpop`, `discovery`, `format`,
  `errors`, `guide`, `read`, `file`, `apply`, `write`, `values`, and
  `local.*` for local mode); `app.history` for the history's wording.
- Annotations: `r[impl cli.*]` in `src/`, `r[verify cli.*]` in the unit
  tests, `tests/local.rs`, `e2e/tests/cli.spec.ts` and
  `e2e/tests/parity.spec.ts`.
