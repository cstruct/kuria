//! Uploading, downloading and holding attachments, against a fresh database
//! for each test, a fake relying host and object storage in memory.

mod support;

use std::fmt::Write as _;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use futures_util::TryStreamExt;
use kuria_server::attachments::Storage;
use kuria_test_support::TestDatabase;
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::decider::FakeDecider;
use support::{Answer, FakeHost, graphql_with, router_storing, send};

const LIMIT: usize = 1024;

struct World {
    _db: TestDatabase,
    router: Router,
    store: Arc<InMemory>,
    decider: Arc<FakeDecider>,
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let store = Arc::new(InMemory::new());
        let decider = FakeDecider::allowing_tenant();
        let router = router_storing(
            &FakeHost::new(),
            db.pool().clone(),
            decider.clone(),
            Storage::new(store.clone(), LIMIT),
        );
        let world = Self {
            _db: db,
            router,
            store,
            decider,
        };
        world
            .data(
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": evidence() }),
            )
            .await;
        world
    }

    async fn upload(&self, token: &str, name: &str, media_type: &str, body: &[u8]) -> Answer {
        let request = Request::post("/attachments")
            .header("authorization", format!("Bearer {token}"))
            .header("x-attachment-name", name)
            .header("content-type", media_type)
            .body(Body::from(body.to_vec()))
            .unwrap();
        send(&self.router, request).await
    }

    /// Uploads `body` as the main subject, and returns its identifier.
    async fn uploaded(&self, name: &str, body: &[u8]) -> String {
        let answer = self.upload("tenant", name, "application/json", body).await;
        assert_eq!(answer.status, 201, "{}", answer.body);
        answer.json()["id"].as_str().unwrap().to_owned()
    }

    async fn download(&self, token: &str, id: &str) -> Answer {
        let request = Request::get(format!("/attachments/{id}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        send(&self.router, request).await
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with("tenant", query, &variables)).await;
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()
    }

    async fn data(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, fields: Value) -> Value {
        self.run(
            "mutation($f: JSON) { createAkt(input: { type: \"evidence\", fields: $f }) {
                number revision
                fields {
                    ... on AttachmentValue {
                        field
                        attachment { id url name mediaType size sha256 uploadedBy { id }
                                     sbom { format specVersion name components } }
                    }
                }
            } }",
            json!({ "f": fields }),
        )
        .await
    }

    async fn stored_objects(&self) -> usize {
        self.store
            .list(None)
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .len()
    }
}

fn evidence() -> Value {
    json!({
        "key": "evidence",
        "name": "Evidence",
        "number": "EV-{seq:4}",
        "fields": [
            { "key": "file", "name": "File", "kind": "attachment" },
            { "key": "sbom", "name": "SBOM", "kind": "attachment", "format": "sbom" }
        ],
        "states": [{ "key": "open", "name": "Open", "initial": true }]
    })
}

fn spdx() -> Vec<u8> {
    json!({
        "spdxVersion": "SPDX-2.3", "name": "backend-4.2.0",
        "packages": [{ "name": "serde" }, { "name": "tokio" }]
    })
    .to_string()
    .into_bytes()
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

// r[verify attachment.upload]
// r[verify attachment.metadata]
// r[verify attachment.download]
#[tokio::test(flavor = "multi_thread")]
async fn an_upload_is_answered_and_downloaded_by_its_uploader() {
    let world = World::new().await;
    let answer = world
        .upload(
            "tenant",
            "r%C3%A4ksm%C3%B6rg%C3%A5s.txt",
            "text/plain",
            b"hello",
        )
        .await;
    assert_eq!(answer.status, 201, "{}", answer.body);
    let uploaded = answer.json();
    assert_eq!(uploaded["name"], "räksmörgås.txt");
    assert_eq!(uploaded["mediaType"], "text/plain");
    assert_eq!(uploaded["size"], 5);
    let sha256 = Sha256::digest(b"hello")
        .iter()
        .fold(String::new(), |mut hex, b| {
            write!(hex, "{b:02x}").unwrap();
            hex
        });
    assert_eq!(uploaded["sha256"], sha256);
    assert_eq!(uploaded["sbom"], Value::Null);

    let download = world
        .download("tenant", uploaded["id"].as_str().unwrap())
        .await;
    assert_eq!(download.status, 200);
    assert_eq!(download.body, "hello");
    assert_eq!(download.headers["content-type"], "text/plain");
    assert_eq!(
        download.headers["content-disposition"],
        "attachment; filename*=UTF-8''r%C3%A4ksm%C3%B6rg%C3%A5s%2Etxt"
    );
    assert_eq!(download.headers["x-content-type-options"], "nosniff");
}

// r[verify attachment.upload]
// r[verify authz.sender-constrained]
#[tokio::test(flavor = "multi_thread")]
async fn an_upload_is_authenticated_and_named() {
    let world = World::new().await;
    // A DPoP-bound token is decided with its accepted presentation
    // (`authz.sender-constrained`).
    for (token, status) in [("invalid", 401), ("identity", 403), ("tenant-dpop", 201)] {
        let answer = world.upload(token, "a.txt", "text/plain", b"a").await;
        assert_eq!(answer.status, status, "{token}: {}", answer.body);
    }
    let unnamed = Request::post("/attachments")
        .header("authorization", "Bearer tenant")
        .body(Body::from("a"))
        .unwrap();
    assert_eq!(send(&world.router, unnamed).await.status, 400);
    assert_eq!(
        world
            .upload("tenant", "a%2Fb", "text/plain", b"a")
            .await
            .status,
        400
    );
    // Only the DPoP-bound upload was stored.
    assert_eq!(world.stored_objects().await, 1);
}

// r[verify attachment.limit]
#[tokio::test(flavor = "multi_thread")]
async fn an_upload_over_the_limit_is_refused_and_not_stored() {
    let world = World::new().await;
    let answer = world
        .upload(
            "tenant",
            "big.bin",
            "application/octet-stream",
            &[0; LIMIT + 1],
        )
        .await;
    assert_eq!(answer.status, 413);
    assert_eq!(world.stored_objects().await, 0);
    let answer = world
        .upload(
            "tenant",
            "fits.bin",
            "application/octet-stream",
            &[0; LIMIT],
        )
        .await;
    assert_eq!(answer.status, 201);
}

// r[verify attachment.immutable]
#[tokio::test(flavor = "multi_thread")]
async fn the_same_file_is_stored_once_per_tenant() {
    let world = World::new().await;
    let first = world.uploaded("a.json", b"{}").await;
    let second = world.uploaded("b.json", b"{}").await;
    assert_ne!(first, second);
    assert_eq!(world.stored_objects().await, 1);
    let other = world
        .upload("other-tenant", "c.json", "application/json", b"{}")
        .await;
    assert_eq!(other.status, 201);
    assert_eq!(world.stored_objects().await, 2);
    // Each keeps its own name.
    let download = world.download("tenant", &second).await;
    assert_eq!(
        download.headers["content-disposition"],
        "attachment; filename*=UTF-8''b%2Ejson"
    );
}

// r[verify attachment.sbom]
#[tokio::test(flavor = "multi_thread")]
async fn an_sbom_is_summarised_at_upload() {
    let world = World::new().await;
    let answer = world
        .upload("tenant", "sbom.spdx.json", "application/spdx+json", &spdx())
        .await;
    assert_eq!(answer.status, 201);
    assert_eq!(
        answer.json()["sbom"],
        json!({ "format": "spdx", "spec_version": "SPDX-2.3", "name": "backend-4.2.0", "components": 2 })
    );
}

// r[verify attachment.download]
#[tokio::test(flavor = "multi_thread")]
async fn an_unheld_attachment_is_only_the_uploaders() {
    let world = World::new().await;
    let id = world.uploaded("a.json", b"{}").await;
    world
        .decider
        .grant("colleague", &["kuria:akter:read", "kuria:akter:create"]);
    assert_eq!(world.download("colleague", &id).await.status, 404);
    assert_eq!(world.download("other-tenant", &id).await.status, 404);
    assert_eq!(world.download("tenant", "not-a-uuid").await.status, 404);
    assert_eq!(
        world
            .download("tenant", "01900000-0000-7000-8000-000000000999")
            .await
            .status,
        404
    );
}

// r[verify attachment.download]
// r[verify attachment.references]
// r[verify akt.field.attachment]
#[tokio::test(flavor = "multi_thread")]
async fn a_held_attachment_is_read_by_whoever_reads_its_akt() {
    let world = World::new().await;
    let id = world.uploaded("a.json", b"{}").await;
    let created = world.create(json!({ "file": id })).await;
    assert!(created.get("errors").is_none(), "{created}");
    let value = &created["data"]["createAkt"]["fields"][0];
    assert_eq!(value["field"], "file");
    assert_eq!(value["attachment"]["name"], "a.json");
    assert_eq!(value["attachment"]["url"], format!("/attachments/{id}"));
    assert_eq!(value["attachment"]["size"], 2);

    world
        .decider
        .grant_at("colleague", "evidence", &["kuria:akter:read"]);
    assert_eq!(world.download("colleague", &id).await.status, 200);
    assert_eq!(world.download("colleague-stranger", &id).await.status, 404);

    // Once no akt holds it, it is the uploader's alone again.
    let number = created["data"]["createAkt"]["number"].as_str().unwrap();
    world
        .data(
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { file: null } }) { revision } }",
            json!({ "n": number }),
        )
        .await;
    assert_eq!(world.download("colleague", &id).await.status, 404);
    assert_eq!(world.download("tenant", &id).await.status, 200);
}

// r[verify akt.field.attachment]
#[tokio::test(flavor = "multi_thread")]
async fn a_field_holds_only_attachments_the_caller_may_read() {
    let world = World::new().await;
    let unknown = world
        .create(json!({ "file": "01900000-0000-7000-8000-000000000999" }))
        .await;
    assert_eq!(code(&unknown), "REFUSED", "{unknown}");
    let foreign = world
        .upload("other-tenant", "x.json", "application/json", b"[]")
        .await;
    let foreign = world.create(json!({ "file": foreign.json()["id"] })).await;
    assert_eq!(code(&foreign), "REFUSED", "{foreign}");

    // A colleague's upload that no akt holds yet isn't the caller's to take.
    let theirs = world
        .upload("colleague", "y.json", "application/json", b"[1]")
        .await;
    assert_eq!(theirs.status, 201);
    let taken = world.create(json!({ "file": theirs.json()["id"] })).await;
    assert_eq!(code(&taken), "REFUSED", "{taken}");
    assert_eq!(
        world.create(json!({ "file": "nope" })).await["errors"][0]["extensions"]["code"],
        "REFUSED"
    );
}

// r[verify akt.field.attachment]
#[tokio::test(flavor = "multi_thread")]
async fn an_sbom_field_holds_only_sboms() {
    let world = World::new().await;
    let plain = world.uploaded("notes.json", b"{}").await;
    let refused = world.create(json!({ "sbom": plain })).await;
    assert_eq!(code(&refused), "REFUSED", "{refused}");

    let sbom = world.uploaded("sbom.spdx.json", &spdx()).await;
    let created = world.create(json!({ "sbom": sbom })).await;
    assert!(created.get("errors").is_none(), "{created}");
    assert_eq!(
        created["data"]["createAkt"]["fields"][0]["attachment"]["sbom"],
        json!({ "format": "SPDX", "specVersion": "SPDX-2.3", "name": "backend-4.2.0", "components": 2 })
    );
}
