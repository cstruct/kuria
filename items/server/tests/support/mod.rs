//! A relying host that stands in for Konklav, and helpers to send requests
//! through the router.

#![allow(dead_code)]

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
pub const PUBLIC_URL: &str = "https://kuria.example";

/// The claims of a token of `kind`, in `TENANT` if it is a tenant token.
pub fn claims(kind: &str) -> AccessClaims {
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
        claims["tid"] = json!(TENANT);
    }
    serde_json::from_value(claims).unwrap()
}

/// Stands in for a started `RelyingClient`. It accepts bearer tokens starting
/// with `tenant` or `identity`, with claims of that kind, refuses `invalid`,
/// refuses the proof of `bad-proof`, fails for `unreachable`, and decides
/// by token kind alone.
#[derive(Default)]
pub struct FakeHost {
    pub accepted: Mutex<Vec<Presented>>,
}

impl FakeHost {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn accepted(&self) -> Vec<Presented> {
        self.accepted.lock().unwrap().clone()
    }

    fn token(presented: &Presented) -> Result<Verified<AccessClaims>, Error> {
        match presented.access_token().unwrap_or_default() {
            token if token.starts_with("tenant") => Ok(Verified::Valid(claims("tenant"))),
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
            Ok(match Self::token(presented)? {
                Verified::Valid(claims) => Verified::Valid(Accepted::new(claims, ())),
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
            Ok(if kind_admitted {
                Decision::allow()
            } else {
                Decision::Deny {
                    reason: DenyReason::TokenKind,
                }
            })
        })
    }

    fn verify_request<'a>(
        &'a self,
        presented: &'a Presented,
    ) -> BoxFuture<'a, Result<Verified<AccessClaims>, Error>> {
        Box::pin(async move { Self::token(presented) })
    }
}

/// The server's router, deciding through `host`.
pub fn router(host: &Arc<FakeHost>) -> Router {
    kuria_server::app(
        host.clone(),
        kuria_server::Options {
            public_url: PUBLIC_URL.to_owned(),
            trusted_proxies: Vec::new(),
            sessions: None,
        },
    )
    .unwrap()
}

/// An answer, read.
pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
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
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
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
