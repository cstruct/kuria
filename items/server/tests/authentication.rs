//! Authentication of requests through Konklav, against a fake relying host.

mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;

use support::{FakeHost, OTHER_TENANT, PUBLIC_URL, SUBJECT, TENANT, graphql, router, send};

const VIEWER: &str = "{ viewer { subject tenant } }";

// r[verify auth.graphql.unauthenticated]
#[tokio::test]
async fn refuses_a_request_without_a_token() {
    let answer = send(&router(&FakeHost::new()), graphql(None, VIEWER)).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert!(answer.headers.contains_key(header::WWW_AUTHENTICATE));
}

// r[verify auth.graphql.unauthenticated]
#[tokio::test]
async fn refuses_an_invalid_token() {
    let answer = send(&router(&FakeHost::new()), graphql(Some("invalid"), VIEWER)).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert!(answer.headers.contains_key(header::WWW_AUTHENTICATE));
}

// r[verify auth.graphql.unauthenticated]
#[tokio::test]
async fn refuses_an_invalid_dpop_proof() {
    let answer = send(
        &router(&FakeHost::new()),
        graphql(Some("bad-proof"), VIEWER),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert!(answer.headers.contains_key(header::WWW_AUTHENTICATE));
}

// r[verify auth.graphql.tenant-token]
#[tokio::test]
async fn refuses_an_identity_token() {
    let answer = send(&router(&FakeHost::new()), graphql(Some("identity"), VIEWER)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
}

// r[verify auth.unavailable]
#[tokio::test]
async fn refuses_requests_while_konklav_is_unreachable() {
    let answer = send(
        &router(&FakeHost::new()),
        graphql(Some("unreachable"), VIEWER),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
}

// r[verify auth.relying-system]
// r[verify auth.viewer]
#[tokio::test]
async fn answers_the_viewer_from_the_verified_token() {
    let host = FakeHost::new();
    let answer = send(&router(&host), graphql(Some("tenant"), VIEWER)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json(),
        json!({ "data": { "viewer": { "subject": SUBJECT, "tenant": TENANT } } })
    );
    assert_eq!(
        host.accepted().len(),
        1,
        "verified through the relying host"
    );
}

// r[verify auth.tenant]
#[tokio::test]
async fn takes_the_tenant_from_the_token_only() {
    let mut request = graphql(Some("tenant"), VIEWER);
    request
        .headers_mut()
        .insert("x-tenant", OTHER_TENANT.parse().unwrap());
    let answer = send(&router(&FakeHost::new()), request).await;
    assert_eq!(answer.json()["data"]["viewer"]["tenant"], json!(TENANT));
}

// r[verify auth.dpop]
#[tokio::test]
async fn checks_proofs_against_the_public_url() {
    let host = FakeHost::new();
    send(&router(&host), graphql(Some("tenant"), VIEWER)).await;
    let accepted = host.accepted();
    let request = accepted[0].request.as_ref().expect("a request descriptor");
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, format!("{PUBLIC_URL}/graphql"));
}

// r[verify auth.health]
#[tokio::test]
async fn answers_health_checks_without_a_token() {
    let host = FakeHost::new();
    let answer = send(
        &router(&host),
        Request::get("/healthz").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, "ok");
    assert!(host.accepted().is_empty(), "no token was read");
}
