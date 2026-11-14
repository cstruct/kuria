//! Batches of akt writes, through GraphQL against a fresh database
//! (`akt.batch`).

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_server::authorize::{EDIT, READ};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};
use uuid::Uuid;

use support::decider::FakeDecider;
use support::{FakeHost, TENANT, graphql_with, router_with, send};

/// Reads plans and tasks, and edits tasks only.
const TASK_EDITOR: &str = "tenant-task-editor";

const BATCH: &str = "mutation($o: [OperationInput!]!) { batch(input: { operations: $o }) { number revision state { key } } }";

struct World {
    db: TestDatabase,
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
        decider.grant_at(TASK_EDITOR, "plan", &[READ]);
        decider.grant_at(TASK_EDITOR, "task", &[READ, EDIT]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { db, router };
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
        run(&self.router, "tenant", query, variables).await
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

async fn run(router: &Router, token: &str, query: &str, variables: Value) -> Value {
    send(router, graphql_with(token, query, &variables))
        .await
        .json()
}

/// A plan with the tasks `titles` in it, made in one batch.
async fn planned(world: &World, titles: &[&str]) {
    let mut operations =
        vec![json!({ "createAkt": { "type": "plan", "fields": { "title": "Plan" } } })];
    for title in titles {
        operations.push(
            json!({ "createAkt": { "type": "task", "in": ["@1"], "fields": { "title": title } } }),
        );
    }
    let made = world.batch(Value::Array(operations)).await;
    assert!(made.get("errors").is_none(), "{made}");
}

/// The refusal a batch answered: its code, and the operation it names.
fn refusal(answer: &Value) -> (Value, Value) {
    let e = &answer["errors"][0]["extensions"];
    assert!(
        answer["data"].is_null() || answer["data"]["batch"].is_null(),
        "{answer}"
    );
    (e["code"].clone(), e["operation"].clone())
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

// r[verify akt.batch.atomic]
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_batch_leaves_no_trace() {
    let world = World::new().await;
    planned(&world, &["Spec"]).await;
    // A create, a link and an edit, then a stale edit: none of it stays,
    // not even the number the create took.
    let refused = world
        .batch(json!([
            { "createAkt": { "type": "task", "in": ["P-1"], "fields": { "title": "Gone" } } },
            { "addLink": { "source": "@1", "kind": "blocks", "target": "P-1-T1" } },
            { "editAkt": { "number": "P-1", "revision": 3, "fields": { "title": "Changed" } } },
            { "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "Stale" } } }
        ]))
        .await;
    assert_eq!(refusal(&refused), (json!("STALE"), json!(4)));
    let plan = world.akt("P-1").await;
    assert_eq!(
        (plan["title"].clone(), plan["revision"].clone()),
        (json!("Plan"), json!(2))
    );
    assert_eq!(world.akt("P-1-T1").await["links"], json!([]));
    let next = world
        .ok(
            "mutation { createAkt(input: { type: \"task\", in: [\"P-1\"], fields: { title: \"Next\" } }) { number } }",
            json!({}),
        )
        .await;
    assert_eq!(
        next["createAkt"]["number"], "P-1-T2",
        "the refused create's number is free"
    );

    // A refusal in the middle stops it there.
    let refused = world
        .batch(json!([
            { "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "First" } } },
            { "addToContainer": { "container": "P-1-T1", "member": "P-1" } },
            { "editAkt": { "number": "P-1-T2", "revision": 1, "fields": { "title": "Never" } } }
        ]))
        .await;
    assert_eq!(refusal(&refused).1, json!(2), "{refused}");
    assert_eq!(world.akt("P-1-T1").await["title"], "Spec");
}

// r[verify akt.batch.refer]
// r[verify akt.batch.limit]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_bad_references_and_sizes_before_running() {
    let world = World::new().await;
    planned(&world, &["Spec"]).await;
    for (operations, at) in [
        (
            json!([{ "editAkt": { "number": "@1", "revision": 1, "fields": {} } }]),
            json!(1),
        ),
        (
            json!([
                { "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "Not run" } } },
                { "addLink": { "source": "@1", "kind": "blocks", "target": "P-1-T1" } }
            ]),
            json!(2),
        ),
    ] {
        let refused = world.batch(operations).await;
        assert_eq!(refusal(&refused), (json!("BAD_REQUEST"), at), "{refused}");
    }
    assert_eq!(world.akt("P-1-T1").await["title"], "Spec");

    let edit = json!({ "editAkt": { "number": "P-1-T1", "revision": 1, "fields": {} } });
    for size in [0, 201] {
        let refused = world.batch(Value::Array(vec![edit.clone(); size])).await;
        assert_eq!(
            refusal(&refused),
            (json!("BAD_REQUEST"), Value::Null),
            "{size}"
        );
    }
    // 200 edits of one task, each on the revision the one before left.
    let most: Vec<Value> = (1..=200)
        .map(|r| json!({ "editAkt": { "number": "P-1-T1", "revision": r, "fields": { "title": format!("Take {r}") } } }))
        .collect();
    let most = world.batch(Value::Array(most)).await;
    assert_eq!(numbers(&most).len(), 200, "{most}");
    assert_eq!(world.akt("P-1-T1").await["revision"], 201);
}

// r[verify akt.batch]
#[tokio::test(flavor = "multi_thread")]
async fn authorizes_each_operation_as_alone() {
    let world = World::new().await;
    planned(&world, &["Spec"]).await;
    let refused = run(
        &world.router,
        TASK_EDITOR,
        BATCH,
        json!({ "o": [
            { "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "Mine" } } },
            { "editAkt": { "number": "P-1", "revision": 2, "fields": { "title": "Not mine" } } }
        ] }),
    )
    .await;
    assert_eq!(refusal(&refused), (json!("FORBIDDEN"), json!(2)));
    assert_eq!(world.akt("P-1-T1").await["title"], "Spec");
    let allowed = run(
        &world.router,
        TASK_EDITOR,
        BATCH,
        json!({ "o": [{ "editAkt": { "number": "P-1-T1", "revision": 1, "fields": { "title": "Mine" } } }] }),
    )
    .await;
    assert_eq!(numbers(&allowed), ["P-1-T1"]);
}

// r[verify akt.batch.published]
// r[verify akt.batch.revision]
#[tokio::test(flavor = "multi_thread")]
async fn records_each_operation_s_revisions_and_change_events_in_order() {
    let world = World::new().await;
    planned(&world, &["Spec", "Build"]).await;
    let mut conn = world.db.pool().get().await.unwrap();
    let tenant = Uuid::parse_str(TENANT).unwrap();
    let before = kuria_store::akter::change_events_after(&mut conn, tenant, 0, 1000)
        .await
        .unwrap()
        .len();
    // Revisions as they will be after the operations before.
    let made = world
        .batch(json!([
            { "addLink": { "source": "P-1-T1", "kind": "blocks", "target": "P-1-T2" } },
            { "editAkt": { "number": "P-1-T1", "revision": 2, "fields": { "title": "Spec it" } } },
            { "performTransition": { "number": "P-1-T1", "revision": 3, "transition": "finish" } }
        ]))
        .await;
    let revisions: Vec<i64> = made["data"]["batch"]
        .as_array()
        .unwrap_or_else(|| panic!("{made}"))
        .iter()
        .map(|a| a["revision"].as_i64().unwrap())
        .collect();
    assert_eq!(revisions, [2, 3, 4]);
    let events = kuria_store::akter::change_events_after(&mut conn, tenant, 0, 1000)
        .await
        .unwrap();
    let id = |number: &str| {
        let pool = world.db.pool().clone();
        let number = number.to_owned();
        async move {
            let mut conn = pool.get().await.unwrap();
            kuria_store::akter::akt_by_number(&mut conn, tenant, &number)
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };
    let (t1, t2) = (id("P-1-T1").await, id("P-1-T2").await);
    let said: Vec<(Uuid, Option<i32>)> = events[before..]
        .iter()
        .map(|e| (e.akt_id, e.revision))
        .collect();
    // The link's two revisions, then the edit's and the transition's.
    assert_eq!(
        said,
        [(t1, Some(2)), (t2, Some(2)), (t1, Some(3)), (t1, Some(4))]
    );
    let seqs: Vec<i64> = events[before..].iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");

    // A batch rolled back leaves no event.
    world
        .batch(json!([
            { "editAkt": { "number": "P-1-T2", "revision": 2, "fields": { "title": "Gone" } } },
            { "editAkt": { "number": "P-1-T2", "revision": 2, "fields": { "title": "Stale" } } }
        ]))
        .await;
    let after = kuria_store::akter::change_events_after(&mut conn, tenant, 0, 1000)
        .await
        .unwrap();
    assert_eq!(after.len(), events.len());
}

// r[verify akt.batch.locks]
#[tokio::test(flavor = "multi_thread")]
async fn batches_and_single_writes_at_once_never_deadlock() {
    let world = World::new().await;
    planned(&world, &["A", "B", "C", "D"]).await;
    let made = world
        .batch(json!([{ "createAkt": { "type": "plan", "fields": { "title": "Other" } } }]))
        .await;
    assert_eq!(numbers(&made), ["P-2"]);
    let task = |i: usize| format!("P-1-T{}", i % 4 + 1);
    let mut workers = Vec::new();
    for w in 0..8 {
        let router = world.router.clone();
        let (a, b, c) = (task(w), task(w + 1), task(w + 2));
        workers.push(tokio::spawn(async move {
            let mut answers = Vec::new();
            for _ in 0..6 {
                if w % 2 == 0 {
                    // A batch: a new task linked both ways, and a move.
                    answers.push(
                        run(&router, "tenant", BATCH, json!({ "o": [
                            { "createAkt": { "type": "task", "in": ["P-1"], "fields": { "title": "New" } } },
                            { "addLink": { "source": "@1", "kind": "blocks", "target": b } },
                            { "addLink": { "source": a, "kind": "blocks", "target": "@1" } },
                            { "moveAkt": { "number": c, "before": a } },
                            { "addToContainer": { "container": "P-2", "member": "@1" } }
                        ] }))
                        .await,
                    );
                } else {
                    // Single writes on the same akter, one after another.
                    for (query, variables) in [
                        (
                            "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m }) { number } }",
                            json!({ "c": "P-2", "m": a }),
                        ),
                        (
                            "mutation($n: String!, $a: String!) { moveAkt(input: { number: $n, after: $a }) { number } }",
                            json!({ "n": a, "a": b }),
                        ),
                        (
                            "mutation($c: String!, $m: String!) { removeFromContainer(input: { container: $c, member: $m }) { number } }",
                            json!({ "c": "P-2", "m": a }),
                        ),
                    ] {
                        answers.push(run(&router, "tenant", query, variables).await);
                    }
                }
            }
            answers
        }));
    }
    let finished = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        let mut all = Vec::new();
        for worker in workers {
            all.extend(worker.await.unwrap());
        }
        all
    })
    .await
    .expect("every write finished");
    // Single writers on the same task take turns on its membership.
    for answer in &finished {
        if let Some(errors) = answer.get("errors") {
            let code = &errors[0]["extensions"]["code"];
            assert!(
                code == "ALREADY_MEMBER" || code == "NOT_MEMBER",
                "only the expected refusals, never a deadlock: {answer}"
            );
        }
    }
    let batches = finished
        .iter()
        .filter(|a| a["data"]["batch"].is_array())
        .count();
    assert_eq!(batches, 4 * 6, "every batch committed");
}
