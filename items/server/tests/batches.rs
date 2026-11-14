//! Batches of akt writes, through GraphQL against a fresh database
//! (`akt.batch`).

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

const BATCH: &str = "mutation($o: [OperationInput!]!) { batch(input: { operations: $o }) { number revision state { key } } }";

struct World {
    _db: TestDatabase,
    router: Router,
}

fn plan() -> Value {
    json!({
        "key": "plan", "name": "Plan", "number": "P-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "task" }]
    })
}

fn task() -> Value {
    json!({
        "key": "task", "name": "Task", "number": "{parent}-T{seq}", "created_in": ["plan"],
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "todo", "name": "To do", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["todo"], "to": "done" }],
        "links": [{ "key": "blocks", "name": "Blocks", "inverse": "Blocked by", "to": ["task"] }]
    })
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for document in [task(), plan()] {
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

    async fn batch(&self, operations: Value) -> Value {
        self.run(BATCH, json!({ "o": operations })).await
    }

    async fn akt(&self, number: &str) -> Value {
        self.ok(
            "query($n: String!) { akt(number: $n) { number revision title state { key } links { name akt { number } } } }",
            json!({ "n": number }),
        )
        .await["akt"]
            .clone()
    }
}

fn numbers(answer: &Value) -> Vec<&str> {
    answer["data"]["batch"]
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .iter()
        .map(|a| a["number"].as_str().unwrap())
        .collect()
}

// r[verify akt.batch]
// r[verify akt.batch.refer]
// r[verify akt.batch.atomic]
#[tokio::test(flavor = "multi_thread")]
async fn a_batch_runs_every_write_or_none() {
    let world = World::new().await;
    // A plan, two tasks in it, a link between them, and a transition.
    let made = world
        .batch(json!([
            { "createAkt": { "type": "plan", "fields": { "title": "Batches" } } },
            { "createAkt": { "type": "task", "in": ["@1"], "fields": { "title": "Spec" } } },
            { "createAkt": { "type": "task", "in": ["@1"], "fields": { "title": "Build" } } },
            { "addLink": { "source": "@2", "kind": "blocks", "target": "@3" } },
            { "performTransition": { "number": "@2", "revision": 2, "transition": "finish" } }
        ]))
        .await;
    assert_eq!(
        numbers(&made),
        ["P-1", "P-1-T1", "P-1-T2", "P-1-T1", "P-1-T1"],
        "{made}"
    );
    let spec = world.akt("P-1-T1").await;
    assert_eq!(spec["state"]["key"], "done");
    assert_eq!(spec["links"][0]["akt"]["number"], "P-1-T2");

    // Refused at its last operation: the edit before it is undone too.
    let refused = world
        .batch(json!([
            { "editAkt": { "number": "P-1-T2", "revision": 2, "fields": { "title": "Build it" } } },
            { "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "Stale" } } }
        ]))
        .await;
    let error = &refused["errors"][0]["extensions"];
    assert_eq!(
        (&error["code"], &error["operation"]),
        (&json!("STALE"), &json!(2)),
        "{refused}"
    );
    assert_eq!(world.akt("P-1-T2").await["title"], "Build");
}
