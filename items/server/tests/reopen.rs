//! Reopening an akt from a final state, through GraphQL against a fresh
//! database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

async fn add(world: &World, container: &str, member: &str) -> Value {
    world
        .run(
            ADMIN,
            "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m }) { number } }",
            json!({ "c": container, "m": member }),
        )
        .await
}

/// A case that closes and reopens, containing notes.
async fn world() -> World {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    let case = json!({
        "key": "case", "name": "Case", "number": "CASE-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "why_reopened", "name": "Why reopened", "kind": "text" }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "closed", "name": "Closed", "final": true }
        ],
        "contains": [{ "type": "note" }],
        "transitions": [
            { "key": "close", "name": "Close", "from": ["open"], "to": "closed" },
            { "key": "reopen", "name": "Reopen", "from": ["closed"], "to": "open",
              "reopens": true, "requires": ["why_reopened"] }
        ]
    });
    let note = json!({
        "key": "note", "name": "Note", "number": "NOTE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }]
    });
    world.define(ADMIN, &case).await;
    world.define(ADMIN, &note).await;
    world
}

// r[verify akt.transition.reopens]
#[tokio::test(flavor = "multi_thread")]
async fn a_closed_case_reopens_and_takes_members_again() {
    let world = world().await;
    let schema = world
        .ok(
            ADMIN,
            "{ aktType(key: \"case\") { transitions { key reopens } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        schema["aktType"]["transitions"],
        json!([{ "key": "close", "reopens": false }, { "key": "reopen", "reopens": true }])
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
    world.transition(ADMIN, &case, "close", None, "Done").await;
    let note = world
        .create(
            ADMIN,
            "note",
            &[],
            json!({ "title": "It recurred" }),
            "Seen",
        )
        .await;
    let frozen: Value = add(&world, &case, &note).await;
    assert_eq!(
        frozen["errors"][0]["extensions"]["code"], "FROZEN",
        "{frozen}"
    );
    world
        .transition(
            ADMIN,
            &case,
            "reopen",
            Some(json!({ "why_reopened": "It recurred" })),
            "Recurred",
        )
        .await;
    let added = add(&world, &case, &note).await;
    assert!(added.get("errors").is_none(), "{added}");
    world
        .edit(
            ADMIN,
            &case,
            json!({ "title": "Late reviews, again" }),
            "Renamed",
        )
        .await;
    let history = world
        .ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { state { key } revisions { transition } } }",
            json!({ "n": case }),
        )
        .await;
    assert_eq!(history["akt"]["state"]["key"], "open");
    let transitions: Vec<&Value> = history["akt"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["transition"])
        .filter(|t| !t.is_null())
        .collect();
    assert_eq!(transitions, [&json!("close"), &json!("reopen")]);
}
