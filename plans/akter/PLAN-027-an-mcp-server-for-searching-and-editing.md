---
number: PLAN-027
type: plan
revision: 26
rank: b5k
state: active
title: "An MCP server for searching and editing documents, alongside live typing"
owner: albin@mattsson.io
---

<!-- field: goal -->
An agent such as Claude, connected to Kuria through MCP (the Model Context Protocol), finds documents by what they say, reads them and edits them, while people type in the same live draft. Its edits merge with theirs as Yjs merges typing: they never undo characters someone typed between the agent's read and its write, and an edit to a span someone has just changed is refused, so the agent reads it again rather than overwriting it. You know it works when, in `mise run example documents`, a person types in one paragraph of a document in the browser while an agent, through `kuria mcp`, searches for a phrase in another paragraph and replaces it: both changes are in the draft, the browser shows the agent's change within a second without losing a character, and the draft's changes name who made each.

<!-- field: design -->
## Constraints And Assumptions

- Live drafts (PLAN-016, [live-drafts.md](docs/spec/live-drafts.md)) are Yjs documents the server keeps with `yrs`: each `markdown` and multiline `text` field a `Y.Text` of its text. `editAkt` on such an akt already writes the draft only, changing a text by the characters that differ (`live.edit`), and every client gets the change from Postgres within a second (`live.sync`, `live.instances`). Akter without live drafts are edited by revisions, with `akt.revision.concurrency`.
- That is not enough for an agent. An agent reads, thinks for tens of seconds and writes a whole field; `editAkt` then diffs it against the draft as it is *now*, so whatever a person typed in between is deleted. Yjs never sees two concurrent edits, only one that removes text.
- Search today (`akt.search`, `store::search_akter`) matches a number's prefix or a title's words only. Documents are found by what they say.
- Kuria is a Konklav relying system: the MCP server authenticates as the CLI does (`cli.login`, `cli.secret-key`, `cli.dpop`), with no token handling of its own.
- Everything goes through GraphQL and the write path, so authorization (`authz.read`, edit rights), checks (`akt.field.kinds`, headings, references) and history (`live.history`) apply to agents unchanged.

## Edits anchored by text

The agent never sends a whole field. It sends replacements, each `{field, old, new}`: `old` a span of the field's text as the agent read it, which must occur exactly once in the field as it is when the edit is applied, and `new` what it becomes. This is the shape coding agents already use for files.

A new mutation, `replaceInAkt(number, replacements)`, applies them on the server, under the akt's row lock, in one change:

- For an akt with a live draft, each replacement is resolved against the draft as stored and applied as the least Yjs change (only the characters of that span that differ, as `LiveDraft::write` does), then checked and kept as any update to the draft is (`live.edit`, `live.history`). People typing elsewhere in the field are untouched; the change reaches them as any update does.
- For an akt without one, the replacements are resolved against its latest revision and make one revision. It doesn't name a revision: a replacement whose `old` still occurs exactly once is not stale.
- A replacement whose `old` occurs nowhere, or more than once, refuses the whole change, with a code (`NOT_FOUND_IN_FIELD`, `AMBIGUOUS`, names to be settled in the spec) and the field, so the agent reads again or widens `old`.

The CLI could do the same with `editAkt` (read, replace, write the whole field), but the gap between its read and its write would still undo typing; on the server, under the lock, there is none.

## Searching what documents say

`akter(search:)` also matches text fields, through [pg_search](https://github.com/paradedb/paradedb/tree/main/pg_search) (PLAN-027-D05): a BM25 index on `akt_search`, a table with a row per text and Markdown field of each akt (tenant, akt, field key, text) as it reads, which for an akt with a live draft is the draft. The write path keeps it, in the transaction of the change. Answers are ranked by relevance and say which field matched, with a highlighted excerpt, so the agent can choose what to read. Readability is checked as for any read.

Kuria then needs Postgres with pg_search everywhere it runs: an image of its own, `postgres:18.x-trixie` with ParadeDB's package and pgvector, preloaded (PLAN-027-T10). Operators run Postgres themselves or on ParadeDB's offerings, since managed ones don't offer it; its indexes live on the primary only in the community edition, and it is AGPL-3.0 (PLAN-027-G02).

## `kuria mcp`

A subcommand of the CLI ([items/cli](items/cli/README.md)) speaking MCP over standard input and output, with the Rust MCP SDK (`rmcp`). It signs in as the CLI does, by profile or `KURIA_KEY`, and in local mode (`--dir`) works on an akt directory, so it can edit `plans/` too. Its tools, kept few:

- `search_documents(text, type?)`: akter whose number, title or text match, with the field and excerpt.
- `read_document(number, field?, heading?)`: an akt's fields as Markdown, as `kuria cat` writes it; with `heading`, only that section of a Markdown field, by its heading id, so long documents needn't be read whole. It says whether the akt has a live draft.
- `edit_document(number, replacements)`: `replaceInAkt`.
- `list_types()` and `schema(type)`: as `kuria types` and `kuria types schema`, so the agent knows field keys.

Refusals come back as MCP tool errors carrying the CLI's error code and field, so the agent can act on them.

Kuria's MCP runs only locally, in the CLI, so that it signs in and proves its key (DPoP) as the CLI does; Kuria serves no MCP over HTTP, and hosted clients that can't start a local process aren't served (PLAN-027-G03, PLAN-027-D06).

## Who made an agent's edit

A change made through an agent is the person's, their access applying, and also names the agent (PLAN-027-D04). Kuria takes the agent from the token's actor chain (`act`), verified by Konklav's relying crates, and keeps it beside `by` on every change: revisions, draft updates and pushes, so that history, `draftChanges` and awareness read "Claude for Albin" (PLAN-027-T08). That also shows who impersonated someone, which Kuria hides today. Konklav issues such tokens by a delegation it doesn't have yet: a person starting a scoped, time-bounded child session for an agent's service principal (PLAN-027-T09). Until then, edits through `kuria mcp` are the person's alone; Kuria records nothing the client merely declares.

## Later: a visible agent

Through the above, people see the agent's changes but not the agent. A later task makes `kuria mcp` a Yjs peer: it joins `GET /akter/{number}/live` (y-websocket) while editing, applies its replacements as Yjs operations there, and sets its awareness cursor to the span it is changing, so the editor shows where it works. Its correctness doesn't change, only what people see.

<!-- field: verification -->
- Server tests (`items/server/tests/`): a `replaceInAkt` applied while another client's update to the same field, elsewhere, has landed since the agent read keeps both; one whose `old` was changed meanwhile is refused and changes nothing; an akt without a live draft gets one revision; who made the change is in `Akt.draftChanges`.
- Store tests: search finds an akt by a word of its body and of its live draft, never one the subject may not read, and names the field and excerpt.
- CLI tests for `kuria mcp`: each tool against a test server and in local mode, through an MCP client in the test, and refusals as tool errors.
- By hand, as the goal says: `mise run example documents`, a browser typing and an agent connected through `kuria mcp` editing the same document.
- `mise run lint` (Tracey references to the new rules) and `mise run test`.

<!-- akt: PLAN-027-T01 -->
---
type: task
revision: 8
rank: b5l
state: done
in: [PLAN-027]
links: [Blocks PLAN-027-T02, Blocks PLAN-027-T03]
title: "Spec replaceInAkt, text search and kuria mcp"
assignee: albin@mattsson.io
---

<!-- field: description -->
Rules: `akt.replace`, `akt.replace.match`, `akt.search.text`, `akt.search.text.match`, `akt.revision.actor` (docs/spec/akter.md), `auth.actor` (authentication.md), and docs/spec/mcp.md (`mcp.stdio`, `mcp.tools`, `mcp.errors`, `mcp.search`, `mcp.read`, `mcp.edit`, `mcp.types`, `mcp.presence`). Terms: actor, replacement, text search, agent, MCP server. Architecture: agents, text search, actors, pg_search. Text search became its own query (PLAN-027-D07). Checked with `mise run lint` (tracey validates).

<!-- field: acceptance -->
Rules in docs/spec (akter.md or live-drafts.md for replaceInAkt, akt.search extended, cli.md or a new mcp.md for the tools), terms in docs/nomenclature.md, the design in docs/architecture.md; PLAN-027-G02 and PLAN-027-G03 closed.

<!-- akt: PLAN-027-T02 -->
---
type: task
revision: 7
rank: b5m
state: done
in: [PLAN-027]
links: [Blocked by PLAN-027-T01, Blocks PLAN-027-T04]
title: Replace spans of text fields on the server
assignee: albin@mattsson.io
---

<!-- field: description -->
`kuria_engine::replace` finds each span in the fields as they are and answers the values an edit writes, refusing `NoMatch`, `Ambiguous` and `BadReplacement`; `replaceInAkt` runs it through `change_in` without a revision (`expected_revision: None`), so a live draft takes it as the least Yjs change and any other akt one revision. Verified: engine unit tests (`replace`), `tests/live_drafts.rs` (a colleague's edit since the read is kept; NO_MATCH, AMBIGUOUS, BAD_REQUEST), `tests/akter.rs` (one revision after another's edit); `mise run lint`.

<!-- field: acceptance -->
replaceInAkt applies replacements under the row lock as the least Yjs change for a live draft, or as one revision otherwise; refuses a missing or ambiguous span with a code and field; tested against a concurrent update to the same field.

<!-- akt: PLAN-027-T03 -->
---
type: task
revision: 6
rank: b5n
state: todo
in: [PLAN-027]
links: [Blocked by PLAN-027-T01, Blocks PLAN-027-T04, Blocked by PLAN-027-T10]
title: Search akter by what their text says
---

<!-- field: acceptance -->
`textSearch` answers matches of text and `markdown` fields through a pg_search BM25 index on `akt_search`, which the write path keeps (a live draft as it reads), ranked by relevance, with the field and a marked excerpt; only readable akter, archived as asked (`akt.search.text`, `akt.search.text.match`); store and server tests.

<!-- akt: PLAN-027-T04 -->
---
type: task
revision: 6
rank: b5o
state: todo
in: [PLAN-027]
links: [Blocked by PLAN-027-T02, Blocked by PLAN-027-T03, Blocks PLAN-027-T05, Blocks PLAN-027-T06, Blocks PLAN-027-T07]
title: kuria mcp over standard input and output
---

<!-- field: acceptance -->
search_documents, read_document (with heading), edit_document, list_types and schema, signed in or in local mode; refusals as tool errors; CLI tests through an MCP client; items/cli README updated.

<!-- akt: PLAN-027-T05 -->
---
type: task
revision: 5
rank: b5p
state: todo
in: [PLAN-027]
links: [Blocked by PLAN-027-T04, Blocked by PLAN-027-T08, Blocked by PLAN-027-T09]
title: Name the agent in history and presence
---

<!-- field: acceptance -->
Edits through `kuria mcp` on a delegated session (PLAN-027-T09) are the person's and name the agent, as PLAN-027-D04 decides: in history, `draftChanges` and awareness.

<!-- akt: PLAN-027-T06 -->
---
type: task
revision: 2
rank: b5q
state: todo
in: [PLAN-027]
links: [Blocked by PLAN-027-T04]
title: "Show the agent's cursor as a Yjs peer"
---

<!-- field: acceptance -->
While editing, kuria mcp joins /akter/{number}/live, applies its replacements there and sets its awareness to the span it changes; the editor shows it.

<!-- akt: PLAN-027-T07 -->
---
type: task
revision: 2
rank: b5r
state: todo
in: [PLAN-027]
links: [Blocked by PLAN-027-T04]
title: Walk the documents example with an agent
---

<!-- field: acceptance -->
In mise run example documents, a person typing and an agent editing through kuria mcp end with both changes, as the plan's goal says; written up in docs/examples/documents.

<!-- akt: PLAN-027-G01 -->
---
type: gap
revision: 3
rank: b5s
state: closed
in: [PLAN-027]
title: "Who an agent's edits are attributed to"
---

<!-- field: description -->
With a person's CLI session, draft history and awareness (`live/presence.rs` stamps the subject's name) show the person, not the agent. Settles PLAN-027-T05.

## What exists

- Kuria records one subject per change (`by`, from the token's `sub`); `Someone` is a user or a contact. It never reads the token's `act` claim.
- Konklav tokens carry an actor chain, `act` (`token.access.actor-chain`): the subject really acting behind a session. Konklav issues it only for impersonation and assumption (`authn.impersonation.actor-chain`): `POST /v1/impersonate` needs `konklav:subject:impersonate`, and the child session lasts at most an hour. So today Kuria also hides who impersonated someone, against the intent that "the real actor is never hidden".
- Service principals (`subject kind` `service`, a secret key) work already: `KURIA_KEY`, `cli.secret-key`, and `person.name` shows one by its subject identifier.

## Options

1. **As the person.** Nothing to build; the history doesn't say an agent wrote it.
2. **A service principal of its own.** Verified and works today, but it acts with its own memberships, not the person's, says nothing of whom it worked for, and one shared by everyone using a hosted MCP (PLAN-027-G03) is wrong.
3. **The person, through the agent, verified.** The change is the person's (their access, their accountability) and records the agent from `act`. Kuria reads `act` and keeps it on each change, `draftChanges` and awareness; Konklav needs a delegation it doesn't have: a person starting a child session for an agent's service principal, scoped and time-bounded, revoked with the parent.
4. **The person, with a label the client declares.** `kuria mcp` says it is an agent, and Kuria keeps that beside `by`. Cheap, and the access stays the person's, but it isn't verified: anything with the session may claim it or leave it out.

<!-- field: resolution -->
The person, through the agent (option 3), as PLAN-027-D04 decides: Kuria reads Konklav's actor chain (PLAN-027-T08), Konklav delegates a session to an agent (PLAN-027-T09), then PLAN-027-T05.

<!-- akt: PLAN-027-G02 -->
---
type: gap
revision: 3
rank: b5t
state: closed
in: [PLAN-027]
title: How search ranks and which language it stems
---

<!-- field: description -->
Ranking (recency as today, or relevance), how text is tokenized and stemmed, and where the index lives.

## pg_search (ParadeDB), proposed

[pg_search](https://github.com/paradedb/paradedb/tree/main/pg_search), at 0.27.0: BM25 ranking, tokenizers per column (unicode, ICU, ngram, stemming by language), highlighting, filters on indexed columns such as `tenant_id` pushed into the index scan. What it asks of Kuria:

- **Postgres 15 to 18**, a prebuilt package (Debian bookworm and trixie, Ubuntu, RHEL, macOS arm64) or the `paradedb/paradedb` image. It needs `shared_preload_libraries = 'pg_search'` and pgvector (`CREATE EXTENSION pg_search CASCADE`, as a superuser). Kuria uses no extension today.
- **Every Postgres Kuria runs on:** `dev/compose.yaml` (`postgres:18.1`), the end-to-end and dogfooding stacks (`konklav-dev-postgres`, shared with Konklav's image), CI and every test database (`TestDatabase`). An image of Kuria's own, `postgres:18.x-trixie` with the package, would serve all of them.
- **Managed Postgres:** ParadeDB documents only its own cloud, BYOC, CloudNativePG and a few PaaS; RDS, Cloud SQL and Azure don't list it (not verified). Operators would run Postgres themselves.
- **Replicas:** the community edition doesn't replicate its indexes, which exist on the primary only; a failover rebuilds them. Physical replication is in ParadeDB Enterprise.
- **Licence:** AGPL-3.0, commercial licences from ParadeDB. Kuria (MIT) doesn't link it, but whoever runs Kuria runs AGPL software as a network service, which some organisations' policies refuse.
- One ParadeDB index per table.

## The alternative

Postgres's own full-text search: `tsvector` with a GIN index, `ts_rank` (not BM25), `ts_headline` for excerpts, stemming by a text search configuration. Runs on any Postgres, managed ones included; weaker ranking, no ngram or fuzzy matching.

## Either way

Index a table of its own, `akt_search` (tenant, akt, field key, text), that the write path keeps: each text and Markdown field of each akt as it reads, its live draft's for an akt with one. A row per field says which field matched and gives its excerpt, and akt types' differing fields need no JSON index.

<!-- field: resolution -->
pg_search, as PLAN-027-D05 decides: BM25 ranking over `akt_search`, a row per text field, tokenized per column; Kuria's own Postgres image with it (PLAN-027-T10). Which tokenizer and language per tenant is settled in PLAN-027-T01's spec.

<!-- akt: PLAN-027-G03 -->
---
type: gap
revision: 3
rank: b5u
state: closed
in: [PLAN-027]
title: "Where MCP runs: a local process or the server"
---

<!-- field: description -->
`kuria mcp` over standard input and output serves clients on the person's machine (Claude Code, desktop apps). claude.ai connectors and other hosted clients need remote MCP over HTTP, which the server would host.

## What remote MCP asks (specification 2026-07-28)

- **Transport:** Streamable HTTP, one endpoint taking `POST`, answering JSON or an event stream; no sessions, `GET` answered 405; `Origin` checked.
- **Kuria as a resource server:** Protected Resource Metadata (RFC 9728) naming the authorization server, found by a 401's `WWW-Authenticate: Bearer resource_metadata=…` or at `/.well-known/oauth-protected-resource`. Tokens arrive as `Authorization: Bearer`; DPoP isn't mentioned. Kuria must accept only tokens issued for it, which the relying crates check by audience.
- **The authorization server:** OpenID Connect discovery is allowed. Clients send RFC 8707's `resource` in every authorization and token request. They get a client id by, in order: pre-registration (or the person entering it), Client ID Metadata Documents, Dynamic Client Registration (deprecated).

## What Konklav has

- An OpenID provider (`oidc.provider.*`): discovery, authorization code flow with PKCE, refresh tokens, a tenant by scope or by choosing at login. Access tokens are Konklav's, with the client's audience, so registering a client with Kuria's audience gives tokens Kuria verifies unchanged.
- Clients are registered in the control plane only, with exact redirect URIs: no Client ID Metadata Documents and no dynamic registration. An operator would register each host (claude.ai, …), and the person would enter its client id, if the host lets them (not verified for claude.ai).
- Not verified: whether Konklav ignores the `resource` parameter or refuses it.
- An OIDC client is a service principal (`client` credentials), yet tokens issued to it name only the person. Naming the client in `act` would be the delegation PLAN-027-D04 needs: a person authorizing a client is a person delegating to an agent, scoped and revocable. `kuria mcp` could sign in the same way, as a public client with a loopback redirect, so PLAN-027-T09 would be one Konklav change for both.

## Shape in Kuria

The tools defined once, in a crate both use, over a GraphQL executor: `kuria mcp` sends the documents over HTTP, the server runs them in-process as the caller, so authorization and checks are the same.

<!-- field: resolution -->
Hosted MCP over HTTP is its own plan, PLAN-028. PLAN-027 is `kuria mcp` alone, with its tools in a crate PLAN-028 can serve from the server.

<!-- akt: PLAN-027-D01 -->
---
type: decision
revision: 3
rank: b5v
state: decided
in: [PLAN-027]
title: "Agents edit by replacing spans of text, resolved on the server"
decided_by: albin@mattsson.io
---

<!-- field: context -->
An agent reads, thinks and writes seconds later; writing a whole field through editAkt then undoes what people typed in between, since the server diffs against the draft as it is.

<!-- field: decision -->
Edits are replacements {field, old, new}, old occurring exactly once in the field when applied; replaceInAkt applies them under the akt's row lock, as the least Yjs change for a live draft.

<!-- field: why -->
Anchoring by text instead of offsets or whole values survives edits elsewhere and refuses edits to a changed span; on the server there is no gap between resolving and writing; it is the shape coding agents already use.

<!-- akt: PLAN-027-D02 -->
---
type: decision
revision: 3
rank: b5w
state: decided
in: [PLAN-027]
title: "Start with GraphQL; join the live socket only to show the agent"
decided_by: albin@mattsson.io
---

<!-- field: context -->
kuria mcp could be a Yjs peer over y-websocket from the start.

<!-- field: decision -->
The first version calls GraphQL only; joining /akter/{number}/live is PLAN-027-T06, for presence.

<!-- field: why -->
Correctness comes from replaceInAkt either way; a peer needs a socket per open document, WebSocket authentication and more state in the MCP process, and adds only what people see.

<!-- akt: PLAN-027-D03 -->
---
type: decision
revision: 3
rank: b5x
state: decided
in: [PLAN-027]
title: The MCP server is a subcommand of the CLI
decided_by: albin@mattsson.io
---

<!-- field: context -->
An MCP server needs sign-in, DPoP, a GraphQL client, akt files and local mode, all of which items/cli has.

<!-- field: decision -->
kuria mcp, in items/cli, with rmcp, reusing the CLI's session and commands.

<!-- field: why -->
Nothing is duplicated, local mode comes free, and it ships with the CLI people already install.

<!-- akt: PLAN-027-D04 -->
---
type: decision
revision: 2
rank: b5y
state: decided
in: [PLAN-027]
title: "An agent's edits are the person's, through the agent, from Konklav's actor chain"
decided_by: albin@mattsson.io
---

<!-- field: context -->
An agent edits with a person's session, so every change and its presence show the person alone (PLAN-027-G01). A service principal of its own acts with its own access and hides whom it worked for; a label the client declares isn't verified.

<!-- field: decision -->
A change made through an agent is the person's: their access applies and they are its author. Kuria also records the agent, taken from the token's actor chain (`act`), on the change, in `draftChanges` and in awareness. Konklav issues such a session by a delegation: a person starting a scoped, time-bounded child session for an agent's service principal, revoked with the parent. Until it can, edits through `kuria mcp` are the person's alone; Kuria records no agent the client merely declares.

<!-- field: why -->
The person is accountable for what they let an agent write, so access and authorship stay theirs, while the record still says truthfully that an agent wrote it. The actor chain is Konklav's way of naming the subject really acting, so Kuria verifies it through the relying crates instead of trusting the client, and reading it also stops Kuria hiding who impersonated someone.

<!-- akt: PLAN-027-T08 -->
---
type: task
revision: 2
rank: b5z
state: todo
in: [PLAN-027]
links: [Blocks PLAN-027-T05]
title: "Record who really acted, from Konklav's actor chain"
---

<!-- field: acceptance -->
A spec rule says Kuria keeps a token's `act` subject beside `by` on every change it records (revisions, draft updates, pushes); history, `draftChanges` and awareness show it as "<actor> for <person>". A test with an impersonation token shows the impersonator. Needs nothing from Konklav.

<!-- akt: PLAN-027-T09 -->
---
type: task
revision: 4
rank: b60
state: todo
in: [PLAN-027]
links: [Blocks PLAN-027-T05]
title: "Konklav: a person delegates a session to an agent"
---

<!-- field: description -->
A possible shape: tokens Konklav issues to an OIDC client through the code flow name the client in `act`, and `kuria mcp` signs in as a public client with a loopback redirect, so that authorizing the client is delegating to it.

<!-- field: acceptance -->
Konklav, in its own repository, lets a person start a child session for a service principal, scoped and time-bounded and revoked with the parent, whose tokens name the person as `sub` and the agent in `act`; the relying crates Kuria pins answer it. `kuria mcp` signs in that way.

<!-- akt: PLAN-027-D05 -->
---
type: decision
revision: 3
rank: b61
state: decided
in: [PLAN-027]
title: Search with pg_search
decided_by: albin@mattsson.io
---

<!-- field: context -->
Documents are found by what they say (PLAN-027-T03), which title search can't do; PLAN-027-G02 compares pg_search with Postgres's own full-text search.

<!-- field: decision -->
Kuria requires Postgres with pg_search (ParadeDB). Its BM25 index is on `akt_search`, a row per text field of each akt as it reads, partitioned or filtered by tenant. Every Postgres Kuria runs on, development, tests, CI, end-to-end and dogfooding, is an image of Kuria's own with it. Operators run Postgres themselves or on ParadeDB's offerings.

<!-- field: why -->
BM25 ranking, tokenizers per language and ngram matching find documents by what they say far better than `ts_rank` over a `tsvector`, and are what an agent searching documents needs.

<!-- akt: PLAN-027-T10 -->
---
type: task
revision: 2
rank: b62
state: todo
in: [PLAN-027]
links: [Blocks PLAN-027-T03]
title: "Kuria's own Postgres image, with pg_search"
---

<!-- field: acceptance -->
An image built from `postgres:18.x-trixie` with ParadeDB's pg_search package (pinned, 0.27.0 or later) and pgvector, `shared_preload_libraries = 'pg_search'`, runs Kuria's development, test, CI, end-to-end and dogfooding Postgres; a migration creates or requires the extension; CONTRIBUTING.md and docs/architecture.md say Kuria needs Postgres with pg_search, and why.

<!-- akt: PLAN-027-D06 -->
---
type: decision
revision: 2
rank: b67
state: decided
in: [PLAN-027]
title: "Kuria's MCP runs only locally, in the CLI"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Hosted clients, such as claude.ai in a browser, need MCP over HTTP served by Kuria, with OAuth through Konklav and client registration it lacks (PLAN-027-G03); clients on the person's machine, such as Claude Code and the desktop apps, start a local process.

<!-- field: decision -->
`kuria mcp` over standard input and output is Kuria's only MCP server; Kuria serves none over HTTP. PLAN-028, which planned one, is dropped.

<!-- field: why -->
Run by the CLI, it signs in, keeps its session and proves its key with DPoP as the CLI does, with no OAuth or client registration to build in Kuria or Konklav; local mode comes with it. Hosted clients aren't needed now.

<!-- akt: PLAN-027-D07 -->
---
type: decision
revision: 2
rank: b68
state: decided
in: [PLAN-027]
title: "Text search is a query of its own, textSearch"
decided_by: albin@mattsson.io
---

<!-- field: context -->
The plan had `akter(search:)` also match text fields. That list is paged by cursor in recency order (`akt.page`) and answers akter, while text search ranks by relevance and answers a field and excerpt per match; the app's search box relies on the quick number-and-title lookup.

<!-- field: decision -->
`textSearch(text, type, first, archived)` answers matches (akt, field, relevance, excerpt), most relevant first, at most `first` (20, at most 50), with no cursor (`akt.search.text`). `akter(search:)` stays as it is. `search_documents` answers both.

<!-- field: why -->
Relevance order and excerpts don't fit a recency-paged list of akter, and an agent wants the best matches, not every page; keeping `akter(search:)` leaves the app's lookup fast and unchanged.
