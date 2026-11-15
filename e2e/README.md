# End-to-end tests

Kuria's app in a real browser, signed in through its login page against a
real Konklav: Playwright drives the built app, served by the Kuria server,
which relies on a seeded Konklav 1.8.0.

## Running

```sh
mise run e2e
```

starts the development services (`mise run db:up`), builds the app, starts
and seeds a fresh Konklav (`mise run e2e:up`), and runs Playwright, which
starts the Kuria server itself and waits for `/healthz`. Konklav keeps
running afterwards; `mise run e2e:down` stops it. To run the tests again
against the same Konklav, or one of them:

```sh
mise exec -- npx --prefix e2e playwright test -g "release"
```

## Clicking around

```sh
mise run e2e:serve
```

starts the same stack, defines the release-management,
test-protocols and documents examples' akt types, and prints who can sign in at <http://localhost:18090> (use
`localhost`, not `127.0.0.1`). Each start begins with an empty Kuria and
a fresh Konklav. Ctrl-C stops the server.

```sh
mise run e2e:walk
```

runs the release walkthrough against that server, from another terminal,
and leaves its release, candidates, hotfix, notes and artifacts behind to
explore; it can run again, numbering the next release. With
`KURIA_E2E_EXISTING=1`, any Playwright run uses a server already
listening instead of starting one.

```sh
mise run e2e:screenshots
```

then takes the screenshots in [the app's README](../items/web/README.md)
from that release, into `items/web/screenshots`.

The first run installs Chromium (`mise run e2e:install`) and compiles the
server. A failed test keeps its trace and screenshots in
`.state/results`; `npx playwright show-trace <trace.zip>` replays it.

## The stack

| Piece | Where | What |
|---|---|---|
| Konklav | `cstruct/konklav:1.8.0`, ports 18080–18082 | Its subject-facing, relying and control surfaces; issuer and login URL are Kuria's |
| Konklav's Postgres | `konklav-dev-postgres:18.6-jsonschema0.3.4` | Built from [`konklav/postgres`](konklav/postgres/Dockerfile), a copy of Konklav's, where missing; in memory |
| Kuria | `localhost:18090`, a fresh `kuria_e2e` database | The server serving `items/web/build`, as Konklav's login application (`KURIA_LOGIN_APP`) |
| Valkey, S3 | the development ones | As for the other tests |

- [`compose.yaml`](compose.yaml) and [`konklav/`](konklav): Konklav, its
  configuration and test-only secrets.
- [`scripts/keys.ts`](scripts/keys.ts): Konklav's signing keys, made once
  into `.state/keys`.
- [`scripts/setup.ts`](scripts/setup.ts): seeds Konklav over the control
  plane with the catalog (Kuria's actions and the `kuria_akt_type` scope
  kind), the tenants `shop` and `lab` (where the admin is a member too, for
  bundles), the release-management example's roles, a developer, a
  release manager and a head of product with test-only passwords, Kuria's
  OIDC client and service principal, the CLI's public DPoP client
  `kuria-cli`, and the admin and the build pipeline
  as software with secret keys. It writes Kuria's configuration to
  `.state/kuria.env` and what the tests use to `.state/seeded.json`.
- [`scripts/serve.ts`](scripts/serve.ts): the Kuria server for the tests,
  and the `kuria` CLI built for them.
- [`support/`](support): signing people in, working in the app by roles
  and labels, the API as software uses it, and DPoP proofs
  ([`support/dpop.ts`](support/dpop.ts)), and running the CLI with a
  directory of its own ([`support/cli.ts`](support/cli.ts)).

Each test's browser sends its own `X-Forwarded-For` address, which Kuria
trusts from localhost here, so the sign-in rate limit counts each test as
it would each person.

Software signs in to Konklav with its secret key and moves into the
tenant, so its tokens carry Konklav's own audience; Kuria accepts that
beside its OIDC client's (`KONKLAV_AUDIENCE=kuria,konklav`).

## The tests

- [`tests/sign-in.spec.ts`](tests/sign-in.spec.ts): signing in through the
  login page, a wrong password, and single sign-on.
- [`tests/cli-login.spec.ts`](tests/cli-login.spec.ts): `kuria login`'s
  flow without the CLI: a link authorized in another browser, the code
  collected from the relay once and exchanged with a DPoP proof, GraphQL
  with the bound token (and refused as a bearer token), a refresh; and
  software's DPoP-bound secret-key sign-in, refused with another key.
- [`tests/cli.spec.ts`](tests/cli.spec.ts): the `kuria` binary: `login`
  by the link it prints, authorized in a browser; `whoami` from the
  profile; a refresh rotating the tokens; `logout` revoking at Konklav;
  and `KURIA_KEY` signing in as software, saving nothing, with its exit
  codes; every read command against the release-management example; and
  every write, on akt types the test defines with the CLI; and a bundle
  moved from `shop` to `lab`; selected types converted to an akt
  directory and back into `lab`; and the planning example, defined from
  `docs/examples/planning` and worked as `kuria guide` says.
- [`tests/parity.spec.ts`](tests/parity.spec.ts): local mode's parity
  with a server. One sequence of some thirty commands (types, creation,
  edits, transitions, links with pins, order, membership, transfers, and
  the refusals of each rule) runs against the server and against a new
  akt directory; every answer, and every akt's `cat` and order after it,
  must be the same.
- [`tests/akt-page.spec.ts`](tests/akt-page.spec.ts): the title first, a
  field edited in place, and the history saying what changed.
- [`tests/accessibility.spec.ts`](tests/accessibility.spec.ts): axe
  (`@axe-core/playwright`) against WCAG 2.2 A and AA on every main page,
  in the light and the dark appearance: the login page, Home, akter by
  type, a new akt, contacts, the design gallery, an akt with a board and
  lists, a member's menu, a transition dialog, a field edited in place,
  linking a contact, and an akt and the drawer at 390 pixels.
- [`tests/contacts.spec.ts`](tests/contacts.spec.ts): a contact linked
  by hand to a user who signed in with a password, and an akt's history
  naming the user a value names.
- [`tests/links.spec.ts`](tests/links.spec.ts): a backlog item linked to
  the one it blocks from its page, read both ways, in the history, and
  unlinked.
- [`tests/members.spec.ts`](tests/members.spec.ts): a container's
  members on its board or listed by akt type, reordered by dragging and
  from the menu, and dropped on another column to take the transition
  that leads there (or refused where none does); an akt type's list
  filtered by state and by field values in the address, sorted, and
  loading more a page at a time.
- [`tests/documents.spec.ts`](tests/documents.spec.ts): the
  [documents example](../docs/examples/documents/README.md):
  - the developer writes a document in the rich-text editor, with a reference to another document's heading;
  - a picture is uploaded as an image akt and a diagram written as code is toggled to its picture, and the image type sits under "Show more";
  - the release manager's release is refused while another reference is broken, and taken once it isn't;
  - version 1 reads as released, its links pinned and its picture and diagram shown, and is downloaded as a PDF with the picture in it and compared with the living version;
  - the index is cloned with git, rebased onto a change made in the app and pushed, and the app shows the pushed revision.
  
  axe checks the diagram as code, the body with its picture and diagram, the refused release, the version and the comparison.
- [`tests/markdown-editor.spec.ts`](tests/markdown-editor.spec.ts): a
  `markdown` field written and formatted in the editor, with a reference
  inserted through the picker and a footnote, saved as Markdown, rendered as
  a link that opens the heading, and downloaded as a PDF; an image uploaded
  from the Image dialog, shown as a figure and on its own page; a diagram
  written as code, previewed through a compile error and back, toggled to
  its picture and saved; axe on each dialog and view.
- [`tests/git-remote.spec.ts`](tests/git-remote.spec.ts): `git-remote-kuria` against the stack:
  - two clones have the same commits;
  - a push becomes a revision, and the other clone fetches the very same commit;
  - a push is refused until `git pull --rebase`, and another until `kuria fmt`;
  - a version becomes a tag.
- [`tests/test-protocols.spec.ts`](tests/test-protocols.spec.ts): the
  [test-protocols example](../docs/examples/test-protocols/README.md):
  a template of three sections copied with two of them from the copy
  dialog, started, and run by two testers (the developer and the release
  manager, who may work on `test_protocol` and `test_case`), giving,
  changing and taking back verdicts in its sections, a collapsible one
  opened, and in its list; a test case taken out of the sections in the
  outline; each test case's, section's and the protocol's outcome; the run
  finished, and the template unchanged. axe checks the copy dialog, the
  run in its sections and the outline.
- [`tests/shell.spec.ts`](tests/shell.spec.ts): search from the keyboard,
  the drawer and no sideways scrolling at 390 pixels, signing out of
  Kuria and of the login page's Konklav session, and akt types under "Show
  more", remembered across reloads.
- [`tests/release-flow.spec.ts`](tests/release-flow.spec.ts): the
  [release-management example](../docs/examples/release-management/README.md)
  from planning to a closed release, as a developer and a release manager
  in two browsers, with the build pipeline through the API. It asserts
  what [`release_flow.rs`](../items/server/tests/release_flow.rs) asserts
  through GraphQL.

Not covered yet: linking a contact at sign-in. Konklav 1.7 reports a
password's email as unverified, so a password sign-in never links one; the
server's tests cover linking with a fake provider.

## Traceability

Playwright specs reference the rules of
[`frontend.md`](../docs/spec/frontend.md) and
[`login.md`](../docs/spec/login.md) as `r[verify ...]`.
