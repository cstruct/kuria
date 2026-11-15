//! Browser sessions, against a fake Konklav OpenID provider, a fake relying
//! host, and a fresh database for each test.

mod support;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use konklav_relying_client::rate_limit::{RateLimit, RateLimitWindow};
use kuria_server::Options;
use kuria_server::oidc::{self, Provider};
use kuria_server::session::{self, BrowserSessions, Cookies, SIGN_IN_WINDOWS, SessionCipher};
use kuria_store::schema::browser_session;
use kuria_test_support::{TestDatabase, unique_name, valkey_url};
use serde_json::json;
use sha2::{Digest, Sha256};

use support::konklav::{CLIENT_ID, CLIENT_SECRET, FakeProvider};
use support::{Answer, FakeHost, PUBLIC_URL, TENANT, send};

const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
const RATE_LIMIT_SECRET: &str = "0123456789abcdef0123456789abcdef";
const SESSION: &str = "__Host-kuria_session";
const LOGIN: &str = "__Host-kuria_login";
const VIEWER: &str = "{ viewer { subject tenant } }";

/// A server with browser sessions, and what it talks to.
struct World {
    db: TestDatabase,
    konklav: FakeProvider,
    router: Router,
}

/// How a test's server limits sign-ins.
struct Limits {
    /// Servers with the same name share the limit.
    name: String,
    windows: Vec<RateLimitWindow>,
    trusted_proxies: Vec<String>,
    /// The tenant a sign-in that names none is for.
    default_tenant: Option<uuid::Uuid>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            name: unique_name("test"),
            windows: SIGN_IN_WINDOWS.to_vec(),
            trusted_proxies: Vec::new(),
            default_tenant: None,
        }
    }
}

async fn sign_in_limit(limits: &Limits) -> RateLimit {
    let valkey = redis::Client::open(valkey_url())
        .unwrap()
        .get_connection_manager()
        .await
        .expect("reaching Valkey (is `mise run db:up` running?)");
    session::sign_in_limit(
        &limits.name,
        limits.windows.clone(),
        valkey,
        RATE_LIMIT_SECRET,
    )
    .unwrap()
}

impl World {
    async fn new() -> Self {
        Self::with(&Limits::default()).await
    }

    async fn with(limits: &Limits) -> Self {
        let db = TestDatabase::new().await;
        let konklav = FakeProvider::start().await;
        let provider = Provider::discover(
            &konklav.issuer,
            oidc::Client {
                id: CLIENT_ID.to_owned(),
                secret: CLIENT_SECRET.to_owned(),
                redirect_uri: format!("{PUBLIC_URL}/auth/callback"),
            },
        )
        .await
        .unwrap();
        let sessions = Arc::new(BrowserSessions {
            pool: db.pool().clone(),
            provider,
            cipher: SessionCipher::from_base64(KEY).unwrap(),
            cookies: Cookies::new(true),
            allowed_origins: vec![PUBLIC_URL.to_owned()],
            sign_in_limit: sign_in_limit(limits).await,
            default_tenant: limits.default_tenant,
        });
        let router = kuria_server::app(
            FakeHost::new(),
            Options {
                public_url: PUBLIC_URL.to_owned(),
                trusted_proxies: limits.trusted_proxies.clone(),
                pool: db.pool().clone(),
                decider: support::decider::FakeDecider::allowing_tenant(),
                sessions: Some(sessions),
                changes: None,
                storage: kuria_server::attachments::Storage::in_memory(1 << 20),
                web: None,
                login_proxy: None,
                cli_relay: None,
                renderer: support::renderer(),
            },
        )
        .unwrap();
        Self {
            db,
            konklav,
            router,
        }
    }

    async fn send(&self, request: Request<Body>) -> Answer {
        send(&self.router, request).await
    }

    /// Starts a sign-in, and returns the login cookie and the authorization
    /// request's parameters.
    async fn start(&self, query: &str) -> (Answer, String, HashMap<String, String>) {
        let answer = self
            .send(
                Request::get(format!("/auth/login{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER);
        let location = answer.headers[header::LOCATION]
            .to_str()
            .unwrap()
            .to_owned();
        let params = url::Url::parse(&location)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        let login = cookie_value(&answer, LOGIN).expect("a login cookie");
        (answer, login, params)
    }

    /// Completes a sign-in for an access token, sending `cookies`.
    async fn complete(
        &self,
        params: &HashMap<String, String>,
        cookies: &str,
        access_token: &str,
    ) -> Answer {
        self.konklav
            .issue_code("code-1", &params["code_challenge"], access_token);
        let uri = format!(
            "/auth/callback?code=code-1&state={}&iss={}",
            params["state"],
            url::form_urlencoded::byte_serialize(self.konklav.issuer.as_bytes())
                .collect::<String>()
        );
        self.send(
            Request::get(uri)
                .header(header::COOKIE, cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Signs in and returns the session cookie.
    async fn sign_in(&self) -> String {
        let (_, login, params) = self.start(&format!("?tenant={TENANT}")).await;
        let answer = self
            .complete(&params, &format!("{LOGIN}={login}"), "tenant-1")
            .await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
        cookie_value(&answer, SESSION).expect("a session cookie")
    }

    async fn sessions(&self) -> i64 {
        let mut conn = self.db.pool().get().await.unwrap();
        browser_session::table
            .count()
            .get_result(&mut conn)
            .await
            .unwrap()
    }

    async fn expire_access_tokens(&self) {
        let mut conn = self.db.pool().get().await.unwrap();
        diesel::update(browser_session::table)
            .set(browser_session::access_expires_at.eq(Utc::now()))
            .execute(&mut conn)
            .await
            .unwrap();
    }
}

/// The value a `Set-Cookie` header of the answer gives `name`.
fn cookie_value(answer: &Answer, name: &str) -> Option<String> {
    set_cookie(answer, name)?
        .split(';')
        .next()?
        .split_once('=')
        .map(|(_, value)| value.to_owned())
}

/// The whole `Set-Cookie` header of the answer for `name`.
fn set_cookie(answer: &Answer, name: &str) -> Option<String> {
    answer
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with(&format!("{name}=")))
        .map(str::to_owned)
}

/// A GraphQL request from the browser, with its session cookie and origin.
fn graphql(cookie: &str, origin: Option<&str>) -> Request<Body> {
    let mut request = Request::post("/graphql")
        .header(header::COOKIE, format!("{SESSION}={cookie}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    request
        .body(Body::from(json!({ "query": VIEWER }).to_string()))
        .unwrap()
}

// r[verify session.login.start]
#[tokio::test(flavor = "multi_thread")]
async fn starts_a_sign_in_at_konklav() {
    let world = World::new().await;
    let (answer, login, params) = world
        .start(&format!("?tenant={TENANT}&return_to=/akt/1"))
        .await;
    let location = answer.headers[header::LOCATION].to_str().unwrap();
    assert!(location.starts_with(&format!("{}/authorize?", world.konklav.issuer)));
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["client_id"], CLIENT_ID);
    assert_eq!(
        params["redirect_uri"],
        format!("{PUBLIC_URL}/auth/callback")
    );
    assert_eq!(
        params["scope"],
        format!("openid profile email tenant:{TENANT}")
    );
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["state"], login);
    assert!(login.len() >= 43, "32 random bytes");

    let set = set_cookie(&answer, LOGIN).unwrap();
    for attribute in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Max-Age=600",
        "Path=/",
    ] {
        assert!(set.contains(attribute), "{set} lacks {attribute}");
    }
}

// r[verify session.login.start]
#[tokio::test(flavor = "multi_thread")]
async fn asks_for_no_tenant_unless_named() {
    let world = World::new().await;
    let (_, _, params) = world.start("").await;
    assert_eq!(params["scope"], "openid profile email");
}

// r[verify session.login.default-tenant]
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_naming_no_tenant_is_for_the_default_one() {
    let other = uuid::Uuid::now_v7();
    let world = World::with(&Limits {
        default_tenant: Some(TENANT.parse().unwrap()),
        ..Limits::default()
    })
    .await;
    let (_, _, params) = world.start("").await;
    assert_eq!(
        params["scope"],
        format!("openid profile email tenant:{TENANT}")
    );
    // A named tenant still wins.
    let (_, _, params) = world.start(&format!("?tenant={other}")).await;
    assert_eq!(
        params["scope"],
        format!("openid profile email tenant:{other}")
    );
}

// r[verify session.login.callback]
// r[verify session.cookie]
// r[verify session.request]
#[tokio::test(flavor = "multi_thread")]
async fn signs_in_and_authenticates_with_the_cookie() {
    let world = World::new().await;
    let (_, login, params) = world
        .start(&format!("?tenant={TENANT}&return_to=/akt/1"))
        .await;
    let answer = world
        .complete(&params, &format!("{LOGIN}={login}"), "tenant-1")
        .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(answer.headers[header::LOCATION], "/akt/1");
    let set = set_cookie(&answer, SESSION).unwrap();
    for attribute in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/"] {
        assert!(set.contains(attribute), "{set} lacks {attribute}");
    }
    assert!(!set.contains("Domain"));
    assert!(set_cookie(&answer, LOGIN).unwrap().contains("Max-Age=0"));

    let cookie = cookie_value(&answer, SESSION).unwrap();
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["data"]["viewer"]["tenant"], json!(TENANT));
}

// r[verify session.login.callback]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_state_from_another_browser() {
    let world = World::new().await;
    let (_, _, params) = world.start("").await;
    let answer = world
        .complete(&params, &format!("{LOGIN}=someone-elses"), "tenant-1")
        .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(cookie_value(&answer, SESSION).is_none());
    assert_eq!(world.sessions().await, 0);
}

// r[verify session.login.callback]
#[tokio::test(flavor = "multi_thread")]
async fn uses_an_attempt_once() {
    let world = World::new().await;
    let (_, login, params) = world.start("").await;
    let cookies = format!("{LOGIN}={login}");
    assert_eq!(
        world.complete(&params, &cookies, "tenant-1").await.status,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        world.complete(&params, &cookies, "tenant-1").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(world.sessions().await, 1);
}

// r[verify session.login.callback]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_another_issuer() {
    let world = World::new().await;
    let (_, login, params) = world.start("").await;
    world
        .konklav
        .issue_code("code-1", &params["code_challenge"], "tenant-1");
    let answer = world
        .send(
            Request::get(format!(
                "/auth/callback?code=code-1&state={}&iss=https%3A%2F%2Fevil.example",
                params["state"]
            ))
            .header(header::COOKIE, format!("{LOGIN}={login}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(world.sessions().await, 0);
}

// r[verify session.login.callback]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_token_that_is_not_a_tenant_token() {
    let world = World::new().await;
    let (_, login, params) = world.start("").await;
    for token in ["identity-1", "invalid"] {
        let answer = world
            .complete(&params, &format!("{LOGIN}={login}"), token)
            .await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{token}");
    }
    assert_eq!(world.sessions().await, 0);
}

// r[verify session.storage]
#[tokio::test(flavor = "multi_thread")]
async fn stores_the_cookie_hashed_and_the_tokens_encrypted() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    let mut conn = world.db.pool().get().await.unwrap();
    let (id_hash, access, refresh): (Vec<u8>, Vec<u8>, Vec<u8>) = browser_session::table
        .select((
            browser_session::id_hash,
            browser_session::access_token,
            browser_session::refresh_token,
        ))
        .first(&mut conn)
        .await
        .unwrap();
    assert_eq!(id_hash, Sha256::digest(cookie.as_bytes()).to_vec());
    assert!(!access.windows(8).any(|w| w == b"tenant-1"));
    assert!(!refresh.windows(9).any(|w| w == b"refresh-1"));
}

// r[verify session.login.replace]
#[tokio::test(flavor = "multi_thread")]
async fn a_new_sign_in_ends_the_previous_session() {
    let world = World::new().await;
    let previous = world.sign_in().await;
    let (_, login, params) = world.start("").await;
    let answer = world
        .complete(
            &params,
            &format!("{LOGIN}={login}; {SESSION}={previous}"),
            "tenant-1",
        )
        .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    assert_eq!(world.sessions().await, 1);
    let answer = world.send(graphql(&previous, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
}

// r[verify session.request]
#[tokio::test(flavor = "multi_thread")]
async fn an_authorization_header_wins_over_the_cookie() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    let mut request = graphql(&cookie, Some(PUBLIC_URL));
    request
        .headers_mut()
        .insert(header::AUTHORIZATION, "Bearer identity".parse().unwrap());
    assert_eq!(world.send(request).await.status, StatusCode::FORBIDDEN);
}

// r[verify session.unknown]
#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_cookie_is_no_token_and_is_cleared() {
    let world = World::new().await;
    let answer = world
        .send(graphql("no-such-session", Some(PUBLIC_URL)))
        .await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert!(set_cookie(&answer, SESSION).unwrap().contains("Max-Age=0"));
}

// r[verify session.origin]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_cookie_requests_from_other_origins() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    for origin in [None, Some("https://evil.example")] {
        let answer = world.send(graphql(&cookie, origin)).await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{origin:?}");
    }
    // A WebSocket upgrade is a GET, but must name an allowed origin too.
    let upgrade = Request::get("/graphql")
        .header(header::COOKIE, format!("{SESSION}={cookie}"))
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "upgrade")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(world.send(upgrade).await.status, StatusCode::FORBIDDEN);
}

// r[verify session.graphql-json]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_graphql_that_is_not_json() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    let request = Request::post("/graphql")
        .header(header::COOKIE, format!("{SESSION}={cookie}"))
        .header(header::ORIGIN, PUBLIC_URL)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(json!({ "query": VIEWER }).to_string()))
        .unwrap();
    assert_eq!(
        world.send(request).await.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

// r[verify session.refresh]
#[tokio::test(flavor = "multi_thread")]
async fn refreshes_an_expiring_token_and_keeps_the_rotated_one() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    world.expire_access_tokens().await;
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(world.konklav.refreshes(), 1);

    // The rotated refresh token is the one used next time.
    world.expire_access_tokens().await;
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(world.konklav.refreshes(), 2);
}

// r[verify session.refresh]
#[tokio::test(flavor = "multi_thread")]
async fn refreshes_a_session_once_for_concurrent_requests() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    world.expire_access_tokens().await;
    world.konklav.set_refresh_delay(Duration::from_millis(200));
    let answers = futures_join(&world, &cookie, 5).await;
    for answer in answers {
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    }
    assert_eq!(world.konklav.refreshes(), 1);
}

async fn futures_join(world: &World, cookie: &str, n: usize) -> Vec<Answer> {
    let mut tasks = Vec::new();
    for _ in 0..n {
        let router = world.router.clone();
        let request = graphql(cookie, Some(PUBLIC_URL));
        tasks.push(tokio::spawn(async move { send(&router, request).await }));
    }
    let mut answers = Vec::new();
    for task in tasks {
        answers.push(task.await.unwrap());
    }
    answers
}

// r[verify session.refresh.failure]
#[tokio::test(flavor = "multi_thread")]
async fn ends_the_session_when_konklav_refuses_the_refresh() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    world.expire_access_tokens().await;
    world.konklav.end_session();
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    assert!(set_cookie(&answer, SESSION).unwrap().contains("Max-Age=0"));
    assert_eq!(world.sessions().await, 0);
}

// r[verify session.refresh.failure]
#[tokio::test(flavor = "multi_thread")]
async fn keeps_the_session_while_konklav_is_unreachable() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    world.expire_access_tokens().await;
    world.konklav.set_failing(true);
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(world.sessions().await, 1);

    world.konklav.set_failing(false);
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

// r[verify session.logout]
#[tokio::test(flavor = "multi_thread")]
async fn signs_out_here_and_at_konklav() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    let answer = world
        .send(
            Request::post("/auth/logout")
                .header(header::COOKIE, format!("{SESSION}={cookie}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT);
    assert!(set_cookie(&answer, SESSION).unwrap().contains("Max-Age=0"));
    assert_eq!(world.konklav.revoked(), vec!["refresh-1".to_owned()]);
    assert_eq!(world.sessions().await, 0);
    let answer = world.send(graphql(&cookie, Some(PUBLIC_URL))).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
}

// r[verify session.origin]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_sign_out_from_another_origin() {
    let world = World::new().await;
    let cookie = world.sign_in().await;
    let answer = world
        .send(
            Request::post("/auth/logout")
                .header(header::COOKIE, format!("{SESSION}={cookie}"))
                .header(header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(world.sessions().await, 1);
}

/// A request to start a sign-in, through a proxy that says it is for
/// `forwarded_for`.
fn start_request(forwarded_for: Option<&str>) -> Request<Body> {
    let mut request = Request::get("/auth/login");
    if let Some(address) = forwarded_for {
        request = request.header("x-forwarded-for", address);
    }
    request.body(Body::empty()).unwrap()
}

fn two_a_minute() -> Vec<RateLimitWindow> {
    vec![RateLimitWindow {
        size: 2,
        period_ms: 60_000,
    }]
}

// r[verify session.login.rate-limit]
#[tokio::test(flavor = "multi_thread")]
async fn limits_sign_ins_per_client_address() {
    let world = World::with(&Limits {
        windows: two_a_minute(),
        ..Limits::default()
    })
    .await;
    for _ in 0..2 {
        assert_eq!(
            world.send(start_request(None)).await.status,
            StatusCode::SEE_OTHER
        );
    }
    let answer = world.send(start_request(None)).await;
    assert_eq!(answer.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(answer.headers.contains_key(header::RETRY_AFTER));
}

// r[verify session.login.rate-limit]
#[tokio::test(flavor = "multi_thread")]
async fn shares_the_sign_in_limit_between_instances() {
    let limits = Limits {
        windows: two_a_minute(),
        ..Limits::default()
    };
    let first = World::with(&limits).await;
    let second = World::with(&limits).await;
    assert_eq!(
        first.send(start_request(None)).await.status,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        second.send(start_request(None)).await.status,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        first.send(start_request(None)).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
}

// r[verify session.login.rate-limit]
#[tokio::test(flavor = "multi_thread")]
async fn a_client_cannot_name_its_own_address() {
    let world = World::with(&Limits {
        windows: two_a_minute(),
        ..Limits::default()
    })
    .await;
    for address in ["192.0.2.1", "192.0.2.2"] {
        assert_eq!(
            world.send(start_request(Some(address))).await.status,
            StatusCode::SEE_OTHER
        );
    }
    let answer = world.send(start_request(Some("192.0.2.3"))).await;
    assert_eq!(answer.status, StatusCode::TOO_MANY_REQUESTS);
}

// r[verify session.login.rate-limit]
#[tokio::test(flavor = "multi_thread")]
async fn a_trusted_proxy_names_the_client() {
    let world = World::with(&Limits {
        windows: two_a_minute(),
        // The tests' requests come from 127.0.0.1.
        trusted_proxies: vec!["127.0.0.1".to_owned()],
        ..Limits::default()
    })
    .await;
    for _ in 0..2 {
        assert_eq!(
            world.send(start_request(Some("192.0.2.1"))).await.status,
            StatusCode::SEE_OTHER
        );
    }
    assert_eq!(
        world.send(start_request(Some("192.0.2.1"))).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        world.send(start_request(Some("192.0.2.2"))).await.status,
        StatusCode::SEE_OTHER
    );
}

async fn person(world: &World) -> Option<(Option<String>, Option<String>)> {
    use kuria_store::schema::person;
    let mut conn = world.db.pool().get().await.unwrap();
    person::table
        .select((person::name, person::email))
        .first(&mut conn)
        .await
        .optional()
        .unwrap()
}

// r[verify person.from-sign-in]
#[tokio::test(flavor = "multi_thread")]
async fn records_the_signed_in_persons_name() {
    let world = World::new().await;
    assert_eq!(person(&world).await, None);
    world.sign_in().await;
    assert_eq!(
        person(&world).await,
        Some((
            Some("Alice Example".to_owned()),
            Some("alice@example.com".to_owned())
        ))
    );
}

// r[verify person.from-sign-in]
// r[verify person.tenant]
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_without_a_name_still_succeeds() {
    let world = World::new().await;
    world.konklav.set_userinfo_failing(true);
    world.sign_in().await;
    assert_eq!(person(&world).await, None);
    // The tenant signed in to is recorded all the same.
    let mut conn = world.db.pool().get().await.unwrap();
    assert!(
        kuria_store::people::is_in(
            &mut conn,
            TENANT.parse().unwrap(),
            support::SUBJECT.parse().unwrap()
        )
        .await
        .unwrap()
    );
    assert!(
        !kuria_store::people::is_in(
            &mut conn,
            support::OTHER_TENANT.parse().unwrap(),
            support::SUBJECT.parse().unwrap()
        )
        .await
        .unwrap()
    );
}

/// Adds a contact with `email` to `tenant`, linked to `linked` if given,
/// and returns its id.
async fn add_contact(
    world: &World,
    tenant: &str,
    email: &str,
    linked: Option<uuid::Uuid>,
) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    let mut conn = world.db.pool().get().await.unwrap();
    kuria_store::contacts::insert(
        &mut conn,
        &kuria_store::contacts::Contact {
            id,
            tenant_id: tenant.parse().unwrap(),
            name: Some("Alice".to_owned()),
            email: Some(email.to_owned()),
            linked_subject: linked,
            linked_at: linked.map(|_| Utc::now()),
            created_by: uuid::Uuid::nil(),
            linked_by: None,
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    id
}

async fn linked_to(world: &World, id: uuid::Uuid) -> Option<uuid::Uuid> {
    use kuria_store::schema::contact;
    let mut conn = world.db.pool().get().await.unwrap();
    contact::table
        .find(id)
        .select(contact::linked_subject)
        .first(&mut conn)
        .await
        .unwrap()
}

// r[verify contact.link]
#[tokio::test(flavor = "multi_thread")]
async fn a_verified_email_links_the_tenants_contact() {
    let world = World::new().await;
    let mine = add_contact(&world, TENANT, "Alice@Example.com", None).await;
    let elsewhere = add_contact(&world, support::OTHER_TENANT, "alice@example.com", None).await;
    world.konklav.set_email("alice@example.com", Some(true));
    world.sign_in().await;
    let alice: uuid::Uuid = support::SUBJECT.parse().unwrap();
    assert_eq!(linked_to(&world, mine).await, Some(alice));
    // Only in the tenant signed in to.
    assert_eq!(linked_to(&world, elsewhere).await, None);
}

// r[verify contact.link]
#[tokio::test(flavor = "multi_thread")]
async fn an_unverified_email_links_nothing() {
    for verified in [Some(false), None] {
        let world = World::new().await;
        let contact = add_contact(&world, TENANT, "alice@example.com", None).await;
        world.konklav.set_email("alice@example.com", verified);
        world.sign_in().await;
        assert_eq!(linked_to(&world, contact).await, None, "{verified:?}");
    }
}

// r[verify contact.link]
#[tokio::test(flavor = "multi_thread")]
async fn a_linked_contact_stays_linked() {
    let world = World::new().await;
    let someone = uuid::Uuid::now_v7();
    let contact = add_contact(&world, TENANT, "alice@example.com", Some(someone)).await;
    world.konklav.set_email("alice@example.com", Some(true));
    world.sign_in().await;
    assert_eq!(linked_to(&world, contact).await, Some(someone));
}
