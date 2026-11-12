//! How akt types are shown, akter's titles, and searching akter, through
//! GraphQL against a fresh database.

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_server::authorize::READ;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

/// Reads only `story` akter.
const STORY_READER: &str = "tenant-story-reader";

struct World {
    _db: TestDatabase,
    router: Router,
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.grant_at(STORY_READER, "story", &[READ]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for document in [story(), bug()] {
            world
                .data(
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

    async fn data(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, key: &str, fields: Value) -> Value {
        self.data(
            "tenant",
            "mutation($t: String!, $f: JSON) { createAkt(input: { type: $t, fields: $f }) { number title revision } }",
            json!({ "t": key, "f": fields }),
        )
        .await["createAkt"]
            .clone()
    }

    /// The numbers of the akter a search by `token` finds, in order.
    async fn search(&self, token: &str, text: &str) -> Vec<String> {
        self.data(
            token,
            "query($s: String!) { akter(search: $s) { number } }",
            json!({ "s": text }),
        )
        .await["akter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect()
    }
}

/// Titled by its presentation's `headline`, though `summary` comes first.
fn story() -> Value {
    json!({
        "key": "story", "name": "Story", "number": "STORY-{seq}",
        "fields": [
            { "key": "summary", "name": "Summary", "kind": "text", "required": true },
            { "key": "headline", "name": "Headline", "kind": "text" },
            { "key": "points", "name": "Points", "kind": "choice",
              "options": [{ "key": "s", "name": "Small" }, { "key": "l", "name": "Large" }] }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }],
        "presentation": {
            "title": "headline", "card": ["points"], "list": ["points", "summary"],
            "tones": { "done": "success" }
        }
    })
}

/// No presentation: titled by its first required single-line text field.
fn bug() -> Value {
    json!({
        "key": "bug", "name": "Bug", "number": "BUG-{seq}",
        "fields": [
            { "key": "steps", "name": "Steps", "kind": "text", "multiline": true },
            { "key": "title", "name": "Title", "kind": "text", "required": true }
        ],
        "states": [{ "key": "new", "name": "New", "initial": true }]
    })
}

// r[verify akt.type.presentation]
// r[verify akt.title]
#[tokio::test(flavor = "multi_thread")]
async fn akt_types_say_how_their_akter_are_shown() {
    let world = World::new().await;
    let types = world
        .data(
            "tenant",
            "{ aktTypes { key presentation { title card list } states { key tone } } }",
            json!({}),
        )
        .await;
    let of = |key: &str| {
        types["aktTypes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == key)
            .unwrap()
            .clone()
    };
    assert_eq!(
        of("story")["presentation"],
        json!({ "title": "headline", "card": ["points"], "list": ["points", "summary"] })
    );
    assert_eq!(
        of("story")["states"],
        json!([{ "key": "open", "tone": "NEUTRAL" }, { "key": "done", "tone": "SUCCESS" }])
    );
    assert_eq!(
        of("bug")["presentation"],
        json!({ "title": "title", "card": [], "list": [] })
    );

    let story = world
        .create(
            "story",
            json!({ "summary": "s", "headline": "  Faster sign-in " }),
        )
        .await;
    assert_eq!(story["title"], "Faster sign-in");
    let bug = world
        .create("bug", json!({ "title": "Crash on save" }))
        .await;
    assert_eq!(bug["title"], "Crash on save");
    let untitled = world.create("story", json!({ "summary": "s" })).await;
    assert_eq!(untitled["title"], Value::Null);

    // Editing the title field changes the title; other edits keep it.
    let edited = world
        .data(
            "tenant",
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { headline: \"Passkeys\" } }) { title } }",
            json!({ "n": story["number"] }),
        )
        .await;
    assert_eq!(edited["editAkt"]["title"], "Passkeys");
    let moved = world
        .data(
            "tenant",
            "mutation($n: String!) { performTransition(input: { number: $n, revision: 2, transition: \"finish\" }) { title state { tone } } }",
            json!({ "n": story["number"] }),
        )
        .await;
    assert_eq!(
        moved["performTransition"],
        json!({ "title": "Passkeys", "state": { "tone": "SUCCESS" } })
    );
}

// r[verify akt.search]
#[tokio::test(flavor = "multi_thread")]
async fn akter_are_found_by_number_or_title_words() {
    let world = World::new().await;
    let sign_in = world
        .create(
            "story",
            json!({ "summary": "s", "headline": "Faster sign-in with passkeys" }),
        )
        .await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let reset = world
        .create(
            "story",
            json!({ "summary": "s", "headline": "Password reset" }),
        )
        .await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let crash = world
        .create(
            "bug",
            json!({ "title": "Crash on sign-in, 100% of the time" }),
        )
        .await["number"]
        .as_str()
        .unwrap()
        .to_owned();

    // Words in any order and case, newest first, across akt types.
    assert_eq!(
        world.search("tenant", "SIGN-IN").await,
        [crash.as_str(), sign_in.as_str()]
    );
    assert_eq!(
        world.search("tenant", "passkeys faster").await,
        [sign_in.as_str()]
    );
    // A number's beginning, whatever its case.
    assert_eq!(
        world.search("tenant", "story-").await,
        [reset.as_str(), sign_in.as_str()]
    );
    assert_eq!(world.search("tenant", "bug-1").await, [crash.as_str()]);
    // Like's wildcards are literal.
    assert_eq!(world.search("tenant", "100%").await, [crash.as_str()]);
    assert_eq!(world.search("tenant", "_").await, Vec::<String>::new());
    assert_eq!(world.search("tenant", "   ").await, Vec::<String>::new());
    // Only what the subject may read.
    assert_eq!(
        world.search(STORY_READER, "sign-in").await,
        [sign_in.as_str()]
    );
}
