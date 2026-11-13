//! Links between akter, through GraphQL against a fresh database.

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_server::authorize::{EDIT, READ};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

/// Reads tasks and notes, and edits notes only.
const NOTE_EDITOR: &str = "tenant-note-editor";
/// Reads tasks only.
const TASK_READER: &str = "tenant-task-reader";

struct World {
    _db: TestDatabase,
    router: Router,
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.grant_at(NOTE_EDITOR, "task", &[READ]);
        decider.grant_at(NOTE_EDITOR, "note", &[READ, EDIT]);
        decider.grant_at(TASK_READER, "task", &[READ]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for document in [task(), note()] {
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

    async fn create(&self, key: &str, title: &str) -> String {
        self.ok(
            "tenant",
            "mutation($t: String!, $f: JSON) { createAkt(input: { type: $t, fields: $f }) { number } }",
            json!({ "t": key, "f": { "title": title } }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn link(
        &self,
        token: &str,
        source: &str,
        kind: &str,
        target: &str,
        pin: Option<i32>,
    ) -> Value {
        self.run(
            token,
            "mutation($s: String!, $k: String!, $t: String!, $p: Int) { addLink(input: { source: $s, kind: $k, target: $t, pinRevision: $p, reason: \"Related\" }) { number } }",
            json!({ "s": source, "k": kind, "t": target, "p": pin }),
        )
        .await
    }

    async fn unlink(&self, token: &str, source: &str, kind: &str, target: &str) -> Value {
        self.run(
            token,
            "mutation($s: String!, $k: String!, $t: String!) { removeLink(input: { source: $s, kind: $k, target: $t }) { number } }",
            json!({ "s": source, "k": kind, "t": target }),
        )
        .await
    }

    /// An akt's links as `token` reads them: name, other number, outgoing.
    async fn links(&self, token: &str, number: &str) -> Vec<(String, String, bool)> {
        self.ok(
            token,
            "query($n: String!) { akt(number: $n) { links { name outgoing akt { number } } } }",
            json!({ "n": number }),
        )
        .await["akt"]["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["name"].as_str().unwrap().to_owned(),
                    l["akt"]["number"].as_str().unwrap().to_owned(),
                    l["outgoing"].as_bool().unwrap(),
                )
            })
            .collect()
    }

    /// The membership part of an akt's latest revision.
    async fn latest_change(&self, number: &str) -> Value {
        let data = self
            .ok(
                "tenant",
                "query($n: String!) { akt(number: $n) { revisions { reason membership } } }",
                json!({ "n": number }),
            )
            .await;
        let revisions = data["akt"]["revisions"].as_array().unwrap();
        revisions.last().unwrap().clone()
    }
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

fn task() -> Value {
    json!({
        "key": "task", "name": "Task", "number": "TASK-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "todo", "name": "To do", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["todo"], "to": "done" }],
        "links": [
            { "key": "blocks", "name": "Blocks", "inverse": "Blocked by", "to": ["task"] },
            { "key": "cites", "name": "Cites", "inverse": "Cited by", "to": ["note"] }
        ]
    })
}

fn note() -> Value {
    json!({
        "key": "note", "name": "Note", "number": "NOTE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "links": [{ "key": "about", "name": "About", "inverse": "Noted in", "to": ["task"] }]
    })
}

// r[verify link.add]
// r[verify link.read]
// r[verify link.revisions]
// r[verify link.remove]
#[tokio::test(flavor = "multi_thread")]
async fn akter_are_linked_both_ways_with_revisions_on_both() {
    let world = World::new().await;
    let a = world.create("task", "A").await;
    let b = world.create("task", "B").await;
    let note = world.create("note", "Why").await;

    assert!(
        world
            .link("tenant", &a, "blocks", &b, None)
            .await
            .get("errors")
            .is_none()
    );
    assert!(
        world
            .link("tenant", &a, "cites", &note, None)
            .await
            .get("errors")
            .is_none()
    );
    assert_eq!(
        world.links("tenant", &a).await,
        [
            ("Blocks".to_owned(), b.clone(), true),
            ("Cites".to_owned(), note.clone(), true)
        ]
    );
    assert_eq!(
        world.links("tenant", &b).await,
        [("Blocked by".to_owned(), a.clone(), false)]
    );
    assert_eq!(
        world.links("tenant", &note).await,
        [("Cited by".to_owned(), a.clone(), false)]
    );

    // Each side records the link, by number and as it reads it.
    let on_b = world.latest_change(&b).await;
    assert_eq!(on_b["reason"], "Related");
    assert_eq!(on_b["membership"]["linked_from"][0]["number"], a.as_str());
    assert_eq!(on_b["membership"]["linked_from"][0]["name"], "Blocked by");
    assert_eq!(
        world.latest_change(&a).await["membership"]["linked"][0]["kind"],
        "cites"
    );

    assert!(
        world
            .unlink("tenant", &a, "blocks", &b)
            .await
            .get("errors")
            .is_none()
    );
    assert_eq!(world.links("tenant", &b).await, Vec::new());
    assert_eq!(
        world.latest_change(&b).await["membership"]["unlinked_from"][0]["number"],
        a.as_str()
    );
    assert_eq!(
        code(&world.unlink("tenant", &a, "blocks", &b).await),
        "NOT_FOUND"
    );
}

// r[verify link.add]
// r[verify link.kinds]
#[tokio::test(flavor = "multi_thread")]
async fn links_go_only_where_their_kind_allows() {
    let world = World::new().await;
    let a = world.create("task", "A").await;
    let b = world.create("task", "B").await;
    let note = world.create("note", "N").await;
    for (source, kind, target, pin, why) in [
        (&a, "relates", &b, None, "a kind the type lacks"),
        (
            &a,
            "blocks",
            &note,
            None,
            "a target type the kind doesn't name",
        ),
        (&a, "blocks", &a, None, "a link to itself"),
        (
            &a,
            "blocks",
            &b,
            Some(7),
            "a revision the target hasn't reached",
        ),
    ] {
        let refused = world.link("tenant", source, kind, target, pin).await;
        assert_eq!(code(&refused), "INVALID_LINK", "{why}: {refused}");
    }
    assert!(
        world
            .link("tenant", &a, "blocks", &b, None)
            .await
            .get("errors")
            .is_none()
    );
    assert_eq!(
        code(&world.link("tenant", &a, "blocks", &b, None).await),
        "INVALID_LINK"
    );
    assert_eq!(
        code(&world.link("tenant", &a, "blocks", "TASK-99", None).await),
        "NOT_FOUND"
    );
}

// r[verify link.pin]
#[tokio::test(flavor = "multi_thread")]
async fn a_pinned_link_shows_its_target_as_it_was() {
    let world = World::new().await;
    let a = world.create("task", "A").await;
    let b = world.create("task", "Draft").await;
    assert!(
        world
            .link("tenant", &a, "blocks", &b, Some(1))
            .await
            .get("errors")
            .is_none()
    );
    // B changes after the pin: revision 2 is the link, 3 the edit, 4 done.
    world
        .ok(
            "tenant",
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 2, fields: { title: \"Final\" } }) { number } }",
            json!({ "n": b }),
        )
        .await;
    world
        .ok(
            "tenant",
            "mutation($n: String!) { performTransition(input: { number: $n, revision: 3, transition: \"finish\" }) { number } }",
            json!({ "n": b }),
        )
        .await;
    let link = world
        .ok(
            "tenant",
            "query($n: String!) { akt(number: $n) { links { pinnedRevision pinned { revision title state { key } } akt { title state { key } } } } }",
            json!({ "n": a }),
        )
        .await["akt"]["links"][0]
        .clone();
    assert_eq!(
        link,
        json!({
            "pinnedRevision": 1,
            "pinned": { "revision": 1, "title": "Draft", "state": { "key": "todo" } },
            "akt": { "title": "Final", "state": { "key": "done" } }
        })
    );
}

// r[verify authz.link]
// r[verify link.read]
#[tokio::test(flavor = "multi_thread")]
async fn linking_needs_edit_at_the_source_and_read_at_the_target() {
    let world = World::new().await;
    let a = world.create("task", "A").await;
    let b = world.create("task", "B").await;
    let note = world.create("note", "N").await;
    // Reading tasks isn't editing them.
    assert_eq!(
        code(&world.link(NOTE_EDITOR, &a, "blocks", &b, None).await),
        "FORBIDDEN"
    );
    // Editing notes, and reading tasks, is enough to link a note to a task.
    assert!(
        world
            .link(NOTE_EDITOR, &note, "about", &a, None)
            .await
            .get("errors")
            .is_none()
    );
    // A target, or source, that can't be read isn't found.
    assert_eq!(
        code(&world.link(TASK_READER, &a, "cites", &note, None).await),
        "NOT_FOUND"
    );
    // Links to what can't be read are left out.
    assert_eq!(
        world.links("tenant", &a).await,
        [("Noted in".to_owned(), note.clone(), false)]
    );
    assert_eq!(world.links(TASK_READER, &a).await, Vec::new());
}
