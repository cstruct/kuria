//! Serving the built app, the login page's proxy to Konklav, and
//! discovering the provider from Konklav directly.

mod support;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use base64::Engine;
use kuria_server::oidc::{self, Provider};
use kuria_server::web::{LoginProxy, Web};
use serde_json::json;
use sha2::{Digest, Sha256};

use support::{FakeHost, PUBLIC_URL, send};

const SCRIPT: &str = "__sveltekit_start()";

/// A built app in a fresh directory.
fn built_app() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kuria-web-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(dir.join("_app/immutable")).unwrap();
    std::fs::write(
        dir.join("index.html"),
        format!("<!doctype html><script type=\"module\">{SCRIPT}</script><script src=\"/x.js\"></script>"),
    )
    .unwrap();
    std::fs::write(dir.join("_app/immutable/start.abc123.js"), "export {}").unwrap();
    std::fs::write(dir.join("favicon.svg"), "<svg/>").unwrap();
    dir
}

fn router(web: Option<Web>, login_proxy: Option<LoginProxy>) -> Router {
    kuria_server::app(
        FakeHost::new(),
        kuria_server::Options {
            public_url: PUBLIC_URL.to_owned(),
            trusted_proxies: Vec::new(),
            pool: support::unused_pool(),
            decider: support::decider::FakeDecider::allowing_tenant(),
            sessions: None,
            changes: None,
            storage: kuria_server::attachments::Storage::in_memory(1 << 20),
            web,
            login_proxy,
            cli_relay: None,
            renderer: support::renderer(),
        },
    )
    .unwrap()
}

fn get_request(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

// r[verify app.serve]
// r[verify app.csp]
// r[verify login.csp]
#[tokio::test(flavor = "multi_thread")]
async fn serves_the_app_and_its_fallback_page() {
    let router = router(Some(Web::load(&built_app()).unwrap()), None);

    for path in ["/", "/akt/RELEASE-2026-0001", "/login?login_request=x"] {
        let page = send(&router, get_request(path)).await;
        assert_eq!(page.status, StatusCode::OK, "{path}");
        assert!(page.body.contains(SCRIPT), "{path}");
        assert_eq!(page.headers[header::CACHE_CONTROL], "no-store", "{path}");
        let csp = page.headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(SCRIPT));
        assert!(
            csp.contains(&format!("script-src 'self' 'sha256-{hash}';")),
            "{csp}"
        );
        assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
        // Only the route announcer's style attribute, by its hash.
        assert!(
            csp.contains("style-src-attr 'unsafe-hashes' 'sha256-S8qMpvofolR8Mpjy4kQvEm7m1q8clzU4dfDH0AmvZjo='"),
            "{csp}"
        );
    }

    let asset = send(&router, get_request("/_app/immutable/start.abc123.js")).await;
    assert_eq!(asset.status, StatusCode::OK);
    assert_eq!(asset.body, "export {}");
    assert_eq!(
        asset.headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    let icon = send(&router, get_request("/favicon.svg")).await;
    assert_eq!(icon.headers[header::CACHE_CONTROL], "no-cache");

    // The server's own routes stay its own.
    assert_eq!(send(&router, get_request("/healthz")).await.body, "ok");
    let unauthenticated = send(
        &router,
        Request::post("/graphql")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert_eq!(unauthenticated.status, StatusCode::UNAUTHORIZED);
    // Only reads load the app.
    let posted = send(
        &router,
        Request::post("/akt/1").body(Body::empty()).unwrap(),
    )
    .await;
    assert_ne!(posted.status, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_built_app_there_is_no_fallback() {
    let router = router(None, None);
    assert_eq!(
        send(&router, get_request("/akt/1")).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(Web::load(&std::env::temp_dir().join("kuria-no-such-app")).is_err());
}

/// What the stand-in for Konklav received.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<(String, HeaderMap, String)>>>);

/// Serves `router` on a local port and answers its base URL.
async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    format!("http://{address}")
}

// r[verify login.proxy]
#[tokio::test(flavor = "multi_thread")]
async fn passes_the_login_pages_requests_to_konklav() {
    let seen = Seen::default();
    let record = |seen: Seen| {
        move |request: Request<Body>| {
            let seen = seen.clone();
            async move {
                let (parts, body) = request.into_parts();
                let body = axum::body::to_bytes(body, 1 << 16).await.unwrap();
                let path = parts.uri.path_and_query().unwrap().to_string();
                seen.0.lock().unwrap().push((
                    path,
                    parts.headers,
                    String::from_utf8(body.to_vec()).unwrap(),
                ));
            }
        }
    };
    let konklav = Router::new()
        .route(
            "/v1/login/password",
            post({
                let record = record(seen.clone());
                move |request: Request<Body>| async move {
                    record(request).await;
                    (
                        [(header::SET_COOKIE, "konklav_refresh=r; HttpOnly; Path=/v1")],
                        axum::Json(json!({ "access_token": "a" })),
                    )
                        .into_response()
                }
            }),
        )
        .route(
            "/v1/oauth2/authorize",
            get(|| async {
                (
                    StatusCode::SEE_OTHER,
                    [(header::LOCATION, "/login?login_request=1")],
                )
                    .into_response()
            }),
        )
        .route(
            "/.well-known/openid-configuration",
            get(|| async { axum::Json(json!({ "issuer": PUBLIC_URL })) }),
        );
    let konklav = serve(konklav).await;
    let router = router(None, Some(LoginProxy::new(&konklav).unwrap()));

    let signed_in = send(
        &router,
        Request::post("/v1/login/password?x=1")
            .header("content-type", "application/json")
            .header("cookie", "__Host-kuria_session=secret; konklav_refresh=old")
            .header("dpop", "proof")
            .body(Body::from(r#"{"identifier":"ada@example.com"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(signed_in.status, StatusCode::OK);
    assert_eq!(signed_in.json()["access_token"], "a");
    assert_eq!(
        signed_in.headers[header::SET_COOKIE],
        "konklav_refresh=r; HttpOnly; Path=/v1"
    );
    let (path, headers, body) = seen.0.lock().unwrap()[0].clone();
    assert_eq!(path, "/v1/login/password?x=1");
    assert_eq!(body, r#"{"identifier":"ada@example.com"}"#);
    assert_eq!(headers["cookie"], "konklav_refresh=old");
    assert_eq!(headers["dpop"], "proof");
    assert_eq!(headers["x-forwarded-for"], "127.0.0.1");

    // Redirects go back to the browser.
    let redirected = send(&router, get_request("/v1/oauth2/authorize?client_id=kuria")).await;
    assert_eq!(redirected.status, StatusCode::SEE_OTHER);
    assert_eq!(
        redirected.headers[header::LOCATION],
        "/login?login_request=1"
    );
    let discovery = send(&router, get_request("/.well-known/openid-configuration")).await;
    assert_eq!(discovery.json()["issuer"], PUBLIC_URL);
}

// r[verify login.discovery]
#[tokio::test(flavor = "multi_thread")]
async fn discovers_the_provider_from_konklav_directly() {
    // Konklav whose issuer is Kuria's origin: its endpoints name that
    // origin, and the server calls them on Konklav instead.
    let konklav = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(|| async {
                axum::Json(json!({
                    "issuer": PUBLIC_URL,
                    "authorization_endpoint": format!("{PUBLIC_URL}/v1/oauth2/authorize"),
                    "token_endpoint": format!("{PUBLIC_URL}/v1/oauth2/token"),
                    "userinfo_endpoint": format!("{PUBLIC_URL}/v1/oauth2/userinfo"),
                }))
            }),
        )
        .route(
            "/v1/oauth2/userinfo",
            get(|| async { axum::Json(json!({ "sub": "s", "name": "Ada" })) }),
        );
    let konklav = serve(konklav).await;
    let client = || oidc::Client {
        id: "kuria".to_owned(),
        secret: "s".to_owned(),
        redirect_uri: format!("{PUBLIC_URL}/auth/callback"),
    };
    let provider = Provider::discover_at(&konklav, PUBLIC_URL, client())
        .await
        .unwrap();
    assert_eq!(provider.issuer(), PUBLIC_URL);
    // Browsers still go to the issuer's origin.
    assert!(
        provider
            .authorization_url("st", "ch", "openid")
            .starts_with(&format!("{PUBLIC_URL}/v1/oauth2/authorize?"))
    );
    // The server reaches Konklav itself.
    assert_eq!(
        provider.userinfo("t").await.unwrap().name.as_deref(),
        Some("Ada")
    );
    // An issuer other than the configured one is still refused.
    assert!(
        Provider::discover_at(&konklav, "https://elsewhere.example", client())
            .await
            .is_err()
    );
}
