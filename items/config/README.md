# kuria-config

Akt type documents: their model, their validation, and number patterns.

## Responsibilities

- `AktTypeDocument`: the document a tenant submits to define an akt type,
  with its fields, states, transitions and number pattern, and for a
  container what it `contains`, what it must be `created_in` and its
  `board` columns, per multiline text field a `draft` recipe, and per
  `attachment` field its `format` (`any` or `sbom`). Unknown
  members are refused, so a typo can't be silently
  ignored.
- `validate`: checks a document and returns every problem with its path
  (`fields[2].options`), or an `AktType` the engine can use.
- `NumberPattern`: parses patterns such as `CAPA-{year}-{seq:4}` or
  `{parent}-RC-{seq:4}`, renders akt numbers from a `NumberContext` (year,
  month and parent number), and gives the scope that numbers sharing a
  sequence have in common, which includes the parent's number.

- `check_state_references`: the states one akt type names of another (board
  columns, draft recipes' `states`), checked against the latest versions of
  the tenant's other akt types in both directions.
- `LineTemplate`: parses and renders a draft recipe's line, such as
  `{number} {field:title}`.

What one document can't check alone, the server checks with
`check_state_references` when it is defined. Whether the akt types named in
`contains` and `created_in` exist is left open, so documents can be defined
in any order.

No I/O: everything here is tested without a database.

## Testing

```sh
cargo test -p kuria-config
```

## Traceability

- Specs: [`docs/spec/akter.md`](../../docs/spec/akter.md) (`akt.type.*`,
  `akt.number.*`), [`containers.md`](../../docs/spec/containers.md)
  (`container.contains`, `container.created-in`) and
  [`boards.md`](../../docs/spec/boards.md) (`board.columns`) and
  [`drafts.md`](../../docs/spec/drafts.md) (`draft.recipe`, `draft.line`).
