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

Commands so far: `login [--no-browser]`, `logout`, `whoami`. The reads and
writes the spec lists come next.

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
  codes, and the command line.
- End to end ([`e2e/tests/cli.spec.ts`](../../e2e/tests/cli.spec.ts)): the
  built binary against the e2e stack. Playwright authorizes the link
  `kuria login` prints; then `whoami` from the profile, a refresh, `logout`
  revoking at Konklav, and `KURIA_KEY` saving nothing, with exit codes 4
  and 5.

## Traceability

- Spec: [`docs/spec/cli.md`](../../docs/spec/cli.md) (`cli.profile`,
  `login`, `secret`, `secret-key`, `dpop`, `discovery`, `format`,
  `errors`).
- Annotations: `r[impl cli.*]` in `src/`, `r[verify cli.*]` in the unit
  tests and `e2e/tests/cli.spec.ts`.
