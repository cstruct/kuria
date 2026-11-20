//! Live drafts through GraphQL against a fresh database: edits that change
//! the live draft only, releases that record it, transitions that carry
//! the last release, and the draft's history (`live.*`).

mod support;

use std::sync::Arc;

use axum::Router;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{COLLEAGUE, FakeHost, SUBJECT, graphql_with, router_with, send};

struct World {
    _db: TestDatabase,
    router: Router,
}

fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "summary", "name": "Summary", "kind": "text", "multiline": true },
            { "key": "body", "name": "Body", "kind": "markdown", "references": "refers_to" },
            { "key": "tags", "name": "Tags", "kind": "choices",
              "options": [{ "key": "a", "name": "A" }, { "key": "b", "name": "B" }] }
        ],
        "states": [
            { "key": "draft", "name": "Draft", "initial": true },
            { "key": "retired", "name": "Retired", "final": true }
        ],
        "transitions": [{ "key": "retire", "name": "Retire", "from": ["draft"], "to": "retired" }],
        "links": [
            { "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document"] }
        ],
        "versions": {},
        "live_drafts": true
    })
}

const AKT: &str = "query($n: String!) { akt(number: $n) {
    title revision state { key }
    fields { ... on TextValue { field text } ... on MarkdownValue { field markdown }
      ... on ChoicesValue { field options { key } } }
    revisions { revision transition changes by { id } }
    links { kind akt { number } }
    versions { number revision } } }";

const AT_VERSION: &str = "query($n: String!, $v: Int!) { akt(number: $n, version: $v) {
    revision fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } } } }";

const AT_REVISION: &str = "query($n: String!, $r: Int!) { akt(number: $n, revision: $r) {
    revision state { key } fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } } } }";

const EDIT: &str = "mutation($n: String!, $v: Int!, $f: JSON!) {
    editAkt(input: { number: $n, revision: $v, fields: $f }) {
      revision title fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } } } }";

const RELEASE: &str = "mutation($n: String!, $v: Int!) {
    releaseVersion(input: { number: $n, revision: $v }) { number revision pins { destination number revision } } }";

const CHANGES: &str = "query($n: String!, $all: Boolean!) { akt(number: $n) {
    draftChanges(all: $all) { seq by { id } at fields } } }";

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.allow_everything("colleague");
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        world
            .ok(
                "mutation($d: JSON!) { defineAktType(document: $d) { key liveDrafts } }",
                json!({ "d": document() }),
            )
            .await;
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

    async fn create(&self, fields: Value) -> String {
        self.ok(
            "mutation($f: JSON) { createAkt(input: { type: \"document\", fields: $f }) { number revision } }",
            json!({ "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn akt(&self, number: &str) -> Value {
        self.ok(AKT, json!({ "n": number })).await["akt"].clone()
    }

    async fn edit_as(&self, token: &str, number: &str, fields: Value) -> Value {
        let revision = self.akt(number).await["revision"].clone();
        self.run_as(
            token,
            EDIT,
            json!({ "n": number, "v": revision, "f": fields }),
        )
        .await
    }

    async fn edit(&self, number: &str, fields: Value) -> Value {
        let answer = self.edit_as("tenant", number, fields).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"]["editAkt"].clone()
    }

    async fn release(&self, number: &str) -> Value {
        let revision = self.akt(number).await["revision"].clone();
        self.run(RELEASE, json!({ "n": number, "v": revision }))
            .await
    }

    async fn changes(&self, number: &str, all: bool) -> Value {
        self.ok(CHANGES, json!({ "n": number, "all": all })).await["akt"]["draftChanges"].clone()
    }
}

fn field<'a>(akt: &'a Value, key: &str) -> Option<&'a Value> {
    akt["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == key)
}

fn text<'a>(akt: &'a Value, key: &str) -> Option<&'a str> {
    field(akt, key).and_then(|f| f["text"].as_str().or(f["markdown"].as_str()))
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

// r[verify live.declare]
// r[verify live.edit]
// r[verify live.read]
// r[verify live.fields]
#[tokio::test(flavor = "multi_thread")]
async fn edits_change_the_live_draft_and_leave_revisions_alone() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "summary": "Short.", "body": "Hello.\n", "tags": ["a"] }))
        .await;
    let created = world.akt(&guide).await;
    assert_eq!(created["revision"], 1);
    assert_eq!(text(&created, "body"), Some("Hello.\n"));

    let edited = world
        .edit(
            &guide,
            json!({ "title": "The guide", "body": "Hello, world.\n", "summary": null }),
        )
        .await;
    assert_eq!(edited["revision"], 1, "{edited}");
    assert_eq!(edited["title"], "The guide");
    assert_eq!(text(&edited, "body"), Some("Hello, world.\n"));

    let read = world.akt(&guide).await;
    assert_eq!(read["revision"], 1);
    assert_eq!(read["title"], "The guide");
    assert_eq!(text(&read, "body"), Some("Hello, world.\n"));
    assert_eq!(text(&read, "summary"), None);
    assert_eq!(
        field(&read, "tags").unwrap()["options"],
        json!([{ "key": "a" }])
    );
    assert_eq!(read["revisions"].as_array().unwrap().len(), 1);
    // The latest revision is still the created one.
    let first = world.ok(AT_REVISION, json!({ "n": guide, "r": 1 })).await["akt"].clone();
    assert_eq!(text(&first, "body"), Some("Hello.\n"));

    // The revision an edit names isn't checked: the draft merges.
    let unchecked = world
        .run(
            EDIT,
            json!({ "n": guide, "v": 0, "f": { "tags": ["a", "b"] } }),
        )
        .await;
    assert!(unchecked.get("errors").is_none(), "{unchecked}");
    // A batch edits the draft too.
    let batch = world
        .run(
            "mutation($n: String!) { batch(input: { operations: [
                { editAkt: { number: $n, revision: 1, fields: { body: \"From a batch.\\n\" } } }
              ] }) { revision } }",
            json!({ "n": guide }),
        )
        .await;
    assert!(batch.get("errors").is_none(), "{batch}");
    let read = world.akt(&guide).await;
    assert_eq!(text(&read, "body"), Some("From a batch.\n"));
    assert_eq!(
        field(&read, "tags").unwrap()["options"],
        json!([{ "key": "a" }, { "key": "b" }])
    );
    assert_eq!(read["revisions"].as_array().unwrap().len(), 1);

    // A list answers the draft too.
    let listed = world
        .ok(
            "{ akter(type: \"document\") { nodes { number title } } }",
            json!({}),
        )
        .await;
    assert_eq!(
        listed["akter"]["nodes"],
        json!([{ "number": guide, "title": "The guide" }])
    );
}

// r[verify live.edit]
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_write_leaves_the_draft_unchanged() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "Hello.\n" }))
        .await;
    world.edit(&guide, json!({ "body": "Edited.\n" })).await;
    for fields in [
        json!({ "title": null }),
        json!({ "tags": ["c"] }),
        json!({ "nothing": "x" }),
        json!({ "body": "Edited.\n" }),
    ] {
        let refused = world.edit_as("tenant", &guide, fields.clone()).await;
        assert_eq!(code(&refused), "REFUSED", "{fields}: {refused}");
    }
    let read = world.akt(&guide).await;
    assert_eq!(read["title"], "Guide");
    assert_eq!(text(&read, "body"), Some("Edited.\n"));
    assert_eq!(field(&read, "tags"), None);
    assert_eq!(
        world.changes(&guide, true).await.as_array().unwrap().len(),
        1
    );
}

// r[verify live.release]
// r[verify version.release]
// r[verify live.read]
#[tokio::test(flavor = "multi_thread")]
async fn a_release_records_the_draft_as_a_revision_and_a_version() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "First.\n" }))
        .await;
    world.edit(&guide, json!({ "body": "Second.\n" })).await;

    // Stale: the revision read must be the latest.
    let stale = world.run(RELEASE, json!({ "n": guide, "v": 7 })).await;
    assert_eq!(code(&stale), "STALE", "{stale}");

    let released = world.release(&guide).await;
    let version = &released["data"]["releaseVersion"];
    assert_eq!(version["number"], 1, "{released}");
    assert_eq!(version["revision"], 2);
    let read = world.akt(&guide).await;
    assert_eq!(read["revision"], 2);
    assert_eq!(
        read["revisions"][1]["changes"],
        json!({ "body": "Second.\n" })
    );
    assert_eq!(read["revisions"][1]["by"]["id"], SUBJECT);
    assert_eq!(read["versions"], json!([{ "number": 1, "revision": 2 }]));

    // The same draft again: refused.
    let again = world.release(&guide).await;
    assert_eq!(code(&again), "BAD_REQUEST", "{again}");

    // Edits after the release leave the version as it was.
    world.edit(&guide, json!({ "body": "Third.\n" })).await;
    let at_one = world.ok(AT_VERSION, json!({ "n": guide, "v": 1 })).await["akt"].clone();
    assert_eq!(at_one["revision"], 2);
    assert_eq!(text(&at_one, "body"), Some("Second.\n"));
    assert_eq!(text(&world.akt(&guide).await, "body"), Some("Third.\n"));
    let second = world.release(&guide).await;
    assert_eq!(second["data"]["releaseVersion"]["number"], 2, "{second}");
    assert_eq!(second["data"]["releaseVersion"]["revision"], 3);

    // A release before any edit records the draft as created.
    let fresh = world
        .create(json!({ "title": "Fresh", "body": "New.\n" }))
        .await;
    let first = world.release(&fresh).await;
    assert_eq!(first["data"]["releaseVersion"]["revision"], 2, "{first}");
    let read = world.akt(&fresh).await;
    assert_eq!(read["revisions"][1]["changes"], json!({}));
}

// r[verify live.revisions]
#[tokio::test(flavor = "multi_thread")]
async fn transitions_carry_the_last_release() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "Released.\n" }))
        .await;
    world.release(&guide).await;
    world
        .edit(&guide, json!({ "title": "Edited", "body": "Not yet.\n" }))
        .await;

    let transition = "mutation($n: String!, $v: Int!, $f: JSON) {
        performTransition(input: { number: $n, revision: $v, transition: \"retire\", fields: $f }) { revision title } }";
    let with_fields = world
        .run(
            transition,
            json!({ "n": guide, "v": 2, "f": { "body": "Sneaky.\n" } }),
        )
        .await;
    assert_eq!(code(&with_fields), "BAD_REQUEST", "{with_fields}");
    // A transition names the latest revision, as ever.
    let stale = world.run(transition, json!({ "n": guide, "v": 1 })).await;
    assert_eq!(code(&stale), "STALE", "{stale}");

    let retired = world.run(transition, json!({ "n": guide, "v": 2 })).await;
    assert!(retired.get("errors").is_none(), "{retired}");
    assert_eq!(retired["data"]["performTransition"]["revision"], 3);
    // Reading still answers the draft.
    assert_eq!(retired["data"]["performTransition"]["title"], "Edited");
    let at_three = world.ok(AT_REVISION, json!({ "n": guide, "r": 3 })).await["akt"].clone();
    assert_eq!(at_three["state"]["key"], "retired");
    assert_eq!(text(&at_three, "title"), Some("Guide"));
    assert_eq!(text(&at_three, "body"), Some("Released.\n"));
    let read = world.akt(&guide).await;
    assert_eq!(read["revisions"][2]["changes"], json!({}));
    assert_eq!(read["revisions"][2]["transition"], "retire");
    assert_eq!(text(&read, "body"), Some("Not yet.\n"));
    // A final akt's draft takes no edits.
    let refused = world
        .edit_as("tenant", &guide, json!({ "body": "More.\n" }))
        .await;
    assert_eq!(code(&refused), "REFUSED", "{refused}");
}

// r[verify live.release]
// r[verify version.references]
#[tokio::test(flavor = "multi_thread")]
async fn references_are_checked_on_the_draft_at_release() {
    let world = World::new().await;
    let setup = world
        .create(json!({ "title": "Setup", "body": "# Install {#install}\n" }))
        .await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "Nothing yet.\n" }))
        .await;
    // An edit naming a heading that isn't there is taken: it's a draft.
    world
        .edit(
            &guide,
            json!({ "body": format!("[run](kuria:{setup}#run)\n") }),
        )
        .await;
    // Its links are kept by releases, not edits.
    assert_eq!(world.akt(&guide).await["links"], json!([]));
    let broken = world.release(&guide).await;
    assert_eq!(code(&broken), "BROKEN_REFERENCES", "{broken}");
    assert_eq!(world.akt(&guide).await["revision"], 1);
    // A heading written in the setup's live draft, never released, isn't
    // there for a release: it names the setup to release with it.
    world
        .edit(
            &setup,
            json!({ "body": "# Install {#install}\n\n## Run {#run}\n" }),
        )
        .await;
    let unreleased = world.release(&guide).await;
    assert_eq!(code(&unreleased), "BROKEN_REFERENCES", "{unreleased}");
    assert_eq!(
        unreleased["errors"][0]["extensions"]["together"],
        json!([setup])
    );
    // Together, the setup's draft is recorded and the guide pinned to it.
    let revision = |akt: &Value| akt["revision"].clone();
    let akter = json!([
        { "number": guide, "revision": revision(&world.akt(&guide).await) },
        { "number": setup, "revision": revision(&world.akt(&setup).await) },
    ]);
    let together = world
        .ok(
            "mutation($a: [ReleaseVersionInput!]!) { releaseVersions(input: { akter: $a }) { number pins { number version revision } } }",
            json!({ "a": akter }),
        )
        .await;
    assert_eq!(
        together["releaseVersions"][0]["pins"],
        json!([{ "number": setup, "version": 1, "revision": null }])
    );
    assert_eq!(world.akt(&setup).await["revision"], 2);
    assert_eq!(
        world.akt(&guide).await["links"],
        json!([{ "kind": "refers_to", "akt": { "number": setup } }])
    );
}

// r[verify live.history]
#[tokio::test(flavor = "multi_thread")]
async fn the_drafts_changes_are_kept_by_author_between_releases() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "One.\n" }))
        .await;
    assert_eq!(world.changes(&guide, false).await, json!([]));
    world.edit(&guide, json!({ "body": "Two.\n" })).await;
    let by_colleague = world
        .edit_as(
            "colleague",
            &guide,
            json!({ "title": "Guide, edited", "tags": ["b"] }),
        )
        .await;
    assert!(by_colleague.get("errors").is_none(), "{by_colleague}");

    let changes = world.changes(&guide, false).await;
    let summary: Vec<Value> = changes
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!({ "seq": c["seq"], "by": c["by"]["id"], "fields": c["fields"] }))
        .collect();
    assert_eq!(
        summary,
        vec![
            json!({ "seq": 1, "by": SUBJECT, "fields": ["body"] }),
            json!({ "seq": 2, "by": COLLEAGUE, "fields": ["tags", "title"] }),
        ]
    );
    assert!(changes[0]["at"].as_str().unwrap() <= changes[1]["at"].as_str().unwrap());

    // A release starts the next stretch; all of them are still there.
    world.release(&guide).await;
    assert_eq!(world.changes(&guide, false).await, json!([]));
    world.edit(&guide, json!({ "body": "Three.\n" })).await;
    let after = world.changes(&guide, false).await;
    assert_eq!(after.as_array().unwrap().len(), 1);
    assert_eq!(after[0]["seq"], 3);
    assert_eq!(
        world.changes(&guide, true).await.as_array().unwrap().len(),
        3
    );
}

const REPLACE: &str = "mutation($n: String!, $r: [ReplacementInput!]!) {
    replaceInAkt(input: { number: $n, replacements: $r }) {
      revision fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } } } }";

// r[verify akt.replace]
// r[verify akt.replace.match]
// r[verify live.edit]
#[tokio::test(flavor = "multi_thread")]
async fn a_replacement_keeps_what_others_typed_since_it_was_read() {
    let world = World::new().await;
    let guide = world
        .create(json!({ "title": "Guide", "body": "Install it.\n\nThen restart.\n" }))
        .await;
    // An agent reads the body, and while it thinks a colleague edits
    // another paragraph of it.
    let read = world.akt(&guide).await;
    assert_eq!(text(&read, "body"), Some("Install it.\n\nThen restart.\n"));
    let colleague = world
        .edit_as(
            "colleague",
            &guide,
            json!({ "body": "Install it.\n\nThen restart, twice.\n" }),
        )
        .await;
    assert!(colleague.get("errors").is_none(), "{colleague}");

    // The agent's replacement, of what it read, keeps the colleague's.
    let replaced = world
        .run(
            REPLACE,
            json!({ "n": guide, "r": [{ "field": "body", "span": "Install it.", "text": "Run `setup`." }] }),
        )
        .await;
    assert!(replaced.get("errors").is_none(), "{replaced}");
    let after = world.akt(&guide).await;
    assert_eq!(
        text(&after, "body"),
        Some("Run `setup`.\n\nThen restart, twice.\n")
    );
    assert_eq!(after["revisions"].as_array().unwrap().len(), 1);
    let changes = world.changes(&guide, false).await;
    assert_eq!(changes.as_array().unwrap().len(), 2, "{changes}");
    assert_eq!(changes[1]["by"]["id"], SUBJECT);

    // A span someone changed since is refused, and changes nothing.
    let gone = world
        .run(
            REPLACE,
            json!({ "n": guide, "r": [{ "field": "body", "span": "Then restart.", "text": "Done." }] }),
        )
        .await;
    assert_eq!(code(&gone), "NO_MATCH", "{gone}");
    assert_eq!(gone["errors"][0]["extensions"]["field"], "body");
    let twice = world
        .run(
            REPLACE,
            json!({ "n": guide, "r": [{ "field": "body", "span": "t", "text": "T" }] }),
        )
        .await;
    assert_eq!(code(&twice), "AMBIGUOUS", "{twice}");
    let tags = world
        .run(
            REPLACE,
            json!({ "n": guide, "r": [{ "field": "tags", "span": "a", "text": "b" }] }),
        )
        .await;
    assert_eq!(code(&tags), "BAD_REQUEST", "{tags}");
    assert_eq!(
        text(&world.akt(&guide).await, "body"),
        Some("Run `setup`.\n\nThen restart, twice.\n")
    );
}
