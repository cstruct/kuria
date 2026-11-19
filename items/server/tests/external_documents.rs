//! Akt types whose versions are released as PDFs, such as external
//! documents (`version.file`, `version.release-file`): declared, released
//! with an uploaded PDF, and read back through the PDF route.

mod support;

use axum::body::Body;
use axum::http::Request;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::send;
use support::world::World;

const TOKEN: &str = "tenant";

fn external(versions: &Value) -> Value {
    json!({
        "key": "external_document",
        "name": "External document",
        "number": "EXT-{seq:3}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "pdf", "name": "PDF", "kind": "attachment", "media_types": ["application/pdf"] }
        ],
        "states": [
            { "key": "draft", "name": "Draft", "initial": true },
            { "key": "retired", "name": "Retired", "final": true }
        ],
        "transitions": [{ "key": "retire", "name": "Retire", "from": ["draft"], "to": "retired" }],
        "versions": versions
    })
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

async fn revision(world: &World, number: &str) -> i64 {
    world
        .ok(
            TOKEN,
            "query($n: String!) { akt(number: $n) { revision } }",
            json!({ "n": number }),
        )
        .await["akt"]["revision"]
        .as_i64()
        .unwrap()
}

/// Releases `number` with the attachment `file`, answering the whole
/// answer.
async fn release(world: &World, number: &str, file: Option<&str>) -> Value {
    let revision = revision(world, number).await;
    world
        .run(
            TOKEN,
            "mutation($n: String!, $r: Int!, $f: ID) { releaseVersion(input: { number: $n, revision: $r, file: $f }) { number revision } }",
            json!({ "n": number, "r": revision, "f": file }),
        )
        .await
}

async fn pdf(world: &World, path: &str) -> support::Answer {
    send(
        &world.router,
        Request::get(path)
            .header("authorization", format!("Bearer {TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

// r[verify version.declare]
// r[verify version.file]
#[tokio::test(flavor = "multi_thread")]
async fn a_type_released_as_files_names_a_pdf_field() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    let problems = |answer: &Value| -> Vec<String> {
        answer["errors"][0]["extensions"]["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["path"].as_str().unwrap().to_owned())
            .collect()
    };
    let define = |document: Value| {
        let world = &world;
        async move {
            world
                .run(
                    TOKEN,
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await
        }
    };
    // Not a PDF field.
    let refused = define(external(&json!({ "file": "title" }))).await;
    assert_eq!(problems(&refused), ["versions.file"], "{refused}");
    let answer = define(external(&json!({ "file": "pdf" }))).await;
    assert!(answer.get("errors").is_none(), "{answer}");
    // A type with text written here, live, may too.
    let answer = define(written(true)).await;
    assert!(answer.get("errors").is_none(), "{answer}");
}

/// A document written here whose versions may carry a PDF, with live
/// drafts or not.
fn written(live: bool) -> Value {
    let mut ty = external(&json!({ "file": "pdf" }));
    ty["key"] = json!("document");
    ty["number"] = json!("DOC-{seq:3}");
    ty["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "key": "body", "name": "Body", "kind": "markdown" }));
    ty["live_drafts"] = json!(live);
    ty
}

async fn edit(world: &World, number: &str, fields: Value) {
    let revision = revision(world, number).await;
    world
        .ok(
            TOKEN,
            "mutation($n: String!, $r: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $r, fields: $f }) { number } }",
            json!({ "n": number, "r": revision, "f": fields }),
        )
        .await;
}

// r[verify version.file]
// r[verify version.release-file]
// r[verify pdf.route]
#[tokio::test(flavor = "multi_thread")]
async fn a_document_written_here_keeps_its_old_versions_as_pdfs() {
    for live in [false, true] {
        let world = World::new(FakeDecider::allowing_tenant()).await;
        world.define(TOKEN, &written(live)).await;
        let number = world
            .create(
                TOKEN,
                "document",
                &[],
                json!({ "title": "Policy" }),
                "Import",
            )
            .await;
        let old = b"%PDF-1.4 old".to_vec();
        let file = world
            .upload(TOKEN, "v1.pdf", "application/pdf", old.clone())
            .await;
        let released = release(&world, &number, Some(&file)).await;
        assert_eq!(
            released["data"]["releaseVersion"]["number"], 1,
            "{released}"
        );

        // Its body written here, then released without a PDF: rendered.
        edit(
            &world,
            &number,
            json!({ "body": "# Policy\n\nWritten here.\n" }),
        )
        .await;
        let released = release(&world, &number, None).await;
        assert_eq!(
            released["data"]["releaseVersion"]["number"], 2,
            "{released}"
        );
        let at = |version: i64| {
            let world = &world;
            let number = number.clone();
            async move {
                world
                    .ok(
                        TOKEN,
                        "query($n: String!, $v: Int) { akt(number: $n, version: $v) { fields { ... on AttachmentValue { field } ... on MarkdownValue { field } } } }",
                        json!({ "n": number, "v": version }),
                    )
                    .await["akt"]["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|f| f["field"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            }
        };
        assert!(at(1).await.contains(&"pdf".to_owned()), "live: {live}");
        assert!(!at(2).await.contains(&"pdf".to_owned()), "live: {live}");

        // Version 1 is its PDF; version 2 and the living version are rendered.
        let first = pdf(&world, &format!("/akter/{number}/pdf?version=1")).await;
        assert_eq!(first.bytes, old, "live: {live}");
        for path in [
            format!("/akter/{number}/pdf?version=2"),
            format!("/akter/{number}/pdf"),
        ] {
            let rendered = pdf(&world, &path).await;
            assert_eq!(rendered.status, 200, "{path}");
            assert_ne!(rendered.bytes, old, "{path}");
            assert!(rendered.bytes.starts_with(b"%PDF"), "{path}");
        }
    }
}

// r[verify version.file]
// r[verify version.release-file]
// r[verify version.history]
// r[verify pdf.route]
#[tokio::test(flavor = "multi_thread")]
#[expect(
    clippy::too_many_lines,
    reason = "one document released from start to end"
)]
async fn each_version_is_released_with_its_pdf() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    world
        .define(TOKEN, &external(&json!({ "file": "pdf" })))
        .await;
    let first = b"%PDF-1.4 first".to_vec();
    let first_id = world
        .upload(TOKEN, "policy.pdf", "application/pdf", first.clone())
        .await;

    // Only a release sets the PDF.
    let refused = world
        .run(
            TOKEN,
            "mutation($f: JSON) { createAkt(input: { type: \"external_document\", fields: $f }) { number } }",
            json!({ "f": { "title": "Supplier policy", "pdf": first_id } }),
        )
        .await;
    assert_eq!(code(&refused), "REFUSED", "{refused}");
    let number = world
        .create(
            TOKEN,
            "external_document",
            &[],
            json!({ "title": "Supplier policy" }),
            "Their policy",
        )
        .await;
    assert_eq!(
        pdf(&world, &format!("/akter/{number}/pdf")).await.status,
        404
    );

    // A release needs its PDF, and a PDF.
    assert_eq!(code(&release(&world, &number, None).await), "BAD_REQUEST");
    let text = world
        .upload(TOKEN, "notes.txt", "text/plain", b"not a PDF".to_vec())
        .await;
    assert_eq!(
        code(&release(&world, &number, Some(&text)).await),
        "REFUSED"
    );

    let released = release(&world, &number, Some(&first_id)).await;
    assert_eq!(
        released["data"]["releaseVersion"],
        json!({ "number": 1, "revision": 2 }),
        "{released}"
    );
    // The living version holds the latest version's PDF.
    let akt = world
        .ok(
            TOKEN,
            "query($n: String!) { akt(number: $n) { fields { ... on AttachmentValue { field attachment { id } } } } }",
            json!({ "n": number }),
        )
        .await;
    let held = akt["akt"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == "pdf")
        .unwrap()["attachment"]["id"]
        .clone();
    assert_eq!(held, first_id);

    // Released again, with no other change: a new revision, the new PDF.
    let second = b"%PDF-1.4 second".to_vec();
    let second_id = world
        .upload(TOKEN, "policy-2.pdf", "application/pdf", second.clone())
        .await;
    let released = release(&world, &number, Some(&second_id)).await;
    assert_eq!(
        released["data"]["releaseVersion"],
        json!({ "number": 2, "revision": 3 }),
        "{released}"
    );

    let latest = pdf(&world, &format!("/akter/{number}/pdf")).await;
    assert_eq!(latest.status, 200);
    assert_eq!(latest.headers["content-type"], "application/pdf");
    assert_eq!(
        latest.headers["content-disposition"],
        format!("inline; filename=\"{number}-v2.pdf\"")
    );
    assert_eq!(latest.bytes, second);
    let earlier = pdf(&world, &format!("/akter/{number}/pdf?version=1&download")).await;
    assert_eq!(earlier.bytes, first);
    assert_eq!(
        earlier.headers["content-disposition"],
        format!("attachment; filename=\"{number}-v1.pdf\"")
    );
    assert_eq!(
        pdf(&world, &format!("/akter/{number}/pdf?version=3"))
            .await
            .status,
        404
    );

    // Retired, it is still released so.
    let revision = revision(&world, &number).await;
    world
        .ok(
            TOKEN,
            "mutation($n: String!, $r: Int!) { performTransition(input: { number: $n, revision: $r, transition: \"retire\" }) { number } }",
            json!({ "n": number, "r": revision }),
        )
        .await;
    let third = world
        .upload(
            TOKEN,
            "policy-3.pdf",
            "application/pdf",
            b"%PDF-1.4 third".to_vec(),
        )
        .await;
    let released = release(&world, &number, Some(&third)).await;
    assert_eq!(
        released["data"]["releaseVersion"]["number"], 3,
        "{released}"
    );
}

// r[verify version.release-file]
#[tokio::test(flavor = "multi_thread")]
async fn a_file_is_refused_for_a_type_not_released_as_files() {
    let world = World::new(FakeDecider::allowing_tenant()).await;
    let mut plain = external(&json!({}));
    plain["fields"].as_array_mut().unwrap().pop();
    world.define(TOKEN, &plain).await;
    let number = world
        .create(
            TOKEN,
            "external_document",
            &[],
            json!({ "title": "T" }),
            "T",
        )
        .await;
    let file = world
        .upload(TOKEN, "a.pdf", "application/pdf", b"%PDF".to_vec())
        .await;
    assert_eq!(
        code(&release(&world, &number, Some(&file)).await),
        "BAD_REQUEST"
    );
}
