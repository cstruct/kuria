# Examples

Configurations of Kuria for kinds of work, each a set of akt type
documents to define with `defineAktType`, and a bundle of akter made with
them:

- [Release management](release-management/README.md): releases, their
  backlogs, candidates, hotfixes, notes and the artifacts they ship.
- [Documents](documents/README.md): document indexes of documents and
  images, edited together and released as versions.
- [Planning](planning/README.md): plans of tasks, gaps and decisions, as
  Kuria's own [`plans/`](../../plans/) are kept.
- [Test protocols](test-protocols/README.md): protocols kept as templates,
  copied for each run, with testers' verdicts.

## Example data

Each example's `bundle/` is an example bundle (`bundle.examples` in
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
than import twice: unset it after the first start. Against the end-to-end
Konklav, with its seeded people,

```sh
mise run e2e:example -- planning
```

runs Kuria as `mise run e2e:serve` does, with the example loaded and its
people shown as, and signing in as, the seeded ones
([`e2e/`](../../e2e/README.md)).

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
