//! Boards, through GraphQL against a fresh database.

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_server::authorize::READ;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

const ALICE: &str = "tenant-alice";

struct World {
    _db: TestDatabase,
    router: Router,
    decider: Arc<FakeDecider>,
}

fn workflow(states: &[&str], transitions: Value) -> (Value, Value) {
    let states: Vec<Value> = states
        .iter()
        .enumerate()
        .map(|(i, key)| json!({ "key": key, "name": key, "initial": i == 0 }))
        .collect();
    (Value::Array(states), transitions)
}

fn types() -> Vec<Value> {
    let (story_states, story_moves) = workflow(
        &["open", "doing", "blocked", "done"],
        json!([
            { "key": "start", "name": "Start", "from": ["open"], "to": "doing" },
            { "key": "block", "name": "Block", "from": ["doing"], "to": "blocked" },
            { "key": "finish", "name": "Finish", "from": ["doing"], "to": "done" }
        ]),
    );
    let (bug_states, bug_moves) = workflow(
        &["new", "fixing", "fixed"],
        json!([
            { "key": "fix", "name": "Fix", "from": ["new"], "to": "fixing" },
            { "key": "close", "name": "Close", "from": ["fixing"], "to": "fixed" }
        ]),
    );
    vec![
        json!({ "key": "story", "name": "Story", "number": "STORY-{seq}",
                "states": story_states, "transitions": story_moves }),
        json!({ "key": "bug", "name": "Bug", "number": "BUG-{seq}",
                "states": bug_states, "transitions": bug_moves }),
        json!({ "key": "epic", "name": "Epic", "number": "EPIC-{seq}",
                "states": [{ "key": "open", "name": "Open", "initial": true }],
                "contains": [{ "type": "story" }, { "type": "bug" }],
                "board": { "columns": [
                    { "key": "todo", "name": "To do",
                      "states": { "story": ["open"], "bug": ["new"] } },
                    { "key": "doing", "name": "In progress",
                      "states": { "story": ["doing"], "bug": ["fixing"] } },
                    { "key": "done", "name": "Done",
                      "states": { "story": ["done"], "bug": ["fixed"] } }
                ] } }),
        json!({ "key": "pile", "name": "Pile", "number": "PILE-{seq}",
                "states": [{ "key": "open", "name": "Open", "initial": true }],
                "contains": [{ "type": "story" }] }),
    ]
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            _db: db,
            router,
            decider,
        };
        for document in types() {
            world
                .ok(
                    "tenant",
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with(token, query, &variables)).await;
        let answer = answer.json();
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, inside: &[&str]) -> String {
        self.ok(
            "tenant",
            "mutation($t: String!, $in: [String!]!) { createAkt(input: { type: $t, in: $in }) { number } }",
            json!({ "t": key, "in": inside }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Performs `transitions` on `number` in order.
    async fn advance(&self, number: &str, transitions: &[&str]) {
        for (i, transition) in transitions.iter().enumerate() {
            self.ok(
                "tenant",
                "mutation($n: String!, $r: Int!, $t: String!) { performTransition(input: { number: $n, revision: $r, transition: $t }) { number } }",
                json!({ "n": number, "r": i + 1, "t": transition }),
            )
            .await;
        }
    }

    /// The board of `container` as column keys with their members' numbers.
    async fn board(&self, token: &str, container: &str) -> Vec<(String, Vec<String>)> {
        let data = self
            .ok(
                token,
                "query($n: String!) { akt(number: $n) { board { columns { key name akter { number } } } } }",
                json!({ "n": container }),
            )
            .await;
        data["akt"]["board"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|column| {
                let numbers = column["akter"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a["number"].as_str().unwrap().to_owned())
                    .collect();
                (column["key"].as_str().unwrap().to_owned(), numbers)
            })
            .collect()
    }
}

fn columns(spec: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    spec.iter()
        .map(|(key, numbers)| {
            (
                (*key).to_owned(),
                numbers.iter().map(|n| (*n).to_owned()).collect(),
            )
        })
        .collect()
}

// r[verify board.placement]
// r[verify board.unmapped]
#[tokio::test(flavor = "multi_thread")]
async fn places_members_of_different_types_in_shared_columns() {
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    let s1 = world.create("story", &[&epic]).await;
    let b1 = world.create("bug", &[&epic]).await;
    let s2 = world.create("story", &[&epic]).await;
    let s3 = world.create("story", &[&epic]).await;
    world.advance(&s2, &["start"]).await;
    world.advance(&b1, &["fix", "close"]).await;
    world.advance(&s3, &["start", "block"]).await;

    assert_eq!(
        world.board("tenant", &epic).await,
        columns(&[
            ("todo", &[s1.as_str()]),
            ("doing", &[s2.as_str()]),
            ("done", &[b1.as_str()]),
            ("unmapped", &[s3.as_str()]),
        ])
    );
}

// r[verify board.placement]
#[tokio::test(flavor = "multi_thread")]
async fn columns_keep_rank_order() {
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    let a = world.create("story", &[&epic]).await;
    let b = world.create("bug", &[&epic]).await;
    let c = world.create("story", &[&epic]).await;
    world
        .ok(
            "tenant",
            "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { number } }",
            json!({ "n": c, "o": a }),
        )
        .await;
    assert_eq!(
        world.board("tenant", &epic).await[0],
        ("todo".to_owned(), vec![c, a, b])
    );
}

// r[verify board.default]
#[tokio::test(flavor = "multi_thread")]
async fn a_container_without_columns_shows_everything_unmapped() {
    let world = World::new().await;
    let pile = world.create("pile", &[]).await;
    let a = world.create("story", &[&pile]).await;
    let b = world.create("story", &[&pile]).await;
    assert_eq!(
        world.board("tenant", &pile).await,
        columns(&[("unmapped", &[a.as_str(), b.as_str()])])
    );
}

// r[verify board.visibility]
#[tokio::test(flavor = "multi_thread")]
async fn a_board_leaves_out_what_the_subject_cant_read() {
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    let story = world.create("story", &[&epic]).await;
    world.create("bug", &[&epic]).await;
    world.decider.grant_at(ALICE, "epic", &[READ]);
    world.decider.grant_at(ALICE, "story", &[READ]);
    assert_eq!(
        world.board(ALICE, &epic).await,
        columns(&[
            ("todo", &[story.as_str()]),
            ("doing", &[]),
            ("done", &[]),
            ("unmapped", &[]),
        ])
    );
}

const DEFINE: &str = "mutation($d: JSON!) { defineAktType(document: $d) { version } }";

fn problems(answer: &Value) -> Vec<String> {
    answer["errors"][0]["extensions"]["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .map(|p| {
                    format!(
                        "{}: {}",
                        p["path"].as_str().unwrap(),
                        p["message"].as_str().unwrap()
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

impl World {
    async fn define(&self, document: &Value) -> Value {
        send(
            &self.router,
            graphql_with("tenant", DEFINE, &json!({ "d": document })),
        )
        .await
        .json()
    }
}

fn type_named(key: &str) -> Value {
    types().into_iter().find(|t| t["key"] == key).unwrap()
}

// r[verify akt.type.state-references]
#[tokio::test(flavor = "multi_thread")]
async fn a_member_type_cant_drop_a_state_the_board_shows() {
    let world = World::new().await;
    // A story without `doing`, which the epic's board shows.
    let mut story = type_named("story");
    story["states"].as_array_mut().unwrap().remove(1);
    story["transitions"] = json!([]);
    let answer = world.define(&story).await;
    assert_eq!(
        problems(&answer),
        [
            "states: `epic` names the state `doing` at board.columns[1].states.story[0]; stop naming it there first"
        ]
    );

    // Taking it off the board first, then dropping it, works.
    let mut epic = type_named("epic");
    epic["board"]["columns"][1]["states"] = json!({ "bug": ["fixing"] });
    assert!(world.define(&epic).await.get("errors").is_none());
    let answer = world.define(&story).await;
    assert!(answer.get("errors").is_none(), "{answer}");
}

// r[verify akt.type.state-references]
#[tokio::test(flavor = "multi_thread")]
async fn a_board_cant_name_a_state_its_member_type_lacks() {
    let world = World::new().await;
    let mut epic = type_named("epic");
    epic["board"]["columns"][2]["states"]["story"] = json!(["done", "shipped"]);
    let answer = world.define(&epic).await;
    assert_eq!(
        problems(&answer),
        ["board.columns[2].states.story[1]: `story` has no state `shipped`"]
    );
}

// r[verify akt.type.state-references]
#[tokio::test(flavor = "multi_thread")]
async fn documents_can_be_defined_in_any_order() {
    let db = TestDatabase::new().await;
    let router = router_with(
        &FakeHost::new(),
        db.pool().clone(),
        FakeDecider::allowing_tenant(),
    );
    // The epic first: its board names `story` and `bug`, not yet defined.
    for key in ["epic", "story", "bug"] {
        let answer = send(
            &router,
            graphql_with("tenant", DEFINE, &json!({ "d": type_named(key) })),
        )
        .await
        .json();
        assert!(answer.get("errors").is_none(), "{key}: {answer}");
    }
}
