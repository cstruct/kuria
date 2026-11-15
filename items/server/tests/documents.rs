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
    db: TestDatabase,
    router: Router,
    decider: Arc<FakeDecider>,
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
        ],
        "versions": { "action": "documents:release" }
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
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            db,
            router,
            decider,
        };
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

const RELEASE: &str = "mutation($n: String!, $v: Int!, $l: String) {
    releaseVersion(input: { number: $n, revision: $v, label: $l }) { number revision label pins { destination number version revision } } }";

impl World {
    async fn release(&self, token: &str, number: &str, label: Option<&str>) -> Value {
        let revision = self.akt(number).await["revision"].clone();
        send(
            &self.router,
            graphql_with(
                token,
                RELEASE,
                &json!({ "n": number, "v": revision, "l": label }),
            ),
        )
        .await
        .json()
    }
}

// r[verify version.release]
// r[verify version.number]
// r[verify version.references]
// r[verify version.list]
// r[verify authz.release]
#[tokio::test(flavor = "multi_thread")]
async fn a_release_names_the_current_revision_once_nothing_is_broken() {
    let world = World::new().await;
    let setup = world.create("Setup", "# Install {#install}\n").await;
    let guide = world
        .create("Guide", &format!("[run](kuria:{setup}#run)\n"))
        .await;
    // Broken: refused, naming the reference.
    let broken = world.release("tenant", &guide, None).await;
    assert_eq!(code(&broken), "BROKEN_REFERENCES", "{broken}");
    assert_eq!(
        broken["errors"][0]["extensions"]["references"],
        json!([format!(
            "`kuria:{setup}#run` in `body`: `{setup}` has no heading `run`"
        )])
    );
    world
        .edit(&guide, &format!("[install](kuria:{setup}#install)\n"))
        .await;
    // Stale: refused.
    let stale = world
        .run(RELEASE, json!({ "n": guide, "v": 1, "l": null }))
        .await;
    assert_eq!(code(&stale), "STALE", "{stale}");
    // Without the type's action: forbidden.
    world
        .decider
        .grant("tenant-editor", &["kuria:akter:read", "kuria:akter:edit"]);
    let forbidden = world.release("tenant-editor", &guide, None).await;
    assert_eq!(code(&forbidden), "FORBIDDEN", "{forbidden}");
    let released = world.release("tenant", &guide, Some("First")).await;
    let version = &released["data"]["releaseVersion"];
    assert_eq!(version["number"], 1, "{released}");
    assert_eq!(version["label"], "First");
    // The target has no version: pinned at its revision.
    assert_eq!(
        version["pins"],
        json!([{ "destination": format!("kuria:{setup}#install"), "number": setup, "version": null, "revision": 1 }])
    );
    // A revision is released once.
    let again = world.release("tenant", &guide, None).await;
    assert_eq!(code(&again), "BAD_REQUEST", "{again}");
    world.edit(&guide, "Changed.\n").await;
    let second = world.release("tenant", &guide, None).await;
    assert_eq!(second["data"]["releaseVersion"]["number"], 2, "{second}");
    let listed = world
        .ok(
            "query($n: String!) { akt(number: $n) { versions { number revision label releasedBy { id } } } }",
            json!({ "n": guide }),
        )
        .await;
    let numbers: Vec<&Value> = listed["akt"]["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| &v["number"])
        .collect();
    assert_eq!(numbers, [&json!(2), &json!(1)], "latest first");
}

// r[verify version.read]
// r[verify version.pins]
// r[verify version.immutable]
// r[verify version.history]
#[tokio::test(flavor = "multi_thread")]
async fn a_version_reads_as_it_was_released() {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    let world = World::new().await;
    let setup = world.create("Setup", "# Install {#install}\n").await;
    world.release("tenant", &setup, None).await;
    let guide = world
        .create("Guide", &format!("[install](kuria:{setup}#install)\n"))
        .await;
    let released = world.release("tenant", &guide, None).await;
    // Pinned to the target's version 1.
    assert_eq!(
        released["data"]["releaseVersion"]["pins"][0]["version"], 1,
        "{released}"
    );
    // The target loses the heading, and the guide changes on.
    world.edit(&setup, "# Running {#run}\n").await;
    world
        .edit(
            &guide,
            &format!("[install](kuria:{setup}#install)\n\nMore.\n"),
        )
        .await;
    let living = world.akt(&guide).await;
    assert_eq!(
        body(&living)["references"][0]["broken"],
        json!(format!("`{setup}` has no heading `install`"))
    );
    let at = world
        .ok(
            "query($n: String!) { akt(number: $n, version: 1) { revision version { number }
               fields { ... on MarkdownValue { markdown references { broken pinnedVersion } } } } }",
            json!({ "n": guide }),
        )
        .await;
    assert_eq!(at["akt"]["version"]["number"], 1);
    assert_eq!(at["akt"]["revision"], 1);
    assert_eq!(
        at["akt"]["fields"][1]["markdown"],
        format!("[install](kuria:{setup}#install)\n")
    );
    assert_eq!(
        at["akt"]["fields"][1]["references"],
        json!([{ "broken": null, "pinnedVersion": 1 }])
    );
    let missing = world
        .ok(
            "query($n: String!) { akt(number: $n, version: 9) { number } }",
            json!({ "n": guide }),
        )
        .await;
    assert!(missing["akt"].is_null());

    // A release is a change event, and versions never change.
    let mut conn = world.db.pool().get().await.unwrap();
    let kinds: Vec<String> = kuria_store::schema::change_event::table
        .filter(kuria_store::schema::change_event::kind.eq("version"))
        .select(kuria_store::schema::change_event::kind)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(kinds.len(), 2);
    let update = diesel::update(kuria_store::schema::akt_version::table)
        .set(kuria_store::schema::akt_version::label.eq("rewritten"))
        .execute(&mut conn)
        .await;
    assert!(update.is_err(), "a version was changed");
    let delete = diesel::delete(kuria_store::schema::akt_version::table)
        .execute(&mut conn)
        .await;
    assert!(delete.is_err(), "a version was removed");
}

/// A PDF of `number`'s body, at `version` if given.
async fn pdf(world: &World, number: &str, version: Option<i32>) -> support::Answer {
    let query = version.map(|v| format!("&version={v}")).unwrap_or_default();
    let request = axum::http::Request::get(format!("/akter/{number}/pdf?field=body{query}"))
        .header("authorization", "Bearer tenant")
        .body(axum::body::Body::empty())
        .unwrap();
    send(&world.router, request).await
}

/// The text of every page of `bytes`, a PDF, and its bookmarks' titles.
fn read_pdf(bytes: &[u8]) -> (String, Vec<String>) {
    let document = lopdf::Document::load_mem(bytes).unwrap();
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    let text = document.extract_text(&pages).unwrap();
    // Words, one space apart, however the PDF breaks its lines.
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut titles = Vec::new();
    if let Ok(outline) = document.get_toc() {
        titles = outline.toc.into_iter().map(|t| t.title).collect();
    }
    (text, titles)
}

/// Every `URI` in `object` and what it holds.
fn uris_in(object: &lopdf::Object, found: &mut Vec<String>) {
    match object {
        lopdf::Object::Dictionary(d) => {
            for (key, value) in d {
                if key == b"URI"
                    && let Ok(uri) = value.as_str()
                {
                    found.push(String::from_utf8_lossy(uri).into_owned());
                }
                uris_in(value, found);
            }
        }
        lopdf::Object::Array(items) => items.iter().for_each(|i| uris_in(i, found)),
        _ => {}
    }
}

// r[verify pdf.render]
// r[verify pdf.route]
// r[verify pdf.repeatable]
#[tokio::test(flavor = "multi_thread")]
async fn a_markdown_field_is_read_as_a_pdf() {
    let world = World::new().await;
    let setup = world.create("Setup", "# Install {#install}\n").await;
    let guide = world
        .create(
            "Guide",
            &format!(
                "# Getting started\n\nRead the [setup](kuria:{setup}#install) first.\n\n## Steps\n\n- one\n- two\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n"
            ),
        )
        .await;
    let answer = pdf(&world, &guide, None).await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.headers["content-type"], "application/pdf");
    assert!(
        answer.headers["content-disposition"]
            .to_str()
            .unwrap()
            .contains(&format!("{guide}.pdf"))
    );
    assert!(answer.bytes.starts_with(b"%PDF"));
    let (text, titles) = read_pdf(&answer.bytes);
    for words in [
        "Getting started",
        "Read the setup first.",
        "Steps",
        "one",
        "two",
    ] {
        assert!(text.contains(words), "{words:?} in {text:?}");
    }
    assert_eq!(titles, ["Getting started", "Steps"]);
    // The reference links to the akt's heading in the app.
    let document = lopdf::Document::load_mem(&answer.bytes).unwrap();
    let mut uris = Vec::new();
    for object in document.objects.values() {
        uris_in(object, &mut uris);
    }
    assert!(
        uris.iter()
            .any(|u| u.ends_with(&format!("/akt/{setup}#install"))),
        "{uris:?}"
    );

    // A version's PDF is of the version, the same each time.
    world.release("tenant", &guide, None).await;
    world.edit(&guide, "# Rewritten\n").await;
    let first = pdf(&world, &guide, Some(1)).await;
    assert_eq!(first.status, 200, "{}", first.body);
    assert!(read_pdf(&first.bytes).0.contains("Getting started"));
    assert_eq!(pdf(&world, &guide, Some(1)).await.bytes, first.bytes);
    assert!(
        read_pdf(&pdf(&world, &guide, None).await.bytes)
            .0
            .contains("Rewritten")
    );

    // 404 for what isn't there, or isn't a markdown field.
    assert_eq!(pdf(&world, &guide, Some(9)).await.status, 404);
    assert_eq!(pdf(&world, "D-99", None).await.status, 404);
    let title = send(
        &world.router,
        axum::http::Request::get(format!("/akter/{guide}/pdf?field=title"))
            .header("authorization", "Bearer tenant")
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(title.status, 404);
}
