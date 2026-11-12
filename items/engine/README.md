# kuria-engine

The rules for changing an akt: what a create, an edit or a transition
produces, or why it is refused.

## Responsibilities

- `Value`: a field value of one of the field kinds, read from a request's
  input (`from_input`) or from what was stored (`from_json`), and written to
  JSON. A `Change` (who, when) is what the server supplies to values such as
  a checkbox's.
- `create`, `edit` and `transition`: take an `AktType` (from `kuria-config`),
  the akt's current state and field values, and the values a request gives,
  and return an `Outcome` (the new state and values, what changed, and the
  hash) or a `Refusal`.
- `rank_between` and `nth_appended`: rank keys, fractional indexing with an
  integer part, so appending keeps keys short and there is always a key
  between two others. Keys are compared byte by byte.
- `board::column_of`: which column of a board a member of a given akt type
  and state falls in, or `unmapped`.
- `draft::render`: a draft's text from the akter its recipe lists, grouped
  by akt type if the recipe says so; `Value::display` gives each value's
  text.
- `sbom::detect`: whether a file is an SPDX or CycloneDX JSON document, and
  its summary: format, specification version, name and component count.
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
  `akt.edit`, `akt.transition`, `akt.field.*`, `akt.revision.record`) and
  [`docs/spec/ordering.md`](../../docs/spec/ordering.md) (`order.move`) and
  [`docs/spec/boards.md`](../../docs/spec/boards.md) (`board.placement`,
  `board.unmapped`, `board.default`) and
  [`docs/spec/attachments.md`](../../docs/spec/attachments.md)
  (`attachment.sbom`).
