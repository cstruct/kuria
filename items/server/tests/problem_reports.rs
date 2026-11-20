//! Problem reports in the release-management example
//! (`docs/examples/release-management`): classified, fixed by tasks, or
//! closed as won't fix, through GraphQL against a fresh database.

mod support;

use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";

const DOCUMENTS: [&str; 3] = [
    include_str!("../../../docs/examples/release-management/backlog_item.json"),
    include_str!("../../../docs/examples/release-management/backlog.json"),
    include_str!("../../../docs/examples/release-management/problem_report.json"),
];

async fn world() -> World {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    for document in DOCUMENTS {
        let document: Value = serde_json::from_str(document).unwrap();
        world.define(ADMIN, &document).await;
    }
    world
}

async fn perform(world: &World, number: &str, transition: &str, fields: Value) -> Value {
    let revision = world.revision(ADMIN, number).await;
    world
        .run(
            ADMIN,
            "mutation($n: String!, $r: Int!, $t: String!, $f: JSON) { performTransition(input: { number: $n, revision: $r, transition: $t, fields: $f }) { state { key } } }",
            json!({ "n": number, "r": revision, "t": transition, "f": fields }),
        )
        .await
}

fn refusal(answer: &Value) -> &str {
    answer["errors"][0]["message"].as_str().unwrap_or("")
}

// r[verify akt.transition.requires-when]
// r[verify akt.transition.links]
#[tokio::test(flavor = "multi_thread")]
async fn a_problem_report_is_classified_then_fixed_by_its_tasks() {
    let world = world().await;
    let backlog = world
        .create(
            ADMIN,
            "backlog",
            &[],
            json!({ "title": "Shop" }),
            "Planning",
        )
        .await;
    let report = world
        .create(
            ADMIN,
            "problem_report",
            &[&backlog],
            json!({ "title": "Checkout: the total is wrong with two coupons" }),
            "Seen in staging",
        )
        .await;
    let classified = json!({ "screening": "uncertain", "severity": "major", "priority": "p1" });
    let refused = perform(&world, &report, "classify", classified.clone()).await;
    assert_eq!(refusal(&refused), "`classify` requires risk_record");
    let mut with_risk = classified;
    with_risk["risk_record"] = json!({ "url": "https://qms.shop.example/risks/RA-12" });
    let done = perform(&world, &report, "classify", with_risk).await;
    assert_eq!(
        done["data"]["performTransition"]["state"]["key"], "classified",
        "{done}"
    );

    let refused = perform(&world, &report, "fixed", json!({})).await;
    assert_eq!(refusal(&refused), "`fixed` waits for a `fixed_by` link");
    let task = world
        .create(
            ADMIN,
            "backlog_item",
            &[&backlog],
            json!({ "title": "Apply one coupon at a time", "kind": "fix" }),
            "The fix",
        )
        .await;
    world
        .link(ADMIN, &report, "fixed_by", &task, "Fixes it")
        .await;
    let refused = perform(&world, &report, "fixed", json!({})).await;
    assert_eq!(
        refusal(&refused),
        format!("`fixed` waits for {task}, which is not final yet")
    );
    world.transition(ADMIN, &task, "start", None, "Taken").await;
    world
        .transition(
            ADMIN,
            &task,
            "review",
            Some(json!({ "change": { "url": "https://git.shop.example/shop/pull/7" } })),
            "Ready",
        )
        .await;
    world
        .transition(ADMIN, &task, "finish", None, "Merged")
        .await;
    let fixed = perform(&world, &report, "fixed", json!({})).await;
    assert_eq!(
        fixed["data"]["performTransition"]["state"]["key"], "fixed",
        "{fixed}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_problem_report_is_closed_as_won_t_fix_only_with_its_reason() {
    let world = world().await;
    let report = world
        .create(
            ADMIN,
            "problem_report",
            &[],
            json!({ "title": "The logo is blurry on one old phone" }),
            "Seen",
        )
        .await;
    let refused = perform(&world, &report, "wont_fix", json!({ "rationale": "Old" })).await;
    assert_eq!(refusal(&refused), "`wont_fix` requires wont_fix_reason");
    let closed = perform(
        &world,
        &report,
        "wont_fix",
        json!({ "wont_fix_reason": "unsupported_version", "rationale": "Android 8 isn't supported." }),
    )
    .await;
    assert_eq!(
        closed["data"]["performTransition"]["state"]["key"], "wont_fix",
        "{closed}"
    );
}
