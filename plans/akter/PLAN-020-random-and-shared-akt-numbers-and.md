---
number: PLAN-020
type: plan
revision: 7
rank: b5O
state: active
title: "Random and shared akt numbers, and external documents"
owner: albin@mattsson.io
---

<!-- field: goal -->
Akt types number akter in three new ways, and a document can live outside Kuria:

- **Random numbers.** A number pattern may hold `{rand:N}`, N random digits, such as `QR-{rand:4}` giving `QR-4821`, alone or beside `{seq}` (`QR-{rand:4}-{seq:2}`). A random number is always free: no akt of the tenant has it.
- **Shared sequences, declared.** Akt types that number from one sequence say so by name, `"sequence": "qms"`, so a Policy and a Procedure both numbered `QMS-{seq:4}` count QMS-0001, QMS-0002… together. Two types with the same pattern that don't name the same sequence are refused, so sharing is never an accident.
- **External documents.** An akt type may declare that each released version carries a PDF uploaded by hand (`"versions": { "file": true }`): such a document has no editor; its page shows the PDF of the version read, and `Download PDF` gives that file.

Success: each is specified as Tracey rules, works the same on a server, in local mode and through bundles, and is shown in the app; the documents example gains an `external_document` type; `mise run lint`, `mise run test` and the end-to-end tests pass.

<!-- field: design -->
## Where it starts

- `NumberPattern` (`items/config/src/number.rs`) parses `{year}`, `{month}`, `{parent}` and exactly one `{seq}`/`{seq:N}`; `scope` renders it with the sequence left in, and the counter `number:<scope>` (`items/engine/src/rules/numbering.rs`, `store::next_counter`) is shared by every type whose pattern renders the same scope. `read` reads a number back for bundle import counters and local mode's `next_from`.
- Cross-type checks on defining a type live in `items/config/src/references.rs` (`check_state_references`, `check_field_references`), called by the server's `define_in` and by local mode's `types define` and `check`.
- A type's `versions` is `{ action? }` (`version.declare`); a version names a revision, with a label and pins (`docs/spec/versions.md`).

## Random numbers (`akt.number.pattern`, `akt.number.random`)

- `{rand:N}`, N from 1 to 12, any number of times; a pattern needs at least one `{seq}` or `{rand:N}`, and at most one `{seq}`.
- With a `{seq}`, the random digits are drawn first and are part of the counter's scope, as `{year}` is: the number is unique by its counter. Without one, a number is drawn until it is free: a few draws at random, then, for N up to 6 digits, uniformly among the free ones; when none is free the create is refused as `NUMBERS_EXHAUSTED`, naming the pattern.
- The server draws under the tenant's order lock, which every create already takes, so two creates can't draw the same number; local mode draws among the directory's numbers. Bundle import keeps numbers as they are; counters are raised only for patterns with a `{seq}`.
- `read` reads `{rand:N}` as exactly N digits.

## Shared sequences (`akt.number.shared`)

- A type document may name `sequence`, a key. Defining a type is refused when another type's latest version has the same number pattern without naming the same sequence (neither naming one counts as not the same), or names the same sequence with another pattern. So a sequence is one pattern, and the counter stays `number:<scope>`, unchanged: sharing is declared, not new machinery.
- Existing directories and tenants whose types already share a pattern unnamed fail `check`/redefinition until they name a sequence; the examples and `plans/` are checked for that first.

## External documents (`version.file`, …)

- `"versions": { "file": true }`: releasing takes an uploaded attachment, a PDF, which the version records; a release without one is refused, as is one for a type without `file`. The file is held by the akt for `attachment.download`.
- The akt's page shows the PDF of the version read, else the latest version's, in place of a `markdown` field's sheets; `Download PDF` gives it. The release dialog asks for the file. The version list links each version's file.
- The CLI releases with `--file <path>`; local mode copies it to `attachments/<number>/` and records it on the version; bundles carry it with the version.
- The documents example gains `external_document`, numbered in the documents' index.

Details of the version store, the release mutation and the PDF route are filled in by the task that builds it (PLAN-020-T03).

<!-- field: verification -->
- `cargo test -p kuria-config` (pattern parsing, reading back, sequence checks), `-p kuria-engine` (drawing free numbers), `-p kuria-server` (creating with random and shared numbers, releasing with a file, bundle round trip), `-p kuria-cli` (local mode: numbers, release with a file).
- `mise run lint` (Tracey references, clippy, svelte-check, `kuria check` of `plans/`), `mise run test`.
- End to end: an external document created in the app, a PDF released, shown and downloaded; a reference to it followed.
- By hand in `mise run e2e:serve`: create several akter of a `{rand:4}` type and of two types sharing a sequence.

<!-- akt: PLAN-020-T01 -->
---
type: task
revision: 5
rank: b5P
state: done
in: [PLAN-020]
title: "Random digits in number patterns: {rand:N}"
assignee: albin@mattsson.io
---

<!-- field: description -->
Done. `{rand:N}` (N from 1 to 12, any number of times) joins `{seq}`; a pattern needs a `{seq}`, a `{rand:N}` or both (`akt.number.pattern`, `akt.number.random`).

- `items/config/src/number.rs`: `Part::Random`, `NumberContext.random`, `has_sequence`, `random_widths`, `like`; `read` reads drawn digits into the scope.
- `items/engine/src/rules/numbering.rs`: `draw`, `draw_free` (64 draws, then, up to 6 digits, uniformly among the free ones), `next_from` for local mode.
- Server `next_number` (`items/server/src/akter.rs`): draws under the order lock, reading taken numbers from `akt_number` with `store::numbers_like`; `NUMBERS_EXHAUSTED`.
- Local mode draws a create's number once (`create_with` takes it), free among the directory's and archived numbers.

Verified: config and engine unit tests, `random_numbers_are_free_until_none_is` (ten concurrent creates take all ten `QR-{rand:1}` numbers, the eleventh refused), `draws_random_numbers_free_among_the_directory_s` in local mode, bundle tests, `mise run lint`.

<!-- field: acceptance -->
akt.number.pattern and akt.number.random specified; NumberPattern parses, renders, scopes and reads {rand:N}; the server and local mode draw free numbers, refusing NUMBERS_EXHAUSTED; bundle import counters only for {seq}; tests in config, engine, server and CLI.

<!-- akt: PLAN-020-T02 -->
---
type: task
revision: 4
rank: b5Q
state: done
in: [PLAN-020]
title: "Shared number sequences, declared by name"
---

<!-- field: description -->
Done. An akt type document may name a `sequence` (`akt.number.shared`); `check_sequences` (`items/config/src/references.rs`) refuses a type numbered as another without naming the same sequence, or naming its sequence with another pattern, on the server's `define_in` (so bundle import too), local `types define` and `kuria check`. The counter stays `number:<scope>`: types sharing a sequence share a pattern, so they already share it. `types schema` says the sequence.

No example or `plans/` types shared a pattern, so nothing had to be renamed.

Verified: `akt_types_share_numbers_only_by_naming_one_sequence` (config), `akt_types_naming_one_sequence_share_its_numbers` on the server (QMS-0001, QMS-0002, QMS-0003 across two types; both refusals) and in local mode, the whole server suite, `mise run lint`.

<!-- field: acceptance -->
akt.number.shared specified; a type document's sequence parsed and validated; defining a type with the same pattern as another without the same sequence, or the same sequence with another pattern, refused on the server, in local mode and on import; examples and plans/ pass.

<!-- akt: PLAN-020-T03 -->
---
type: task
revision: 1
rank: b5R
state: todo
in: [PLAN-020]
title: "External documents: a PDF per released version"
---

<!-- field: acceptance -->
version.file and its app, CLI and bundle rules specified; versions { file: true } validated; releasing takes and records a PDF; the page shows and downloads it; CLI and local mode release with --file; bundles carry it; external_document in the documents example; end-to-end test.

<!-- akt: PLAN-020-D01 -->
---
type: decision
revision: 2
rank: b5S
state: decided
in: [PLAN-020]
title: "Shared sequences are declared, and one sequence is one pattern"
decided_by: albin@mattsson.io
---

<!-- field: context -->
Types with identical patterns already shared a counter, implicitly.

<!-- field: decision -->
A type names its sequence; the same pattern without the same sequence, or the same sequence with another pattern, is refused. The counter stays number:<scope>.

<!-- field: why -->
Sharing should never be an accident (the user's choice), and requiring one pattern per sequence keeps numbers unique without new counters.

<!-- akt: PLAN-020-D02 -->
---
type: decision
revision: 2
rank: b5T
state: decided
in: [PLAN-020]
title: An external document is a type whose versions carry a file
decided_by: albin@mattsson.io
---

<!-- field: context -->
Kuria knows no akt type; an external document is written elsewhere and released as a PDF.

<!-- field: decision -->
versions declares file: true; each release takes an uploaded PDF, recorded on the version. No built-in type kind.

<!-- field: why -->
Keeps akt types generic: any type may release files, and the app shows a version's file wherever it would show its rendering.
