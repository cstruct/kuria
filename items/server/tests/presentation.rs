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

    /// The numbers of the akter `query` lists for the tenant, in order.
    async fn listed(&self, query: &str) -> Vec<String> {
        self.data("tenant", query, json!({})).await["akter"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The numbers of the akter a search by `token` finds, in order.
    async fn search(&self, token: &str, text: &str) -> Vec<String> {
        self.data(
            token,
            "query($s: String!) { akter(search: $s) { nodes { number } } }",
            json!({ "s": text }),
        )
        .await["akter"]["nodes"]
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
              "options": [{ "key": "s", "name": "Small" }, { "key": "l", "name": "Large" }] },
            { "key": "due", "name": "Due", "kind": "date" },
            { "key": "notes", "name": "Notes", "kind": "text", "multiline": true }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }],
        "presentation": {
            "title": "headline", "card": ["points"], "list": ["points", "summary"],
            "tones": { "done": "success" }, "icon": "book-open", "colour": "teal"
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
            "{ aktTypes { key presentation { title card list icon colour } states { key tone } } }",
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
        json!({ "title": "headline", "card": ["points"], "list": ["points", "summary"],
                "icon": "BOOK_OPEN", "colour": "TEAL" })
    );
    assert_eq!(
        of("story")["states"],
        json!([{ "key": "open", "tone": "NEUTRAL" }, { "key": "done", "tone": "SUCCESS" }])
    );
    assert_eq!(
        of("bug")["presentation"],
        json!({ "title": "title", "card": [], "list": [], "icon": null, "colour": "NEUTRAL" })
    );

    // An icon or colour not in its list is refused.
    for presentation in [json!({ "icon": "unicorn" }), json!({ "colour": "plaid" })] {
        let mut document = bug();
        document["presentation"] = presentation.clone();
        let refused = world
            .run(
                "tenant",
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": document }),
            )
            .await;
        assert!(refused.get("errors").is_some(), "{presentation}: {refused}");
    }

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

// r[verify authz.read]
#[tokio::test(flavor = "multi_thread")]
async fn akt_types_count_their_akter_for_those_who_read_them() {
    let world = World::new().await;
    world.create("story", json!({ "summary": "s" })).await;
    world.create("story", json!({ "summary": "t" })).await;
    world.create("bug", json!({ "title": "b" })).await;
    let counts = |data: Value| -> Vec<(String, Value)> {
        data["aktTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    t["key"].as_str().unwrap().to_owned(),
                    t["akterCount"].clone(),
                )
            })
            .collect()
    };
    let query = "{ aktTypes { key akterCount } viewer { user { id } } }";
    let all = world.data("tenant", query, json!({})).await;
    assert_eq!(
        counts(all.clone()),
        [("bug".to_owned(), json!(1)), ("story".to_owned(), json!(2))]
    );
    assert_eq!(all["viewer"]["user"]["id"], support::SUBJECT);
    // A reader of stories sees only stories, and counts them.
    let stories = world.data(STORY_READER, query, json!({})).await;
    assert_eq!(counts(stories), [("story".to_owned(), json!(2))]);
}

// r[verify akt.list]
#[tokio::test(flavor = "multi_thread")]
async fn akter_are_listed_most_recently_changed_first() {
    let world = World::new().await;
    let mut numbers = Vec::new();
    for summary in ["a", "b", "c"] {
        numbers.push(
            world.create("story", json!({ "summary": summary })).await["number"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let bug = world.create("bug", json!({ "title": "b" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    // Changing the first story makes it the most recent.
    world
        .data(
            "tenant",
            "mutation($n: String!) { performTransition(input: { number: $n, revision: 1, transition: \"finish\" }) { number } }",
            json!({ "n": numbers[0] }),
        )
        .await;
    assert_eq!(
        world.listed("{ akter { nodes { number } } }").await,
        [
            numbers[0].as_str(),
            bug.as_str(),
            numbers[2].as_str(),
            numbers[1].as_str()
        ]
    );
    assert_eq!(
        world
            .listed("{ akter(type: \"story\", first: 2) { nodes { number } } }")
            .await,
        [numbers[0].as_str(), numbers[2].as_str()]
    );
    assert_eq!(
        world
            .listed("{ akter(type: \"story\", state: \"open\") { nodes { number } } }")
            .await,
        [numbers[2].as_str(), numbers[1].as_str()]
    );
    // Only what the subject may read.
    let stories = world
        .data(STORY_READER, "{ akter { nodes { number } } }", json!({}))
        .await;
    assert_eq!(stories["akter"]["nodes"].as_array().unwrap().len(), 3);
}

impl World {
    /// A page of `query`'s akter: their numbers, the end cursor and whether
    /// more follow. `query` takes `$after`.
    async fn page(&self, query: &str, after: Option<&str>) -> (Vec<String>, Value, bool) {
        let akter = self.data("tenant", query, json!({ "after": after })).await["akter"].clone();
        let numbers = akter["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect();
        (
            numbers,
            akter["pageInfo"]["endCursor"].clone(),
            akter["pageInfo"]["hasNextPage"].as_bool().unwrap(),
        )
    }
}

// r[verify akt.page]
#[tokio::test(flavor = "multi_thread")]
async fn akter_come_a_page_at_a_time() {
    let world = World::new().await;
    let mut numbers = Vec::new();
    for summary in ["a", "b", "c", "d", "e"] {
        numbers.push(
            world
                .create(
                    "story",
                    json!({ "summary": summary, "headline": "Paged story" }),
                )
                .await["number"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    // Newest first: e d | c b | a.
    let query = "query($after: String) { akter(type: \"story\", first: 2, after: $after) { nodes { number } pageInfo { endCursor hasNextPage } } }";
    let (first, cursor, more) = world.page(query, None).await;
    assert_eq!(first, [numbers[4].as_str(), numbers[3].as_str()]);
    assert!(more);
    // `b` changes between pages, so it moves to the front: the next page
    // goes on after `d` and neither repeats nor skips what kept its place.
    world
        .data(
            "tenant",
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { headline: \"Moved\" } }) { number } }",
            json!({ "n": numbers[1] }),
        )
        .await;
    let (second, cursor, more) = world.page(query, cursor.as_str()).await;
    assert_eq!(second, [numbers[2].as_str(), numbers[0].as_str()]);
    assert!(!more);
    let (empty, end, more) = world.page(query, cursor.as_str()).await;
    assert!(empty.is_empty() && end.is_null() && !more);

    // Search pages too, newest first.
    let search = "query($after: String) { akter(search: \"paged\", first: 3, after: $after) { nodes { number } pageInfo { endCursor hasNextPage } } }";
    let (found, cursor, more) = world.page(search, None).await;
    assert_eq!(
        found,
        [
            numbers[4].as_str(),
            numbers[3].as_str(),
            numbers[2].as_str()
        ]
    );
    assert!(more);
    let (rest, _, more) = world.page(search, cursor.as_str()).await;
    assert_eq!(rest, [numbers[0].as_str()]);
    assert!(!more);

    // A cursor belongs to the list that gave it.
    let (_, list_cursor, _) = world.page(query, None).await;
    for (other, cursor) in [
        (search, list_cursor.clone()),
        (
            "query($after: String) { akter(type: \"bug\", after: $after) { nodes { number } } }",
            list_cursor.clone(),
        ),
        (query, json!("not-a-cursor")),
    ] {
        let refused = world.run("tenant", other, json!({ "after": cursor })).await;
        assert_eq!(
            refused["errors"][0]["extensions"]["code"], "BAD_REQUEST",
            "{other}: {refused}"
        );
    }
}

// r[verify akt.filter]
#[tokio::test(flavor = "multi_thread")]
async fn akter_are_filtered_by_field_values() {
    let world = World::new().await;
    let number = |v: Value| v["number"].as_str().unwrap().to_owned();
    let small = number(
        world
            .create(
                "story",
                json!({ "summary": "Alpha", "points": "s", "due": "2026-12-01" }),
            )
            .await,
    );
    let large = number(
        world
            .create(
                "story",
                json!({ "summary": "Beta", "points": "l", "due": "2026-12-01" }),
            )
            .await,
    );
    let undated = number(
        world
            .create("story", json!({ "summary": "Alpha", "points": "s" }))
            .await,
    );
    let listed = |conditions: Value| {
        let world = &world;
        async move {
            world
                .run(
                    "tenant",
                    "query($w: [FieldCondition!]) { akter(type: \"story\", where: $w) { nodes { number } } }",
                    json!({ "w": conditions }),
                )
                .await
        }
    };
    let numbers = |answer: Value| -> Vec<String> {
        answer["data"]["akter"]["nodes"]
            .as_array()
            .unwrap_or_else(|| panic!("{answer}"))
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        numbers(listed(json!([{ "field": "points", "equals": "s" }])).await),
        [undated.as_str(), small.as_str()]
    );
    assert_eq!(
        numbers(listed(json!([{ "field": "due", "equals": "2026-12-01" }])).await),
        [large.as_str(), small.as_str()]
    );
    assert_eq!(
        numbers(listed(json!([{ "field": "summary", "equals": "Alpha" }])).await),
        [undated.as_str(), small.as_str()]
    );
    // Every condition applies.
    assert_eq!(
        numbers(
            listed(json!([
                { "field": "summary", "equals": "Alpha" },
                { "field": "due", "equals": "2026-12-01" }
            ]))
            .await
        ),
        [small.as_str()]
    );
    for (conditions, why) in [
        (
            json!([{ "field": "notes", "equals": "x" }]),
            "a multiline field",
        ),
        (
            json!([{ "field": "gone", "equals": "x" }]),
            "a field the type lacks",
        ),
        (
            json!([{ "field": "points", "equals": "huge" }]),
            "an option the field lacks",
        ),
        (json!([{ "field": "due", "equals": "soon" }]), "not a date"),
    ] {
        let refused = listed(conditions).await;
        assert_eq!(
            refused["errors"][0]["extensions"]["code"], "BAD_REQUEST",
            "{why}: {refused}"
        );
    }
    // Without one akt type, there is nothing to filter by.
    let refused = world
        .run(
            "tenant",
            "{ akter(where: [{ field: \"points\", equals: \"s\" }]) { nodes { number } } }",
            json!({}),
        )
        .await;
    assert_eq!(refused["errors"][0]["extensions"]["code"], "BAD_REQUEST");
}
