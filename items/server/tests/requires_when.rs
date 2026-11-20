//! Fields a transition requires only while others hold certain values,
//! through GraphQL against a fresh database.

mod support;

use serde_json::json;

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

// r[verify akt.transition.requires-when]
#[tokio::test(flavor = "multi_thread")]
async fn a_corrective_case_needs_its_root_cause() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    world
        .define(
            ADMIN,
            &json!({
                "key": "case", "name": "Case", "number": "CASE-{seq}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "kind", "name": "Kind", "kind": "choices", "options": [
                        { "key": "corrective", "name": "Corrective" },
                        { "key": "preventive", "name": "Preventive" } ] },
                    { "key": "root_cause", "name": "Root cause", "kind": "text" }
                ],
                "states": [
                    { "key": "open", "name": "Open", "initial": true },
                    { "key": "planned", "name": "Planned" }
                ],
                "transitions": [{ "key": "plan", "name": "Plan", "from": ["open"], "to": "planned",
                    "requires_when": [{ "field": "root_cause", "when": { "kind": ["corrective"] } }] }]
            }),
        )
        .await;
    let schema = world
        .ok(
            ADMIN,
            "{ aktType(key: \"case\") { transitions { requiresWhen { field when { field options } } } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        schema["aktType"]["transitions"][0]["requiresWhen"],
        json!([{ "field": "root_cause", "when": [{ "field": "kind", "options": ["corrective"] }] }])
    );
    let plan = |number: String| {
        let world = &world;
        async move {
            let revision = world.revision(ADMIN, &number).await;
            world
                .run(
                    ADMIN,
                    "mutation($n: String!, $r: Int!) { performTransition(input: { number: $n, revision: $r, transition: \"plan\" }) { state { key } } }",
                    json!({ "n": number, "r": revision }),
                )
                .await
        }
    };
    let preventive = world
        .create(
            ADMIN,
            "case",
            &[],
            json!({ "title": "Trend", "kind": ["preventive"] }),
            "Seen",
        )
        .await;
    let planned = plan(preventive).await;
    assert_eq!(
        planned["data"]["performTransition"]["state"]["key"], "planned",
        "{planned}"
    );
    let corrective = world
        .create(
            ADMIN,
            "case",
            &[],
            json!({ "title": "Bug", "kind": ["corrective"] }),
            "Seen",
        )
        .await;
    let refused = plan(corrective.clone()).await;
    assert_eq!(
        refused["errors"][0]["extensions"]["code"], "REFUSED",
        "{refused}"
    );
    assert_eq!(
        refused["errors"][0]["message"],
        "`plan` requires root_cause"
    );
    world
        .edit(
            ADMIN,
            &corrective,
            json!({ "root_cause": "No rota" }),
            "Found it",
        )
        .await;
    let planned = plan(corrective).await;
    assert_eq!(
        planned["data"]["performTransition"]["state"]["key"], "planned",
        "{planned}"
    );
}
