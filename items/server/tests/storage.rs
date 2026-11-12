//! Attachments in S3-compatible object storage: the development gateway, as
//! mise configures it (`KURIA_S3_*`; start it with `mise run db:up`).

mod support;

use axum::body::Body;
use axum::http::Request;
use kuria_server::StorageConfig;
use kuria_server::attachments::Storage;
use kuria_test_support::TestDatabase;

use support::decider::FakeDecider;
use support::{FakeHost, router_storing, send};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("set {name} (mise sets it; start the gateway with `mise run db:up`)")
    })
}

// r[verify attachment.upload]
// r[verify attachment.download]
#[tokio::test(flavor = "multi_thread")]
async fn attachments_round_trip_through_s3() {
    let db = TestDatabase::new().await;
    let storage = Storage::s3(
        &StorageConfig {
            bucket: env("KURIA_S3_BUCKET"),
            endpoint: Some(env("KURIA_S3_ENDPOINT")),
            region: "us-east-1".to_owned(),
            credentials: Some((env("KURIA_S3_ACCESS_KEY"), env("KURIA_S3_SECRET_KEY"))),
        },
        1 << 20,
    )
    .unwrap();
    let router = router_storing(
        &FakeHost::new(),
        db.pool().clone(),
        FakeDecider::allowing_tenant(),
        storage,
    );
    // New bytes every run, so the object is written, not found.
    let body = format!("stored at {}", uuid::Uuid::now_v7());
    for _ in 0..2 {
        let upload = send(
            &router,
            Request::post("/attachments")
                .header("authorization", "Bearer tenant")
                .header("x-attachment-name", "log.txt")
                .header("content-type", "text/plain")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await;
        assert_eq!(upload.status, 201, "{}", upload.body);
        let id = upload.json()["id"].as_str().unwrap().to_owned();
        let download = send(
            &router,
            Request::get(format!("/attachments/{id}"))
                .header("authorization", "Bearer tenant")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(download.status, 200);
        assert_eq!(download.body, body);
    }
}
