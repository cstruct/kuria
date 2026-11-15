//! Pushes from clones through GraphQL against a fresh database: live drafts'
//! updates made, as a clone makes them, against the state `liveState`
//! answers, and edits of other akter, all kept or none (`clone.server`,
//! `live.state`).

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use kuria_config::AktType;
use kuria_engine::Value as FieldValue;
use kuria_engine::live::LiveDraft;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{COLLEAGUE, FakeHost, SUBJECT, graphql_with, router_with, send};

struct World {
    _db: TestDatabase,
    router: Router,
}

fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "summary", "name": "Summary", "kind": "text", "multiline": true },
            { "key": "body", "name": "Body", "kind": "markdown" },
            { "key": "tags", "name": "Tags", "kind": "choices",
              "options": [{ "key": "a", "name": "A" }, { "key": "b", "name": "B" }] }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "versions": {},
        "live_drafts": true
    })
}

fn task() -> Value {
    json!({
        "key": "task", "name": "Task", "number": "T-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }]
    })
}

/// The document type, as a clone reads it to make updates.
fn document_type() -> AktType {
    kuria_config::validate(serde_json::from_value(document()).unwrap()).unwrap()
}

const LIVE_STATE: &str =
    "query($n: String!) { akt(number: $n) { liveState { state stateVector base } } }";

const AKT: &str = "query($n: String!) { akt(number: $n) {
    title revision
    fields { ... on TextValue { field text } ... on MarkdownValue { field markdown }
      ... on ChoicesValue { field options { key } } }
    revisions { revision reason } } }";

const CHANGES: &str = "query($n: String!) { akt(number: $n) {
    draftChanges(all: true) { seq by { id } fields message } } }";

const PUSH: &str = "mutation($i: PushCloneInput!) { pushClone(input: $i) {
    drafts { number revision title fields { ... on MarkdownValue { field markdown } } }
    operations { number revision title } } }";

const EDIT: &str = "mutation($n: String!, $v: Int!, $f: JSON!) {
    editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }";

/// A live draft as a clone has it: loaded from `liveState`, and the base it
/// read.
struct Clone {
    draft: LiveDraft,
    base: String,
}

impl Clone {
    fn fields(&self) -> BTreeMap<String, FieldValue> {
        self.draft.fields(&document_type()).unwrap()
    }

    /// The update that makes the clone's draft hold `fields`, made against
    /// its base, as a push sends it.
    fn write(&mut self, fields: &BTreeMap<String, FieldValue>) -> String {
        BASE64.encode(self.draft.write(&document_type(), fields).unwrap())
    }

    /// The update that sets `key` to the text or Markdown `value`, or
    /// clears it.
    fn set(&mut self, key: &str, value: Option<&str>) -> String {
        let mut fields = self.fields();
        match (key, value) {
            (_, None) => {
                fields.remove(key);
            }
            ("body", Some(v)) => {
                fields.insert(key.to_owned(), FieldValue::Markdown(v.to_owned()));
            }
            (_, Some(v)) => {
                fields.insert(key.to_owned(), FieldValue::Text(v.to_owned()));
            }
        }
        self.write(&fields)
    }
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.allow_everything("colleague");
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for ty in [document(), task()] {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": ty }),
                )
                .await;
        }
        world
    }

    async fn run_as(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        self.run_as("tenant", query, variables).await
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, fields: Value) -> String {
        self.ok(
            "mutation($k: String!, $f: JSON) { createAkt(input: { type: $k, fields: $f }) { number } }",
            json!({ "k": key, "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn akt(&self, number: &str) -> Value {
        self.ok(AKT, json!({ "n": number })).await["akt"].clone()
    }

    async fn live_state(&self, number: &str) -> Value {
        self.ok(LIVE_STATE, json!({ "n": number })).await["akt"]["liveState"].clone()
    }

    /// The akt `number`'s live draft, as a clone reads it.
    async fn clone_of(&self, number: &str) -> Clone {
        let live = self.live_state(number).await;
        let state = BASE64.decode(live["state"].as_str().unwrap()).unwrap();
        Clone {
            draft: LiveDraft::load(&state, []).unwrap(),
            base: live["base"].as_str().unwrap().to_owned(),
        }
    }

    async fn changes(&self, number: &str) -> Vec<Value> {
        self.ok(CHANGES, json!({ "n": number })).await["akt"]["draftChanges"]
            .as_array()
            .unwrap()
            .clone()
    }

    async fn push_as(&self, token: &str, input: Value) -> Value {
        self.run_as(token, PUSH, json!({ "i": input })).await
    }
}

fn text<'a>(akt: &'a Value, key: &str) -> Option<&'a str> {
    akt["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == key)
        .and_then(|f| f["text"].as_str().or(f["markdown"].as_str()))
}

/// Whether the base64 state vectors `a` and `b` are the same, however
/// each is encoded.
fn same_vector(a: &Value, b: &str) -> bool {
    use yrs::StateVector;
    use yrs::updates::decoder::Decode;
    let decode = |s: &str| StateVector::decode_v1(&BASE64.decode(s).unwrap()).unwrap();
    decode(a.as_str().unwrap()) == decode(b)
}

/// Whether the base64 bases `a` and `b`, Yjs snapshots, are the same,
/// however each is encoded.
fn same_base(a: &Value, b: &str) -> bool {
    use yrs::Snapshot;
    use yrs::updates::decoder::Decode;
    let decode = |s: &str| Snapshot::decode_v1(&BASE64.decode(s).unwrap()).unwrap();
    decode(a.as_str().unwrap()) == decode(b)
}

fn extensions(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]
}

// r[verify clone.server]
// r[verify live.history]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_from_the_current_base_is_kept_as_one_change_with_its_message() {
    let world = World::new().await;
    let guide = world
        .create("document", json!({ "title": "Guide", "body": "Hello.\n" }))
        .await;
    let mut clone = world.clone_of(&guide).await;
    let mut fields = clone.fields();
    fields.insert("title".to_owned(), FieldValue::Text("The guide".to_owned()));
    fields.insert(
        "body".to_owned(),
        FieldValue::Markdown("Hello, clone.\n".to_owned()),
    );
    let update = clone.write(&fields);
    let pushed = world
        .push_as(
            "colleague",
            json!({ "message": "Reword the guide",
                    "drafts": [{ "number": guide, "base": clone.base, "update": update }] }),
        )
        .await;
    assert!(pushed.get("errors").is_none(), "{pushed}");
    let written = &pushed["data"]["pushClone"]["drafts"][0];
    assert_eq!(written["number"], guide.as_str());
    assert_eq!(written["title"], "The guide");
    assert_eq!(written["revision"], 1, "a push makes no revision");
    assert_eq!(text(written, "body"), Some("Hello, clone.\n"));
    assert_eq!(pushed["data"]["pushClone"]["operations"], json!([]));

    let read = world.akt(&guide).await;
    assert_eq!(read["title"], "The guide");
    assert_eq!(text(&read, "body"), Some("Hello, clone.\n"));
    assert_eq!(read["revisions"].as_array().unwrap().len(), 1);
    // One change, by who pushed, with the message, of both fields.
    let changes = world.changes(&guide).await;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0]["by"]["id"], COLLEAGUE);
    assert_eq!(changes[0]["message"], "Reword the guide");
    assert_eq!(changes[0]["fields"], json!(["body", "title"]));

    // The clone's draft is the server's now: it pushes again from it.
    let live = world.live_state(&guide).await;
    let current = BASE64.decode(live["base"].as_str().unwrap()).unwrap();
    assert!(clone.draft.is_at(&current).unwrap());
    clone.base = live["base"].as_str().unwrap().to_owned();
    let update = clone.set("summary", Some("Short."));
    let again = world
        .push_as(
            "tenant",
            json!({ "message": "Summarize",
                    "drafts": [{ "number": guide, "base": clone.base, "update": update }] }),
        )
        .await;
    assert!(again.get("errors").is_none(), "{again}");
    // Live typing has no message.
    world
        .ok(EDIT, json!({ "n": guide, "v": 1, "f": { "tags": ["a"] } }))
        .await;
    let changes = world.changes(&guide).await;
    assert_eq!(changes.len(), 3, "{changes:?}");
    assert_eq!(changes[1]["by"]["id"], SUBJECT);
    assert_eq!(changes[1]["message"], "Summarize");
    assert_eq!(changes[1]["fields"], json!(["summary"]));
    assert_eq!(changes[2]["message"], Value::Null);
}

// r[verify clone.server]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_from_a_base_the_draft_has_moved_on_from_is_stale() {
    let world = World::new().await;
    let guide = world
        .create("document", json!({ "title": "Guide", "body": "Hello.\n" }))
        .await;
    let mut clone = world.clone_of(&guide).await;
    // Someone edits the draft after the clone read it.
    world
        .ok(
            EDIT,
            json!({ "n": guide, "v": 1, "f": { "title": "Edited" } }),
        )
        .await;
    let update = clone.set("body", Some("Hello, clone.\n"));
    let refused = world
        .push_as(
            "tenant",
            json!({ "message": "Reword",
                    "drafts": [{ "number": guide, "base": clone.base, "update": update }] }),
        )
        .await;
    let ext = extensions(&refused);
    assert_eq!(ext["code"], "STALE", "{refused}");
    assert_eq!(ext["number"], guide.as_str());
    assert_eq!(ext["draft"], 1);
    let live = world.live_state(&guide).await;
    assert!(same_base(&ext["base"], live["base"].as_str().unwrap()));
    assert!(same_vector(
        &ext["stateVector"],
        live["stateVector"].as_str().unwrap()
    ));
    assert!(!same_base(&ext["base"], &clone.base));
    // Nothing was kept.
    let read = world.akt(&guide).await;
    assert_eq!(read["title"], "Edited");
    assert_eq!(text(&read, "body"), Some("Hello.\n"));
    let changes = world.changes(&guide).await;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["message"], Value::Null);

    // An edit that only deletes leaves the draft's state vector as it was,
    // but moves its base on all the same.
    let mut clone = world.clone_of(&guide).await;
    let before = world.live_state(&guide).await;
    world
        .ok(
            EDIT,
            json!({ "n": guide, "v": 1, "f": { "title": "Edited", "body": "Hello" } }),
        )
        .await;
    let after = world.live_state(&guide).await;
    assert!(same_vector(
        &after["stateVector"],
        before["stateVector"].as_str().unwrap()
    ));
    let update = clone.set("summary", Some("Short."));
    let refused = world
        .push_as(
            "tenant",
            json!({ "message": "Summarize",
                    "drafts": [{ "number": guide, "base": clone.base, "update": update }] }),
        )
        .await;
    assert_eq!(extensions(&refused)["code"], "STALE", "{refused}");
    assert_eq!(text(&world.akt(&guide).await, "summary"), None);
}

// r[verify clone.server]
// r[verify live.edit]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_that_leaves_a_field_invalid_keeps_nothing() {
    let world = World::new().await;
    let one = world
        .create("document", json!({ "title": "One", "body": "First.\n" }))
        .await;
    let two = world
        .create("document", json!({ "title": "Two", "body": "Second.\n" }))
        .await;
    let chore = world.create("task", json!({ "title": "Chore" })).await;
    let mut first = world.clone_of(&one).await;
    let mut second = world.clone_of(&two).await;
    let fine = first.set("body", Some("First, edited.\n"));
    // The second clears a required title.
    let invalid = second.set("title", None);
    let refused = world
        .push_as(
            "tenant",
            json!({ "message": "Tidy up",
                    "drafts": [
                        { "number": one, "base": first.base, "update": fine },
                        { "number": two, "base": second.base, "update": invalid }
                    ],
                    "operations": [
                        { "editAkt": { "number": chore, "revision": 1, "fields": { "title": "Done" } } }
                    ] }),
        )
        .await;
    let ext = extensions(&refused);
    assert_eq!(ext["code"], "REFUSED", "{refused}");
    assert_eq!(ext["draft"], 2);
    for (number, title, body) in [(&one, "One", "First.\n"), (&two, "Two", "Second.\n")] {
        let read = world.akt(number).await;
        assert_eq!(read["title"], title);
        assert_eq!(text(&read, "body"), Some(body));
        assert_eq!(world.changes(number).await, Vec::<Value>::new());
    }
    let chore_read = world.akt(&chore).await;
    assert_eq!(chore_read["revision"], 1);
    assert_eq!(chore_read["title"], "Chore");

    // The same, without what was invalid, is taken: both from the bases
    // the clones read, as nothing was kept.
    let refused_base = Value::from(first.base.clone());
    let mut first = world.clone_of(&one).await;
    assert!(same_base(&refused_base, &first.base));
    let fine = first.set("body", Some("First, edited.\n"));
    let taken = world
        .push_as(
            "tenant",
            json!({ "message": "Tidy up",
                    "drafts": [{ "number": one, "base": first.base, "update": fine }],
                    "operations": [
                        { "editAkt": { "number": chore, "revision": 1, "fields": { "title": "Done" } } }
                    ] }),
        )
        .await;
    assert!(taken.get("errors").is_none(), "{taken}");
    assert_eq!(
        text(&world.akt(&one).await, "body"),
        Some("First, edited.\n")
    );
}

// r[verify clone.server]
// r[verify akt.revision.concurrency]
#[tokio::test(flavor = "multi_thread")]
async fn operations_become_revisions_with_the_message_as_their_reason() {
    let world = World::new().await;
    let chore = world.create("task", json!({ "title": "Chore" })).await;
    let guide = world
        .create("document", json!({ "title": "Guide", "body": "Hello.\n" }))
        .await;
    let pushed = world
        .push_as(
            "tenant",
            json!({ "message": "Rename the chore",
                    "operations": [
                        { "editAkt": { "number": chore, "revision": 1,
                                       "fields": { "title": "Errand" }, "reason": "ignored" } }
                    ] }),
        )
        .await;
    assert!(pushed.get("errors").is_none(), "{pushed}");
    let written = &pushed["data"]["pushClone"]["operations"][0];
    assert_eq!(written["revision"], 2);
    assert_eq!(written["title"], "Errand");
    let read = world.akt(&chore).await;
    assert_eq!(read["revisions"][1]["reason"], "Rename the chore");

    // From the revision the clone read, now stale, it is refused, and
    // takes the draft's update with it.
    let mut clone = world.clone_of(&guide).await;
    let update = clone.set("body", Some("Hello, clone.\n"));
    let refused = world
        .push_as(
            "tenant",
            json!({ "message": "Again",
                    "drafts": [{ "number": guide, "base": clone.base, "update": update }],
                    "operations": [
                        { "editAkt": { "number": chore, "revision": 1, "fields": { "title": "Chore" } } }
                    ] }),
        )
        .await;
    let ext = extensions(&refused);
    assert_eq!(ext["code"], "STALE", "{refused}");
    assert_eq!(ext["currentRevision"], 2);
    assert_eq!(ext["operation"], 1);
    assert_eq!(world.akt(&chore).await["title"], "Errand");
    assert_eq!(text(&world.akt(&guide).await, "body"), Some("Hello.\n"));
    assert_eq!(world.changes(&guide).await, Vec::<Value>::new());

    // A draft for an akt without one, a message of two lines, or nothing
    // to push, is a bad request.
    let no_draft = world
        .push_as(
            "tenant",
            json!({ "message": "Wrong",
                    "drafts": [{ "number": chore, "base": clone.base, "update": "" }] }),
        )
        .await;
    assert_eq!(extensions(&no_draft)["code"], "BAD_REQUEST", "{no_draft}");
    for input in [
        json!({ "message": "two\nlines", "operations": [
            { "editAkt": { "number": chore, "revision": 2, "fields": {} } }] }),
        json!({ "message": "x".repeat(201), "operations": [
            { "editAkt": { "number": chore, "revision": 2, "fields": {} } }] }),
        json!({ "message": "Nothing" }),
    ] {
        let answer = world.push_as("tenant", input).await;
        assert_eq!(extensions(&answer)["code"], "BAD_REQUEST", "{answer}");
    }
}

// r[verify live.state]
#[tokio::test(flavor = "multi_thread")]
async fn the_live_state_loads_as_the_draft_the_server_has() {
    let world = World::new().await;
    let guide = world
        .create(
            "document",
            json!({ "title": "Guide", "summary": "Short.", "body": "# Intro {#intro}\n\nHello.\n", "tags": ["a", "b"] }),
        )
        .await;
    world
        .ok(
            EDIT,
            json!({ "n": guide, "v": 1, "f": { "summary": "Shorter." } }),
        )
        .await;
    let clone = world.clone_of(&guide).await;
    let expected: BTreeMap<String, FieldValue> = [
        ("title", FieldValue::Text("Guide".to_owned())),
        ("summary", FieldValue::Text("Shorter.".to_owned())),
        (
            "body",
            FieldValue::Markdown("# Intro {#intro}\n\nHello.\n".to_owned()),
        ),
        (
            "tags",
            FieldValue::Choices(vec!["a".to_owned(), "b".to_owned()]),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(clone.fields(), expected);
    // Its state vector is the one answered.
    assert!(same_base(
        &BASE64.encode(clone.draft.base()).into(),
        &clone.base
    ));
    let live = world.live_state(&guide).await;
    assert!(same_vector(
        &BASE64.encode(clone.draft.state_vector()).into(),
        live["stateVector"].as_str().unwrap()
    ));

    // A colleague who may read it reads it too; an akt without a live
    // draft, or one read at a version, has none.
    let colleague = world
        .run_as("colleague", LIVE_STATE, json!({ "n": guide }))
        .await;
    assert!(same_base(
        &colleague["data"]["akt"]["liveState"]["base"],
        &clone.base
    ));
    let chore = world.create("task", json!({ "title": "Chore" })).await;
    assert_eq!(world.live_state(&chore).await, Value::Null);
    let at_revision = world
        .ok(
            "query($n: String!) { akt(number: $n, revision: 1) { liveState { state } } }",
            json!({ "n": guide }),
        )
        .await;
    assert_eq!(at_revision["akt"]["liveState"], Value::Null);
}
