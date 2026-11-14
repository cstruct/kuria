---
number: PLAN-007
type: plan
revision: 36
rank: b20
state: active
title: "A CLI for people and agents, and planning in Kuria itself"
---

<!-- field: goal -->
Plan Kuria's own work in Kuria, instead of in `.agents/plans/*.md`, through
a command-line client that a person or an LLM agent uses as easily as it
reads and edits files. After this work:

- **Sign in by link:** `kuria login` prints a link (and opens it when it
  can). Authorizing it in a browser signs the CLI in with a DPoP-bound
  session. Agents and CI sign in with a secret key, also DPoP-bound.
- **Read like files:** `kuria ls`, `kuria cat`, `kuria show`, `kuria find`
  and `kuria schema` answer in Markdown by default and in JSON on request.
  `cat` gives an akt as a Markdown file with front matter.
- **Edit like files:** edit that file and `kuria apply` it. The revision
  in its front matter makes a change made elsewhere meanwhile a refusal,
  never a silent overwrite. State, membership and order change through
  explicit verbs, because they aren't edits.
- **Move data between tenants:** `kuria export` and `kuria import` move
  selected akt types, with or without their akter, or a whole tenant,
  between tenants and instances. Akter keep their numbers and full
  history.
- **Ask precise questions:** akter can be filtered by their fields' values
  (`kuria ls task --where assignee=@me`), linked to each other ("blocks",
  "relates to"), and listed past any limit, page by page.
- **Plan in Kuria:** a planning example (plan, task, gap, decision) runs on
  a deployed Kuria, the existing plans are migrated into it, and
  AGENTS.md and PLANS.md say to plan there.

Success: this plan's own tasks are tracked in the deployed Kuria through
the CLI, and an agent can follow AGENTS.md to read a plan, pick a task,
record progress and close it, without touching a Markdown plan.

<!-- field: design -->
## Where It Starts

- **API:** GraphQL covers reading and writing akter, types, containers,
  order, drafts and contacts. Every write that changes fields or state
  takes the revision the caller saw (`akt.revision.concurrency`).
- **Software clients:** they sign in like the build pipeline does in
  `e2e/support/api.ts`: a Konklav secret key, then
  `/v1/login/tenant`. Their tokens have audience `konklav`.
- **DPoP:** Kuria already verifies DPoP proofs (`auth.dpop`).
- **What Konklav offers:** public OIDC clients, PKCE, DPoP-bound sessions
  from the token endpoint, and `dpop_jkt` binding an authorization code
  to a key (`oidc.provider.dpop.*`). It has no device grant, and redirect
  URIs must match exactly, so a loopback redirect on a random port won't
  do.
- **Numbering:** akter can't be created with a given number, and nothing
  writes revisions except the write path. An import that keeps numbers
  and history is new server work.
- **Deployment:** nothing exists yet. There's no image for Kuria, no
  hosting and no Git remote; Konklav ships as `cstruct/konklav`.

## Constraints And Assumptions

- **Nothing is deleted.** Revisions are never removed or changed
  (`akt.revision.record`). The CLI has no `rm` that deletes: removing
  takes an akt out of a container, and closing it is a transition.
- **The CLI knows no akt type**, like the app (`app.akt-types`). The
  planning example is data.
- **Rust**, in this workspace (`items/cli`, binary `kuria`), reusing
  `kuria-config` to validate type documents offline. It follows
  `.agents/skills/konklav-rust/SKILL.md`.
- **Secrets stay off disk in clear text where the OS allows it:** the DPoP
  key and refresh token go in the OS keychain, falling back to a `0600`
  file.
- **Imported subjects may differ:** an export from one Konklav, imported
  into another, names subjects the new one doesn't know. The import maps
  them.

## Design

### Commands

`kuria <command>`. Every command takes:
- `--format md|json`: Markdown by default, JSON for scripts;
- `--url` and `--tenant`, or `KURIA_URL` and `KURIA_TENANT`, or the
  profile `kuria login` saved.

| Command | What it does |
|---|---|
| `login`, `logout`, `whoami` | Sign in by link; end the session; who and which tenant. |
| `types` | The akt types with counts. |
| `schema <type>` | A type's fields (kind, required, options), states, transitions (and what they require), what it contains, its board and presentation. |
| `ls [<type>] [--state s] [--in C] [--where k=v]… [--search text] [--limit n] [--all]` | Akter as a table: number, title, state and the presentation's list fields. `--where` filters by a field's value (`@me` for oneself); `--all` follows every page. |
| `find <text>` | Search by number or title words (`akt.search`). |
| `cat <NUMBER>…` | Each akt as an editable file (below). |
| `show <NUMBER>` | Everything about one akt: fields, containers, members by column and type, the transitions it can take, and with `--history` its revisions. |
| `history <NUMBER>` | Its revisions, as the app's history says them. |
| `new <type> [--in C]… [--set k=v]… [--file f.md]` | Create an akt; prints its number. |
| `apply <file.md>… [--dry-run] [--reason r]` | Write the fields a file changed; refuse STALE, saying what changed meanwhile. |
| `set <NUMBER> k=v… [--reason r]` | Edit fields without a file. |
| `do <NUMBER> <transition> [--set k=v]… [--reason r]` | Perform a transition, giving the fields it requires. |
| `add <CONTAINER> <MEMBER>…`, `remove …` | Membership. |
| `transfer <MEMBER> <FROM> <TO>` | Move between containers in one change. |
| `move <NUMBER> --before X` / `--after X` | Reorder (`moveAkt`). |
| `link <A> <kind> <B> [--reason r]`, `unlink …` | Link two akter by a kind A's type declares, such as `PLAN-007-T03 blocks PLAN-007-T04`. |
| `draft <NUMBER> <field>` | Draft a field from its recipe. |
| `types export [<key>…] [-o dir]`, `types define <file>…` | Akt type documents out and in; `define --check` validates offline with `kuria-config`. |
| `export (--types k,… [--akter] \| --whole-tenant) -o dir`, `import dir [--map-subjects file]` | Selected types, or the whole tenant, out and in (below). |
| `users`, `contacts` | Who values can name. |
| `guide` | A short guide for agents: these commands, the file format, the exit codes, the planning conventions. |

### Output, errors and exit codes

- **Markdown answers** read like the app: headings, tables, and values
  as `textOf` shows them. JSON answers are stable objects documented in
  the spec, not raw GraphQL.
- **Refusals** print the server's code and message, with the field they
  belong to. Under `--format json` they print `{ "error": { "code",
  "message", "field" } }` to stderr.
- **Exit codes:** 0 done; 1 refused (bad request, invalid value); 2 not
  found; 3 stale; 4 not signed in or forbidden; 5 unavailable.

### The akt file

`kuria cat` writes, and `apply` and `new --file` read, Markdown with YAML
front matter:

```markdown
---
number: PLAN-007-T03
type: task
revision: 4          # the revision this file was read at
state: doing         # read-only: change it with `kuria do`
in: [PLAN-007]       # read-only: change it with add/remove/transfer
title: Export and import with history
assignee: albin@mattsson.io   # users and contacts by email or name; "@me"
---

<!-- field: description -->
What to build, in Markdown…

<!-- field: acceptance -->
- Export and import keep numbers…
```

- Single-line fields go in the front matter, by key. Each multiline
  field is a section under its own marker, in the type's field order, so
  edits survive a round trip.
- `apply` compares the file with the akt at the file's revision and
  sends only the fields that changed (`oneField`, as the app does).
- Front-matter keys that are read-only, or that the type doesn't have,
  are refused, never ignored.

### Login by link, DPoP-bound (`cli.login`)

`kuria-cli` is a public Konklav client that requires DPoP. Its one
redirect URI is Kuria's own `https://<kuria>/cli/authorized`, with
audience `kuria`.

1. **Start:** the CLI makes an ES256 DPoP key, a PKCE verifier and a
   random `state`. It prints the authorization URL (with
   `code_challenge`, `dpop_jkt` and a `tenant:` scope) and opens it if it
   can.
2. **Authorize:** the person signs in through Kuria's login page and
   Konklav redirects to `/cli/authorized?code&state`. Kuria keeps the
   code for that state for two minutes and says "You can return to your
   terminal".
3. **Collect:** the CLI polls `POST /cli/authorized/collect` with the
   state and gets the code, once.
4. **Exchange:** the CLI exchanges the code at Konklav's token endpoint
   with the verifier and a DPoP proof. The tokens are DPoP-bound, and
   the refresh token is kept in the keychain.

The link may be opened on any machine. The code that passes through Kuria
is useless without the verifier and the key, which never leave the CLI
(`oidc.provider.dpop.code-binding`). Secret-key sign-in (`KURIA_KEY`)
sends a DPoP proof too (`session.dpop.bind`).

### Filtering by field values (`akt.filter`, was G2)

- **The argument:** `akter(where: [{ field, equals }])` filters by the
  value of a field of the type asked for. So does `members(where:)`.
- **Kinds:**
  - a choice by option key;
  - a user field or a checkbox's `for` by a user or contact (`me` for the
    caller);
  - a date;
  - a single-line text field exactly.
- **Conditions:** several conditions all apply. A field that isn't
  filterable, or a type that isn't given, is refused.
- **Index:** it uses the existing GIN index on `akt.fields`.
- **The app:** a type's list page gets the same filters, and a "My work"
  view on Home lists the akter whose user fields name the viewer (the
  earlier G2 of usable-design).

### Links between akter (`link.*`, was G3)

- **Declaring them:** an akt type document declares the kinds of link its
  akter may have, by key: `"links": [{ "key": "blocks", "name": "Blocks",
  "inverse": "Blocked by", "to": ["task"] }]`. They're validated like
  `contains`: the target types must exist, and kinds are unique.
- **Making them:** `addLink` and `removeLink` (with a reason) record a
  revision on both akter, as membership does (`membership` gains `linked`
  and `unlinked`). They need `kuria:akter:edit` on the source's type and
  read on the target's.
- **Reading them:** `Akt.links` lists the outgoing links by kind and the
  incoming ones by inverse name, each only if the reader may read the
  other akt.
- **Shown:** on the akt page as a "Links" card with a picker, and in
  `kuria show` and `cat` (read-only in the file; changed with
  `kuria link`).
- **No cycle checks:** a task may block one that blocks it; this is data,
  not a scheduler.

### Cursor pagination (`akt.page`, was G4)

- **Connections:** `akter` becomes a connection: `akter(first, after, …)
  { nodes, pageInfo { endCursor, hasNextPage } }`, and so do `members`
  and search.
- **Cursors** are opaque and encode the sort key: `(updated_at, id)` for
  lists and search, `(rank, id)` for members. A cursor from another query
  is refused. Paging is keyset, so a list changing between pages neither
  repeats nor skips an akt that kept its place.
- **Limits:** `first` still caps a page at 200. Boards keep their own
  limit.
- **The app:** it moves to `nodes`, with "Load more" on type lists. The
  CLI's `--all` follows pages.

### Export and import with numbers and history (`bundle.*`)

**Export** (`GET /export`, `kuria:tenant:export`, tenant-wide) is one
consistent snapshot. It writes a directory:
- `bundle.json`: every version of the chosen types (akter are pinned to
  versions); the akter, with every revision as stored (author, time,
  transition, reason, changes, state, hash); memberships and ranks; the
  tenant's number counters; the contacts values name; and the people
  that authors and values name (subject, name, email).
- `attachments/`: the bytes, by content hash.

**Import** (`POST /import`, `kuria:tenant:import`, tenant-wide) has these
properties:
- **All or nothing:** one transaction into the caller's tenant.
- **No clashes:** an akt number, type key or contact email the tenant
  already has is refused.
- **Checked:** every revision's hash is checked before anything is
  written.
- **Numbering continues:** counters move past the imported numbers.
- **Subjects mapped:** they map by `--map-subjects` (old subject to new),
  else by email to a user who has signed in to the tenant, else they stay
  as they are, with their recorded names. A mapping is recorded as a
  subject alias, not written into revisions, so revisions and their
  hashes never change.
- **Recorded:** the import itself is a change event, for audit, but
  revisions are restored as they were, not re-signed.

**Two scopes**, both inside the caller's tenant and never across tenants:
- **Selected types** (`--types k,…`): those types, without akter or with
  them (`--akter`). Without akter, the bundle holds type documents only,
  and the import defines them like `defineAktType`. With akter, it holds
  the parts above that those akter need; a member, link or value naming an
  akt outside the selection is refused at export, unless `--types` widens
  to include it.
- **The whole tenant** (`--whole-tenant`): everything Kuria holds for the
  tenant, for a backup or a move to another instance:
  - every type with all its versions, and every akt with its history,
    memberships, links and order;
  - all its contacts, linked or not, with who linked them;
  - its counters;
  - the users who signed in to it (`tenant_person`) and their recorded
    names;
  - its attachments.

  A tenant import goes only into a tenant that holds nothing yet, and
  restores it whole, so the tenant reads as it did. Konklav's side (the
  tenant, memberships, roles and grants) isn't Kuria's to move: it is set
  up in the target Konklav first, and subjects map as above.

**Format:** the bundle names its format version and scope. An import
refuses a format it doesn't know, or a tenant bundle into a tenant that
isn't empty.

### The planning example (`docs/examples/planning`)

| Akt type | Number | Contains | Fields | States |
|---|---|---|---|---|
| `plan` | `PLAN-{seq:3}` | tasks, gaps, decisions | title, goal (md), design (md), verification (md) | draft, active, done, dropped |
| `task` | `{parent}-T{seq:2}` | | title, description (md), acceptance (md), assignee | todo, doing, review, done, dropped |
| `gap` | `{parent}-G{seq:2}` | | title, description (md), resolution (md) | open, closed |
| `decision` | `{parent}-D{seq:2}` | | title, context (md), decision (md), why (md) | proposed, decided, superseded |

- **The plan's board:** tasks by state. Gaps and decisions are listed by
  type (`app.members`).
- **Required fields:** `done` requires nothing, `closed` requires a
  resolution, and `decided` requires the decision and why.
- **Presentation:** icons and colours for each type.

### Deployment (postponed)

On Fly.io (chosen by the user), after the CLI is built and used against a
local stack first.

- **Image:** one image for Kuria, built in two stages (the app's build,
  then the server serving it).
- **Services:** Konklav's published image, Postgres, Valkey, and
  S3-compatible storage.
- **Konklav setup:** registrations for Kuria's app (confidential) and
  `kuria-cli` (public, DPoP).
- **Host:** Fly.io, with Postgres, Valkey and storage chosen when this is
  picked up.
- **Backups:** a nightly `kuria export --akter` from CI, plus the
  databases' own backups.

<!-- field: verification -->
- **CLI integration tests:** the server's test router with a fake host,
  as `items/server/tests` do, for every command, the file round trip,
  STALE refusals and exit codes.
- **End to end:** against the e2e stack's real Konklav, `kuria login`
  through Playwright authorizing the link, DPoP-bound tokens, and a
  refresh.
- **Pagination, filters and links:** server tests for keyset paging while
  the list changes between pages, refused cursors, every filterable kind
  and `me`, links authorized both ways with revisions on both akter; the
  app's end-to-end tests and axe for "Load more", "My work" and "Links".
- **Export and import:** a full round trip between two databases, with
  identical revisions and hashes, attachments' bytes, continued
  numbering and mapped subjects. A whole tenant restored into an empty
  one reads the same through GraphQL; a tenant bundle into a non-empty
  tenant, and a selection that leaves out an akt it names, are refused.
- **Dogfooding:** this plan's remaining tasks tracked through the CLI,
  first on a local stack (T11), then on Fly.io (T12).
