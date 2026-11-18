# kuria-config

Akt type documents: their model, their validation, and number patterns.

## Responsibilities

- `AktTypeDocument`: the document a tenant submits to define an akt type,
  with its fields, states, transitions and number pattern, and for a
  container what it `contains`, what it must be `created_in` and its
  `board` columns and the link kind naming its parent boards, per multiline text field a `draft` recipe, and per
  `attachment` field its `format` (`any` or `sbom`), and its `presentation`:
  the title field, the fields on cards and in lists, and a tone per state.
- `AktType::title_field` and `AktType::tone`: the presentation's title
  field, else the first required single-line text field, and a state's
  tone, `neutral` unless named. Unknown
  members are refused, so a typo can't be silently
  ignored.
- `validate`: checks a document and returns every problem with its path
  (`fields[2].options`), or an `AktType` the engine can use.
- `NumberPattern`: parses patterns such as `CAPA-{year}-{seq:4}`,
  `{parent}-RC-{seq:4}` or `QR-{rand:4}`, renders akt numbers from a
  `NumberContext` (year, month, parent number and the digits drawn for each
  `{rand:N}`), gives the scope that numbers sharing a sequence have in
  common, which includes the parent's number and the drawn digits, and,
  for a pattern without `{seq}`, the SQL `LIKE` pattern of every number it
  can give. Drawing the digits is the engine's (`rules/numbering.rs`).

- `check_sequences`: refuses an akt type numbered as another without naming
  the same `sequence`, or naming its sequence with another pattern
  (`akt.number.shared`).
- `check_state_references`: the states one akt type names of another (board
  columns, draft recipes' `states`), checked against the latest versions of
  the tenant's other akt types in both directions.
- `check_field_references`: the fields and options draft recipes name of
  the akt types they list (`latest_by`, `exclude`), checked the same way:
  some listed akt type must have each, and a listed akt type can't drop one
  a recipe still needs.
- `LineTemplate`: parses and renders a draft recipe's line, such as
  `{number} {field:title}`.

What one document can't check alone, the server checks with
`check_state_references` and `check_field_references` when it is defined. Whether the akt types named in
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
  [`boards.md`](../../docs/spec/boards.md) (`board.columns`, `board.parent`) and
  [`drafts.md`](../../docs/spec/drafts.md) (`draft.recipe`, `draft.line`).
