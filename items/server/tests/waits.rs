//! Transitions that wait for an akt's members and linked akter, and that
//! need a link, through GraphQL against a fresh database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

/// A case contains actions and notes, links to tasks it is implemented by
/// and to tests it is verified by. Closing it waits for its actions and the
/// tasks implementing it, and needs a test verifying it.
fn case() -> Value {
    json!({
        "key": "case", "name": "Case", "number": "CASE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "closed", "name": "Closed", "final": true }
        ],
        "contains": [{ "type": "action" }, { "type": "note" }],
        "links": [
            { "key": "implemented_by", "name": "Implemented by", "inverse": "Implements", "to": ["task"] },
            { "key": "verified_by", "name": "Verified by", "inverse": "Verifies", "to": ["task"] }
        ],
        "transitions": [
            { "key": "close", "name": "Close", "from": ["open"], "to": "closed",
              "waits_for": { "members": ["action"], "links": ["implemented_by"] },
              "requires_links": ["verified_by"] }
        ]
    })
}

/// An open and a done state, for actions, notes and tasks alike.
fn simple(key: &str, prefix: &str) -> Value {
    json!({
        "key": key, "name": key, "number": format!("{prefix}-{{seq}}"),
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }]
    })
}

async fn world() -> World {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    for document in [
        case(),
        simple("action", "ACT"),
        simple("note", "NOTE"),
        simple("task", "TASK"),
    ] {
        world.define(ADMIN, &document).await;
    }
    world
}

async fn close(world: &World, number: &str) -> Value {
    let revision = world.revision(ADMIN, number).await;
    world
        .run(
            ADMIN,
            "mutation($n: String!, $r: Int!) { performTransition(input: { number: $n, revision: $r, transition: \"close\" }) { state { key } } }",
            json!({ "n": number, "r": revision }),
        )
        .await
}

async fn waits(world: &World, number: &str) -> Value {
    world
        .ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { waits { transition open lacking } } }",
            json!({ "n": number }),
        )
        .await["akt"]["waits"]
        .clone()
}

// r[verify akt.transition.members]
// r[verify akt.transition.links]
// r[verify akt.transition.waits]
#[tokio::test(flavor = "multi_thread")]
async fn a_case_closes_once_what_it_waits_for_is_final() {
    let world = world().await;
    let schema = world
        .ok(
            ADMIN,
            "{ aktType(key: \"case\") { transitions { key waitsForMembers waitsForLinks requiresLinks } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        schema["aktType"]["transitions"][0],
        json!({ "key": "close", "waitsForMembers": ["action"],
                "waitsForLinks": ["implemented_by"], "requiresLinks": ["verified_by"] })
    );
    let case = world
        .create(
            ADMIN,
            "case",
            &[],
            json!({ "title": "Late reviews" }),
            "Opened",
        )
        .await;
    let action = world
        .create(
            ADMIN,
            "action",
            &[&case],
            json!({ "title": "Assign reviewers" }),
            "Planned",
        )
        .await;
    // A note isn't waited for.
    world
        .create(
            ADMIN,
            "note",
            &[&case],
            json!({ "title": "Context" }),
            "Noted",
        )
        .await;
    let task = world
        .create(
            ADMIN,
            "task",
            &[],
            json!({ "title": "Reviewer rota" }),
            "Planned",
        )
        .await;
    let test = world
        .create(
            ADMIN,
            "task",
            &[],
            json!({ "title": "Check the rota" }),
            "Planned",
        )
        .await;
    world
        .link(ADMIN, &case, "implemented_by", &task, "Fixes it")
        .await;

    assert_eq!(
        waits(&world, &case).await,
        json!([{ "transition": "close", "open": [action, task], "lacking": ["Verified by"] }])
    );
    let refused = close(&world, &case).await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "REFUSED",
        "{refused}"
    );
    assert_eq!(
        refused["errors"][0]["message"],
        format!(
            "`close` waits for {action} and {task}, which are not final yet, and a `verified_by` link"
        )
    );

    world
        .transition(ADMIN, &action, "finish", None, "Done")
        .await;
    world
        .transition(ADMIN, &task, "finish", None, "Merged")
        .await;
    world
        .link(ADMIN, &case, "verified_by", &test, "Checks it")
        .await;
    // The test verifying it needn't be done.
    assert_eq!(waits(&world, &case).await, json!([]));
    let closed = close(&world, &case).await;
    assert_eq!(
        closed["data"]["performTransition"]["state"]["key"], "closed",
        "{closed}"
    );
    assert_eq!(
        waits(&world, &case).await,
        json!([]),
        "a final akt waits for nothing"
    );
}
