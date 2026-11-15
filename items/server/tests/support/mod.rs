//! A relying host that stands in for Konklav, and helpers to send requests
//! through the router.

#![allow(dead_code)]

pub mod decider;
pub mod konklav;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use konklav_authz::{Decision, DenyReason, Requirements};
use konklav_axum::{Accepted, BoxFuture, RelyingHost};
use konklav_relying_client::{Error, Presented, Verified};
use konklav_token::AccessClaims;
use serde_json::{Value, json};
use tower::ServiceExt;

pub const TENANT: &str = "01900000-0000-7000-8000-000000000001";
pub const OTHER_TENANT: &str = "01900000-0000-7000-8000-000000000002";
pub const SUBJECT: &str = "01900000-0000-7000-8000-0000000000aa";
/// The subject of `colleague` tokens: another subject of `TENANT`.
pub const COLLEAGUE: &str = "01900000-0000-7000-8000-0000000000cc";
pub const PUBLIC_URL: &str = "https://kuria.example";

/// The claims of a token of `kind`, in `TENANT` if it is a tenant token.
pub fn claims(kind: &str) -> AccessClaims {
    claims_in(kind, TENANT)
}

/// The claims of a token of `kind`, in `tenant` if it is a tenant token.
pub fn claims_in(kind: &str, tenant: &str) -> AccessClaims {
    let mut claims = json!({
        "iss": "https://auth.example",
        "aud": "kuria",
        "sub": SUBJECT,
        "jti": "j-1",
        "iat": 0,
        "exp": i64::MAX,
        "knd": kind,
        "sid": "01900000-0000-7000-8000-0000000000bb",
        "sv": 1,
        "amr": ["pwd"],
        "auth_time": 0,
    });
    if kind == "tenant" {
        claims["tid"] = json!(tenant);
    }
    serde_json::from_value(claims).unwrap()
}

/// Stands in for a started `RelyingClient`. It accepts bearer tokens starting
/// with `tenant` or `identity`, with claims of that kind, `tenant-expiring`,
/// a tenant token that expires in two seconds, `tenant-dpop`, a
/// tenant token bound to a DPoP key, and `other-tenant`, a tenant token in
/// `OTHER_TENANT`; refuses `invalid`,
/// refuses the proof of `bad-proof`, fails for `unreachable`, and decides
/// by token kind alone.
#[derive(Default)]
pub struct FakeHost {
    pub accepted: Mutex<Vec<Presented>>,
    /// Decides what handlers ask of a request's presentation, by its token,
    /// once the router is built.
    decider: Mutex<Option<Arc<decider::FakeDecider>>>,
    /// How many presentations were released.
    released: Mutex<usize>,
}

impl FakeHost {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn accepted(&self) -> Vec<Presented> {
        self.accepted.lock().unwrap().clone()
    }

    pub fn released(&self) -> usize {
        *self.released.lock().unwrap()
    }

    fn token(presented: &Presented) -> Result<Verified<AccessClaims>, Error> {
        match presented.access_token().unwrap_or_default() {
            "tenant-expiring" => {
                let mut soon = claims("tenant");
                soon.exp = chrono::Utc::now().timestamp() + 2;
                Ok(Verified::Valid(soon))
            }
            "tenant-dpop" => {
                let mut bound = claims("tenant");
                bound.cnf = Some(konklav_token::Confirmation {
                    jkt: "thumbprint".to_owned(),
                });
                Ok(Verified::Valid(bound))
            }
            token if token.starts_with("tenant") => Ok(Verified::Valid(claims("tenant"))),
            token if token.starts_with("colleague") => {
                let mut colleague = claims("tenant");
                colleague.sub = COLLEAGUE.parse().unwrap();
                Ok(Verified::Valid(colleague))
            }
            "other-tenant" => Ok(Verified::Valid(claims_in("tenant", OTHER_TENANT))),
            token if token.starts_with("identity") => Ok(Verified::Valid(claims("identity"))),
            "bad-proof" => Ok(Verified::InvalidProof),
            "unreachable" => Err(Error::Exhausted),
            _ => Ok(Verified::Invalid),
        }
    }
}

impl RelyingHost for FakeHost {
    fn accept_request<'a>(
        &'a self,
        presented: &'a Presented,
    ) -> BoxFuture<'a, Result<Verified<Accepted>, Error>> {
        Box::pin(async move {
            self.accepted.lock().unwrap().push(presented.clone());
            let token = presented.access_token().unwrap_or_default().to_owned();
            Ok(match Self::token(presented)? {
                Verified::Valid(claims) => Verified::Valid(Accepted::new(claims, token)),
                Verified::Invalid => Verified::Invalid,
                Verified::InvalidProof => Verified::InvalidProof,
            })
        })
    }

    fn decide_accepted<'a>(
        &'a self,
        accepted: &'a Accepted,
        requirements: &'a Requirements,
    ) -> BoxFuture<'a, Result<Decision, Error>> {
        Box::pin(async move {
            // The relying core refuses a token of a kind the requirements
            // don't admit; nothing else is decided here.
            let kind_admitted = requirements
                .token_kinds
                .as_ref()
                .is_none_or(|kinds| kinds.contains(&accepted.claims().knd));
            if !kind_admitted {
                return Ok(Decision::Deny {
                    reason: DenyReason::TokenKind,
                });
            }
            // A handler's own decisions, by the token's grants.
            let decider = self.decider.lock().unwrap().clone();
            match (decider, accepted.handle::<String>()) {
                (Some(decider), Some(token))
                    if !requirements.tenant.is_empty() || !requirements.scopes.is_empty() =>
                {
                    decider
                        .decide_now(token, requirements)
                        .map_err(|_| Error::Exhausted)
                }
                _ => Ok(Decision::allow()),
            }
        })
    }

    fn verify_request<'a>(
        &'a self,
        presented: &'a Presented,
    ) -> BoxFuture<'a, Result<Verified<AccessClaims>, Error>> {
        Box::pin(async move { Self::token(presented) })
    }

    fn release_accepted(&self, _: &Accepted) {
        *self.released.lock().unwrap() += 1;
    }
}

/// The server's router, deciding through `host`.
pub fn router(host: &Arc<FakeHost>) -> Router {
    router_with(host, unused_pool(), decider::FakeDecider::allowing_tenant())
}

/// The server's router, verifying through `host`, deciding permissions
/// through `decider` and storing in `pool`.
pub fn router_with(
    host: &Arc<FakeHost>,
    pool: kuria_store::Pool,
    decider: Arc<decider::FakeDecider>,
) -> Router {
    router_storing(
        host,
        pool,
        decider,
        kuria_server::attachments::Storage::in_memory(1 << 20),
    )
}

/// Like [`router_with`], storing attachments in `storage`.
pub fn router_storing(
    host: &Arc<FakeHost>,
    pool: kuria_store::Pool,
    decider: Arc<decider::FakeDecider>,
    storage: kuria_server::attachments::Storage,
) -> Router {
    *host.decider.lock().unwrap() = Some(decider.clone());
    kuria_server::app(
        host.clone(),
        kuria_server::Options {
            public_url: PUBLIC_URL.to_owned(),
            trusted_proxies: Vec::new(),
            pool,
            decider,
            sessions: None,
            changes: None,
            storage,
            web: None,
            login_proxy: None,
            cli_relay: None,
            renderer: renderer(),
        },
    )
    .unwrap()
}

/// The server's own binary as the renderer, with the default limits.
pub fn renderer() -> kuria_typst::Renderer {
    kuria_typst::Renderer {
        program: env!("CARGO_BIN_EXE_kuria-server").into(),
        limits: kuria_typst::Limits::default(),
    }
}

/// A pool for tests that never touch the database: it connects only when a
/// connection is first checked out.
pub fn unused_pool() -> kuria_store::Pool {
    kuria_store::connect(&kuria_store::ConnectOptions {
        url: "postgres://unused@127.0.0.1:1/unused".to_owned(),
        max_connections: 1,
    })
    .unwrap()
}

/// An answer, read.
pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
    /// The body as it came, for one that isn't text.
    #[allow(dead_code, reason = "not every test reads binary answers")]
    pub bytes: Vec<u8>,
}

impl Answer {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

/// Sends `request` through `router`, from a client address.
pub async fn send(router: &Router, mut request: Request<Body>) -> Answer {
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Answer {
        status,
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
        bytes: bytes.to_vec(),
    }
}

/// A GraphQL request for `query` with `variables`, with `token` as a bearer
/// token.
pub fn graphql_with(token: &str, query: &str, variables: &Value) -> Request<Body> {
    Request::post("/graphql")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({ "query": query, "variables": variables }).to_string(),
        ))
        .unwrap()
}

/// A GraphQL request for `query`, with `token` as a bearer token if given.
pub fn graphql(token: Option<&str>, query: &str) -> Request<Body> {
    let mut request = Request::post("/graphql")
        .header("host", "kuria-internal:8080")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request
        .body(Body::from(json!({ "query": query }).to_string()))
        .unwrap()
}
