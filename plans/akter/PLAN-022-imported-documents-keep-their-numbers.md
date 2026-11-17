---
number: PLAN-022
type: plan
revision: 6
rank: b5b
state: done
title: Imported documents keep their numbers and references
owner: albin@mattsson.io
---

<!-- field: goal -->
Documents imported from another system keep their numbers and the references between them: an import may give the akt's old number, such as `FOOID-1234`, and a pattern with named groups `id` and `label`, such as `(?<id>FOOID-\d{4})(?<label>(?:\s?\p{Lu}\w+)*)`, turns each mention of another document in the draft into a reference, `[Some Title](kuria:FOOID-1234)`, which is broken until that document is imported and works from then on.

Success: two folders that mention each other import in either order and, once both are in, each links to the other; numbers given fit the type's pattern, `{seq}` or `{rand:N}`, and are never handed out again.

<!-- field: design -->
## A given number (`akt.number.given`)

- `createAkt(input: { number })`, `kuria new --number`, and `"number"` in an import's `metadata.json`. The number must be one the type's pattern can give an akt created in the containers named, read as `NumberPattern::read`/`parent_of` read it, any `{year}` and `{month}` and drawn digits, `{parent}` the first container's number; and free in the tenant, archived akter included. Else `BAD_REQUEST` and `NUMBER_TAKEN`.
- With a `{seq}`, the counter of the number's scope is raised to at least its sequence, as a bundle import raises it (`bundle.import.counters`), so it is never handed out again. With only `{rand:N}`, a free draw already skips taken numbers. Local mode reads its counters from the directory's numbers, so nothing more.
- The server checks it under the tenant's order lock, as numbering does.

## References captured on import (`cli.docs-import`)

- `--references <regex>` on `kuria docs import`, overridden by a folder's `metadata.json` `references`. Groups `id` (required) and `label` (optional); each match outside code and outside an existing link becomes `[label](kuria:id)`, the label trimmed, else the id. The draft is saved with references that may be broken (`app.references` shows them so); releases come before the draft, so they don't check it.

<!-- field: verification -->
- `cargo test -p kuria-config` (a number fits a pattern), server tests creating with given numbers (seq, rand, parent, taken, counter raised), CLI local tests (`new --number`, import keeping the number, references captured), and the e2e import test with two folders mentioning each other, imported in turn.
- `mise run lint`, `mise run test`.

<!-- akt: PLAN-022-T01 -->
---
type: task
revision: 5
rank: b5c
state: done
in: [PLAN-022]
title: Create an akt with a given number
assignee: albin@mattsson.io
---

<!-- field: description -->
Done. `createAkt(input: { number })`, batch creates, and `kuria new --number` give an akt its number (`akt.number.given`): `NumberPattern::fits` checks the pattern can give it in the first container (any year, month and drawn digits; `{parent}` must match), `numbering::given` answers the counter to raise, and the server refuses a taken number (`NUMBER_TAKEN`) and raises the `{seq}` counter past it under the order lock (`items/server/src/akter/create.rs`). Local mode checks it against the directory's numbers; its counters are read from them anyway.

Verified: `says_whether_a_given_number_fits` (config), `an_akt_may_be_given_a_number_its_pattern_gives` (server: seq, counter raised, earlier free number, taken, wrong pattern, rand never drawn again), local `imported_documents_keep_their_numbers_and_refer_to_each_other`.

<!-- field: acceptance -->
akt.number.given specified; createAkt and kuria new take a number that must fit the pattern (seq, rand, parent) and be free; the seq counter is raised past it; server and local tests.

<!-- akt: PLAN-022-T02 -->
---
type: task
revision: 5
rank: b5d
state: done
in: [PLAN-022]
title: Imports keep numbers and capture references
assignee: albin@mattsson.io
---

<!-- field: description -->
Done. `metadata.json`'s `number` is given to the created akt; `--references <regex>`, overridden by `metadata.json`'s `references`, captures each match's `id` and `label` groups outside code (fenced and inline), links, pictures and autolinks into `[label](kuria:<id>)` (`capture`, `kept` in `items/cli/src/docs_import.rs`); a pattern without an `id` group is refused. A reference to an akt not yet imported is saved and shown as broken until it is.

Verified: `captures_references_outside_code_and_links` with the user's pattern, local `imported_documents_keep_their_numbers_and_refer_to_each_other` (two procedures numbered FOOID-…, imported in turn), e2e `imported documents keep their numbers and refer to each other` (broken, then a link both ways, followed in the app).

<!-- field: acceptance -->
metadata number used; --references and metadata references turn matches into kuria: references outside code and links; tests local and e2e.

<!-- akt: PLAN-022-D01 -->
---
type: decision
revision: 2
rank: b5e
state: decided
in: [PLAN-022]
title: An import may give an akt its old number
decided_by: albin@mattsson.io
---

<!-- field: context -->
References captured from an imported draft name the target's old id; Kuria numbers new akter from the type's pattern.

<!-- field: decision -->
createAkt takes an optional number that must fit the pattern and be free; a {seq} counter is raised past it.

<!-- field: why -->
The user's choice: references then work as soon as the target is imported, with no re-linking.
