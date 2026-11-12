//! Drafting release notes from a release's candidates and hotfixes, through
//! GraphQL against a fresh database.

mod support;

use std::sync::Arc;

use axum::Router;
use chrono::{Datelike, Utc};
use kuria_server::authorize::{EDIT, READ};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

const ALICE: &str = "tenant-alice";

struct World {
    _db: TestDatabase,
    router: Router,
    decider: Arc<FakeDecider>,
}

fn open_only() -> Value {
    json!([
        { "key": "open", "name": "Open", "initial": true },
        { "key": "done", "name": "Done", "final": true }
    ])
}

fn types() -> Vec<Value> {
    let title = json!({ "key": "title", "name": "Title", "kind": "text" });
    vec![
        json!({ "key": "item", "name": "Backlog items", "number": "ITEM-{seq}",
                "fields": [title, { "key": "kind", "name": "Kind", "kind": "choice",
                                    "options": [{ "key": "fix", "name": "Fix" },
                                                { "key": "feature", "name": "Feature" }] }],
                "states": open_only() }),
        json!({ "key": "epic", "name": "Epics", "number": "EPIC-{seq}", "fields": [title],
                "states": open_only(), "contains": [{ "type": "item" }] }),
        json!({ "key": "backlog", "name": "Backlog", "number": "BACKLOG-{seq}",
                "states": open_only(), "contains": [{ "type": "item" }, { "type": "epic" }] }),
        json!({ "key": "release_candidate", "name": "Release candidate",
                "number": "{parent}-RC-{seq:4}", "created_in": ["release"],
                "states": open_only(), "contains": [{ "type": "epic" }, { "type": "item" }] }),
        json!({ "key": "hotfix", "name": "Hotfix", "number": "{parent}-HOTFIX-{seq:4}",
                "created_in": ["release"], "states": open_only(),
                "contains": [{ "type": "item" }] }),
        json!({ "key": "release_notes", "name": "Release notes",
                "number": "{parent}-NOTES-{seq}", "created_in": ["release"],
                "fields": [{ "key": "text", "name": "Text", "kind": "text", "multiline": true,
                             "draft": { "through": ["release_candidate", "hotfix", "epic"],
                                        "depth": 3, "types": ["epic", "item"],
                                        "line": "{number} {field:title} ({field:kind})",
                                        "group_by": "type" } },
                           { "key": "summary", "name": "Summary", "kind": "text" }],
                "states": open_only(),
                "transitions": [{ "key": "publish", "name": "Publish", "from": ["open"], "to": "done" }] }),
        json!({ "key": "release", "name": "Release", "number": "RELEASE-{year}-{seq:4}",
                "states": open_only(),
                "contains": [{ "type": "backlog" }, { "type": "release_candidate" },
                             { "type": "hotfix" }, { "type": "release_notes" }] }),
    ]
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            _db: db,
            router,
            decider,
        };
        for document in types() {
            world
                .ok(
                    "tenant",
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, inside: &[&str], fields: Value) -> String {
        self.ok(
            "tenant",
            "mutation($t: String!, $in: [String!]!, $f: JSON) { createAkt(input: { type: $t, in: $in, fields: $f }) { number } }",
            json!({ "t": key, "in": inside, "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn item(&self, inside: &[&str], title: &str, kind: &str) -> String {
        self.create("item", inside, json!({ "title": title, "kind": kind }))
            .await
    }

    async fn draft(&self, token: &str, notes: &str, revision: i32, field: &str) -> Value {
        self.run(
            token,
            "mutation($n: String!, $r: Int!, $f: String!) { draftField(input: { number: $n, revision: $r, field: $f }) { revision fields { ... on TextValue { field text } } revisions { reason } } }",
            json!({ "n": notes, "r": revision, "f": field }),
        )
        .await
    }
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

/// A release with two candidates, a hotfix and notes, and the items in them.
async fn release(world: &World) -> (String, [String; 6]) {
    let release = world.create("release", &[], json!({})).await;
    let backlog = world.create("backlog", &[&release], json!({})).await;
    let rc1 = world
        .create("release_candidate", &[&release], json!({}))
        .await;
    let rc2 = world
        .create("release_candidate", &[&release], json!({}))
        .await;
    let hotfix = world.create("hotfix", &[&release], json!({})).await;
    let notes = world.create("release_notes", &[&release], json!({})).await;

    let epic = world
        .create("epic", &[&rc1, &backlog], json!({ "title": "Sign-in" }))
        .await;
    let login = world.item(&[&epic, &backlog], "Passkeys", "feature").await;
    let crash = world.item(&[&rc1, &rc2], "Crash on save", "fix").await;
    let typo = world.item(&[&rc2], "Typo in the menu", "fix").await;
    let urgent = world.item(&[&hotfix], "Lost sessions", "fix").await;
    // Planned, but never in a candidate or hotfix: not in the notes.
    world.item(&[&backlog], "Dark mode", "feature").await;
    (notes, [epic, login, crash, typo, urgent, release])
}

// r[verify draft.sources]
// r[verify draft.render]
// r[verify draft.edit]
#[tokio::test(flavor = "multi_thread")]
async fn drafts_the_notes_from_candidates_and_hotfixes() {
    let world = World::new().await;
    let (notes, [epic, login, crash, typo, urgent, release]) = release(&world).await;
    assert_eq!(notes, format!("RELEASE-{}-0001-NOTES-1", Utc::now().year()));
    let _ = release;

    let answer = world.draft("tenant", &notes, 1, "text").await;
    assert!(answer.get("errors").is_none(), "{answer}");
    let drafted = &answer["data"]["draftField"];
    assert_eq!(drafted["revision"], 2);
    assert_eq!(drafted["revisions"][1]["reason"], "Drafted text");
    let text = drafted["fields"][0]["text"].as_str().unwrap();
    assert_eq!(
        text,
        [
            "## Epics".to_owned(),
            format!("{epic} Sign-in ()"),
            String::new(),
            "## Backlog items".to_owned(),
            format!("{login} Passkeys (Feature)"),
            format!("{crash} Crash on save (Fix)"),
            format!("{typo} Typo in the menu (Fix)"),
            format!("{urgent} Lost sessions (Fix)"),
        ]
        .join("\n")
    );

    // A draft is a starting point: it can be edited by hand afterwards.
    let edited = world
        .run(
            "tenant",
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 2, fields: { text: \"Hand-written\" } }) { revision } }",
            json!({ "n": notes }),
        )
        .await;
    assert_eq!(edited["data"]["editAkt"]["revision"], 3);
}

// r[verify draft.sources]
#[tokio::test(flavor = "multi_thread")]
async fn leaves_out_what_the_drafter_cant_read() {
    let world = World::new().await;
    let (notes, [epic, _, crash, typo, urgent, _]) = release(&world).await;
    for key in ["release", "release_candidate", "hotfix", "item"] {
        world.decider.grant_at(ALICE, key, &[READ]);
    }
    world
        .decider
        .grant_at(ALICE, "release_notes", &[READ, EDIT]);
    let answer = world.draft(ALICE, &notes, 1, "text").await;
    let text = answer["data"]["draftField"]["fields"][0]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    // Alice can't read epics: neither the epic nor what is only inside it.
    assert!(!text.contains(&epic), "{text}");
    assert!(!text.contains("Passkeys"), "{text}");
    for number in [&crash, &typo, &urgent] {
        assert!(text.contains(number.as_str()), "{text}");
    }
}

// r[verify draft.edit]
#[tokio::test(flavor = "multi_thread")]
async fn drafting_is_refused_as_an_edit_would_be() {
    let world = World::new().await;
    let (notes, _) = release(&world).await;
    assert_eq!(
        code(&world.draft("tenant", &notes, 1, "summary").await),
        "REFUSED"
    );
    assert_eq!(
        code(&world.draft("tenant", &notes, 7, "text").await),
        "STALE"
    );
    world.decider.grant_at(ALICE, "release_notes", &[READ]);
    assert_eq!(
        code(&world.draft(ALICE, &notes, 1, "text").await),
        "FORBIDDEN"
    );
    world
        .ok(
            "tenant",
            "mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: \"publish\" }) { revision } }",
            json!({ "n": notes }),
        )
        .await;
    assert_eq!(
        code(&world.draft("tenant", &notes, 2, "text").await),
        "REFUSED"
    );
}
