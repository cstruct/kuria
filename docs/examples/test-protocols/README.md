# Test protocols

An example configuration: two akt types for test protocols, each a set of
test cases laid out in sections, kept as templates and copied for each run,
where testers record their verdicts and the outcome follows. Nothing in
Kuria knows these names: they are ordinary akt type documents, using the
`sections`, multiple `user` and `verdict` field kinds and copying, which any
akt type can use ([spec](../../spec/akter.md)).

| Akt type | Number | Contains | Created in |
|---|---|---|---|
| `test_protocol` | `TP-001` | test cases | |
| `test_case` | `TP-001-C01` | | a test protocol |

The documents are [`test_case.json`](test_case.json) and
[`test_protocol.json`](test_protocol.json). Define test cases first, since
a protocol names them:

```sh
kuria types define docs/examples/test-protocols/{test_case,test_protocol}.json
```

## How it fits together

- **A test case** says what to do (steps) and what should happen (expected
  result), and who runs it (testers, any number of users or contacts).
  Each tester gives it a verdict, `pass`, `fail` or `skip`, for
  themselves; anyone who may edit it can give one for a tester. Notes say
  what was seen.
- **Its outcome** follows from its testers' verdicts: it fails if any
  tester failed it; otherwise it is pending until every tester has
  answered (or while it has none); it is skipped if all skipped it, and
  passes otherwise. Outcomes are worked out when read, never stored, so
  they never make a revision.
- **A test protocol** holds its test cases, numbered after it, and lays
  them out in sections (its layout): each section with an optional title,
  Markdown text, and whether it is collapsible, nested as deep as needed.
  A section's outcome, and the protocol's, follow from the test cases
  they hold by the same rules: fail if any fails, pending while any is
  pending or there are none, skip if all skip, else pass.
- **Templates:** a protocol is drafted and then kept as a template. A run
  is a copy of it: a new protocol in draft, with copies of the test cases
  chosen (the sections that hold them come along, and empty ones are
  left out), each with its steps, expected result and testers, and no
  verdicts. The copy says what it was copied from. A run is started,
  and finished with its conclusion once its outcome is `pass`, `fail` or
  `skip`: while any test case is pending, it can't be finished
  (`"outcomes"` on the `finish` transition). It can be withdrawn before
  then.
- **A tester's work:** test cases naming a tester among their testers are
  in the tester's "My work" on Home, and `kuria ls test_case --where
  testers=@me` lists them. The test cases still to run are those whose
  outcome is pending: the list of test cases filters by outcome, as does
  `kuria ls test_case --outcome pending`.
- **Changing a run:** add a test case to it and place it in a section,
  move test cases between sections, or remove one, as with any akt;
  verdicts belong to the test case, not to its section.
- **Presentation:** protocols are a purple clipboard, test cases a blue
  flask; lists show a protocol's owner and a test case's testers.

## A run, in the app

1. Open the template and choose **Make a copy**. Every section and test
   case is chosen at first; clear the ones this run leaves out, then
   **Copy**. The copy opens.
2. **Start** it. Its test cases are listed in their sections, collapsible
   ones closed until opened, each with its testers, their verdicts and
   its outcome.
3. Each tester gives their verdict with **Pass**, **Fail** or **Skip**
   beside the test case, and can change it or take it back. The outcome
   of the test case, its sections and the protocol follow.
4. **Finish** it with a conclusion. While a test case is pending,
   **Finish** is unavailable, and says the outcomes it allows.

A tester finds their test cases under **My work** on Home, and those
still pending on the test cases' list, with **Outcome** set to Pending.

## A run, from the command line

```sh
kuria copy TP-001 --only TP-001-C01 --only TP-001-C03 --reason "Run for 4.2"
kuria do TP-002 start
kuria verdict TP-002-C01 pass                         # my own verdict
kuria verdict TP-002-C01 skip --for ben@shop.example  # for another tester
kuria show TP-002                                     # sections and outcomes
kuria ls test_case --in TP-002                        # with an Outcome column
kuria ls test_case --where testers=@me --outcome pending  # what I have left
kuria do TP-002 finish --set conclusion=@conclusion.md
```

`--only` names the test cases to keep; the sections holding them come
along. In a file (`kuria cat`), the layout is a section of YAML, testers a
list, and verdicts and the outcome are read-only:

```markdown
---
number: TP-002
type: test_protocol
revision: 7
state: running
outcome: fail
title: Checkout regression
---

<!-- field: layout -->
- title: Signing in
  items:
  - TP-002-C01
- title: Paying
  text: Use the test card 4242 4242 4242 4242.
  items:
  - TP-002-C03
```

The same works in an akt directory, without a server (`--dir`).

## With example data

[`bundle/`](bundle/) is an example bundle of these akt types with akter:
an acceptance test kept as a template, in sections; two runs copied from
it, one passed and one failed, with verdicts from two testers; and a draft
protocol. Devi Larsen (`dev@shop.example`), Rami Moberg
(`rm@shop.example`) and Hedda Lund (`hedda@shop.example`) wrote them, the
people the end-to-end seed signs in as. To start a server with it, set

```sh
KURIA_BOOTSTRAP_BUNDLE=docs/examples/test-protocols/bundle
```

and the server imports it into its default tenant (or
`KURIA_BOOTSTRAP_TENANT`) before it serves; the tenant must hold nothing
yet ([`items/server`](../../../items/server/README.md#running)).
`kuria import docs/examples/test-protocols/bundle` takes it to a running server's
tenant that hasn't defined these types. Against the end-to-end Konklav,

```sh
mise run e2e:example -- test-protocols
```

serves it on <http://localhost:18090> to the seeded people, as themselves.
[`docs/examples`](../README.md) says how the bundles are made.

## Konklav roles

- **Test lead**, tenant-wide: `kuria:akter:read`; and at `test_protocol`
  and `test_case`: `kuria:akter:create`, `kuria:akter:edit` and
  `kuria:akter:transition`, to keep templates and run them.
- **Tester**: `kuria:akter:read`, and `kuria:akter:edit` at `test_case`,
  to give verdicts. A verdict is an edit of the test case: only its
  testers give their own, and anyone who may edit it can give one for a
  tester.
