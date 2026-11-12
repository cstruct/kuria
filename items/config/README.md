# kuria-config

Akt type documents: their model, their validation, and number patterns.

## Responsibilities

- `AktTypeDocument`: the document a tenant submits to define an akt type,
  with its fields, states, transitions and number pattern, and for a
  container what it `contains`, what it must be `created_in` and its
  `board` columns. Unknown members are refused, so a typo can't be silently
  ignored.
- `validate`: checks a document and returns every problem with its path
  (`fields[2].options`), or an `AktType` the engine can use.
- `NumberPattern`: parses patterns such as `CAPA-{year}-{seq:4}` or
  `{parent}-RC-{seq:4}`, renders akt numbers from a `NumberContext` (year,
  month and parent number), and gives the scope that numbers sharing a
  sequence have in common, which includes the parent's number.

What the document can't check is left to the server: whether the akt types
it names in `contains`, `created_in` and board columns exist, and whether the
states a column names are states of those types.

No I/O: everything here is tested without a database.

## Testing

```sh
cargo test -p kuria-config
```

## Traceability

- Specs: [`docs/spec/akter.md`](../../docs/spec/akter.md) (`akt.type.*`,
  `akt.number.*`), [`containers.md`](../../docs/spec/containers.md)
  (`container.contains`, `container.created-in`) and
  [`boards.md`](../../docs/spec/boards.md) (`board.columns`).
