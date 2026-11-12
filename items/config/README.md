# kuria-config

Akt type documents: their model, their validation, and number patterns.

## Responsibilities

- `AktTypeDocument`: the document a tenant submits to define an akt type,
  with its fields, states, transitions and number pattern. Unknown members
  are refused, so a typo can't be silently ignored.
- `validate`: checks a document and returns every problem with its path
  (`fields[2].options`), or an `AktType` the engine can use.
- `NumberPattern`: parses patterns such as `CAPA-{year}-{seq:4}`, renders
  akt numbers, and gives the scope that numbers sharing a sequence have in
  common.

No I/O: everything here is tested without a database.

## Testing

```sh
cargo test -p kuria-config
```

## Traceability

- Spec: [`docs/spec/akter.md`](../../docs/spec/akter.md) (`akt.type.*`,
  `akt.number.*`).
