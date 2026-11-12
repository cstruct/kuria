# kuria-web

Kuria's app: a single-page app in SvelteKit (SPA mode, on Vite) that the
server serves from its own origin, and Kuria's login page.

## Responsibilities

- Building every view from the akt types the server serves; nothing here
  names an akt type ([spec](../../docs/spec/frontend.md)).
- Talking to the server only through GraphQL and the attachment endpoints,
  on its own origin, with the server's session cookie: urql with
  Graphcache, the `changes` subscription over a WebSocket, and sending the
  browser to `/auth/login` when it isn't signed in.
- The login page at `/login`, Konklav's login application
  ([spec](../../docs/spec/login.md)).

## Layout

- [`src/lib/graphql`](src/lib/graphql): the client, its cache keys, and
  every operation, in `.ts` modules only, so `graphql-codegen` types them.
  The types are generated into `src/lib/graphql/generated` (not checked
  in) from [`schema.graphql`](schema.graphql), which `kuria-server schema`
  prints.
- [`src/routes`](src/routes): the app's pages.

## Running

```sh
mise run web:dev
```

serves the app on <http://localhost:5173>, passing `/graphql`, `/auth`,
`/attachments`, `/v1` and `/.well-known` to `mise run serve` (or
`KURIA_DEV_SERVER`). `mise run web:build` builds it into `build/`, which
the server serves with `KURIA_WEB_DIR`.

After changing the server's schema, run `mise run web:schema`;
`mise run lint` fails while the file differs from what the server prints.

## Testing

```sh
npm test
```

runs the Vitest unit tests; `npm run check` type-checks and
`npm run lint` checks formatting and lints. `mise run lint` and
`mise run test` include them.

## Traceability

- Specs: [`frontend.md`](../../docs/spec/frontend.md) (`app.*`) and
  [`login.md`](../../docs/spec/login.md) (`login.*`).
