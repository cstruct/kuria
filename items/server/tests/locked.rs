//! Fields locked in some states, changed only with a reason, through
//! GraphQL against a fresh database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

async fn edit(world: &World, number: &str, fields: Value, reason: Option<&str>) -> Value {
    let revision = world.revision(ADMIN, number).await;
    world
        .run(
            ADMIN,
            "mutation($n: String!, $r: Int!, $f: JSON!, $why: String) { editAkt(input: { number: $n, revision: $r, fields: $f, reason: $why }) { revision } }",
            json!({ "n": number, "r": revision, "f": fields, "why": reason }),
        )
        .await
}

// r[verify akt.field.locked]
#[tokio::test(flavor = "multi_thread")]
async fn a_due_date_moves_only_with_a_reason_once_work_starts() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    world
        .define(
            ADMIN,
            &json!({
                "key": "action", "name": "Action", "number": "A-{seq}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "due", "name": "Due", "kind": "date", "locked": ["doing"] }
                ],
                "states": [
                    { "key": "planned", "name": "Planned", "initial": true },
                    { "key": "doing", "name": "Doing" }
                ],
                "transitions": [{ "key": "start", "name": "Start", "from": ["planned"], "to": "doing" }]
            }),
        )
        .await;
    let schema = world
        .ok(
            ADMIN,
            "{ aktType(key: \"action\") { fields { key locked } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        schema["aktType"]["fields"][1],
        json!({ "key": "due", "locked": ["doing"] })
    );
    let action = world
        .create(
            ADMIN,
            "action",
            &[],
            json!({ "title": "Assign reviewers", "due": "2026-12-01" }),
            "Planned",
        )
        .await;
    // Moved freely while planned.
    let moved = edit(&world, &action, json!({ "due": "2026-12-15" }), None).await;
    assert!(moved.get("errors").is_none(), "{moved}");
    world
        .transition(ADMIN, &action, "start", None, "Started")
        .await;
    let refused = edit(&world, &action, json!({ "due": "2027-01-15" }), None).await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "LOCKED",
        "{refused}"
    );
    assert_eq!(
        refused["errors"][0]["message"],
        "`due` is locked while `doing`: changing it needs a reason"
    );
    let reasoned = edit(
        &world,
        &action,
        json!({ "due": "2027-01-15" }),
        Some("Waiting on the supplier"),
    )
    .await;
    assert!(reasoned.get("errors").is_none(), "{reasoned}");
    let history = world
        .ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { revisions { reason changes } } }",
            json!({ "n": action }),
        )
        .await;
    let last = history["akt"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["reason"] == "Waiting on the supplier")
        .cloned();
    assert!(last.is_some(), "{history}");
}
