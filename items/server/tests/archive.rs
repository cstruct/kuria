//! Archiving akter through GraphQL against a fresh database (`archive.*`):
//! who may, what it records, what it refuses, and what reads answer.

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

struct World {
    _db: TestDatabase,
    router: Router,
    decider: Arc<FakeDecider>,
}

fn folder() -> Value {
    json!({
        "key": "folder", "name": "Folder", "number": "F-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "item" }],
        "board": { "columns": [{ "key": "all", "name": "All", "states": { "item": ["open", "done"] } }] }
    })
}

fn item() -> Value {
    json!({
        "key": "item", "name": "Item", "number": "I-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }],
        "links": [{ "key": "relates", "name": "Relates to", "inverse": "Related to", "to": ["item"] }],
        "versions": {}
    })
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            _db: db,
            router,
            decider,
        };
        for document in [item(), folder()] {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn run_as(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        self.run_as("tenant", query, variables).await
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, title: &str, inside: &[&str]) -> String {
        self.ok(
            "mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }",
            json!({ "t": key, "f": { "title": title }, "in": inside }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn archive_as(&self, token: &str, number: &str, archive: bool) -> Value {
        let mutation = if archive {
            "mutation($n: String!) { archiveAkt(input: { number: $n, reason: \"Done with\" }) { number archived { by { id } rule reason } } }"
        } else {
            "mutation($n: String!) { unarchiveAkt(input: { number: $n }) { number archived { reason } } }"
        };
        self.run_as(token, mutation, json!({ "n": number })).await
    }

    async fn revision(&self, number: &str) -> i64 {
        self.ok(
            "query($n: String!) { akt(number: $n) { revision } }",
            json!({ "n": number }),
        )
        .await["akt"]["revision"]
            .as_i64()
            .unwrap()
    }

    async fn numbers(&self, query: &str, variables: Value, path: &[&str]) -> Vec<String> {
        let mut at = self.ok(query, variables).await;
        for key in path {
            at = at[key].clone();
        }
        at.as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

// r[verify archive.act]
// r[verify archive.authz]
// r[verify archive.flag]
// r[verify archive.history]
#[tokio::test(flavor = "multi_thread")]
async fn archiving_needs_its_action_and_is_kept_in_history() {
    let world = World::new().await;
    let item = world.create("item", "Old", &[]).await;
    world
        .decider
        .grant("tenant-editor", &["kuria:akter:read", "kuria:akter:edit"]);
    let forbidden = world.archive_as("tenant-editor", &item, true).await;
    assert_eq!(code(&forbidden), "FORBIDDEN", "{forbidden}");

    let archived = world.archive_as("tenant", &item, true).await;
    let akt = &archived["data"]["archiveAkt"];
    assert_eq!(akt["archived"]["reason"], "Done with", "{archived}");
    assert!(akt["archived"]["by"]["id"].is_string(), "{archived}");
    assert_eq!(akt["archived"]["rule"], Value::Null);
    let again = world.archive_as("tenant", &item, true).await;
    assert_eq!(code(&again), "BAD_REQUEST", "{again}");

    let unarchived = world.archive_as("tenant", &item, false).await;
    assert_eq!(
        unarchived["data"]["unarchiveAkt"]["archived"],
        Value::Null,
        "{unarchived}"
    );
    let history = world
        .ok(
            "query($n: String!) { akt(number: $n) { state { key } revisions { revision transition reason changes } } }",
            json!({ "n": item }),
        )
        .await;
    let revisions = history["akt"]["revisions"].as_array().unwrap();
    let transitions: Vec<&Value> = revisions.iter().map(|r| &r["transition"]).collect();
    assert_eq!(
        transitions,
        [&Value::Null, &json!("archive"), &json!("unarchive")]
    );
    assert_eq!(revisions[1]["reason"], "Done with");
    assert_eq!(revisions[1]["changes"], json!({}));
    // Its state is as it was.
    assert_eq!(history["akt"]["state"]["key"], "open");
}

// r[verify archive.readonly]
#[tokio::test(flavor = "multi_thread")]
async fn an_archived_akt_refuses_every_write() {
    let world = World::new().await;
    let folder = world.create("folder", "Box", &[]).await;
    let other = world.create("folder", "Shelf", &[]).await;
    let item = world.create("item", "Old", &[&folder]).await;
    let live = world.create("item", "New", &[]).await;
    world.archive_as("tenant", &item, true).await;
    let revision = world.revision(&item).await;
    let refusals = [
        (
            "mutation($n: String!, $v: Int!) { editAkt(input: { number: $n, revision: $v, fields: { title: \"Changed\" } }) { revision } }",
            json!({ "n": item, "v": revision }),
        ),
        (
            "mutation($n: String!, $v: Int!) { performTransition(input: { number: $n, revision: $v, transition: \"finish\" }) { revision } }",
            json!({ "n": item, "v": revision }),
        ),
        (
            "mutation($n: String!, $o: String!) { addLink(input: { source: $o, kind: \"relates\", target: $n }) { number } }",
            json!({ "n": item, "o": live }),
        ),
        (
            "mutation($n: String!, $c: String!) { removeFromContainer(input: { container: $c, member: $n }) { number } }",
            json!({ "n": item, "c": folder }),
        ),
        (
            "mutation($n: String!, $c: String!) { addToContainer(input: { container: $c, member: $n }) { number } }",
            json!({ "n": item, "c": other }),
        ),
        (
            "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { number } }",
            json!({ "n": item, "o": live }),
        ),
        (
            "mutation($n: String!, $v: Int!) { releaseVersion(input: { number: $n, revision: $v }) { number } }",
            json!({ "n": item, "v": revision }),
        ),
    ];
    for (mutation, variables) in refusals {
        let answer = world.run(mutation, variables).await;
        assert_eq!(code(&answer), "ARCHIVED", "{mutation}: {answer}");
    }
    // An archived container takes nothing new.
    world.archive_as("tenant", &other, true).await;
    let created = world
        .run(
            "mutation($c: String!) { createAkt(input: { type: \"item\", fields: { title: \"In\" }, in: [$c] }) { number } }",
            json!({ "c": other }),
        )
        .await;
    assert_eq!(code(&created), "ARCHIVED", "{created}");
    // Read as before, and editable once unarchived.
    let read = world
        .ok(
            "query($n: String!) { akt(number: $n) { title archived { at } } }",
            json!({ "n": item }),
        )
        .await;
    assert_eq!(read["akt"]["title"], "Old");
    world.archive_as("tenant", &item, false).await;
    world
        .ok(
            "mutation($n: String!, $v: Int!) { editAkt(input: { number: $n, revision: $v, fields: { title: \"Changed\" } }) { revision } }",
            json!({ "n": item, "v": world.revision(&item).await }),
        )
        .await;
}

// r[verify archive.read]
// r[verify archive.board]
#[tokio::test(flavor = "multi_thread")]
async fn reads_answer_live_akter_unless_asked() {
    let world = World::new().await;
    let folder = world.create("folder", "Box", &[]).await;
    let old = world.create("item", "Old thing", &[&folder]).await;
    let new = world.create("item", "New thing", &[&folder]).await;
    world.archive_as("tenant", &old, true).await;

    let listed =
        |archived: &str| format!("{{ akter(type: \"item\"{archived}) {{ nodes {{ number }} }} }}");
    assert_eq!(
        world
            .numbers(&listed(""), json!({}), &["akter", "nodes"])
            .await,
        std::slice::from_ref(&new)
    );
    assert_eq!(
        world
            .numbers(
                &listed(", archived: ARCHIVED"),
                json!({}),
                &["akter", "nodes"]
            )
            .await,
        std::slice::from_ref(&old)
    );
    assert_eq!(
        world
            .numbers(&listed(", archived: ALL"), json!({}), &["akter", "nodes"])
            .await
            .len(),
        2
    );
    let search = "{ akter(search: \"thing\") { nodes { number } } }";
    assert_eq!(
        world.numbers(search, json!({}), &["akter", "nodes"]).await,
        std::slice::from_ref(&new)
    );
    let counts = world
        .ok(
            "{ akterTypeCount: aktType(key: \"item\") { live: akterCount all: akterCount(archived: ALL) } }",
            json!({}),
        )
        .await;
    assert_eq!(counts["akterTypeCount"], json!({ "live": 1, "all": 2 }));

    let members = "query($n: String!) { akt(number: $n) { archivedMemberCount members { nodes { number } } } }";
    assert_eq!(
        world
            .numbers(
                members,
                json!({ "n": folder }),
                &["akt", "members", "nodes"]
            )
            .await,
        std::slice::from_ref(&new)
    );
    let count = world.ok(members, json!({ "n": folder })).await;
    assert_eq!(count["akt"]["archivedMemberCount"], 1);
    let board =
        "query($n: String!) { akt(number: $n) { board { columns { key akter { number } } } } }";
    let board_of = async |world: &World| {
        world.ok(board, json!({ "n": folder })).await["akt"]["board"]["columns"][0]["akter"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!(board_of(&world).await, 1);
    // An archived container's board shows all its members.
    world.archive_as("tenant", &folder, true).await;
    assert_eq!(board_of(&world).await, 2);
    // An archived akt still opens by its number.
    let opened = world
        .ok(
            "query($n: String!) { akt(number: $n) { number archived { reason } } }",
            json!({ "n": old }),
        )
        .await;
    assert_eq!(opened["akt"]["archived"]["reason"], "Done with");
}

impl World {
    /// Defines `document`'s type again, with `rules`, as its next version.
    async fn rules(&self, mut document: Value, rules: Value) {
        document["archive"] = json!({ "rules": rules });
        self.ok(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": document }),
        )
        .await;
    }

    async fn finish(&self, number: &str) {
        self.ok(
            "mutation($n: String!, $v: Int!) { performTransition(input: { number: $n, revision: $v, transition: \"finish\" }) { revision } }",
            json!({ "n": number, "v": self.revision(number).await }),
        )
        .await;
    }

    /// The rule that archived `number`, `"-"` if a person did, and `None`
    /// while it is live.
    async fn archived_by(&self, number: &str) -> Option<String> {
        let akt = self
            .ok(
                "query($n: String!) { akt(number: $n) { archived { rule by { id } } } }",
                json!({ "n": number }),
            )
            .await;
        let archived = &akt["akt"]["archived"];
        if archived.is_null() {
            return None;
        }
        Some(archived["rule"].as_str().unwrap_or("-").to_owned())
    }
}

// r[verify archive.rules.on-change]
// r[verify archive.rules.state-since]
// r[verify archive.history]
#[tokio::test(flavor = "multi_thread")]
async fn an_akt_two_newer_ones_overtook_is_archived() {
    let world = World::new().await;
    world
        .rules(item(), json!([{ "newer": 2, "in": ["done"] }]))
        .await;
    let mut items = Vec::new();
    for title in ["One", "Two", "Three", "Four"] {
        items.push(world.create("item", title, &[]).await);
    }
    world.finish(&items[0]).await;
    world.finish(&items[1]).await;
    assert_eq!(world.archived_by(&items[0]).await, None);
    world.finish(&items[2]).await;
    assert_eq!(world.archived_by(&items[0]).await.as_deref(), Some("newer"));
    assert_eq!(world.archived_by(&items[1]).await, None);
    world.finish(&items[3]).await;
    assert_eq!(world.archived_by(&items[1]).await.as_deref(), Some("newer"));
    assert_eq!(world.archived_by(&items[2]).await, None);
    // By Kuria, naming the rule.
    let history = world
        .ok(
            "query($n: String!) { akt(number: $n) { revisions { transition reason by { id } } } }",
            json!({ "n": items[0] }),
        )
        .await;
    let last = history["akt"]["revisions"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last["transition"], "archive");
    assert_eq!(last["reason"], "by the rule `newer`");
    assert_eq!(last["by"]["id"], "00000000-0000-0000-0000-000000000000");
}

// r[verify archive.rules]
#[tokio::test(flavor = "multi_thread")]
async fn newer_within_counts_only_akter_sharing_a_container() {
    let world = World::new().await;
    world
        .rules(
            item(),
            json!([{ "newer": 1, "in": ["done"], "within": "folder" }]),
        )
        .await;
    let (a, b) = (
        world.create("folder", "A", &[]).await,
        world.create("folder", "B", &[]).await,
    );
    let a1 = world.create("item", "A1", &[&a]).await;
    let b1 = world.create("item", "B1", &[&b]).await;
    let a2 = world.create("item", "A2", &[&a]).await;
    world.finish(&a1).await;
    world.finish(&b1).await;
    assert_eq!(world.archived_by(&a1).await, None, "B1 isn't in A");
    world.finish(&a2).await;
    assert_eq!(world.archived_by(&a1).await.as_deref(), Some("newer"));
    assert_eq!(world.archived_by(&b1).await, None);
}

// r[verify archive.rules.container]
// r[verify archive.rules.on-change]
#[tokio::test(flavor = "multi_thread")]
async fn members_go_with_their_container_and_come_back_with_it() {
    let world = World::new().await;
    let shelf = json!({
        "key": "shelf", "name": "Shelf", "number": "S-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "folder" }]
    });
    world
        .ok(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": shelf }),
        )
        .await;
    world
        .rules(folder(), json!([{ "with_container": "shelf" }]))
        .await;
    world
        .rules(item(), json!([{ "with_container": "folder" }]))
        .await;
    let shelf = world.create("shelf", "Top", &[]).await;
    let folder = world.create("folder", "Box", &[&shelf]).await;
    let kept = world.create("item", "Kept", &[&folder]).await;
    let by_hand = world.create("item", "By hand", &[&folder]).await;
    world.archive_as("tenant", &by_hand, true).await;

    world.archive_as("tenant", &shelf, true).await;
    assert_eq!(
        world.archived_by(&folder).await.as_deref(),
        Some("with_container")
    );
    assert_eq!(
        world.archived_by(&kept).await.as_deref(),
        Some("with_container")
    );
    assert_eq!(world.archived_by(&by_hand).await.as_deref(), Some("-"));

    // Back with it: what it took, not what a person archived.
    world.archive_as("tenant", &shelf, false).await;
    assert_eq!(world.archived_by(&folder).await, None);
    assert_eq!(world.archived_by(&kept).await, None);
    assert_eq!(world.archived_by(&by_hand).await.as_deref(), Some("-"));
}

// r[verify archive.rules.kept]
#[tokio::test(flavor = "multi_thread")]
async fn an_akt_unarchived_by_hand_is_kept_from_rules() {
    let world = World::new().await;
    world
        .rules(item(), json!([{ "with_container": "folder" }]))
        .await;
    let folder = world.create("folder", "Box", &[]).await;
    let item = world.create("item", "Wanted", &[&folder]).await;
    world.archive_as("tenant", &folder, true).await;
    assert_eq!(
        world.archived_by(&item).await.as_deref(),
        Some("with_container")
    );
    // A person wants it back: it stays, though its container is archived.
    world.archive_as("tenant", &item, false).await;
    world.archive_as("tenant", &folder, false).await;
    world.archive_as("tenant", &folder, true).await;
    assert_eq!(world.archived_by(&item).await, None, "kept");
    // Archived by hand, the mark goes; unarchived again, it isn't set
    // again, as no rule archived it: the rule applies once more.
    world.archive_as("tenant", &item, true).await;
    world.archive_as("tenant", &item, false).await;
    world.archive_as("tenant", &folder, false).await;
    world.archive_as("tenant", &folder, true).await;
    assert_eq!(
        world.archived_by(&item).await.as_deref(),
        Some("with_container")
    );
}
