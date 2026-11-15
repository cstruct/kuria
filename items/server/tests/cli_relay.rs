//! The CLI's login relay, against the local Valkey.

mod support;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use konklav_relying_client::rate_limit::RateLimitWindow;
use kuria_server::cli_relay::{CliRelay, Discovery, HOLD, RELAY_WINDOWS};
use kuria_server::{Options, session};
use kuria_test_support::{unique_name, valkey_url};
use serde_json::{Value, json};

use support::{Answer, FakeHost, PUBLIC_URL, send};

const RATE_LIMIT_SECRET: &str = "0123456789abcdef0123456789abcdef";

async fn router_with(windows: &[RateLimitWindow], hold: Duration) -> Router {
    let valkey = redis::Client::open(valkey_url())
        .unwrap()
        .get_connection_manager()
        .await
        .expect("reaching Valkey (is `mise run db:up` running?)");
    let relay = CliRelay {
        limit: session::sign_in_limit(
            &unique_name("cli-relay"),
            windows.to_vec(),
            valkey.clone(),
            RATE_LIMIT_SECRET,
        )
        .unwrap(),
        valkey,
        hold,
        discovery: Discovery::new(PUBLIC_URL, "https://konklav.example", None),
    };
    kuria_server::app(
        FakeHost::new(),
        Options {
            public_url: PUBLIC_URL.to_owned(),
            trusted_proxies: Vec::new(),
            pool: support::unused_pool(),
            decider: support::decider::FakeDecider::allowing_tenant(),
            sessions: None,
            changes: None,
            storage: kuria_server::attachments::Storage::in_memory(1 << 20),
            web: None,
            login_proxy: None,
            cli_relay: Some(Arc::new(relay)),
            renderer: support::renderer(),
        },
    )
    .unwrap()
}

async fn router() -> Router {
    router_with(&RELAY_WINDOWS, HOLD).await
}

/// A `state` no other test uses.
fn state() -> String {
    unique_name("state")
}

async fn redirect(router: &Router, query: &str) -> Answer {
    send(
        router,
        Request::get(format!("/cli/authorized?{query}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn collect(router: &Router, state: &str) -> Answer {
    send(
        router,
        Request::post("/cli/authorized/collect")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "state": state }).to_string()))
            .unwrap(),
    )
    .await
}

fn collected(answer: &Answer) -> Value {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_code_until_the_cli_collects_it_once() {
    let router = router().await;
    let state = state();
    assert_eq!(
        collect(&router, &state).await.status,
        StatusCode::NO_CONTENT
    );

    let page = redirect(&router, &format!("code=the-code&state={state}")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains("return to your terminal"),
        "{}",
        page.body
    );
    assert!(!page.body.contains("the-code"));
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
    ] {
        assert_eq!(page.headers[name], value);
    }

    let answer = collect(&router, &state).await;
    assert_eq!(answer.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(collected(&answer), json!({ "code": "the-code" }));
    assert_eq!(
        collect(&router, &state).await.status,
        StatusCode::NO_CONTENT
    );
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn keeps_an_error_for_the_cli_too() {
    let router = router().await;
    let state = state();
    let page = redirect(
        &router,
        &format!("error=access_denied&error_description=Not+today&state={state}"),
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains("<code>access_denied</code>"),
        "{}",
        page.body
    );
    assert_eq!(
        collected(&collect(&router, &state).await),
        json!({ "error": "access_denied", "error_description": "Not today" })
    );
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_first_redirect_for_a_state() {
    let router = router().await;
    let state = state();
    redirect(&router, &format!("code=first&state={state}")).await;
    let again = redirect(&router, &format!("code=second&state={state}")).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(
        collected(&collect(&router, &state).await),
        json!({ "code": "first" })
    );
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn forgets_a_code_nobody_collects() {
    let router = router_with(&RELAY_WINDOWS, Duration::from_millis(200)).await;
    let state = state();
    redirect(&router, &format!("code=late&state={state}")).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        collect(&router, &state).await.status,
        StatusCode::NO_CONTENT
    );
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_what_no_cli_or_authorization_server_sends() {
    let router = router().await;
    let state = state();
    for query in [
        "code=c".to_owned(),
        "code=c&state=short".to_owned(),
        format!("state={state}"),
        format!("error=%3Cscript%3E&state={state}"),
        format!("code=c&error=access_denied&state={state}"),
    ] {
        let page = redirect(&router, &query).await;
        assert_eq!(page.status, StatusCode::BAD_REQUEST, "{query}");
        assert!(!page.body.contains("<script>"));
    }
    assert_eq!(
        collect(&router, "short").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        collect(&router, &state).await.status,
        StatusCode::NO_CONTENT
    );
}

// r[verify cli.relay]
#[tokio::test(flavor = "multi_thread")]
async fn limits_the_relay_per_client_address() {
    let two_a_minute = [RateLimitWindow {
        size: 2,
        period_ms: 60_000,
    }];
    let router = router_with(&two_a_minute, HOLD).await;
    let state = state();
    redirect(&router, &format!("code=c&state={state}")).await;
    assert_eq!(collect(&router, &state).await.status, StatusCode::OK);
    assert_eq!(
        collect(&router, &state).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

// r[verify cli.discovery]
#[tokio::test(flavor = "multi_thread")]
async fn tells_the_cli_where_to_sign_in() {
    let router = router().await;
    let answer = send(
        &router,
        Request::get("/cli/config").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json(),
        json!({
            "issuer": "https://konklav.example",
            "client_id": "kuria-cli",
            "redirect_uri": format!("{PUBLIC_URL}/cli/authorized"),
            "tenant": null
        })
    );
}
