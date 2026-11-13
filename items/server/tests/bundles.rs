//! Bundles: exporting from one database and importing into another, and
//! into another tenant of the same one (`bundle.*`).

mod support;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use http_body_util::BodyExt;
use kuria_server::bundles::archive;
use kuria_store::people::{self, Person};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use support::decider::FakeDecider;
use support::{COLLEAGUE, FakeHost, SUBJECT, graphql_with, router_storing, send};

/// A Kuria with a database of its own.
struct Kuria {
    db: TestDatabase,
    router: Router,
}

impl Kuria {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.allow_everything("other-tenant");
        decider.allow_everything("colleague");
        let router = router_storing(
            &FakeHost::new(),
            db.pool().clone(),
            decider,
            kuria_server::attachments::Storage::in_memory(1 << 20),
        );
        Self { db, router }
    }

    async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn raw(&self, request: Request<Body>) -> (StatusCode, Vec<u8>) {
        let mut request = request;
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        (
            status,
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
    }

    async fn export(&self, token: &str, query: &str) -> (StatusCode, Vec<u8>) {
        self.raw(
            Request::get(format!("/export?{query}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Exports `query`, which must succeed.
    async fn exported(&self, token: &str, query: &str) -> Vec<u8> {
        let (status, body) = self.export(token, query).await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        body
    }

    async fn import(&self, token: &str, archive: Vec<u8>) -> (StatusCode, Value) {
        let (status, body) = self
            .raw(
                Request::post("/import")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/x-tar")
                    .body(Body::from(archive))
                    .unwrap(),
            )
            .await;
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    async fn define(&self, token: &str, document: Value) {
        self.ok(
            token,
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": document }),
        )
        .await;
    }

    async fn create(&self, token: &str, key: &str, fields: Value, inside: &[&str]) -> String {
        self.ok(
            token,
            "mutation($t: String!, $f: JSON, $in: [String!]!) { createAkt(input: { type: $t, fields: $f, in: $in }) { number } }",
            json!({ "t": key, "f": fields, "in": inside }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn upload(&self, body: &[u8]) -> String {
        let answer = send(
            &self.router,
            Request::post("/attachments")
                .header("authorization", "Bearer tenant")
                .header("x-attachment-name", "notes.txt")
                .header("content-type", "text/plain")
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await;
        assert_eq!(answer.status, 201, "{}", answer.body);
        answer.json()["id"].as_str().unwrap().to_owned()
    }

    async fn record_person(&self, subject: &str, name: &str, email: &str, signed_in: bool) {
        let mut conn = self.db.pool().get().await.unwrap();
        let subject: Uuid = subject.parse().unwrap();
        people::record(
            &mut conn,
            &Person {
                subject_id: subject,
                name: Some(name.to_owned()),
                email: Some(email.to_owned()),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        if signed_in {
            people::signed_in_to(
                &mut conn,
                support::TENANT.parse().unwrap(),
                subject,
                Utc::now(),
            )
            .await
            .unwrap();
        }
    }

    /// Everything GraphQL says of the tenant's akter, contacts and types.
    async fn everything(&self, token: &str) -> Value {
        self.ok(
            token,
            "{
              aktTypes { key version akterCount }
              contacts { id displayName email user { id } }
              akter(first: 200) { nodes {
                number title revision rank createdAt updatedAt state { key }
                type { key version }
                fields { __typename ... on UserValue { user { ... on User { id name } ... on Contact { id displayName } } }
                         ... on AttachmentValue { attachment { id name sha256 size } } ... on TextValue { text } }
                containers { number } members { nodes { number } }
                links { kind name outgoing pinnedRevision akt { number } }
                revisions { revision author by { name } transition reason changes membership state hash createdAt }
              } }
            }",
            json!({}),
        )
        .await
    }
}

fn plan() -> Value {
    json!({
        "key": "plan", "name": "Plan", "number": "PLAN-{seq:3}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "task" }]
    })
}

fn task() -> Value {
    json!({
        "key": "task", "name": "Task", "number": "{parent}-T{seq:2}",
        "created_in": ["plan"],
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "notes", "name": "Notes", "kind": "text", "multiline": true },
            { "key": "owner", "name": "Owner", "kind": "user" },
            { "key": "reviewer", "name": "Reviewer", "kind": "user" },
            { "key": "file", "name": "File", "kind": "attachment" }
        ],
        "states": [
            { "key": "todo", "name": "To do", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["todo"], "to": "done" }],
        "links": [{ "key": "blocks", "name": "Blocks", "inverse": "Blocked by", "to": ["task"] }]
    })
}

fn note() -> Value {
    json!({
        "key": "note", "name": "Note", "number": "NOTE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }]
    })
}

/// A tenant with plans and tasks, edited, transitioned, linked, with a
/// contact and an attachment; answers the tasks' numbers.
async fn seeded(kuria: &Kuria) -> (String, String, String) {
    kuria
        .record_person(SUBJECT, "Ada", "ada@shop.example", true)
        .await;
    kuria.define("tenant", plan()).await;
    kuria.define("tenant", task()).await;
    let plan = kuria
        .create("tenant", "plan", json!({ "title": "Autumn" }), &[])
        .await;
    let first = kuria
        .create("tenant", "task", json!({ "title": "First" }), &[&plan])
        .await;
    let second = kuria
        .create("tenant", "task", json!({ "title": "Second" }), &[&plan])
        .await;
    let contact = kuria
        .ok(
            "tenant",
            "mutation { addContact(input: { name: \"Auditor\", email: \"audit@x.example\" }) { id } }",
            json!({}),
        )
        .await["addContact"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let file = kuria.upload(b"what we found").await;
    kuria
        .ok(
            "tenant",
            "mutation($n: String!, $f: JSON!) { editAkt(input: { number: $n, revision: 1, fields: $f, reason: \"Assigned\" }) { number } }",
            json!({ "n": first, "f": { "owner": SUBJECT, "reviewer": { "contact": contact }, "file": file, "notes": "Line one\n\nLine two" } }),
        )
        .await;
    kuria
        .ok(
            "tenant",
            "mutation($n: String!) { performTransition(input: { number: $n, revision: 2, transition: \"finish\" }) { number } }",
            json!({ "n": first }),
        )
        .await;
    kuria
        .ok(
            "tenant",
            "mutation($s: String!, $t: String!) { addLink(input: { source: $s, kind: \"blocks\", target: $t, pinRevision: 1 }) { number } }",
            json!({ "s": second, "t": first }),
        )
        .await;
    (plan, first, second)
}

fn code(answer: &Value) -> &str {
    answer["error"]["code"].as_str().unwrap_or_default()
}

// r[verify bundle.format]
// r[verify bundle.contents]
// r[verify bundle.export.tenant]
// r[verify bundle.import.tenant]
// r[verify bundle.import.history]
// r[verify bundle.import.counters]
// r[verify bundle.import.atomic]
// r[verify authz.bundles]
#[tokio::test(flavor = "multi_thread")]
async fn a_whole_tenant_moves_to_another_kuria_as_it_was() {
    let source = Kuria::new().await;
    let (plan, first, _) = seeded(&source).await;
    let target = Kuria::new().await;

    // Only those who may.
    assert_eq!(
        source.export("tenant-nobody", "tenant=true").await.0,
        StatusCode::FORBIDDEN
    );
    let archive = source.exported("tenant", "tenant=true").await;
    let unpacked = archive::unpack(&archive).unwrap();
    assert_eq!(unpacked.attachments.len(), 1);
    let bundle: Value = serde_json::from_slice(&unpacked.bundle).unwrap();
    assert_eq!(bundle["format"], 1);
    assert_eq!(bundle["scope"], "tenant");
    assert_eq!(bundle["people"][0]["email"], "ada@shop.example");
    assert_eq!(bundle["signedIn"][0]["subject"], SUBJECT);

    assert_eq!(
        target.import("tenant-nobody", archive.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, imported) = target.import("tenant", archive.clone()).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(
        imported,
        json!({ "scope": "tenant", "aktTypes": 2, "akter": 3, "aliases": 0 })
    );

    // It reads the same: numbers, history, hashes, links, contacts and
    // attachments.
    assert_eq!(
        target.everything("tenant").await,
        source.everything("tenant").await
    );
    let held = target.everything("tenant").await["akter"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["number"] == first.as_str())
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["__typename"] == "AttachmentValue")
        .unwrap()["attachment"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, bytes) = target
        .raw(
            Request::get(format!("/attachments/{held}"))
                .header("authorization", "Bearer tenant")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        (status, bytes.as_slice()),
        (StatusCode::OK, b"what we found".as_slice())
    );

    // Numbering goes on after the imported akter.
    let next = target
        .create("tenant", "task", json!({ "title": "Third" }), &[&plan])
        .await;
    assert_eq!(next, format!("{plan}-T03"));
    assert_eq!(
        target
            .create("tenant", "plan", json!({ "title": "Winter" }), &[])
            .await,
        "PLAN-002"
    );

    // A whole tenant goes only into an empty one.
    let (status, again) = target.import("tenant", archive).await;
    assert_eq!((status, code(&again)), (StatusCode::CONFLICT, "CONFLICT"));
}

// r[verify bundle.export.types]
// r[verify bundle.import.clashes]
// r[verify bundle.import.order]
// r[verify bundle.import.ids]
// r[verify bundle.import.recorded]
#[tokio::test(flavor = "multi_thread")]
async fn selected_types_move_between_tenants_without_clashing() {
    let kuria = Kuria::new().await;
    seeded(&kuria).await;
    // Tasks are in plans: a selection without them is refused.
    let (status, body) = kuria.export("tenant", "types=task&akter=true").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let refused: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(refused["error"]["code"], "INCOMPLETE_SELECTION");
    assert_eq!(refused["error"]["missing"], json!(["plan"]));
    assert_eq!(
        kuria.export("tenant", "types=nope").await.0,
        StatusCode::NOT_FOUND
    );

    // Notes move on their own, after the other tenant's own akter.
    kuria.define("tenant", note()).await;
    for title in ["One", "Two"] {
        kuria
            .create("tenant", "note", json!({ "title": title }), &[])
            .await;
    }
    kuria.define("other-tenant", plan()).await;
    let own = kuria
        .create("other-tenant", "plan", json!({ "title": "Mine" }), &[])
        .await;
    let archive = kuria.exported("tenant", "types=note&akter=true").await;
    let (status, imported) = kuria.import("other-tenant", archive.clone()).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["akter"], 2);
    let listed = kuria
        .ok(
            "other-tenant",
            "{ akter(first: 10) { nodes { number rank } } }",
            json!({}),
        )
        .await;
    let mut ranked: Vec<(String, String)> = listed["akter"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            (
                n["rank"].as_str().unwrap().to_owned(),
                n["number"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    ranked.sort();
    let numbers: Vec<&str> = ranked.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(numbers, [own.as_str(), "NOTE-1", "NOTE-2"]);
    assert_eq!(
        kuria
            .create("other-tenant", "note", json!({ "title": "Three" }), &[])
            .await,
        "NOTE-3"
    );
    // The other tenant's own changes see the import.
    let mut conn = kuria.db.pool().get().await.unwrap();
    let events = kuria_store::akter::change_events_after(
        &mut conn,
        support::OTHER_TENANT.parse().unwrap(),
        0,
        100,
    )
    .await
    .unwrap();
    assert!(events.len() >= 3, "{events:?}");
    drop(conn);

    // Again, its key and numbers clash.
    let (status, again) = kuria.import("other-tenant", archive).await;
    assert_eq!((status, code(&again)), (StatusCode::CONFLICT, "CONFLICT"));
    assert!(
        again["error"]["message"].as_str().unwrap().contains("note"),
        "{again}"
    );
    // So do contacts and attachments another tenant of this Kuria holds.
    let whole = kuria.exported("tenant", "types=plan,task&akter=true").await;
    let (status, clash) = kuria.import("other-tenant", whole).await;
    assert_eq!((status, code(&clash)), (StatusCode::CONFLICT, "CONFLICT"));
}

// r[verify bundle.import.clashes]
#[tokio::test(flavor = "multi_thread")]
async fn types_without_akter_are_defined_as_new_versions() {
    let kuria = Kuria::new().await;
    kuria.define("tenant", note()).await;
    kuria
        .create("tenant", "note", json!({ "title": "Kept here" }), &[])
        .await;
    let archive = kuria.exported("tenant", "types=note").await;
    let bundle: Value = serde_json::from_slice(&archive::unpack(&archive).unwrap().bundle).unwrap();
    assert_eq!(bundle["holdsAkter"], false);
    assert_eq!(bundle["akter"], json!([]));
    for version in [1, 2] {
        let (status, imported) = kuria.import("other-tenant", archive.clone()).await;
        assert_eq!(status, StatusCode::OK, "{imported}");
        let types = kuria
            .ok(
                "other-tenant",
                "{ aktTypes { key version akterCount } }",
                json!({}),
            )
            .await;
        assert_eq!(
            types["aktTypes"],
            json!([{ "key": "note", "version": version, "akterCount": 0 }])
        );
    }
}

/// `archive` with its `bundle.json` changed by `change`, and `subjects`.
fn changed(archive: &[u8], change: impl FnOnce(&mut Value), subjects: Option<Value>) -> Vec<u8> {
    let unpacked = archive::unpack(archive).unwrap();
    let mut bundle: Value = serde_json::from_slice(&unpacked.bundle).unwrap();
    change(&mut bundle);
    let files: Vec<(String, Vec<u8>)> = unpacked.attachments.into_iter().collect();
    let packed = archive::pack(&serde_json::to_vec(&bundle).unwrap(), &files).unwrap();
    let Some(subjects) = subjects else {
        return packed;
    };
    let mut builder = tar::Builder::new(Vec::new());
    let mut entries = tar::Archive::new(packed.as_slice());
    for entry in entries.entries().unwrap() {
        let mut entry = entry.unwrap();
        let mut header = entry.header().clone();
        let path = entry.path().unwrap().into_owned();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        builder
            .append_data(&mut header, path, bytes.as_slice())
            .unwrap();
    }
    let bytes = serde_json::to_vec(&subjects).unwrap();
    let mut header = tar::Header::new_ustar();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    builder
        .append_data(&mut header, "subjects.json", bytes.as_slice())
        .unwrap();
    builder.into_inner().unwrap()
}

// r[verify bundle.import.format]
// r[verify bundle.import.hashes]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_bundle_it_can_t_trust_before_writing() {
    let source = Kuria::new().await;
    seeded(&source).await;
    let archive = source.exported("tenant", "tenant=true").await;
    let target = Kuria::new().await;
    for (why, broken) in [
        (
            "an unknown format",
            changed(&archive, |b| b["format"] = json!(99), None),
        ),
        (
            "a revision's hash",
            changed(
                &archive,
                |b| b["akter"][1]["revisions"][0]["hash"] = json!("00".repeat(32)),
                None,
            ),
        ),
        (
            "a revision's changes",
            changed(
                &archive,
                |b| b["akter"][1]["revisions"][0]["changes"]["title"] = json!("Forged"),
                None,
            ),
        ),
        ("an attachment's bytes", {
            let unpacked = archive::unpack(&archive).unwrap();
            archive::pack(&unpacked.bundle, &[]).unwrap()
        }),
        ("not a tar", b"nonsense".to_vec()),
    ] {
        let (status, refused) = target.import("tenant", broken).await;
        assert_eq!(
            (status, code(&refused)),
            (StatusCode::BAD_REQUEST, "INVALID_BUNDLE"),
            "{why}: {refused}"
        );
    }
    // Nothing was written.
    assert_eq!(
        target
            .ok("tenant", "{ aktTypes { key } contacts { id } }", json!({}))
            .await,
        json!({ "aktTypes": [], "contacts": [] })
    );
}

// r[verify bundle.import.subjects]
// r[verify bundle.import.request]
#[tokio::test(flavor = "multi_thread")]
async fn subjects_are_shown_and_filter_as_the_users_they_map_to() {
    let source = Kuria::new().await;
    let (_, first, _) = seeded(&source).await;
    let archive = source.exported("tenant", "tenant=true").await;
    let assigned = "{ akter(type: \"task\", where: [{ field: \"owner\", equals: \"me\" }]) { nodes { number } } }";

    // By email: Ada signed in to the target as another subject.
    let target = Kuria::new().await;
    target
        .record_person(COLLEAGUE, "Ada Lovelace", "ADA@shop.example", true)
        .await;
    let (status, imported) = target.import("tenant", archive.clone()).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["aliases"], 1);
    let mine = target.ok("colleague", assigned, json!({})).await;
    assert_eq!(mine["akter"]["nodes"], json!([{ "number": first }]));
    let shown = target
        .ok(
            "colleague",
            "query($n: String!) { akt(number: $n) { revisions { author by { name } } fields { ... on UserValue { user { ... on User { name } } } } } }",
            json!({ "n": first }),
        )
        .await;
    // Revisions still name the subject they recorded, shown as Ada here.
    assert_eq!(shown["akt"]["revisions"][0]["author"], SUBJECT);
    assert_eq!(shown["akt"]["revisions"][0]["by"]["name"], "Ada Lovelace");

    // By a mapping the import is given.
    let mapped = Kuria::new().await;
    let with_subjects = changed(&archive, |_| {}, Some(json!({ SUBJECT: COLLEAGUE })));
    let (status, imported) = mapped.import("tenant", with_subjects).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["aliases"], 1);
    assert_eq!(
        mapped.ok("colleague", assigned, json!({})).await["akter"]["nodes"],
        json!([{ "number": first }])
    );
}
