//! A Kuria with a fresh database of its own, driven through its router:
//! GraphQL, uploads and bundles, for tests that make data to export or
//! import.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use kuria_server::attachments::Storage;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use super::decider::FakeDecider;
use super::{FakeHost, graphql_with, router_storing, send};

pub struct World {
    pub db: TestDatabase,
    pub router: Router,
    pub storage: Storage,
}

impl World {
    /// A fresh database, deciding through `decider`.
    pub async fn new(decider: Arc<FakeDecider>) -> Self {
        let db = TestDatabase::new().await;
        let storage = Storage::in_memory(1 << 24);
        let router = router_storing(
            &FakeHost::new(),
            db.pool().clone(),
            decider,
            storage.clone(),
        );
        Self {
            db,
            router,
            storage,
        }
    }

    pub async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    pub async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    pub async fn define(&self, token: &str, document: &Value) {
        self.ok(
            token,
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": document }),
        )
        .await;
    }

    /// Creates an akt of `key` in `inside`, answering its number.
    pub async fn create(
        &self,
        token: &str,
        key: &str,
        inside: &[&str],
        fields: Value,
        reason: &str,
    ) -> String {
        self.ok(
            token,
            "mutation($t: String!, $in: [String!]!, $f: JSON, $r: String) { createAkt(input: { type: $t, in: $in, fields: $f, reason: $r }) { number } }",
            json!({ "t": key, "in": inside, "f": fields, "r": reason }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// `number`'s latest revision.
    pub async fn revision(&self, token: &str, number: &str) -> i64 {
        self.ok(
            token,
            "query($n: String!) { akt(number: $n) { revision } }",
            json!({ "n": number }),
        )
        .await["akt"]["revision"]
            .as_i64()
            .unwrap()
    }

    pub async fn edit(&self, token: &str, number: &str, fields: Value, reason: &str) {
        let revision = self.revision(token, number).await;
        self.ok(
            token,
            "mutation($n: String!, $r: Int!, $f: JSON!, $why: String) { editAkt(input: { number: $n, revision: $r, fields: $f, reason: $why }) { number } }",
            json!({ "n": number, "r": revision, "f": fields, "why": reason }),
        )
        .await;
    }

    pub async fn transition(
        &self,
        token: &str,
        number: &str,
        transition: &str,
        fields: Option<Value>,
        reason: &str,
    ) {
        let revision = self.revision(token, number).await;
        self.ok(
            token,
            "mutation($n: String!, $r: Int!, $t: String!, $f: JSON, $why: String) { performTransition(input: { number: $n, revision: $r, transition: $t, fields: $f, reason: $why }) { number } }",
            json!({ "n": number, "r": revision, "t": transition, "f": fields, "why": reason }),
        )
        .await;
    }

    pub async fn link(&self, token: &str, source: &str, kind: &str, target: &str, reason: &str) {
        self.ok(
            token,
            "mutation($s: String!, $k: String!, $t: String!, $why: String) { addLink(input: { source: $s, kind: $k, target: $t, reason: $why }) { number } }",
            json!({ "s": source, "k": kind, "t": target, "why": reason }),
        )
        .await;
    }

    pub async fn add_to(&self, token: &str, container: &str, member: &str, reason: &str) {
        self.ok(
            token,
            "mutation($c: String!, $m: String!, $why: String) { addToContainer(input: { container: $c, member: $m, reason: $why }) { number } }",
            json!({ "c": container, "m": member, "why": reason }),
        )
        .await;
    }

    /// Names `number`'s latest revision as its next version.
    pub async fn release(&self, token: &str, number: &str, label: Option<&str>) {
        let revision = self.revision(token, number).await;
        self.ok(
            token,
            "mutation($n: String!, $r: Int!, $l: String) { releaseVersion(input: { number: $n, revision: $r, label: $l }) { number } }",
            json!({ "n": number, "r": revision, "l": label }),
        )
        .await;
    }

    /// Uploads `body` as `name`, answering the attachment's id.
    pub async fn upload(&self, token: &str, name: &str, media_type: &str, body: Vec<u8>) -> String {
        let answer = send(
            &self.router,
            Request::post("/attachments")
                .header("authorization", format!("Bearer {token}"))
                .header("x-attachment-name", name)
                .header("content-type", media_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await;
        assert_eq!(answer.status, 201, "{}", answer.body);
        answer.json()["id"].as_str().unwrap().to_owned()
    }

    /// `GET /export?<query>`, which must succeed: the bundle's tar archive.
    pub async fn export(&self, token: &str, query: &str) -> Vec<u8> {
        let answer = send(
            &self.router,
            Request::get(format!("/export?{query}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        answer.bytes
    }
}

/// Writes the bundle in `archive` to the new directory `dir`, as `kuria
/// export` would: `bundle.json`, indented, and `attachments/<sha256>`.
pub fn write_bundle(archive: &[u8], dir: &Path) {
    let unpacked = kuria_server::bundles::archive::unpack(archive).unwrap();
    if dir.exists() {
        std::fs::remove_dir_all(dir).unwrap();
    }
    std::fs::create_dir_all(dir).unwrap();
    let bundle: Value = serde_json::from_slice(&unpacked.bundle).unwrap();
    let mut text = serde_json::to_string_pretty(&bundle).unwrap();
    text.push('\n');
    std::fs::write(dir.join("bundle.json"), text).unwrap();
    if !unpacked.attachments.is_empty() {
        std::fs::create_dir_all(dir.join("attachments")).unwrap();
        for (sha256, bytes) in &unpacked.attachments {
            std::fs::write(dir.join("attachments").join(sha256), bytes).unwrap();
        }
    }
}

/// A new, empty directory under the test target's temporary directory.
pub fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("{name}-{}", uuid::Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
