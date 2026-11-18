# Examples

Configurations of Kuria for kinds of work, each a set of akt type
documents to define with `defineAktType`, and, optionally, a bundle of
akter made with them:

- [Release management](release-management/README.md): releases, their
  backlogs, candidates, hotfixes, notes and the artifacts they ship.
- [Documents](documents/README.md): document indexes of documents and
  images, edited together and released as versions.
- [Planning](planning/README.md): plans of tasks, gaps and decisions, as
  Kuria's own [`plans/`](../../plans/) are kept.
- [Test protocols](test-protocols/README.md): protocols kept as templates,
  copied for each run, with testers' verdicts.

## Example data

An example may hold a `bundle/`, an example bundle (`bundle.examples` in
[the bundles spec](../spec/bundles.md)): a bundle of its akt types with
realistic akter and their history, as `kuria export --types … --akter`
would write it. Load one into a server as it starts, into a tenant that
holds nothing yet:

```sh
KURIA_BOOTSTRAP_BUNDLE=docs/examples/planning/bundle mise run serve
```

(`KURIA_BOOTSTRAP_TENANT`, else `KURIA_DEFAULT_TENANT`, names the tenant;
[`items/server`](../../items/server/README.md#running) lists the variables.)
A server started again with the variable still set refuses to start rather
than import twice: unset it after the first start. To click around in one,

```sh
mise run example planning
```

serves it on <http://localhost:18290> with only that example loaded, to its
own people, who sign in as themselves, and with the app hot-reloading as you
edit it. It runs on a stack of its own, beside `mise run e2e`
([`example/`](../../example/README.md)).

### Making them

The bundles aren't written by hand: `write_example_bundles` in
[`items/server/tests/examples.rs`](../../items/server/tests/examples.rs)
defines each example's akt types in a fresh database, makes its akter
through GraphQL as the end-to-end seed's people would in the app, with
transitions, reasons, links and versions, and exports them.

```sh
mise run examples:bundles
```

writes every bundle again, with new identifiers and times. Run it after
changing an example's akt types or the bundle format: `mise run test`
imports each bundle into a fresh database and fails if one doesn't import,
or holds akt types other than its example's files.
