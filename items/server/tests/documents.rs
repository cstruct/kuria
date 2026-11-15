//! `markdown` fields through GraphQL against a fresh database: heading ids,
//! references and the links they keep (`akt.field.markdown`,
//! `akt.field.markdown.references`, `link.referenced`).

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
}

fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown", "references": "refers_to" }
        ],
        "states": [
            { "key": "draft", "name": "Draft", "initial": true },
            { "key": "retired", "name": "Retired", "final": true }
        ],
        "transitions": [{ "key": "retire", "name": "Retire", "from": ["draft"], "to": "retired" }],
        "links": [
            { "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document"] },
            { "key": "replaces", "name": "Replaces", "inverse": "Replaced by", "to": ["document"] }
        ]
    })
}

const AKT: &str = "query($n: String!) { akt(number: $n) {
    revision
    links { kind name outgoing akt { number } }
    fields { ... on MarkdownValue { field markdown headings { id level text }
      references { destination number version heading broken akt { number } } } } } }";

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        world
            .ok(
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": document() }),
            )
            .await;
        world
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with("tenant", query, &variables))
            .await
            .json()
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, title: &str, body: &str) -> String {
        self.ok(
            "mutation($f: JSON) { createAkt(input: { type: \"document\", in: [], fields: $f }) { number } }",
            json!({ "f": { "title": title, "body": body } }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn akt(&self, number: &str) -> Value {
        self.ok(AKT, json!({ "n": number })).await["akt"].clone()
    }

    async fn edit(&self, number: &str, body: &str) -> Value {
        let revision = self.akt(number).await["revision"].clone();
        self.run(
            "mutation($n: String!, $v: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }",
            json!({ "n": number, "v": revision, "f": { "body": body } }),
        )
        .await
    }
}

fn body(akt: &Value) -> &Value {
    akt["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == "body")
        .unwrap()
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

// r[verify akt.field.markdown]
#[tokio::test(flavor = "multi_thread")]
async fn headings_get_ids_that_stay() {
    let world = World::new().await;
    let guide = world
        .create("Guide", "# Installing\n\nText.\n\n## Installing\n")
        .await;
    let read = world.akt(&guide).await;
    assert_eq!(
        body(&read)["markdown"],
        "# Installing {#installing}\n\nText.\n\n## Installing {#installing-2}\n"
    );
    assert_eq!(
        body(&read)["headings"],
        json!([
            { "id": "installing", "level": 1, "text": "Installing" },
            { "id": "installing-2", "level": 2, "text": "Installing" }
        ])
    );
    // A renamed heading keeps its id; one id given twice is refused.
    let renamed = world
        .edit(
            &guide,
            "# Setting up {#installing}\n\nText.\n\n## Installing {#installing-2}\n",
        )
        .await;
    assert!(renamed.get("errors").is_none(), "{renamed}");
    let twice = world.edit(&guide, "# A {#same}\n\n# B {#same}\n").await;
    assert_eq!(code(&twice), "REFUSED", "{twice}");
}

// r[verify akt.field.markdown.references]
// r[verify link.referenced]
#[tokio::test(flavor = "multi_thread")]
async fn references_keep_links_and_say_when_broken() {
    let world = World::new().await;
    let setup = world.create("Setup", "# Install {#install}\n").await;
    let target_revision = world.akt(&setup).await["revision"].clone();
    let guide = world
        .create(
            "Guide",
            &format!(
                "See [install]({0}#install), [gone]({0}#gone), [v1]({0}@1) and [nothing](kuria:D-99).\n",
                format!("kuria:{setup}")
            ),
        )
        .await;
    let read = world.akt(&guide).await;
    let broken: Vec<&Value> = body(&read)["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["broken"])
        .collect();
    assert_eq!(
        broken,
        [
            &Value::Null,
            &json!(format!("`{setup}` has no heading `gone`")),
            &json!(format!("`{setup}` has no version 1")),
            &json!("there is no akt `D-99`")
        ]
    );
    assert_eq!(
        body(&read)["references"][0]["akt"]["number"],
        setup.as_str()
    );
    // One link to the akt referenced, read from both sides; the target
    // gets no revision.
    assert_eq!(
        read["links"],
        json!([{ "kind": "refers_to", "name": "Refers to", "outgoing": true, "akt": { "number": setup } }])
    );
    let target = world.akt(&setup).await;
    assert_eq!(target["links"][0]["name"], "Referred to by");
    assert_eq!(target["revision"], target_revision);

    // The text owns those links: by hand they are refused.
    for mutation in ["addLink", "removeLink"] {
        let answer = world
            .run(
                &format!("mutation($s: String!, $t: String!) {{ {mutation}(input: {{ source: $s, kind: \"refers_to\", target: $t }}) {{ number }} }}"),
                json!({ "s": guide, "t": setup }),
            )
            .await;
        assert_eq!(code(&answer), "INVALID_LINK", "{mutation}: {answer}");
    }
    // A link of another kind is still made by hand.
    world
        .ok(
            "mutation($s: String!, $t: String!) { addLink(input: { source: $s, kind: \"replaces\", target: $t }) { number } }",
            json!({ "s": guide, "t": setup }),
        )
        .await;

    // Taking the reference out of the text takes the link away, even to a
    // final akt.
    let revision = world.akt(&setup).await["revision"].clone();
    world
        .ok(
            "mutation($n: String!, $v: Int!) { performTransition(input: { number: $n, revision: $v, transition: \"retire\" }) { revision } }",
            json!({ "n": setup, "v": revision }),
        )
        .await;
    let edited = world.edit(&guide, "No references now.\n").await;
    assert!(edited.get("errors").is_none(), "{edited}");
    assert_eq!(
        world.akt(&guide).await["links"],
        json!([{ "kind": "replaces", "name": "Replaces", "outgoing": true, "akt": { "number": setup } }])
    );
}
