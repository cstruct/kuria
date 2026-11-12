//! Who may do what with akt types and akter, against a fake decider that
//! grants Kuria's actions per token, and a fresh database for each test.

mod support;

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use kuria_server::authorize::{CREATE, DEFINE_TYPES, EDIT, READ, TRANSITION};
use kuria_store::schema::akt_revision;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

/// The tokens: `tenant` may do everything and sets the scene; the others
/// are granted what each test gives them.
const ADMIN: &str = "tenant";
const ALICE: &str = "tenant-alice";

struct World {
    db: TestDatabase,
    decider: Arc<FakeDecider>,
    router: Router,
}

impl World {
    /// A tenant with the akt types `capa` and `bug`, one akt of each, made by
    /// the admin.
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            db,
            decider,
            router,
        };
        for (key, prefix) in [("capa", "CAPA"), ("bug", "BUG")] {
            world
                .ok(
                    ADMIN,
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": akt_type(key, prefix) }),
                )
                .await;
            world
                .ok(
                    ADMIN,
                    &format!("mutation {{ createAkt(input: {{ type: \"{key}\", fields: {{ title: \"T\" }} }}) {{ number }} }}"),
                    json!({}),
                )
                .await;
        }
        world
    }

    async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with(token, query, &variables)).await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        answer.json()
    }

    async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn revisions(&self) -> i64 {
        let mut conn = self.db.pool().get().await.unwrap();
        akt_revision::table
            .count()
            .get_result(&mut conn)
            .await
            .unwrap()
    }
}

fn akt_type(key: &str, prefix: &str) -> Value {
    json!({
        "key": key,
        "name": prefix,
        "number": format!("{prefix}-{{seq}}"),
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "closed", "name": "Closed", "final": true }
        ],
        "transitions": [
            { "key": "close", "name": "Close", "from": ["open"], "to": "closed",
              "action": "quality:approve_closure" }
        ]
    })
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

const EDIT_CAPA: &str = "mutation { editAkt(input: { number: \"CAPA-1\", revision: 1, fields: { title: \"U\" } }) { revision } }";
const CLOSE_CAPA: &str = "mutation { performTransition(input: { number: \"CAPA-1\", revision: 1, transition: \"close\" }) { state { key } } }";

// r[verify authz.define]
#[tokio::test(flavor = "multi_thread")]
async fn defining_akt_types_needs_the_define_action() {
    let world = World::new().await;
    let define = "mutation($d: JSON!) { defineAktType(document: $d) { version } }";
    let document = json!({ "d": akt_type("capa", "CAPA") });
    assert_eq!(
        code(&world.run(ALICE, define, document.clone()).await),
        "FORBIDDEN"
    );
    world.decider.grant(ALICE, &[DEFINE_TYPES]);
    assert_eq!(
        world.ok(ALICE, define, document).await["defineAktType"]["version"],
        2
    );
}

// r[verify authz.read-types]
#[tokio::test(flavor = "multi_thread")]
async fn akt_types_are_visible_to_definers_and_readers_only() {
    let world = World::new().await;
    let types = "{ aktTypes { key } bug: aktType(key: \"bug\") { key } }";
    assert_eq!(
        world.ok(ALICE, types, json!({})).await,
        json!({ "aktTypes": [], "bug": null })
    );
    world.decider.grant_at(ALICE, "capa", &[READ]);
    assert_eq!(
        world.ok(ALICE, types, json!({})).await,
        json!({ "aktTypes": [{ "key": "capa" }], "bug": null })
    );
    world.decider.grant(ALICE, &[DEFINE_TYPES]);
    assert_eq!(
        world.ok(ALICE, types, json!({})).await,
        json!({ "aktTypes": [{ "key": "bug" }, { "key": "capa" }], "bug": { "key": "bug" } })
    );
}

// r[verify authz.read]
// r[verify authz.catalog]
#[tokio::test(flavor = "multi_thread")]
async fn akter_are_visible_only_where_their_type_may_be_read() {
    let world = World::new().await;
    world.decider.grant_at(ALICE, "capa", &[READ]);
    let data = world
        .ok(
            ALICE,
            "{ capa: akt(number: \"CAPA-1\") { number revisions { revision } } bug: akt(number: \"BUG-1\") { number } akter { number } }",
            json!({}),
        )
        .await;
    assert_eq!(
        data,
        json!({
            "capa": { "number": "CAPA-1", "revisions": [{ "revision": 1 }] },
            "bug": null,
            "akter": [{ "number": "CAPA-1" }],
        })
    );
}

// r[verify authz.read]
#[tokio::test(flavor = "multi_thread")]
async fn an_unreadable_akt_is_not_found_even_to_a_stale_change() {
    let world = World::new().await;
    world.decider.grant_at(ALICE, "capa", &[EDIT]);
    // Revision 7 is stale, but a subject who can't read the akt mustn't
    // learn that it exists.
    let answer = world
        .run(
            ALICE,
            "mutation { editAkt(input: { number: \"CAPA-1\", revision: 7, fields: { title: \"U\" } }) { revision } }",
            json!({}),
        )
        .await;
    assert_eq!(code(&answer), "NOT_FOUND");
}

// r[verify authz.create]
#[tokio::test(flavor = "multi_thread")]
async fn creating_needs_the_create_action_at_the_type() {
    let world = World::new().await;
    let create =
        "mutation { createAkt(input: { type: \"capa\", fields: { title: \"T\" } }) { number } }";
    assert_eq!(
        code(&world.run(ALICE, create, json!({})).await),
        "UNKNOWN_AKT_TYPE"
    );
    world.decider.grant_at(ALICE, "capa", &[READ]);
    assert_eq!(
        code(&world.run(ALICE, create, json!({})).await),
        "FORBIDDEN"
    );
    world.decider.grant_at(ALICE, "capa", &[CREATE]);
    assert_eq!(
        world.ok(ALICE, create, json!({})).await["createAkt"]["number"],
        "CAPA-2"
    );
    // A grant at one type doesn't reach another.
    let bug =
        "mutation { createAkt(input: { type: \"bug\", fields: { title: \"T\" } }) { number } }";
    assert_eq!(
        code(&world.run(ALICE, bug, json!({})).await),
        "UNKNOWN_AKT_TYPE"
    );
}

// r[verify authz.edit]
#[tokio::test(flavor = "multi_thread")]
async fn editing_needs_the_edit_action_at_the_type() {
    let world = World::new().await;
    world.decider.grant_at(ALICE, "capa", &[READ]);
    assert_eq!(
        code(&world.run(ALICE, EDIT_CAPA, json!({})).await),
        "FORBIDDEN"
    );
    world.decider.grant_at(ALICE, "capa", &[EDIT]);
    assert_eq!(
        world.ok(ALICE, EDIT_CAPA, json!({})).await["editAkt"]["revision"],
        2
    );
}

// r[verify authz.transition]
// r[verify authz.transition-action]
#[tokio::test(flavor = "multi_thread")]
async fn a_transition_needs_its_own_action_too() {
    let world = World::new().await;
    world.decider.grant_at(ALICE, "capa", &[READ, TRANSITION]);
    assert_eq!(
        code(&world.run(ALICE, CLOSE_CAPA, json!({})).await),
        "FORBIDDEN"
    );
    world
        .decider
        .grant_at(ALICE, "capa", &["quality:approve_closure"]);
    assert_eq!(
        world.ok(ALICE, CLOSE_CAPA, json!({})).await["performTransition"]["state"]["key"],
        "closed"
    );
}

// r[verify authz.catalog]
#[tokio::test(flavor = "multi_thread")]
async fn a_tenant_wide_grant_reaches_every_type() {
    let world = World::new().await;
    world.decider.grant(ALICE, &[READ, EDIT]);
    world.ok(ALICE, EDIT_CAPA, json!({})).await;
    world
        .ok(
            ALICE,
            "mutation { editAkt(input: { number: \"BUG-1\", revision: 1, fields: { title: \"U\" } }) { revision } }",
            json!({}),
        )
        .await;
}

// r[verify authz.decide]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_when_konklav_cannot_decide() {
    let world = World::new().await;
    let before = world.revisions().await;
    world.decider.set_failing(true);
    let answer = world.run(ADMIN, EDIT_CAPA, json!({})).await;
    assert_eq!(code(&answer), "UNAVAILABLE");
    let answer = world.run(ADMIN, "{ akter { number } }", json!({})).await;
    assert_eq!(code(&answer), "UNAVAILABLE");
    assert_eq!(world.revisions().await, before);
}

// r[verify authz.decide]
#[tokio::test(flavor = "multi_thread")]
async fn decisions_last_for_one_request_only() {
    let world = World::new().await;
    world.decider.grant_at(ALICE, "capa", &[READ]);
    let query = "{ a: akt(number: \"CAPA-1\") { number } b: akt(number: \"CAPA-1\") { number } c: akt(number: \"CAPA-1\") { number } }";
    let before = world.decider.decisions();
    world.ok(ALICE, query, json!({})).await;
    assert_eq!(
        world.decider.decisions() - before,
        1,
        "one decision for the request"
    );
    world.ok(ALICE, query, json!({})).await;
    assert_eq!(
        world.decider.decisions() - before,
        2,
        "and another for the next"
    );
}

// r[verify authz.sender-constrained]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_sender_constrained_tokens() {
    let world = World::new().await;
    let answer = send(
        &world.router,
        graphql_with("tenant-dpop", "{ akter { number } }", &json!({})),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.json()["error"]["code"],
        "sender_constrained_unsupported"
    );
}
