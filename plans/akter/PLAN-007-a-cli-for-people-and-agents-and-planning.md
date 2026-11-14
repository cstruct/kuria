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

<!-- akt: PLAN-007-T01 -->
---
type: task
revision: 3
rank: b21
state: done
in: [PLAN-007]
title: T1 The spec
---

<!-- field: description -->
The spec:
- `akt.page` and `akt.filter` in `akter.md`; `list` and `search` now
  answer pages.
- `links.md` (`link.kinds`, `add`, `pin`, `remove`, `revisions`,
  `read`).
- `bundles.md` (`bundle.format`, `contents`, `export.*`, `import.*`);
  subjects are mapped by alias, so hashes hold.
- `cli.md` (`cli.profile`, `login`, `secret`, `secret-key`, `dpop`,
  `relay`, `format`, `errors`, `guide`, `read`, `file`, `apply`,
  `write`, `values`).
- `authz.link`, `authz.bundles` and the two `kuria:tenant:*` actions.
- `authz.sender-constrained` now says to accept DPoP-bound requests once
  Konklav 1.8 allows it, and to refuse them until then.
- `app.links`, `app.filters` and `app.my-work`.
- The planning example's README; nomenclature (`page`, `cursor`,
  `filter`, `bundle`, `subject alias`, `kuria`, `profile`, `akt file`,
  `relay`); the architecture's overview, endpoints, schema and open
  decisions.

<!-- akt: PLAN-007-T02 -->
---
type: task
revision: 3
rank: b22
state: done
in: [PLAN-007]
title: T2 Pages
---

<!-- field: description -->
Pages:
- **Store:** `list_akter` and `search_akter` take a `ChangedAfter`
  keyset, `(updated_at, id)` descending; the state filter is now inside
  the search query, so pages count right. `member_page` pages a
  container's members by rank.
- **GraphQL:** `akter` and `Akt.members` answer `AktConnection`, from
  `graphql/page.rs`. Cursors are base64url JSON naming their list by a
  digest of its kind and arguments, so another list's cursor is
  refused as `BAD_REQUEST`. Each query fetches one row past the page to
  know whether more follow. Boards keep their own limit.
- **App:** search, pickers and Home read `nodes`; the type list loads
  50 at a time with "Load more" (`paging.ts` merges pages).
- **Tests:** `presentation.rs` (lists and search, a change between
  pages), `containers.rs` (members), `page.rs` (cursors), and
  `members.spec.ts` (51 epics, "Load more").

<!-- akt: PLAN-007-T03 -->
---
type: task
revision: 4
rank: b23
state: done
in: [PLAN-007]
links: [Resolves PLAN-005-G02]
title: T3 Filters
---

<!-- field: description -->
Filters:
- **Store:** a `Condition` is any of its `Match`es: JSON containment
  (`akt.fields @> …`, served by the GIN index), or a box checked for
  oneself (its `checked_by`, and no `for`). Conditions filter lists,
  search and members through an id subquery.
- **GraphQL:** `where: [FieldCondition!]` on `akter` and
  `Akt.members` (`graphql/filter.rs`), read by the type's latest
  version. It needs `type`; it refuses unfilterable kinds, unknown
  fields, options and dates. `me`, or a named user, also matches the
  contacts linked to them. The conditions are part of the cursor's
  list.
- **App:** a type's list has a `FilterBar` per filterable field,
  filtered by `?where.<field>=`, with people from `peopleOf`. Home
  shows "My work": one `AkterWhere` per user field and checkbox,
  `equals: me`.
- **Fix:** `query()` asked again on every cache update, because
  variables built from cached answers were new objects; it now
  resubscribes only when their JSON changes. This also removed the
  endless refetching that filters exposed.
- **Tests:** `presentation.rs` and `contacts.rs` cover every kind,
  `me` and the refusals; `filters.test.ts`, `work.test.ts`;
  `members.spec.ts` (Kind, Assignee: Me, kept across a reload) and
  `home.spec.ts` (My work).

<!-- akt: PLAN-007-T04 -->
---
type: task
revision: 3
rank: b24
state: done
in: [PLAN-007]
title: T4 Links
---

<!-- field: description -->
Links:
- **Config:** `links: [LinkDef { key, name, inverse, to }]`, validated
  like `contains`. Target types needn't be defined yet, so types are
  defined in any order; `link.kinds` now says so instead of requiring
  them to exist.
- **Store:** migration 11 adds `akt_link`, with any pinned revision.
- **Write path:** `akter/links.rs`; `add_link` and `remove_link` lock
  both akter and record a membership-style revision on each (`linked`
  and `linked_from`, `unlinked` and `unlinked_from`), with the kind,
  the other akt's number, the name as that akt reads it, and any pin.
  A kind a newer version dropped can still be unlinked.
- **Refusals:** a self-link, an unknown kind, a target type the kind
  doesn't name, a duplicate, or a revision the target hasn't reached
  is refused as `INVALID_LINK`; a link that doesn't exist, as
  `NOT_FOUND`.
- **GraphQL:** `addLink`, `removeLink`, and `Akt.links { kind name
  outgoing akt pinnedRevision pinned { revision title state } }`. A
  pinned view replays the target's revisions up to the pin.
- **App:** a `LinksCard` on the akt page, grouped by name, with a
  picker per kind limited to its target types. The history says "Linked:
  Blocks X" and "changed its links".
- **Tests:** `config` (link kinds), `tests/links.rs`,
  `history.test.ts`, `e2e/tests/links.spec.ts` (on a version of
  `backlog_item` with a `blocks` kind).

<!-- akt: PLAN-007-T05 -->
---
type: task
revision: 3
rank: b25
state: done
in: [PLAN-007]
title: T4b Konklav 1.8
---

<!-- field: description -->
Konklav 1.8:
- **Konklav 1.8.0** (cstruct/konklav#43, released): an accepted
  presentation answers every decision of its request until it expires
  or is released; `release_accepted` in the core, the Rust client and
  Python; konklav-axum's `RequestDecider` extractor, with release after
  the handler.
- **Kuria:** the pins move to `=1.8.0` and the e2e Konklav to
  `cstruct/konklav:1.8.0`. `Authorizer::for_request` decides a
  DPoP-bound request on `/graphql` and the attachment endpoints with the
  `RequestDecider`; the WebSocket still refuses DPoP
  (`authz.sender-constrained` reworded).
- **Tests:** `tests/authorization.rs` decides a sender-constrained
  request with its accepted presentation and checks it is released;
  `tests/attachments.rs` uploads with a DPoP-bound token.

<!-- akt: PLAN-007-T06 -->
---
type: task
revision: 3
rank: b26
state: done
in: [PLAN-007]
title: T5 The login relay
---

<!-- field: description -->
The login relay:
- **Server:** `src/cli_relay.rs`. `GET /cli/authorized` keeps the code
  or OAuth error under the hash of its `state` in Valkey (`SET NX PX`,
  two minutes; the first redirect wins, a second is 409) and answers a
  small page with `no-store`, `no-referrer` and a strict CSP;
  `POST /cli/authorized/collect` takes `{"state"}` and answers `{"code"}`
  or `{"error", "error_description"}` once (`GETDEL`), 204 while nothing
  is kept. One limit per client address (`kuria.cli-relay`, 120 a
  minute, 1000 an hour) covers both. `Options.cli_relay` is optional, so
  tests and other embeddings leave it out.
- **Konklav:** `kuria-cli` registered in the e2e seed (also the local
  stack `e2e:serve` runs): public, PKCE and DPoP required, refresh,
  audience `kuria`, redirect `<Kuria>/cli/authorized`.
- **Tests:** `tests/cli_relay.rs`; `e2e/tests/cli-login.spec.ts` walks
  the whole link flow against the real Konklav (another browser
  authorizes, collect once, DPoP code exchange, GraphQL, bearer refused,
  refresh) and the DPoP-bound secret-key sign-in, with
  `e2e/support/dpop.ts` for proofs.

<!-- akt: PLAN-007-T07 -->
---
type: task
revision: 3
rank: b27
state: done
in: [PLAN-007]
title: "T6 The CLI's foundation (`items/cli`, binary `kuria`)"
---

<!-- field: description -->
The CLI's foundation (`items/cli`, binary `kuria`):
- **Discovery:** Kuria serves `GET /cli/config` (`cli.discovery`, new):
  Konklav's issuer, `kuria-cli`, the relay's URL and the default tenant,
  so `kuria login --url <Kuria>` is enough.
- **Modules:** `args` (clap), `profile` (`config.toml`, flags over env
  over profile), `secret` (keyring, else a `0600` file;
  `KURIA_SECRET_STORE=file`), `dpop` (P-256, proofs checked in tests by
  `konklav_token::verify_proof`), `http` (a proof on every request,
  nonce retry), `konklav` (discovery, grants, revocation, secret-key
  login and sign-out), `session` (saved, refreshed a minute ahead, or
  `KURIA_KEY`), `graphql`, `output` (`Answer { markdown, json }`),
  `error` (`CliError`, exit codes 1 to 5), `account` (`login`,
  `logout`, `whoami`).
- **Tests:** 11 unit tests; `e2e/tests/cli.spec.ts` runs the binary
  (`serve.ts` builds it): login by link through Playwright, whoami from
  the profile, a forced refresh, logout revoking at Konklav, and
  `KURIA_KEY` saving nothing, with exit codes 4 and 5.
- **Tasks:** `mise run kuria -- …` and `mise run cli:install`.
- **Noted:** a person who has only ever signed in through the CLI
  shows by email or identifier, since names are recorded at browser
  sign-in (`person.from-sign-in`).

<!-- akt: PLAN-007-T08 -->
---
type: task
revision: 3
rank: b28
state: done
in: [PLAN-007]
title: T7 Reads
---

<!-- field: description -->
Reads: `types`, `schema`, `ls`, `find`, `cat`, `show`,
`history`, `users`, `contacts`.
- **Modules:** `model` (typed answers and the GraphQL fragments),
  `values` (text as the app's `textOf`, the file's form, `--where`
  values by kind), `file` (the akt file, hand-written YAML: plain where
  it reads back the same, JSON-quoted otherwise, flow style for lists
  and objects), `history` (a port of the app's `describe`), `md`,
  `reads/{catalog,list,akter}`.
- **`schema`** reads what the GraphQL shape lacks (`contains`,
  `created_in`, `links`, `board`, drafts, transition actions) from
  `AktType.document`.
- **`ls --in`** pages `Akt.members`, which takes no state: `--state`
  with `--in` filters each page here, so a page may come out short.
- **`show`** lists members by the type's board columns when it has a
  board, else by type, 200 at most, pointing to `ls --in` for more.
- **Tests:** 18 unit tests; `e2e/tests/cli.spec.ts` gains every read
  against the release-management example.
- **For T8:** the akt file has no reader yet (`apply`), so YAML parsing
  comes with it; values in `--set` reuse `values::condition_value`'s
  kind dispatch.

<!-- akt: PLAN-007-T09 -->
---
type: task
revision: 3
rank: b29
state: done
in: [PLAN-007]
title: T8 Writes
---

<!-- field: description -->
Writes: `new`, `apply` (`--dry-run`), `set`, `do`, `draft`,
`add`, `remove`, `transfer`, `move`, `link`, `unlink`, `types export`,
`types define` (`--check`).
- **Values:** `values::input_of` reads a file's or `--set`'s value as
  the server's input; `current_input` gives the akt's own, so `apply`
  compares inputs (a user by name equals the same user by email) and
  sends only what differs. A field a file leaves out is cleared.
- **The file:** read with `serde_yaml_ng`, through its own `Value` so a
  key given twice is refused; sections by `<!-- field: … -->`; text
  outside a section is refused.
- **`apply`** refuses a stale file before anything else, naming the
  fields, state or membership changed since from the revisions; then
  read-only keys and unknown fields, with the key as the field.
- **`new --file`** ignores the file's `number`, `revision`, `state` and
  `links`, and adds its `in` to `--in`.
- **`types define --check`** runs without a session, with
  `kuria-config::validate`; cross-type references are left to the
  server. Refused documents list the server's `problems`.
- **`move`** takes no `--reason`: it makes no revision.
- **Tests:** 21 unit tests; e2e defines `cli_task` and `cli_plan` with
  the CLI and walks every write, including stale, read-only and unknown
  keys, a pinned link, and a draft (a recipe lists the containers'
  members, so drafting a task lists its plan's other tasks).

<!-- akt: PLAN-007-T10 -->
---
type: task
revision: 3
rank: b2A
state: done
in: [PLAN-007]
title: "T9 Export and import (`bundle.*`)"
---

<!-- field: description -->
Export and import (`bundle.*`):
- **Spec:** `bundle.import.request` (`subjects.json` in the archive),
  `bundle.import.ids` (new ids for akter and type versions; contacts and
  attachments keep theirs, since hashed values name them) and
  `bundle.import.order` (types' akter after the tenant's own).
- **Config:** `NumberPattern::read` reads a number back into its counter
  scope and sequence, `{parent}` and wider-than-width sequences
  included, so an import raises `number:<scope>` counters without
  copying another tenant's.
- **Store:** migration 12 (`subject_alias`, `bundle_import`);
  `bundles.rs`: bulk reads, clash checks, bulk inserts, aliases, and the
  import's record with a change event per akt.
- **Server:** `src/bundles.rs` (`GET /export`, `POST /import`, errors
  `INCOMPLETE_SELECTION`, `INVALID_BUNDLE`, `CONFLICT`,
  `UNKNOWN_AKT_TYPE`), `format`, `archive` (tar), `export` (a
  repeatable-read, read-only snapshot; bytes read after it, since
  attachments never change), `import` (checks, replaying each akt's
  revisions against their hashes, then one transaction).
  `akter::define_in` lets a bundle without akter define types inside the
  import's transaction. Aliases are followed by `User.name/email`,
  `someone_name`, and user and checkbox conditions (`me` included).
- **CLI:** `export` and `import` (`--map-subjects`).
- **e2e:** the seed adds the actions and a second tenant, `lab`, where
  the admin is a member.
- **Tests:** `tests/bundles.rs` (five, across two databases and two
  tenants); `e2e/tests/cli.spec.ts` moves a bundle into `lab`.
- **Known limits:** a bundle holding contacts or attachments can't be
  imported into a second tenant of the same Kuria (their ids clash), so
  such moves go between instances; and an import records the names a
  bundle holds even for a subject Konklav has since erased, until the
  next erasure catch-up, which doesn't revisit it. The latter is worth a
  check against Konklav's erased list at import.

<!-- akt: PLAN-007-T11 -->
---
type: task
revision: 3
rank: b2B
state: done
in: [PLAN-007]
title: "T10 The planning example and `kuria guide`"
---

<!-- field: description -->
The planning example and `kuria guide`:
- **Documents:** `docs/examples/planning/{plan,task,gap,decision}.json`.
  A plan contains tasks, gaps and decisions (each `created_in` a plan,
  numbered `{parent}-T/G/D{seq:2}`), with a board of task states;
  activating needs the goal. Tasks link by `blocks` and carry a
  `change` reference; gaps are `resolved_by` a task or decision and
  close with a resolution; decisions `supersede` decisions and are
  decided with context, decision and why. Define tasks, gaps and
  decisions before plans, whose board names task states.
- **Guide:** `items/cli/src/guide.md`, built into `kuria guide`
  (`{guide}` in JSON); a unit test fails if a command or exit code is
  missing from it.
- **Tests:** e2e defines the example from its files and works a plan as
  the guide says: board, links, a task through review to done with
  progress applied from its file, a gap and a decision refused without
  what they require.

<!-- akt: PLAN-007-T12 -->
---
type: task
revision: 5
rank: b2C
state: done
in: [PLAN-007]
title: T11 Dogfood locally
assignee: albin@mattsson.io
---

<!-- field: description -->
Done in T11:

- `dogfood/`: a stack that keeps its data (`mise run dogfood:serve`, `dogfood:kuria`, `dogfood:down`, `dogfood:reset`).
- `dogfood/scripts/migrate.ts` moved the seven Markdown plans into PLAN-001 to PLAN-007.
- AGENTS.md and PLANS.md point at the CLI.
- Found and fixed: a marker inside a code block ended a field; `show` listed unmapped members as a column.
- Found and recorded: PLAN-007-G05 to G08.

The task as planned:

Dogfood locally: migrate `.agents/plans/*.md` into plans,
tasks, gaps and decisions on a local stack, and point AGENTS.md and
PLANS.md at the CLI.

<!-- field: acceptance -->
- The Markdown plans are plans, tasks, gaps and decisions in a Kuria that keeps its data.
- AGENTS.md and PLANS.md send agents to the CLI.
- An agent reads a plan, takes a task, records progress and finishes it.

<!-- akt: PLAN-007-T13 -->
---
type: task
revision: 2
rank: b2D
state: todo
in: [PLAN-007]
links: [Blocked by PLAN-008-T06]
title: "T12 Deployment on Fly.io (postponed)"
---

<!-- field: description -->
Deployment on Fly.io (postponed):
- the image, services, Konklav's registrations, TLS, backups and a
  runbook;
- moving the local plans there with export and import.

<!-- akt: PLAN-007-G01 -->
---
type: gap
revision: 2
rank: b2E
state: closed
in: [PLAN-007]
title: G1 Where to deploy
---

<!-- field: description -->
Where to deploy: Fly.io, postponed to T12 (chosen by the user).

<!-- field: resolution -->
Where to deploy: Fly.io, postponed to T12 (chosen by the user).

<!-- akt: PLAN-007-G02 -->
---
type: gap
revision: 2
rank: b2F
state: closed
in: [PLAN-007]
title: "G2 Akter filtered by a field's value"
---

<!-- field: description -->
Akter filtered by a field's value: added as `akt.filter` (T3).

<!-- field: resolution -->
Akter filtered by a field's value: added as `akt.filter` (T3).

<!-- akt: PLAN-007-G03 -->
---
type: gap
revision: 2
rank: b2G
state: closed
in: [PLAN-007]
title: G3 Links between akter
---

<!-- field: description -->
Links between akter: added as `link.*` (T4).

<!-- field: resolution -->
Links between akter: added as `link.*` (T4).

<!-- akt: PLAN-007-G04 -->
---
type: gap
revision: 2
rank: b2H
state: closed
in: [PLAN-007]
title: G4 Listing past 200 akter
---

<!-- field: description -->
Listing past 200 akter: cursor pagination, `akt.page` (T2).

<!-- field: resolution -->
Listing past 200 akter: cursor pagination, `akt.page` (T2).

<!-- akt: PLAN-007-D01 -->
---
type: decision
revision: 2
rank: b2I
state: decided
in: [PLAN-007]
title: Verbs plus a file round trip
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Verbs plus a file round trip**, not a Unix file mapping (chosen by the
user):
- reads feel like files, and an akt edits as a Markdown file;
- transitions, membership and order stay verbs, since nothing is
  deleted and a state change isn't a write.

<!-- field: why -->
, not a Unix file mapping (chosen by the
user):
- reads feel like files, and an akt edits as a Markdown file;
- transitions, membership and order stay verbs, since nothing is
  deleted and a state change isn't a write.

<!-- akt: PLAN-007-D02 -->
---
type: decision
revision: 2
rank: b2J
state: decided
in: [PLAN-007]
title: "Login by a link through Kuria's own redirect"
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Login by a link through Kuria's own redirect** (asked for by the
user, DPoP preferred):
- Konklav requires exact redirect URIs and has no device grant, so
  Kuria relays the code;
- the code is bound to the CLI's PKCE verifier and DPoP key, so the
  relay can't use it.

<!-- field: why -->
(asked for by the
user, DPoP preferred):
- Konklav requires exact redirect URIs and has no device grant, so
  Kuria relays the code;
- the code is bound to the CLI's PKCE verifier and DPoP key, so the
  relay can't use it.

<!-- akt: PLAN-007-D03 -->
---
type: decision
revision: 2
rank: b2K
state: decided
in: [PLAN-007]
title: Import keeps numbers and history
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Import keeps numbers and history** (chosen by the user), as a
privileged, all-or-nothing server import that checks each revision's
hash.

<!-- field: why -->
(chosen by the user), as a
privileged, all-or-nothing server import that checks each revision's
hash.

<!-- akt: PLAN-007-D04 -->
---
type: decision
revision: 2
rank: b2L
state: decided
in: [PLAN-007]
title: A deployed instance
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**A deployed instance** (chosen by the user): dogfooding is real only
when the plans live where everyone, people and agents, reaches them.
Fly.io, postponed until the CLI works against a local stack (T11
before T12).

<!-- field: why -->
(chosen by the user): dogfooding is real only
when the plans live where everyone, people and agents, reaches them.
Fly.io, postponed until the CLI works against a local stack (T11
before T12).

<!-- akt: PLAN-007-D05 -->
---
type: decision
revision: 2
rank: b2M
state: decided
in: [PLAN-007]
title: "Filters, links and pagination before the CLI"
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Filters, links and pagination before the CLI** (chosen by the user):
the CLI is built on the API they shape, so `ls --where`, `link` and
`--all` arrive with their commands, not as later changes.

<!-- field: why -->
(chosen by the user):
the CLI is built on the API they shape, so `ls --where`, `link` and
`--all` arrive with their commands, not as later changes.

<!-- akt: PLAN-007-D06 -->
---
type: decision
revision: 2
rank: b2N
state: decided
in: [PLAN-007]
title: Export and import have two scopes
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Export and import have two scopes**, selected types and the whole
tenant (asked for by the user). A whole tenant goes only into an empty
one, so a restore is exact rather than a merge; Konklav's tenant and
grants are set up there separately.

<!-- field: why -->
, selected types and the whole
tenant (asked for by the user). A whole tenant goes only into an empty
one, so a restore is exact rather than a merge; Konklav's tenant and
grants are set up there separately.

<!-- akt: PLAN-007-D07 -->
---
type: decision
revision: 2
rank: b2O
state: decided
in: [PLAN-007]
title: DPoP needs Konklav 1.8
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**DPoP needs Konklav 1.8** (chosen by the user): Kuria refuses DPoP-bound
GraphQL requests because a request's accepted proof answers one
decision. Konklav will let it answer every decision of its request
(T4b), rather than the CLI starting with bearer tokens.

<!-- field: why -->
(chosen by the user): Kuria refuses DPoP-bound
GraphQL requests because a request's accepted proof answers one
decision. Konklav will let it answer every decision of its request
(T4b), rather than the CLI starting with bearer tokens.

<!-- akt: PLAN-007-D08 -->
---
type: decision
revision: 2
rank: b2P
state: decided
in: [PLAN-007]
title: "Bundles, not `transfer`"
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Bundles, not `transfer`:** the spec names export and import `bundle.*`,
since a transfer already means moving a member between containers.

<!-- field: why -->
the spec names export and import `bundle.*`,
since a transfer already means moving a member between containers.

<!-- akt: PLAN-007-D09 -->
---
type: decision
revision: 2
rank: b2Q
state: decided
in: [PLAN-007]
title: Rank sorting on a type list covers what is loaded
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Rank sorting on a type list covers what is loaded:** the server pages
most recently changed first, so "Sort by rank" orders the loaded pages
only. A server-side rank order for lists is a later option.

<!-- field: why -->
the server pages
most recently changed first, so "Sort by rank" orders the loaded pages
only. A server-side rank order for lists is a later option.

<!-- akt: PLAN-007-D10 -->
---
type: decision
revision: 2
rank: b2R
state: decided
in: [PLAN-007]
title: "\"My work\" is asked of the server per field"
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**"My work" is asked of the server per field**, since conditions all
apply and "any user field names me" isn't one: a few small lists,
merged in the app.

<!-- field: why -->
, since conditions all
apply and "any user field names me" isn't one: a few small lists,
merged in the app.

<!-- akt: PLAN-007-D11 -->
---
type: decision
revision: 2
rank: b2S
state: decided
in: [PLAN-007]
title: Subjects map by alias
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Subjects map by alias:** writing a mapped subject into revisions would
break their hashes, so an import records aliases that showing and
filtering follow.

<!-- field: why -->
writing a mapped subject into revisions would
break their hashes, so an import records aliases that showing and
filtering follow.

<!-- akt: PLAN-007-D12 -->
---
type: decision
revision: 2
rank: b2T
state: decided
in: [PLAN-007]
title: Links are declared per akt type
---

<!-- field: context -->
Recorded in the plan "A CLI for people and agents, and planning in Kuria itself", migrated from `.agents/plans/cli-and-dogfooding.md`.

<!-- field: decision -->
**Links are declared per akt type**, like `contains`, so a QMS can say
which relations a type has ("verifies", "caused by") instead of
allowing any link between any akter.

<!-- field: why -->
, like `contains`, so a QMS can say
which relations a type has ("verifies", "caused by") instead of
allowing any link between any akter.

<!-- akt: PLAN-007-G05 -->
---
type: gap
revision: 1
rank: b2U
state: open
in: [PLAN-007]
title: An agent is shown by its subject identifier
---

<!-- field: description -->
Kuria records names at browser sign-in only, so a service principal signing in with `KURIA_KEY` shows as its identifier in `whoami`, `by` and assignees. Konklav's subject metadata, or a name Kuria is told, could name it.

<!-- akt: PLAN-007-G06 -->
---
type: gap
revision: 1
rank: b2V
state: open
in: [PLAN-007]
title: "A bundle with contacts or attachments can't move between tenants of one Kuria"
---

<!-- field: description -->
They keep their identifiers on import (`bundle.import.ids`), since the hashed values that name them are kept, so a second tenant of the same Kuria clashes. Moves go between instances for now.

<!-- akt: PLAN-007-G07 -->
---
type: gap
revision: 1
rank: b2W
state: open
in: [PLAN-007]
title: An import records names a bundle holds for subjects Konklav has since erased
---

<!-- field: description -->
`bundle.import.subjects` writes the bundle's names where Kuria has none, and erasure catch-up doesn't revisit them. Checking Konklav's erased subjects at import would close it.

<!-- akt: PLAN-007-G08 -->
---
type: gap
revision: 3
rank: b2X
state: open
in: [PLAN-007]
title: Two end-to-end tests flaked once each
---

<!-- field: description -->
The members test's keyboard "Move down" (T9's runs) and the release flow (T4b's) each failed once in a full run and passed on reruns.

In T11, the contacts test once added a contact that its list never showed (no error; the form cleared); the rerun passed. Three flakes in three areas suggest the app's cache refresh after a mutation, under the load of a full run.

In PLAN-008-T03, the contacts test and the release flow (waiting for an akt's number) failed again in one full run, and passed on the next.

<!-- akt: PLAN-007-D13 -->
---
type: decision
revision: 2
rank: b2Y
state: decided
in: [PLAN-007]
title: Plans live on a local dogfooding stack
---

<!-- field: context -->
Plans had to survive restarts, and the end-to-end stack starts empty every run.

<!-- field: decision -->
Kuria's own plans live on a dogfooding stack of their own (`dogfood/`): a Konklav keeping its data on a volume, the database `kuria_dogfood`, ports apart from the tests', and an agent signing in with a secret key through `mise run dogfood:kuria`.

<!-- field: why -->
It keeps planning local until Fly.io (T12), lets tests run beside it, and dogfoods the same CLI agents will use there.
