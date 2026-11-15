//! Diagram blocks rendered on write, through GraphQL against a fresh
//! database (`akt.field.markdown.diagrams`, `diagram.preview`).

mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use kuria_server::bundles::archive;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{Answer, FakeHost, graphql_with, router_with, send};

struct World {
    _db: TestDatabase,
    router: Router,
}

fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown", "diagrams": true },
            { "key": "notes", "name": "Notes", "kind": "markdown" }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "versions": { "action": "documents:release" }
    })
}

const FLOW: &str = "#diagram(node((0, 0), [Write]), edge(\"-|>\"), node((1, 0), [Release]))";
const PLOT: &str = "#canvas({ plot.plot(size: (3, 2), { plot.add(((0, 0), (1, 1), (2, 4))) }) })";

fn block(source: &str) -> String {
    format!("```diagram\n{source}\n```\n")
}

const DIAGRAMS: &str = "query($n: String!, $v: Int) { akt(number: $n, version: $v) { revision
    fields { ... on MarkdownValue { field diagrams { source attachment { id url mediaType } } } } } }";

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

    async fn create(&self, fields: Value) -> Value {
        self.run(
            "mutation($f: JSON) { createAkt(input: { type: \"document\", in: [], fields: $f }) { number } }",
            json!({ "f": fields }),
        )
        .await
    }

    async fn edit(&self, number: &str, revision: i64, fields: Value) -> Value {
        self.run(
            "mutation($n: String!, $v: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }",
            json!({ "n": number, "v": revision, "f": fields }),
        )
        .await
    }

    /// Each `markdown` field's diagram blocks, by field.
    async fn diagrams(&self, number: &str, version: Option<i64>) -> Value {
        let akt = self
            .ok(DIAGRAMS, json!({ "n": number, "v": version }))
            .await["akt"]
            .clone();
        let mut by_field = serde_json::Map::new();
        for f in akt["fields"].as_array().unwrap() {
            if let Some(field) = f["field"].as_str()
                && f.get("diagrams").is_some()
            {
                by_field.insert(field.to_owned(), f["diagrams"].clone());
            }
        }
        Value::Object(by_field)
    }

    async fn get(&self, path: &str) -> Answer {
        send(
            &self.router,
            Request::get(path)
                .header("authorization", "Bearer tenant")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

// r[verify akt.field.markdown.diagrams]
// r[verify attachment.download]
#[tokio::test(flavor = "multi_thread")]
async fn diagram_blocks_are_rendered_when_written() {
    let world = World::new().await;
    let created = world
        .create(json!({
            "title": "Flow",
            "body": format!("Before.\n\n{}\n{}", block(FLOW), block(PLOT)),
            "notes": block(FLOW),
        }))
        .await;
    assert!(created.get("errors").is_none(), "{created}");
    let number = created["data"]["createAkt"]["number"].as_str().unwrap();
    let drawn = world.diagrams(number, None).await;
    let body = drawn["body"].as_array().unwrap();
    assert_eq!(body.len(), 2, "{drawn}");
    assert_eq!(body[0]["source"], format!("{FLOW}\n"));
    assert_eq!(body[0]["attachment"]["mediaType"], "image/svg+xml");
    // A field that doesn't allow diagrams holds code.
    assert_eq!(drawn["notes"], json!([]));
    let svg = world
        .get(body[1]["attachment"]["url"].as_str().unwrap())
        .await;
    assert_eq!(svg.status, 200, "{}", svg.body);
    assert!(svg.body.starts_with("<svg"), "{}", svg.body);
}

// r[verify akt.field.markdown.diagrams]
#[tokio::test(flavor = "multi_thread")]
async fn a_diagram_that_doesnt_compile_refuses_the_write() {
    let world = World::new().await;
    let refused = world
        .create(json!({
            "title": "Broken",
            "body": format!("{}\n{}", block(FLOW), block("#diagram(node((0, 0), [A])")),
        }))
        .await;
    assert_eq!(code(&refused), "INVALID_DIAGRAM", "{refused}");
    let extensions = &refused["errors"][0]["extensions"];
    assert_eq!(extensions["field"], "body");
    assert_eq!(extensions["block"], 2);
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.starts_with("diagram 2 in `body` doesn't compile: "),
        "{message}"
    );
    // Importing anything else is refused the same way.
    let other = world
        .create(json!({
            "title": "Other",
            "body": block("#import \"@preview/tablex:0.0.8\": tablex"),
        }))
        .await;
    assert_eq!(code(&other), "INVALID_DIAGRAM", "{other}");
}

// r[verify akt.field.markdown.diagrams]
// r[verify version.read]
#[tokio::test(flavor = "multi_thread")]
async fn a_version_shows_the_diagrams_it_had() {
    let world = World::new().await;
    let created = world
        .create(json!({ "title": "Flow", "body": block(FLOW) }))
        .await;
    let number = created["data"]["createAkt"]["number"].as_str().unwrap();
    let first = world.diagrams(number, None).await["body"][0]["attachment"]["id"].clone();
    world
        .ok(
            "mutation($n: String!) { releaseVersion(input: { number: $n, revision: 1 }) { number } }",
            json!({ "n": number }),
        )
        .await;
    let edited = world.edit(number, 1, json!({ "body": block(PLOT) })).await;
    assert!(edited.get("errors").is_none(), "{edited}");
    let now = world.diagrams(number, None).await["body"][0]["attachment"]["id"].clone();
    assert_ne!(now, first);
    let then = world.diagrams(number, Some(1)).await["body"][0]["attachment"]["id"].clone();
    assert_eq!(then, first);
}

// r[verify diagram.preview]
#[tokio::test(flavor = "multi_thread")]
async fn a_diagram_previews_without_being_kept() {
    let world = World::new().await;
    let query = "query($s: String!) { renderDiagram(source: $s) { svg error } }";
    let rendered = world.ok(query, json!({ "s": FLOW })).await["renderDiagram"].clone();
    assert!(rendered["svg"].as_str().unwrap().starts_with("<svg"));
    assert_eq!(rendered["error"], Value::Null);
    let refused = world.ok(query, json!({ "s": "#diagram(" })).await["renderDiagram"].clone();
    assert_eq!(refused["svg"], Value::Null);
    assert!(refused["error"].as_str().is_some_and(|e| !e.is_empty()));
}

// r[verify bundle.contents]
#[tokio::test(flavor = "multi_thread")]
async fn diagrams_move_with_their_akter() {
    let source = World::new().await;
    let created = source
        .create(json!({ "title": "Flow", "body": block(FLOW) }))
        .await;
    let number = created["data"]["createAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let exported = source.get("/export?tenant=true").await;
    assert_eq!(exported.status, 200);
    let unpacked = archive::unpack(&exported.bytes).unwrap();
    let bundle: Value = serde_json::from_slice(&unpacked.bundle).unwrap();
    assert_eq!(
        bundle["akter"][0]["diagrams"][0]["field"], "body",
        "{bundle}"
    );

    let target = World::empty().await;
    let imported = send(
        &target.router,
        Request::post("/import")
            .header("authorization", "Bearer tenant")
            .header("content-type", "application/x-tar")
            .body(Body::from(exported.bytes))
            .unwrap(),
    )
    .await;
    assert_eq!(imported.status, 200, "{}", imported.body);
    let there = target.diagrams(&number, None).await;
    let here = source.diagrams(&number, None).await;
    assert_eq!(there, here);
    let svg = target
        .get(there["body"][0]["attachment"]["url"].as_str().unwrap())
        .await;
    assert_eq!(svg.status, 200);
}

impl World {
    /// A database of its own with no akt types, for a whole tenant to go
    /// into.
    async fn empty() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        Self { _db: db, router }
    }
}

// r[verify pdf.render]
#[tokio::test(flavor = "multi_thread")]
async fn a_pdf_draws_its_diagrams_from_their_source() {
    let world = World::new().await;
    let created = world
        .create(json!({
            "title": "Flow",
            "body": format!("# Flow\n\n{}\n```diagram\n#set text(fill: red)\n#diagram(node((0, 0), [Kept]))\n```\n\nAfter.\n", block(FLOW)),
            "notes": block(FLOW),
        }))
        .await;
    assert!(created.get("errors").is_none(), "{created}");
    let number = created["data"]["createAkt"]["number"].as_str().unwrap();
    let answer = world.get(&format!("/akter/{number}/pdf?field=body")).await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let document = lopdf::Document::load_mem(&answer.bytes).unwrap();
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    let text = document.extract_text(&pages).unwrap();
    for words in ["Write", "Release", "Kept", "After."] {
        assert!(text.contains(words), "{words:?} in {text:?}");
    }
    // A field without diagrams shows the block as code.
    let notes = world.get(&format!("/akter/{number}/pdf?field=notes")).await;
    assert_eq!(notes.status, 200, "{}", notes.body);
    let document = lopdf::Document::load_mem(&notes.bytes).unwrap();
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    let text = document.extract_text(&pages).unwrap();
    assert!(text.contains("#diagram"), "{text:?}");
}
