//! A Kuria server on a local port, against a fresh database, for the CLI
//! to speak to: a relying host that stands in for Konklav, accepting the
//! token `tenant` as a subject of one tenant allowed everything.

use std::net::SocketAddr;
use std::sync::Arc;

use konklav_authz::{Decision, Requirements};
use konklav_axum::{Accepted, BoxFuture, RelyingHost};
use konklav_relying_client::{Error, Presented, Verified};
use konklav_token::AccessClaims;
use kuria_cli::dpop::DpopKey;
use kuria_cli::http::Http;
use kuria_cli::konklav::Tokens;
use kuria_cli::session::Session;
use kuria_server::authorize::Decider;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

pub const TENANT: &str = "01900000-0000-7000-8000-000000000001";
pub const SUBJECT: &str = "01900000-0000-7000-8000-0000000000aa";

/// A tenant token's claims, bound to a DPoP key as the CLI's are, so that
/// Kuria decides with the presentation its route accepted.
fn claims() -> AccessClaims {
    let mut claims: AccessClaims = serde_json::from_value(json!({
        "iss": "https://auth.example",
        "aud": "kuria",
        "sub": SUBJECT,
        "jti": "j-1",
        "iat": 0,
        "exp": i64::MAX,
        "knd": "tenant",
        "tid": TENANT,
        "sid": "01900000-0000-7000-8000-0000000000bb",
        "sv": 1,
        "amr": ["pwd"],
        "auth_time": 0,
    }))
    .unwrap();
    claims.cnf = Some(konklav_token::Confirmation {
        jkt: "thumbprint".to_owned(),
    });
    claims
}

/// Stands in for a started relying client: the token `tenant` is valid,
/// and everything is allowed.
struct FakeHost;

impl FakeHost {
    fn token(presented: &Presented) -> Verified<AccessClaims> {
        if presented.access_token() == Some("tenant") {
            Verified::Valid(claims())
        } else {
            Verified::Invalid
        }
    }
}

impl RelyingHost for FakeHost {
    fn accept_request<'a>(
        &'a self,
        presented: &'a Presented,
    ) -> BoxFuture<'a, Result<Verified<Accepted>, Error>> {
        Box::pin(async move {
            let token = presented.access_token().unwrap_or_default().to_owned();
            Ok(match Self::token(presented) {
                Verified::Valid(claims) => Verified::Valid(Accepted::new(claims, token)),
                Verified::Invalid => Verified::Invalid,
                Verified::InvalidProof => Verified::InvalidProof,
            })
        })
    }

    fn decide_accepted<'a>(
        &'a self,
        _: &'a Accepted,
        _: &'a Requirements,
    ) -> BoxFuture<'a, Result<Decision, Error>> {
        Box::pin(async move { Ok(Decision::allow()) })
    }

    fn verify_request<'a>(
        &'a self,
        presented: &'a Presented,
    ) -> BoxFuture<'a, Result<Verified<AccessClaims>, Error>> {
        Box::pin(async move { Ok(Self::token(presented)) })
    }

    fn release_accepted(&self, _: &Accepted) {}
}

/// Allows everything.
struct Allowing;

impl Decider for Allowing {
    fn decide<'a>(
        &'a self,
        _: &'a str,
        _: &'a Requirements,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Decision, String>> + Send + 'a>>
    {
        Box::pin(async move { Ok(Decision::allow()) })
    }
}

/// A Kuria server on a local port, with a fresh database, and a session
/// with it as the CLI has one.
pub struct Server {
    _db: TestDatabase,
    pub url: String,
    pub session: Session,
    http: reqwest::Client,
}

impl Server {
    pub async fn start() -> Self {
        let db = TestDatabase::new().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let router = kuria_server::app(
            Arc::new(FakeHost),
            kuria_server::Options {
                public_url: url.clone(),
                trusted_proxies: Vec::new(),
                pool: db.pool().clone(),
                decider: Arc::new(Allowing),
                sessions: None,
                changes: None,
                storage: kuria_server::attachments::Storage::in_memory(1 << 20),
                web: None,
                login_proxy: None,
                cli_relay: None,
                // No test renders: none of its documents holds a diagram.
                renderer: kuria_typst::Renderer {
                    program: "kuria-server".into(),
                    limits: kuria_typst::Limits::default(),
                },
            },
        )
        .unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let session = Session::started(
            Http::new(DpopKey::generate()),
            url.clone(),
            "test".to_owned(),
            &Tokens {
                access_token: "tenant".to_owned(),
                refresh_token: None,
                expires_at: i64::MAX,
            },
        );
        Self {
            _db: db,
            url,
            session,
            http: reqwest::Client::new(),
        }
    }

    /// `query` with `variables`, as the app sends it, answering `data` and
    /// panicking on an error.
    pub async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer["errors"].is_null(), "{answer}");
        answer["data"].clone()
    }

    /// `query` with `variables`, answering what Kuria said.
    pub async fn run(&self, query: &str, variables: Value) -> Value {
        self.http
            .post(format!("{}/graphql", self.url))
            .header("authorization", "Bearer tenant")
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}
