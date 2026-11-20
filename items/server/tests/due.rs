//! Due dates and overdue akter, through GraphQL against a fresh database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

async fn overdue_numbers(world: &World, key: &str) -> Value {
    world
        .run(
            ADMIN,
            "query($t: String!) { akter(type: $t, overdue: true) { nodes { number overdue } } }",
            json!({ "t": key }),
        )
        .await
}

// r[verify akt.field.due]
// r[verify akt.filter.overdue]
#[tokio::test(flavor = "multi_thread")]
async fn an_action_is_overdue_once_its_due_date_passes_while_open() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    world
        .define(
            ADMIN,
            &json!({
                "key": "action", "name": "Action", "number": "A-{seq}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "due", "name": "Due", "kind": "date", "due": true }
                ],
                "states": [
                    { "key": "open", "name": "Open", "initial": true },
                    { "key": "done", "name": "Done", "final": true }
                ],
                "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }]
            }),
        )
        .await;
    world
        .define(
            ADMIN,
            &json!({
                "key": "note", "name": "Note", "number": "N-{seq}",
                "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
                "states": [{ "key": "open", "name": "Open", "initial": true }]
            }),
        )
        .await;
    let late = world
        .create(
            ADMIN,
            "action",
            &[],
            json!({ "title": "Late", "due": "2020-01-01" }),
            "Planned",
        )
        .await;
    world
        .create(
            ADMIN,
            "action",
            &[],
            json!({ "title": "Later", "due": "2999-01-01" }),
            "Planned",
        )
        .await;
    world
        .create(
            ADMIN,
            "action",
            &[],
            json!({ "title": "Undated" }),
            "Planned",
        )
        .await;
    let listed = overdue_numbers(&world, "action").await;
    assert_eq!(
        listed["data"]["akter"]["nodes"],
        json!([{ "number": late, "overdue": ["due"] }]),
        "{listed}"
    );
    world.transition(ADMIN, &late, "finish", None, "Done").await;
    let listed = overdue_numbers(&world, "action").await;
    assert_eq!(listed["data"]["akter"]["nodes"], json!([]), "{listed}");
    let read = world
        .ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { overdue } }",
            json!({ "n": late }),
        )
        .await;
    assert_eq!(
        read["akt"]["overdue"],
        json!([]),
        "a final akt isn't overdue"
    );
    let refused = overdue_numbers(&world, "note").await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "BAD_REQUEST",
        "{refused}"
    );
}
