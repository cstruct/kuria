# kuria-engine

The rules for changing an akt: what a create, an edit or a transition
produces, or why it is refused.

## Responsibilities

- `Value`: a field value of one of the field kinds, read from and written to
  JSON.
- `create`, `edit` and `transition`: take an `AktType` (from `kuria-config`),
  the akt's current state and field values, and the values a request gives,
  and return an `Outcome` (the new state and values, what changed, and the
  hash) or a `Refusal`.
- `state_hash`: the SHA-256 of an akt's state and field values as canonical
  JSON (no whitespace, object members in key order), independent of how
  `serde_json` orders maps.

No I/O: the server loads the akt type and the akt, asks the engine, and
stores what it answers.

## Testing

```sh
cargo test -p kuria-engine
```

## Traceability

- Spec: [`docs/spec/akter.md`](../../docs/spec/akter.md) (`akt.create`,
  `akt.edit`, `akt.transition`, `akt.field.*`, `akt.revision.record`).
