//! Sections, verdicts, outcomes and copies, through GraphQL against a
//! fresh database (`akt.field.sections`, `akt.field.verdict`,
//! `akt.outcome`, `akt.copy`).

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, SUBJECT, graphql_with, router_with, send};

struct World {
    db: TestDatabase,
    router: Router,
    /// Another tester: a contact, who needs no sign-in.
    other: String,
}

fn types() -> [Value; 4] {
    let states = json!([
        { "key": "template", "name": "Template" },
        { "key": "draft", "name": "Draft", "initial": true },
        { "key": "done", "name": "Done", "final": true }
    ]);
    [
        json!({
            "key": "requirement", "name": "Requirement", "number": "R-{seq}",
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [{ "key": "open", "name": "Open", "initial": true }]
        }),
        json!({
            "key": "test", "name": "Test", "number": "{parent}-T{seq}", "created_in": ["protocol"],
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "testers", "name": "Testers", "kind": "user", "multiple": true },
                { "key": "verdict", "name": "Verdict", "kind": "verdict", "assignees": "testers" },
                { "key": "reviewed", "name": "Reviewed", "kind": "checkbox" }
            ],
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "links": [{ "key": "verifies", "name": "Verifies", "inverse": "Verified by", "to": ["requirement"] }]
        }),
        json!({
            "key": "protocol", "name": "Protocol", "number": "TP-{seq}",
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "sections", "name": "Sections", "kind": "sections", "of": ["test"] }
            ],
            "states": states,
            "transitions": [
                { "key": "make_template", "name": "Make template", "from": ["draft"], "to": "template" },
                { "key": "finish", "name": "Finish", "from": ["draft"], "to": "done", "outcomes": ["pass", "fail", "skip"] }
            ],
            "contains": [{ "type": "test", "exclusive": true }]
        }),
        json!({
            "key": "suite", "name": "Suite", "number": "S-{seq}",
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "contains": [{ "type": "protocol" }]
        }),
    ]
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let mut world = Self {
            db,
            router,
            other: String::new(),
        };
        world.other.clone_from(
            &world
                .ok(
                    "mutation { addContact(input: { name: \"Tess Ter\" }) { id } }",
                    json!({}),
                )
                .await["addContact"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        for document in types() {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with("tenant", query, &variables))
            .await
            .json()
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, inside: &[&str], fields: Value) -> String {
        self.ok(
            "mutation($t: String!, $i: [String!]!, $f: JSON) { createAkt(input: { type: $t, in: $i, fields: $f }) { number } }",
            json!({ "t": key, "i": inside, "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn akt(&self, number: &str) -> Value {
        self.ok(
            "query($n: String!) { akt(number: $n) {
               number revision outcome state { key } copiedFrom { number }
               links { name pinnedRevision akt { number } }
               members(first: 50) { nodes { number } }
               fields { __typename
                 ... on TextValue { field text }
                 ... on CheckboxValue { field }
                 ... on VerdictValue { field verdicts { verdict } }
                 ... on SectionsValue { field items { ...Item section { items { ...Item section { items { ...Item } } } } } } } } }
             fragment Item on SectionItem { number section { title outcome } }",
            json!({ "n": number }),
        )
        .await["akt"]
            .clone()
    }

    async fn edit(&self, number: &str, fields: Value) -> Value {
        let revision = self.akt(number).await["revision"].clone();
        self.run(
            "mutation($n: String!, $v: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $v, fields: $f }) { number revision } }",
            json!({ "n": number, "v": revision, "f": fields }),
        )
        .await
    }

    /// Performs `finish` on `number`, through `router`, which a spawned
    /// task may hold.
    async fn finish(router: &Router, number: &str, revision: &Value) -> Value {
        send(
            router,
            graphql_with(
                "tenant",
                "mutation($n: String!, $v: Int!) { performTransition(input: { number: $n, revision: $v, transition: \"finish\" }) { state { key } } }",
                &json!({ "n": number, "v": revision }),
            ),
        )
        .await
        .json()
    }

    /// Every test's verdicts given: `pass` from us, `skip` for the contact.
    async fn pass_all(&self, tests: &[String]) {
        for test in tests {
            for verdict in [
                json!("pass"),
                json!({ "verdict": "skip", "for": { "contact": self.other } }),
            ] {
                let given = self.edit(test, json!({ "verdict": verdict })).await;
                assert!(given.get("errors").is_none(), "{given}");
            }
        }
    }

    async fn count(&self) -> usize {
        let mut n = 0;
        for key in ["protocol", "test", "requirement"] {
            n += self
                .ok(
                    "query($t: String!) { akter(type: $t, first: 200) { nodes { number } } }",
                    json!({ "t": key }),
                )
                .await["akter"]["nodes"]
                .as_array()
                .unwrap()
                .len();
        }
        n
    }
}

fn field<'a>(akt: &'a Value, key: &str) -> Option<&'a Value> {
    akt["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == key)
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

/// A protocol with three sections of tests, assigned to the signed-in
/// subject and a contact, and a requirement the first test verifies.
async fn protocol(world: &World) -> (String, Vec<String>, String) {
    let tp = world
        .create("protocol", &[], json!({ "title": "Release checks" }))
        .await;
    let mut tests = Vec::new();
    for title in ["Boots", "Logs in", "Exports", "Imports"] {
        tests.push(
            world
                .create(
                    "test",
                    &[&tp],
                    json!({ "title": title, "testers": [SUBJECT, { "contact": world.other }] }),
                )
                .await,
        );
    }
    let requirement = world
        .create("requirement", &[], json!({ "title": "Starts" }))
        .await;
    world
        .ok(
            "mutation($s: String!, $t: String!) { addLink(input: { source: $s, kind: \"verifies\", target: $t, pinRevision: 1 }) { number } }",
            json!({ "s": tests[0], "t": requirement }),
        )
        .await;
    let sections = json!([
        { "title": "Start", "collapsible": true, "items": [tests[0]] },
        { "title": "Data", "text": "With *real* data.", "items": [tests[2], { "title": "Back", "items": [tests[3]] }] },
        { "title": "Session", "items": [tests[1]] }
    ]);
    let answer = world.edit(&tp, json!({ "sections": sections })).await;
    assert!(answer.get("errors").is_none(), "{answer}");
    (tp, tests, requirement)
}

// r[verify akt.field.sections]
#[tokio::test(flavor = "multi_thread")]
async fn sections_place_only_members_and_lose_those_that_leave() {
    let world = World::new().await;
    let (tp, tests, requirement) = protocol(&world).await;
    let refused = world.edit(&tp, json!({ "sections": [requirement] })).await;
    assert_eq!(code(&refused), "REFUSED", "{refused}");
    let twice = world
        .edit(
            &tp,
            json!({ "sections": [tests[0], { "items": [tests[0]] }] }),
        )
        .await;
    assert_eq!(code(&twice), "REFUSED", "{twice}");
    let made = world
        .run(
            "mutation($f: JSON) { createAkt(input: { type: \"protocol\", fields: $f }) { number } }",
            json!({ "f": { "title": "New", "sections": [tests[0]] } }),
        )
        .await;
    assert_eq!(
        code(&made),
        "REFUSED",
        "a new akt has no members to place: {made}"
    );

    // Removed from the protocol, a test leaves its section in that revision.
    let before = world.akt(&tp).await["revision"].as_i64().unwrap();
    world
        .ok(
            "mutation($c: String!, $m: String!) { removeFromContainer(input: { container: $c, member: $m }) { number } }",
            json!({ "c": tp, "m": tests[3] }),
        )
        .await;
    let after = world.akt(&tp).await;
    assert_eq!(after["revision"], before + 1);
    let data = &field(&after, "sections").unwrap()["items"][1]["section"];
    assert_eq!(data["items"][1]["section"]["items"], json!([]), "{after}");
}

// r[verify akt.field.verdict]
// r[verify akt.outcome]
// r[verify akt.outcome.section]
#[tokio::test(flavor = "multi_thread")]
async fn verdicts_make_outcomes_up_to_the_protocol() {
    let world = World::new().await;
    let (tp, tests, _) = protocol(&world).await;
    assert_eq!(world.akt(&tp).await["outcome"], "PENDING");
    // Our own, then the contact's, given for them.
    for test in &tests {
        let ours = world.edit(test, json!({ "verdict": "pass" })).await;
        assert!(ours.get("errors").is_none(), "{ours}");
    }
    assert_eq!(world.akt(&tests[0]).await["outcome"], "PENDING");
    for (test, verdict) in tests.iter().zip(["skip", "skip", "pass", "skip"]) {
        let theirs = world
            .edit(
                test,
                json!({ "verdict": { "verdict": verdict, "for": { "contact": world.other } } }),
            )
            .await;
        assert!(theirs.get("errors").is_none(), "{theirs}");
    }
    let protocol = world.akt(&tp).await;
    assert_eq!(protocol["outcome"], "PASS");
    let sections = &field(&protocol, "sections").unwrap()["items"];
    assert_eq!(sections[0]["section"]["outcome"], "PASS");

    // One failure fails its test, its sections and the protocol.
    world.edit(&tests[3], json!({ "verdict": "fail" })).await;
    let protocol = world.akt(&tp).await;
    assert_eq!(protocol["outcome"], "FAIL");
    let sections = &field(&protocol, "sections").unwrap()["items"];
    let outcomes: Vec<&Value> = sections
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["section"]["outcome"])
        .collect();
    assert_eq!(outcomes, [&json!("PASS"), &json!("FAIL"), &json!("PASS")]);

    // Only for an assignee.
    let stranger = world
        .edit(
            &tests[0],
            json!({ "verdict": { "verdict": "fail", "for": SUPPORT_STRANGER } }),
        )
        .await;
    assert_eq!(code(&stranger), "REFUSED", "{stranger}");
}

// r[verify akt.transition.outcomes]
#[tokio::test(flavor = "multi_thread")]
async fn a_protocol_finishes_only_with_an_outcome_it_allows() {
    let world = World::new().await;
    let (tp, tests, _) = protocol(&world).await;
    let schema = world
        .ok(
            "{ aktType(key: \"protocol\") { transitions { key outcomes } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        schema["aktType"]["transitions"][1],
        json!({ "key": "finish", "outcomes": ["PASS", "FAIL", "SKIP"] })
    );
    let revision = world.akt(&tp).await["revision"].clone();
    let pending = World::finish(&world.router, &tp, &revision).await;
    assert_eq!(code(&pending), "REFUSED", "{pending}");
    assert_eq!(
        pending["errors"][0]["message"],
        "`finish` requires the outcome to be pass, fail or skip, and it is pending"
    );
    world.pass_all(&tests).await;
    let finished = World::finish(&world.router, &tp, &revision).await;
    assert_eq!(
        finished["data"]["performTransition"]["state"]["key"], "done",
        "{finished}"
    );
}

/// A verdict taken back while a protocol finishes: the finish waits for it,
/// and then sees the protocol pending (PLAN-013-D02).
// r[verify akt.transition.outcomes]
#[tokio::test(flavor = "multi_thread")]
async fn a_finish_waits_for_a_verdict_in_its_tree() {
    use diesel_async::SimpleAsyncConnection;

    let world = World::new().await;
    let (tp, tests, _) = protocol(&world).await;
    world.pass_all(&tests).await;
    let revision = world.akt(&tp).await["revision"].clone();
    // As a verdict being taken back would: the test locked and changed,
    // not yet committed.
    let mut conn = world.db.pool().get().await.unwrap();
    conn.batch_execute(&format!(
        "BEGIN; UPDATE akt SET fields = fields - 'verdict' WHERE number = '{}'",
        tests[3]
    ))
    .await
    .unwrap();
    let router = world.router.clone();
    let finishing = tokio::spawn(async move { World::finish(&router, &tp, &revision).await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !finishing.is_finished(),
        "the finish didn't wait for the test"
    );
    conn.batch_execute("COMMIT").await.unwrap();
    let answer = finishing.await.unwrap();
    assert_eq!(code(&answer), "REFUSED", "{answer}");
    assert!(
        answer["errors"][0]["message"]
            .as_str()
            .unwrap()
            .ends_with("it is pending"),
        "{answer}"
    );
}

impl World {
    /// The numbers of every page of `list`, a page of `first` at a time:
    /// `list` is an `akter` or `members` field taking `$after`, under
    /// `at`.
    async fn pages(&self, query: &str, at: &[&str], first: usize) -> Vec<String> {
        let mut numbers = Vec::new();
        let mut after = Value::Null;
        loop {
            let answer = self
                .run(query, json!({ "first": first, "after": after }))
                .await;
            assert!(answer.get("errors").is_none(), "{answer}");
            let mut list = &answer["data"];
            for key in at {
                list = &list[*key];
            }
            let nodes = list["nodes"].as_array().unwrap();
            assert!(nodes.len() <= first, "{answer}");
            numbers.extend(
                nodes
                    .iter()
                    .map(|n| n["number"].as_str().unwrap().to_owned()),
            );
            if list["pageInfo"]["hasNextPage"] != true {
                return numbers;
            }
            after = list["pageInfo"]["endCursor"].clone();
        }
    }
}

fn sorted(mut numbers: Vec<String>) -> Vec<String> {
    numbers.sort();
    numbers
}

// r[verify akt.filter.outcome]
// r[verify akt.page]
#[tokio::test(flavor = "multi_thread")]
async fn tests_and_protocols_filter_by_outcome_a_page_at_a_time() {
    let world = World::new().await;
    let suite = world
        .create("suite", &[], json!({ "title": "Nightly" }))
        .await;
    let (tp, tests, _) = protocol(&world).await;
    let (other, other_tests, _) = protocol(&world).await;
    for member in [&tp, &other] {
        world
            .ok(
                "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m }) { number } }",
                json!({ "c": suite, "m": member }),
            )
            .await;
    }
    world.pass_all(&tests).await;
    world.edit(&tests[1], json!({ "verdict": "fail" })).await;
    world.pass_all(&other_tests[..2]).await;

    // Tests, by their own verdicts, tenant-wide and as a protocol's members.
    let tenant_wide = |outcome: &str| {
        format!(
            "query($first: Int, $after: String) {{ akter(type: \"test\", outcome: {outcome}, first: $first, after: $after) {{ nodes {{ number }} pageInfo {{ hasNextPage endCursor }} }} }}"
        )
    };
    let pending = world.pages(&tenant_wide("PENDING"), &["akter"], 1).await;
    assert_eq!(sorted(pending), sorted(other_tests[2..].to_vec()));
    let failed = world.pages(&tenant_wide("FAIL"), &["akter"], 2).await;
    assert_eq!(failed, std::slice::from_ref(&tests[1]));
    let mut passed = tests.clone();
    passed.remove(1);
    passed.extend(other_tests[..2].iter().cloned());
    let passing = world.pages(&tenant_wide("PASS"), &["akter"], 2).await;
    assert_eq!(sorted(passing), sorted(passed));
    let members = format!(
        "query($first: Int, $after: String) {{ akt(number: \"{other}\") {{ members(type: \"test\", outcome: PASS, first: $first, after: $after) {{ nodes {{ number }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
    );
    assert_eq!(
        world.pages(&members, &["akt", "members"], 1).await,
        other_tests[..2]
    );

    // Protocols, by their members', only as a suite's members.
    let suite_of = |outcome: &str| {
        format!(
            "query($first: Int, $after: String) {{ akt(number: \"{suite}\") {{ members(type: \"protocol\", outcome: {outcome}, first: $first, after: $after) {{ nodes {{ number }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
        )
    };
    assert_eq!(
        world.pages(&suite_of("FAIL"), &["akt", "members"], 1).await,
        std::slice::from_ref(&tp)
    );
    assert_eq!(
        world
            .pages(&suite_of("PENDING"), &["akt", "members"], 1)
            .await,
        std::slice::from_ref(&other)
    );
    assert_eq!(
        world.pages(&suite_of("PASS"), &["akt", "members"], 1).await,
        Vec::<String>::new()
    );
    world.pass_all(&other_tests[2..]).await;
    world.edit(&tests[1], json!({ "verdict": "pass" })).await;
    assert_eq!(
        world.pages(&suite_of("PASS"), &["akt", "members"], 1).await,
        [tp.clone(), other.clone()]
    );
}

// r[verify akt.filter.outcome]
#[tokio::test(flavor = "multi_thread")]
async fn other_lists_refuse_an_outcome() {
    let world = World::new().await;
    let suite = world
        .create("suite", &[], json!({ "title": "Nightly" }))
        .await;
    for query in [
        "{ akter(type: \"protocol\", outcome: PASS) { nodes { number } } }",
        "{ akter(type: \"requirement\", outcome: PASS) { nodes { number } } }",
        "{ akter(outcome: PASS) { nodes { number } } }",
    ] {
        let answer = world.run(query, json!({})).await;
        assert_eq!(code(&answer), "BAD_REQUEST", "{query}: {answer}");
    }
    let unfiltered = world
        .run(
            &format!("{{ akt(number: \"{suite}\") {{ members(outcome: PASS) {{ nodes {{ number }} }} }} }}"),
            json!({}),
        )
        .await;
    assert_eq!(code(&unfiltered), "BAD_REQUEST", "{unfiltered}");
}

const SUPPORT_STRANGER: &str = "01900000-0000-7000-8000-0000000000ee";

// r[verify akt.copy]
// r[verify akt.copy.only]
// r[verify akt.copy.links]
// r[verify akt.copy.record]
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_keeps_the_chosen_part_of_the_tree() {
    let world = World::new().await;
    let (tp, tests, requirement) = protocol(&world).await;
    world
        .edit(&tests[0], json!({ "verdict": "pass", "reviewed": true }))
        .await;
    let revision = world.akt(&tp).await["revision"].clone();
    world
        .ok(
            "mutation($n: String!, $v: Int!) { performTransition(input: { number: $n, revision: $v, transition: \"make_template\" }) { number } }",
            json!({ "n": tp, "v": revision }),
        )
        .await;

    // The "Start" and "Data" sections, but only one of Data's tests.
    let copy = world
        .ok(
            "mutation($n: String!, $o: [String!]) { copyAkt(input: { number: $n, only: $o }) { number } }",
            json!({ "n": tp, "o": [tests[0], tests[3]] }),
        )
        .await["copyAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let copied = world.akt(&copy).await;
    assert_eq!(copied["state"]["key"], "draft", "in its initial state");
    assert_eq!(copied["copiedFrom"]["number"], json!(tp));
    let members: Vec<&str> = copied["members"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["number"].as_str().unwrap())
        .collect();
    assert_eq!(members.len(), 2, "{copied}");
    let sections = &field(&copied, "sections").unwrap()["items"];
    assert_eq!(
        sections.as_array().unwrap().len(),
        2,
        "the Session section held nothing kept"
    );
    assert_eq!(sections[0]["section"]["title"], "Start");
    let start = &sections[0]["section"]["items"][0]["number"];
    assert!(members.contains(&start.as_str().unwrap()));

    // The test copy: its fields, not its verdicts or checkboxes; its link out.
    let first = world.akt(start.as_str().unwrap()).await;
    assert_eq!(first["copiedFrom"]["number"], json!(tests[0]));
    assert_eq!(field(&first, "title").unwrap()["text"], "Boots");
    assert!(field(&first, "verdict").is_none() && field(&first, "reviewed").is_none());
    assert_eq!(first["outcome"], "PENDING");
    assert_eq!(
        first["links"],
        json!([{ "name": "Verifies", "pinnedRevision": 1, "akt": { "number": requirement } }])
    );

    // Naming an akt outside the tree, nothing is copied.
    let before = world.count().await;
    let refused = world
        .run(
            "mutation($n: String!, $o: [String!]) { copyAkt(input: { number: $n, only: $o }) { number } }",
            json!({ "n": tp, "o": [requirement] }),
        )
        .await;
    assert_eq!(code(&refused), "BAD_REQUEST", "{refused}");
    // Into a container that can't take it: all or nothing.
    let refused = world
        .run(
            "mutation($n: String!, $i: [String!]) { copyAkt(input: { number: $n, in: $i }) { number } }",
            json!({ "n": tp, "i": [requirement] }),
        )
        .await;
    assert_eq!(code(&refused), "NOT_CONTAINABLE", "{refused}");
    assert_eq!(world.count().await, before);
}
