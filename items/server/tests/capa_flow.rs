//! A CAPA from the problem report it originates from to its closure,
//! reopened and closed again, with the example release-management
//! configuration in `docs/examples/release-management`, a developer and
//! QA/RA, through GraphQL against a fresh database.

mod support;

use kuria_server::authorize::{CREATE, EDIT, READ, TRANSITION};
use serde_json::{Value, json};

use support::SUBJECT;
use support::decider::FakeDecider;
use support::world::World;

const ADMIN: &str = "tenant";
const DEV: &str = "tenant-dev";
const QA: &str = "tenant-qa";

const DOCUMENTS: [&str; 13] = [
    include_str!("../../../docs/examples/release-management/artifact.json"),
    include_str!("../../../docs/examples/release-management/backlog_item.json"),
    include_str!("../../../docs/examples/release-management/epic.json"),
    include_str!("../../../docs/examples/release-management/backlog.json"),
    include_str!("../../../docs/examples/release-management/release.json"),
    include_str!("../../../docs/examples/release-management/release_candidate.json"),
    include_str!("../../../docs/examples/release-management/hotfix.json"),
    include_str!("../../../docs/examples/release-management/release_notes.json"),
    include_str!("../../../docs/examples/release-management/test_case.json"),
    include_str!("../../../docs/examples/release-management/test_protocol.json"),
    include_str!("../../../docs/examples/release-management/problem_report.json"),
    include_str!("../../../docs/examples/release-management/capa.json"),
    include_str!("../../../docs/examples/release-management/capa_action.json"),
];

/// The README's developer, who may work CAPAs but not confirm or close
/// them, and QA/RA, who may.
async fn world() -> World {
    let decider = FakeDecider::allowing_tenant();
    decider.grant(DEV, &[READ]);
    decider.grant(QA, &[READ]);
    for key in [
        "backlog",
        "backlog_item",
        "problem_report",
        "capa",
        "capa_action",
    ] {
        decider.grant_at(DEV, key, &[CREATE, EDIT, TRANSITION]);
    }
    for key in ["capa", "capa_action"] {
        decider.grant_at(
            QA,
            key,
            &[
                CREATE,
                EDIT,
                TRANSITION,
                "capa:confirm",
                "capa:approve_closure",
            ],
        );
    }
    let world = World::new(decider).await;
    for document in DOCUMENTS {
        let document: Value = serde_json::from_str(document).unwrap();
        world.define(ADMIN, &document).await;
    }
    world
}

async fn perform(
    world: &World,
    token: &str,
    number: &str,
    transition: &str,
    fields: Value,
) -> Value {
    let revision = world.revision(token, number).await;
    world
        .run(
            token,
            "mutation($n: String!, $r: Int!, $t: String!, $f: JSON, $why: String) { performTransition(input: { number: $n, revision: $r, transition: $t, fields: $f, reason: $why }) { state { key } } }",
            json!({ "n": number, "r": revision, "t": transition, "f": fields, "why": transition }),
        )
        .await
}

fn state(answer: &Value) -> &str {
    answer["data"]["performTransition"]["state"]["key"]
        .as_str()
        .unwrap_or_else(|| panic!("{answer}"))
}

fn refused(answer: &Value) -> (&str, &str) {
    (
        answer["errors"][0]["extensions"]["code"]
            .as_str()
            .unwrap_or(""),
        answer["errors"][0]["message"].as_str().unwrap_or(""),
    )
}

fn link(url: &str) -> Value {
    json!({ "url": url })
}

/// A confirmed, planned and implementing corrective CAPA, from a problem
/// report, with one action and the task implementing it; answers the
/// CAPA, the action and the task.
async fn implementing(world: &World) -> (String, String, String) {
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
            DEV,
            "problem_report",
            &[&backlog],
            json!({ "title": "Released images can't be traced to their SBOM" }),
            "Audit finding",
        )
        .await;
    let created = world
        .ok(
            DEV,
            "mutation($f: JSON, $l: [NewLinkInput!]) { createAkt(input: { type: \"capa\", fields: $f, links: $l, reason: \"Escalated\" }) { number } }",
            json!({
                "f": { "title": "Untraceable images", "problem": "Two released images lack SBOM evidence." },
                "l": [{ "kind": "originated_from", "to": report }]
            }),
        )
        .await;
    let capa = created["createAkt"]["number"].as_str().unwrap().to_owned();
    assert!(
        capa.starts_with("CAPA-") && capa.ends_with("-0001"),
        "{capa}"
    );

    // Only QA/RA confirms it.
    let confirmation = json!({
        "severity": "major",
        "impact": "Releases can't be shown to match their dependencies.",
        "risk_record": link("https://qms.shop.example/risks/RA-31"),
    });
    let forbidden = perform(world, DEV, &capa, "confirm", confirmation.clone()).await;
    assert_eq!(refused(&forbidden).0, "FORBIDDEN", "{forbidden}");
    assert_eq!(
        state(&perform(world, QA, &capa, "confirm", confirmation).await),
        "investigating"
    );

    // A corrective CAPA needs its root cause.
    let plan = json!({
        "kind": ["correction", "corrective"],
        "affected": "Releases 2026.1 and 2026.2.",
        "vigilance": "not_required",
        "communication": "not_required",
    });
    let without = perform(world, DEV, &capa, "plan", plan.clone()).await;
    assert_eq!(refused(&without), ("REFUSED", "`plan` requires root_cause"));
    let mut planned = plan;
    planned["root_cause"] = json!("No one was assigned to post deployment records.");
    assert_eq!(
        state(&perform(world, DEV, &capa, "plan", planned).await),
        "planning"
    );
    let started = perform(
        world,
        DEV,
        &capa,
        "implement",
        json!({
            "effectiveness_method": "Every release of the next two has its records.",
            "effectiveness_criteria": "No release without image digests and SBOMs.",
        }),
    )
    .await;
    assert_eq!(state(&started), "implementing");

    let action = world
        .create(
            DEV,
            "capa_action",
            &[&capa],
            json!({ "title": "Name who posts deployment records", "kind": "corrective", "due": "2020-01-01", "owner": SUBJECT }),
            "Planned",
        )
        .await;
    assert_eq!(action, format!("{capa}-A01"));
    let task = world
        .create(
            DEV,
            "backlog_item",
            &[&backlog],
            json!({ "title": "Post deployment records from the pipeline", "kind": "chore" }),
            "The change",
        )
        .await;
    world
        .link(DEV, &action, "implemented_by", &task, "Implements it")
        .await;
    (capa, action, task)
}

// r[verify akt.transition.members]
// r[verify akt.transition.links]
// r[verify akt.transition.requires-when]
// r[verify akt.field.locked]
// r[verify akt.field.due]
// r[verify link.create]
#[tokio::test(flavor = "multi_thread")]
#[expect(clippy::too_many_lines, reason = "one CAPA told from start to end")]
async fn a_capa_waits_for_its_actions_and_closes_with_qa_s_approval() {
    let world = world().await;
    let (capa, action, task) = implementing(&world).await;

    assert_eq!(
        state(&perform(&world, DEV, &action, "start", json!({})).await),
        "in_progress"
    );
    let overdue = world
        .ok(
            DEV,
            "query($n: String!) { akt(number: $n) { overdue } }",
            json!({ "n": action }),
        )
        .await;
    assert_eq!(overdue["akt"]["overdue"], json!(["due"]));
    // Its due date moves only with a reason now.
    let revision = world.revision(DEV, &action).await;
    let moved = world
        .run(
            DEV,
            "mutation($n: String!, $r: Int!) { editAkt(input: { number: $n, revision: $r, fields: { due: \"2020-02-01\" } }) { number } }",
            json!({ "n": action, "r": revision }),
        )
        .await;
    assert_eq!(refused(&moved).0, "LOCKED", "{moved}");
    world
        .edit(
            DEV,
            &action,
            json!({ "due": "2999-02-01" }),
            "Waiting on the pipeline team",
        )
        .await;

    let early = perform(&world, DEV, &capa, "verify", json!({})).await;
    assert_eq!(
        refused(&early).1,
        format!("`verify` waits for {action}, which is not final yet")
    );
    let evidence = json!({ "evidence": link("https://git.shop.example/shop/pull/42") });
    let unfinished = perform(&world, DEV, &action, "finish", evidence.clone()).await;
    assert_eq!(
        refused(&unfinished).1,
        format!("`finish` waits for {task}, which is not final yet")
    );
    world.transition(DEV, &task, "start", None, "Taken").await;
    world
        .transition(
            DEV,
            &task,
            "review",
            Some(json!({ "change": link("https://git.shop.example/shop/pull/42") })),
            "Ready",
        )
        .await;
    world.transition(DEV, &task, "finish", None, "Merged").await;
    assert_eq!(
        state(&perform(&world, DEV, &action, "finish", evidence).await),
        "done"
    );
    assert_eq!(
        state(&perform(&world, DEV, &capa, "verify", json!({})).await),
        "verifying"
    );

    let closing = json!({
        "effectiveness_results": "2026.3 and 2026.4 have every record.",
        "adverse_impact": "None: the pipeline only adds records.",
        "closure_outcome": "completed",
    });
    let forbidden = perform(&world, DEV, &capa, "close", closing.clone()).await;
    assert_eq!(refused(&forbidden).0, "FORBIDDEN", "{forbidden}");
    assert_eq!(
        state(&perform(&world, QA, &capa, "close", closing).await),
        "closed"
    );

    // It recurred: reopened with what the procedure asks, and closed again.
    let bare = perform(
        &world,
        QA,
        &capa,
        "reopen",
        json!({ "reopen_reason": "It recurred" }),
    )
    .await;
    assert_eq!(refused(&bare).0, "REFUSED", "{bare}");
    let reopened = perform(
        &world,
        QA,
        &capa,
        "reopen",
        json!({
            "reopen_reason": "Release 2026.5 shipped without its SBOM.",
            "remaining_actions": "Make the pipeline fail without one.",
            "revised_target": "2999-03-01",
            "owner_confirmed": true,
            "delay_impact": "One more release to check; no escalation needed.",
        }),
    )
    .await;
    assert_eq!(state(&reopened), "implementing");
    assert_eq!(
        state(&perform(&world, DEV, &capa, "verify", json!({})).await),
        "verifying"
    );
    let again = json!({
        "effectiveness_results": "2026.6 and 2026.7 have every record.",
        "adverse_impact": "None.",
        "closure_outcome": "completed",
    });
    assert_eq!(
        state(&perform(&world, QA, &capa, "close", again).await),
        "closed"
    );
    let history = world
        .ok(
            QA,
            "query($n: String!) { akt(number: $n) { revisions { transition } } }",
            json!({ "n": capa }),
        )
        .await;
    let closed = history["akt"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["transition"] == "close")
        .count();
    assert_eq!(closed, 2, "{history}");
}

// r[verify akt.transition.links]
#[tokio::test(flavor = "multi_thread")]
async fn a_capa_is_superseded_only_by_a_linked_one_and_declined_with_a_reason() {
    let world = world().await;
    let (capa, _, _) = implementing(&world).await;
    let unlinked = perform(&world, QA, &capa, "supersede", json!({})).await;
    assert_eq!(
        refused(&unlinked).1,
        "`supersede` waits for a `superseded_by` link"
    );
    let other = world
        .create(
            QA,
            "capa",
            &[],
            json!({ "title": "Traceability, broader", "problem": "Every artifact, not only images." }),
            "Wider",
        )
        .await;
    world
        .link(QA, &capa, "superseded_by", &other, "Wider")
        .await;
    assert_eq!(
        state(&perform(&world, QA, &capa, "supersede", json!({})).await),
        "superseded"
    );
    let declined = perform(
        &world,
        QA,
        &other,
        "decline",
        json!({ "not_required_reason": "Covered by the superseded CAPA's actions." }),
    )
    .await;
    assert_eq!(state(&declined), "not_required");
}
