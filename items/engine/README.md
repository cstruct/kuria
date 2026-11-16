# kuria-engine

The rules for changing an akt: what a create, an edit or a transition
produces, or why it is refused.

## Responsibilities

- `Value`: a field value of one of the field kinds, read from a request's
  input (`from_input`) or from what was stored (`from_json`), and written to
  JSON. A `Change` (who, when) is what the server supplies to values such as
  a checkbox's. `Someone` is a user or a contact, as `user` values and
  whom a box was checked for name them; `Value::named` lists them, and
  `Value::display` takes their names from the server.
- `create`, `edit` and `transition`: take an `AktType` (from `kuria-config`),
  the akt's current state and field values, and the values a request gives,
  and return an `Outcome` (the new state and values, what changed, and the
  hash) or a `Refusal`.
- `rank_between` and `nth_appended`: rank keys, fractional indexing with an
  integer part, so appending keeps keys short and there is always a key
  between two others. Keys are compared byte by byte. `placement`: where a
  move lands between two neighbours, or nowhere if the akt is already
  there.
- `rules` ([`src/rules.rs`](src/rules.rs)): the rules the server's write
  path and the CLI's local mode both enforce (`cli.local.rules`), over an
  `AktRef` (number, state, type, revision), refusing with a `Broken` the
  server answers as its usual codes. What they need to know of other akter
  the caller finds and passes in:
  - `numbering`: the counter a new akt's number takes its sequence from,
    the number itself, and `next_from`, the next number from the numbers
    that exist, for a store without counters;
  - `containment`: `check_created_in`, `check_distinct`,
    `check_create_inside`, `check_unfrozen`, `check_not_itself`,
    `check_joining` (given whether the member is already in, another
    container of the type it is in, and whether it contains the
    container), and `contains`, a cycle search over members;
  - `links`: `check_not_itself`, `to_add` (the kind, its target types and
    the pin), `check_new`, `to_remove` and `check_existed`;
  - `records`: the entries membership and link revisions hold.
- `board::column_of`: which column of a board a member of a given akt type
  and state falls in, or `unmapped`.
- `draft::render`: a draft's text from the akter its recipe lists, grouped
  by akt type if the recipe says so; `Value::display` gives each value's
  text. `draft::select` keeps, with `latest_by`, only the newest akt per
  value of that field, and then leaves out what `exclude` names.
- `sbom::detect`: whether a file is an SPDX or CycloneDX JSON document, and
  its summary: format, specification version, name and component count.
- `live::LiveDraft`: an akt's live draft as a Yjs document (`yrs`, v1
  updates): each `markdown` and multiline `text` field a `Y.Text`, the rest
  in the `Y.Map` `fields`. It reads field values from the document, and
  writes them as the least change, a text by the characters that differ
  (`similar`), so that concurrent edits merge.
- `state_hash`: the SHA-256 of an akt's state and field values as canonical
  JSON (no whitespace, object members in key order), independent of how
  `serde_json` orders maps.

No I/O: the server, or the CLI in local mode, loads the akt type and the
akt, asks the engine, and stores what it answers.

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
  (`attachment.sbom`) and [`docs/spec/containers.md`](../../docs/spec/containers.md)
  (`container.*`), [`docs/spec/links.md`](../../docs/spec/links.md)
  (`link.*`) and [`docs/spec/cli.md`](../../docs/spec/cli.md)
  (`cli.local.numbers`, `cli.local.rules`).
