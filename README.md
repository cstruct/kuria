# Kuria

Kuria is a case and record management system for regulated work: issues,
nested work, test protocols and records, release records and CAPA. It
hardcodes as little as possible about what those are. Their kinds, fields,
states and rules are akt types, documents defined at runtime, and every
change to an akt is kept as a revision.

Kuria is a Konklav relying system: Konklav decides who someone is and what
they may do, and Kuria enforces it on its own data.

It is early. What exists today is listed under "Coverage so far" in
[docs/requirements.md](docs/requirements.md).

## What's here

| Item | What it is |
|---|---|
| [`items/server`](items/server/README.md) | The server: GraphQL over HTTP and WebSocket, attachments, sign-in (Rust, axum) |
| [`items/web`](items/web/README.md) | The app (Svelte) |
| [`items/cli`](items/cli/README.md) | `kuria`, the command-line client, for people and agents, against a server or a directory of Markdown files |
| [`items/engine`](items/engine/README.md), [`items/config`](items/config/README.md), [`items/store`](items/store/README.md) | Kuria's rules, akt type documents, and Postgres |
| [`docs/`](docs) | [Architecture](docs/architecture.md), [requirements](docs/requirements.md), [nomenclature](docs/nomenclature.md), and the [spec](docs/spec/README.md) |
| [`plans/`](plans) | Kuria's own plans, as an akt directory the CLI works on |

## Trying it

You need [mise](https://mise.jdx.dev) and Docker.

```bash
mise install
```

```bash
mise run e2e:serve
```

The second command starts a seeded Konklav and Kuria with the
release-management example's akt types, to click around in on
http://localhost:18090, and prints who can sign in.

The CLI explains itself:

```bash
mise run kuria -- guide
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT; see [LICENSE](LICENSE).
