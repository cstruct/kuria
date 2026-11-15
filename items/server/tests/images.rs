//! Images as akter, embedded in `markdown` fields through GraphQL against a
//! fresh database (`akt.field.markdown.images`, `attachment.media-types`,
//! `attachment.download`).

mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{Answer, FakeHost, graphql_with, router_with, send};

/// A database of its own, with a document type and an image type.
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
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "links": [
            { "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document", "image"] }
        ],
        "versions": { "action": "documents:release" }
    })
}

fn image() -> Value {
    json!({
        "key": "image", "name": "Image", "number": "IMG-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "file", "name": "File", "kind": "attachment",
              "media_types": ["image/png", "image/svg+xml"] }
        ],
        "states": [{ "key": "current", "name": "Current", "initial": true }],
        "presentation": { "image": "file", "navigation": "more" }
    })
}

const IMAGES: &str = "query($n: String!, $v: Int) { akt(number: $n, version: $v) {
    links { kind outgoing akt { number } }
    fields { ... on MarkdownValue { images { destination number version broken
      attachment { id mediaType } } } } } }";

const SVG: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"/>";
/// A one-pixel red PNG.
const PNG: &[u8] = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90\x77\x53\xde\x00\x00\x00\x0c\x49\x44\x41\x54\x78\x9c\x63\xf8\xcf\xc0\x00\x00\x03\x01\x01\x00\xc9\xfe\x92\xef\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82";

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for d in [image(), document()] {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": d }),
                )
                .await;
        }
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

    async fn upload(&self, name: &str, media_type: &str, bytes: &[u8]) -> String {
        let answer = send(
            &self.router,
            Request::post("/attachments")
                .header("authorization", "Bearer tenant")
                .header("x-attachment-name", name)
                .header("content-type", media_type)
                .body(Body::from(bytes.to_vec()))
                .unwrap(),
        )
        .await;
        assert_eq!(answer.status, 201, "{}", answer.body);
        answer.json()["id"].as_str().unwrap().to_owned()
    }

    async fn download(&self, id: &str) -> Answer {
        send(
            &self.router,
            Request::get(format!("/attachments/{id}"))
                .header("authorization", "Bearer tenant")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    async fn create(&self, key: &str, fields: Value) -> Value {
        self.run(
            "mutation($t: String!, $f: JSON) { createAkt(input: { type: $t, in: [], fields: $f }) { number revision } }",
            json!({ "t": key, "f": fields }),
        )
        .await
    }

    async fn created(&self, key: &str, fields: Value) -> String {
        let answer = self.create(key, fields).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"]["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn edit(&self, number: &str, revision: i64, fields: Value) {
        self.ok(
            "mutation($n: String!, $v: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }",
            json!({ "n": number, "v": revision, "f": fields }),
        )
        .await;
    }

    async fn images(&self, number: &str, version: Option<i64>) -> Value {
        let akt = self.ok(IMAGES, json!({ "n": number, "v": version })).await["akt"].clone();
        akt["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f.get("images").is_some())
            .unwrap()["images"]
            .clone()
    }

    async fn release(&self, number: &str, revision: i64) -> Value {
        self.run(
            "mutation($n: String!, $v: Int!) { releaseVersion(input: { number: $n, revision: $v }) { number } }",
            json!({ "n": number, "v": revision }),
        )
        .await
    }
}

fn code(answer: &Value) -> &Value {
    &answer["errors"][0]["extensions"]["code"]
}

// r[verify attachment.media-types]
#[tokio::test(flavor = "multi_thread")]
async fn a_picture_must_be_of_a_media_type_its_field_takes() {
    let world = World::new().await;
    let text = world.upload("notes.txt", "text/plain", b"notes").await;
    let refused = world
        .create("image", json!({ "title": "Notes", "file": text }))
        .await;
    assert_eq!(code(&refused), "REFUSED", "{refused}");
    assert!(
        refused["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("the attachment is text/plain, and the field takes image/png, image/svg+xml"),
        "{refused}"
    );
    let svg = world.upload("logo.svg", "image/svg+xml", SVG).await;
    world
        .created("image", json!({ "title": "Logo", "file": svg }))
        .await;
}

// r[verify akt.field.markdown.images]
// r[verify akt.field.markdown.references]
// r[verify version.references]
#[tokio::test(flavor = "multi_thread")]
async fn embedded_images_show_their_picture_or_say_why_they_are_broken() {
    let world = World::new().await;
    let png = world.upload("chart.png", "image/png", PNG).await;
    let chart = world
        .created("image", json!({ "title": "Chart", "file": png }))
        .await;
    let blank = world.created("image", json!({ "title": "Blank" })).await;
    let other = world.created("document", json!({ "title": "Other" })).await;
    let guide = world
        .created(
            "document",
            json!({ "title": "Guide", "body": format!(
                "![The chart](kuria:{chart})\n\n![Nothing](kuria:{blank}) ![Text](kuria:{other}) ![Heading](kuria:{chart}#top) ![Gone](kuria:IMG-99)\n"
            ) }),
        )
        .await;
    let images = world.images(&guide, None).await;
    assert_eq!(images[0]["destination"], format!("kuria:{chart}"));
    assert_eq!(images[0]["attachment"]["id"], json!(png));
    assert_eq!(images[0]["attachment"]["mediaType"], "image/png");
    assert_eq!(images[0]["broken"], Value::Null);
    let broken: Vec<&str> = images.as_array().unwrap()[1..]
        .iter()
        .map(|i| i["broken"].as_str().unwrap())
        .collect();
    assert_eq!(
        broken,
        [
            format!("`{blank}` has no picture").as_str(),
            &format!("`{other}` isn't an image"),
            &format!("`kuria:{chart}#top` names a heading, which an embedded image can't"),
            "there is no akt `IMG-99`",
        ]
    );
    assert!(
        images.as_array().unwrap()[1..]
            .iter()
            .all(|i| i["attachment"].is_null())
    );
    // Kept as the field's references, as a link to each.
    let links = world.ok(IMAGES, json!({ "n": guide })).await["akt"]["links"].clone();
    assert!(
        links
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["kind"] == "refers_to" && l["akt"]["number"] == json!(chart)),
        "{links}"
    );
    // A release is refused while one is broken.
    let refused = world.release(&guide, 1).await;
    assert_eq!(code(&refused), "BROKEN_REFERENCES", "{refused}");
}

// r[verify akt.field.markdown.images]
// r[verify version.read]
// r[verify attachment.download]
#[tokio::test(flavor = "multi_thread")]
async fn a_version_shows_the_picture_it_was_released_with() {
    let world = World::new().await;
    let first = world.upload("v1.png", "image/png", PNG).await;
    let chart = world
        .created("image", json!({ "title": "Chart", "file": first }))
        .await;
    let guide = world
        .created(
            "document",
            json!({ "title": "Guide", "body": format!("![The chart](kuria:{chart})\n") }),
        )
        .await;
    let released = world.release(&guide, 1).await;
    assert!(released.get("errors").is_none(), "{released}");
    // The image's file is replaced afterwards.
    let second = world.upload("v2.svg", "image/svg+xml", SVG).await;
    world.edit(&chart, 1, json!({ "file": second })).await;

    let now = world.images(&guide, None).await;
    assert_eq!(now[0]["attachment"]["id"], json!(second));
    let then = world.images(&guide, Some(1)).await;
    assert_eq!(then[0]["attachment"]["id"], json!(first), "{then}");
    // The picture it was released with still downloads.
    let answer = world.download(&first).await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.bytes, PNG);
}

// r[verify pdf.render]
// r[verify pdf.packages]
#[tokio::test(flavor = "multi_thread")]
async fn a_pdf_draws_its_embedded_pictures() {
    let world = World::new().await;
    let png = world.upload("chart.png", "image/png", PNG).await;
    let chart = world
        .created("image", json!({ "title": "Chart", "file": png }))
        .await;
    let svg = world.upload("logo.svg", "image/svg+xml", SVG).await;
    let logo = world
        .created("image", json!({ "title": "Logo", "file": svg }))
        .await;
    let guide = world
        .created(
            "document",
            json!({ "title": "Guide", "body": format!(
                "# Pictures\n\n![The chart](kuria:{chart})\n\nA logo ![logo](kuria:{logo}) inline, and ![Missing](kuria:IMG-99).\n"
            ) }),
        )
        .await;
    let answer = send(
        &world.router,
        Request::get(format!("/akter/{guide}/pdf"))
            .header("authorization", "Bearer tenant")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let document = lopdf::Document::load_mem(&answer.bytes).unwrap();
    let images = document
        .objects
        .values()
        .filter(|o| {
            o.as_stream().is_ok_and(|s| {
                s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok()
                    == Some(b"Image".as_slice())
            })
        })
        .count();
    assert!(images >= 1, "the PNG is an image in the PDF");
}
