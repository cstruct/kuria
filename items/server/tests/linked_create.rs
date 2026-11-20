//! Creating an akt linked to another, through GraphQL against a fresh
//! database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

/// A report, a case that originates from one, and a task that implements a
/// case.
async fn world() -> World {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    let simple = |key: &str, number: &str, links: Value| {
        json!({
            "key": key, "name": key, "number": number,
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "links": links
        })
    };
    world
        .define(ADMIN, &simple("report", "PR-{seq}", json!([])))
        .await;
    world
        .define(ADMIN, &simple("task", "T-{seq}", json!([])))
        .await;
    world
        .define(
            ADMIN,
            &simple(
                "case",
                "CASE-{seq}",
                json!([
                    { "key": "originated_from", "name": "Originated from", "inverse": "Led to", "to": ["report"] },
                    { "key": "implemented_by", "name": "Implemented by", "inverse": "Implements", "to": ["task"] }
                ]),
            ),
        )
        .await;
    world
}

async fn create(world: &World, key: &str, title: &str, links: Value) -> Value {
    world
        .run(
            ADMIN,
            "mutation($t: String!, $f: JSON, $l: [NewLinkInput!]) { createAkt(input: { type: $t, fields: $f, links: $l, reason: \"Escalated\" }) { number links { kind name outgoing akt { number } } } }",
            json!({ "t": key, "f": { "title": title }, "l": links }),
        )
        .await
}

// r[verify link.create]
#[tokio::test(flavor = "multi_thread")]
async fn a_case_is_created_from_the_report_it_originates_from() {
    let world = world().await;
    let report = world
        .create(
            ADMIN,
            "report",
            &[],
            json!({ "title": "Wrong total" }),
            "Seen",
        )
        .await;
    let case = create(
        &world,
        "case",
        "Totals",
        json!([{ "kind": "originated_from", "to": report }]),
    )
    .await;
    assert_eq!(
        case["data"]["createAkt"]["links"],
        json!([{ "kind": "originated_from", "name": "Originated from", "outgoing": true,
                 "akt": { "number": report } }]),
        "{case}"
    );
    let number = case["data"]["createAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    // A task implementing the case: the new akt is the link's target.
    let task = create(
        &world,
        "task",
        "Fix rounding",
        json!([{ "kind": "implemented_by", "from": number }]),
    )
    .await;
    assert_eq!(
        task["data"]["createAkt"]["links"],
        json!([{ "kind": "implemented_by", "name": "Implements", "outgoing": false,
                 "akt": { "number": number } }]),
        "{task}"
    );
    // A refused link refuses the creation: nothing is left of it.
    let refused = create(
        &world,
        "case",
        "Elsewhere",
        json!([{ "kind": "originated_from", "to": "T-1" }]),
    )
    .await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "INVALID_LINK",
        "{refused}"
    );
    let cases = world
        .ok(
            ADMIN,
            "{ akter(type: \"case\") { nodes { number } } }",
            json!({}),
        )
        .await;
    assert_eq!(cases["akter"]["nodes"], json!([{ "number": number }]));
}
