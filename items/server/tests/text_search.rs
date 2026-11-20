//! Text search through GraphQL against a fresh database: akter found by
//! the words their text fields hold, a live draft as it reads, ranked, with
//! excerpts, only those the subject may read (`akt.search.text`).

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_server::authorize::READ;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

struct World {
    _db: TestDatabase,
    router: Router,
}

const SEARCH: &str = "query($t: String!, $type: String, $archived: ArchivedFilter) {
    textSearch(text: $t, type: $type, archived: $archived) {
      akt { number } field relevance excerpt } }";

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.grant_at("colleague", "guide", &[READ]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for (key, live) in [("guide", true), ("note", false)] {
            let mut document = json!({
                "key": key, "name": key, "number": format!("{}-{{seq}}", key.to_uppercase()),
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "body", "name": "Body", "kind": "markdown" },
                    { "key": "due", "name": "Due", "kind": "date" }
                ],
                "states": [{ "key": "open", "name": "Open", "initial": true }]
            });
            if live {
                document["versions"] = json!({});
                document["live_drafts"] = json!(true);
            }
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

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run_as("tenant", query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, fields: Value) -> String {
        self.ok(
            "mutation($k: String!, $f: JSON) { createAkt(input: { type: $k, fields: $f }) { number } }",
            json!({ "k": key, "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn search_as(&self, token: &str, text: &str, variables: Value) -> Vec<Value> {
        let mut variables = variables;
        variables["t"] = json!(text);
        let answer = self.run_as(token, SEARCH, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"]["textSearch"].as_array().unwrap().clone()
    }

    async fn search(&self, text: &str) -> Vec<Value> {
        self.search_as("tenant", text, json!({})).await
    }
}

fn found(matches: &[Value]) -> Vec<(String, String)> {
    matches
        .iter()
        .map(|m| {
            (
                m["akt"]["number"].as_str().unwrap().to_owned(),
                m["field"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

// r[verify akt.search.text]
// r[verify akt.search.text.match]
#[tokio::test(flavor = "multi_thread")]
async fn finds_akter_by_the_words_of_their_text_ranked_with_excerpts() {
    let world = World::new().await;
    let install = world
        .create(
            "guide",
            json!({ "title": "Setup", "body": "# Installing\n\nRun the installer, then restart the server.\n" }),
        )
        .await;
    let policy = world
        .create(
            "note",
            json!({ "title": "Restart policy", "body": "Servers restart at night. Install updates first.\n" }),
        )
        .await;
    world
        .create(
            "note",
            json!({ "title": "Unrelated", "body": "Nothing here.\n" }),
        )
        .await;

    // Every word, stemmed: "install" finds "Installing" and "installer".
    let matches = world.search("install restart").await;
    let numbers = found(&matches);
    assert_eq!(numbers.len(), 2, "{matches:?}");
    assert!(numbers.contains(&(install.clone(), "body".to_owned())));
    assert!(numbers.contains(&(policy.clone(), "body".to_owned())));
    assert!(matches[0]["relevance"].as_f64() >= matches[1]["relevance"].as_f64());
    let excerpt = matches
        .iter()
        .find(|m| m["akt"]["number"] == json!(install))
        .unwrap()["excerpt"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(excerpt.contains("«Installing»"), "{excerpt}");
    assert!(excerpt.contains("«restart»"), "{excerpt}");
    // Titles are text fields too.
    assert_eq!(
        found(&world.search("policy").await),
        vec![(policy.clone(), "title".to_owned())]
    );
    // Of one akt type.
    assert_eq!(
        found(
            &world
                .search_as("tenant", "restart", json!({ "type": "guide" }))
                .await
        ),
        vec![(install.clone(), "body".to_owned())]
    );

    // A live draft is searched as it reads, once the edit is committed.
    world
        .ok(
            "mutation($n: String!) { replaceInAkt(input: { number: $n, replacements: [
                { field: \"body\", span: \"restart the server\", text: \"reboot the machine\" } ] }) { number } }",
            json!({ "n": install }),
        )
        .await;
    assert_eq!(
        found(&world.search("reboot").await),
        vec![(install.clone(), "body".to_owned())]
    );
    assert_eq!(
        found(&world.search("install restart").await),
        vec![(policy.clone(), "body".to_owned())]
    );

    // Only akter the subject may read.
    assert_eq!(
        found(&world.search_as("colleague", "install", json!({})).await),
        vec![(install.clone(), "body".to_owned())]
    );

    // Archived akter only as asked.
    world
        .ok(
            "mutation($n: String!) { archiveAkt(input: { number: $n }) { number } }",
            json!({ "n": policy }),
        )
        .await;
    assert_eq!(found(&world.search("policy").await), Vec::new());
    assert_eq!(
        found(
            &world
                .search_as("tenant", "policy", json!({ "archived": "ARCHIVED" }))
                .await
        ),
        vec![(policy.clone(), "title".to_owned())]
    );
    assert_eq!(world.search("   ").await, Vec::<Value>::new());
}
